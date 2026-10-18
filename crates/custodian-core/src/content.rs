//! Bounded, local content primitives for the first Stage 6 format subset.
//!
//! This module deliberately has no planner, model, shell, network, or desktop
//! integration.  Text read from a file is data only.  A destination is carried
//! by a separate [`ContentWriteTarget`] prepared by trusted native code, so a
//! document cannot select a path or grant itself a write.

use custodian_winsec::fsops::{file_identity, is_local_volume, rename_no_replace, FileIdentity};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;
use thiserror::Error;
use uuid::Uuid;

/// The hard upper bound for a single local source read.
pub const MAX_INPUT_BYTES: u64 = 64 * 1024 * 1024;
const MAX_DECODED_BYTES: usize = 128 * 1024 * 1024;
const MAX_ROWS: usize = 100_000;
const MAX_COLUMNS: usize = 4_096;
const MAX_FIELD_BYTES: usize = 8 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 128 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum ContentError {
    #[error("content access refused by the current privacy policy")]
    PrivacyRefused,
    #[error("content source changed while it was being read")]
    SourceChanged,
    #[error("content input exceeds the {max} byte local-read bound")]
    InputTooLarge { max: u64 },
    #[error("decoded content exceeds the local decoded-data bound")]
    DecodedTooLarge,
    #[error("invalid content request: {0}")]
    InvalidRequest(String),
    #[error("invalid declared text encoding")]
    InvalidEncoding,
    #[error("staged content failed independent structured verification: {0}")]
    VerificationFailed(String),
    #[error("destination already exists; replacement needs a separate write plan")]
    DestinationExists,
    #[error("I/O error while processing local content")]
    Io(#[from] io::Error),
}

/// Supported input/output kinds.  The parser never dispatches based on text
/// inside a document; callers choose the format in the typed request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentFormat {
    Txt,
    Markdown,
    Csv,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextEncoding {
    Utf8,
    Utf8Bom,
    Utf16Le,
    Utf16Be,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NewlineStyle {
    Lf,
    CrLf,
    Cr,
    Mixed,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NewlineHandling {
    /// Detect the source style and normalize the in-memory document to `\n`.
    Detect,
    /// Use this style when the document is written.  In-memory content is
    /// still normalized to `\n`.
    Force(NewlineStyle),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentLimits {
    pub max_input_bytes: u64,
    pub max_decoded_bytes: usize,
    pub max_rows: usize,
    pub max_columns: usize,
    pub max_field_bytes: usize,
}

impl Default for ContentLimits {
    fn default() -> Self {
        Self {
            max_input_bytes: MAX_INPUT_BYTES,
            max_decoded_bytes: MAX_DECODED_BYTES,
            max_rows: MAX_ROWS,
            max_columns: MAX_COLUMNS,
            max_field_bytes: MAX_FIELD_BYTES,
        }
    }
}

impl ContentLimits {
    fn validate(self) -> Result<Self, ContentError> {
        if self.max_input_bytes == 0
            || self.max_input_bytes > MAX_INPUT_BYTES
            || self.max_decoded_bytes == 0
            || self.max_decoded_bytes > MAX_DECODED_BYTES
            || self.max_rows == 0
            || self.max_rows > MAX_ROWS
            || self.max_columns == 0
            || self.max_columns > MAX_COLUMNS
            || self.max_field_bytes == 0
            || self.max_field_bytes > MAX_FIELD_BYTES
        {
            return Err(ContentError::InvalidRequest(
                "content limits must be positive and within the offline bounds".into(),
            ));
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Delimiter {
    Auto,
    Character(char),
}

impl Delimiter {
    fn resolve(self, input: &str, limits: ContentLimits) -> char {
        match self {
            Self::Character(delimiter) => delimiter,
            Self::Auto => infer_delimiter(input, limits),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CsvType {
    Text,
    Integer,
    Number,
    Boolean,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CsvOptions {
    pub delimiter: Delimiter,
    pub has_header: bool,
    /// When present, these are applied by column.  Missing entries use
    /// inference; invalid conversions remain text and produce a warning.
    pub column_types: Option<Vec<CsvType>>,
}

impl Default for CsvOptions {
    fn default() -> Self {
        Self {
            delimiter: Delimiter::Auto,
            has_header: false,
            column_types: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentRequest {
    pub format: ContentFormat,
    pub encoding: TextEncoding,
    pub newline: NewlineHandling,
    pub csv: CsvOptions,
    pub limits: ContentLimits,
}

impl ContentRequest {
    pub fn text(format: ContentFormat, encoding: TextEncoding, newline: NewlineHandling) -> Self {
        Self {
            format,
            encoding,
            newline,
            csv: CsvOptions::default(),
            limits: ContentLimits::default(),
        }
    }

    pub fn csv(encoding: TextEncoding, newline: NewlineHandling, csv: CsvOptions) -> Self {
        Self {
            format: ContentFormat::Csv,
            encoding,
            newline,
            csv,
            limits: ContentLimits::default(),
        }
    }
}

impl Default for ContentRequest {
    fn default() -> Self {
        Self::text(
            ContentFormat::Txt,
            TextEncoding::Utf8,
            NewlineHandling::Detect,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceVersion {
    pub identity: FileIdentity,
    pub bytes: u64,
    pub modified_unix_nanos: Option<u128>,
    pub blake3: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceProvenance {
    pub path: PathBuf,
    pub version: SourceVersion,
    pub format: ContentFormat,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentLocation {
    WholeFile,
    TextLine { line: u64, column: u64 },
    CsvRow { row: u64 },
    CsvCell { row: u64, column: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WarningCode {
    MalformedRow,
    InconsistentColumns,
    TypeConversionFailed,
    LimitExceeded,
    MixedNewlines,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentWarning {
    pub code: WarningCode,
    pub message: String,
    pub location: ContentLocation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Completion {
    Complete,
    Partial,
}

impl Completion {
    pub fn is_partial(self) -> bool {
        matches!(self, Self::Partial)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContentResult {
    pub document: ContentDocument,
    pub provenance: SourceProvenance,
    pub warnings: Vec<ContentWarning>,
    pub completion: Completion,
}

impl ContentResult {
    pub fn is_partial(&self) -> bool {
        self.completion.is_partial()
    }

    pub fn root_provenance(&self) -> Provenance {
        Provenance {
            source: self.provenance.clone(),
            location: ContentLocation::WholeFile,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    pub source: SourceProvenance,
    pub location: ContentLocation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ContentDocument {
    Txt(TextDocument),
    Markdown(MarkdownDocument),
    Csv(CsvDocument),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextDocument {
    pub text: String,
    pub encoding: TextEncoding,
    pub newline: NewlineStyle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarkdownDocument {
    pub text: String,
    pub encoding: TextEncoding,
    pub newline: NewlineStyle,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CsvDocument {
    pub delimiter: char,
    pub has_header: bool,
    pub encoding: TextEncoding,
    pub newline: NewlineStyle,
    pub headers: Option<Vec<String>>,
    pub column_types: Vec<CsvType>,
    pub rows: Vec<CsvRow>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CsvRow {
    pub row: u64,
    pub values: Vec<CsvValue>,
    pub location: ContentLocation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CsvValue {
    Null,
    Text(String),
    Integer(i64),
    Number(f64),
    Boolean(bool),
}

/// Reads a supported local file after the current privacy policy has admitted
/// both its requested and canonical path.  The file is read through one handle,
/// bounded, hashed, and checked for identity/version changes before returning.
pub fn read_content(
    path: impl AsRef<Path>,
    request: &ContentRequest,
) -> Result<ContentResult, ContentError> {
    let limits = request.limits.validate()?;
    let source = read_bounded(path.as_ref(), limits)?;
    let (text, newline_style) =
        decode_text(&source.bytes, request.encoding, request.newline, limits)?;
    let mut warnings = Vec::new();
    if newline_style == NewlineStyle::Mixed {
        warnings.push(ContentWarning {
            code: WarningCode::MixedNewlines,
            message: "source contains more than one newline style; in-memory text uses LF".into(),
            location: ContentLocation::WholeFile,
        });
    }

    let document = match request.format {
        ContentFormat::Txt => ContentDocument::Txt(TextDocument {
            text,
            encoding: request.encoding,
            newline: newline_style,
        }),
        ContentFormat::Markdown => ContentDocument::Markdown(MarkdownDocument {
            text,
            encoding: request.encoding,
            newline: newline_style,
        }),
        ContentFormat::Csv => {
            let (csv, mut csv_warnings, partial) =
                parse_csv(&text, newline_style, request.encoding, &request.csv, limits)?;
            warnings.append(&mut csv_warnings);
            return Ok(ContentResult {
                document: ContentDocument::Csv(csv),
                provenance: SourceProvenance {
                    path: source.path,
                    version: source.version,
                    format: request.format,
                },
                warnings,
                completion: if partial {
                    Completion::Partial
                } else {
                    Completion::Complete
                },
            });
        }
    };

    Ok(ContentResult {
        document,
        provenance: SourceProvenance {
            path: source.path,
            version: source.version,
            format: request.format,
        },
        warnings,
        completion: Completion::Complete,
    })
}

/// Convenience wrapper for a UTF-8 text file.
pub fn read_txt(path: impl AsRef<Path>) -> Result<ContentResult, ContentError> {
    read_content(
        path,
        &ContentRequest::text(
            ContentFormat::Txt,
            TextEncoding::Utf8,
            NewlineHandling::Detect,
        ),
    )
}

/// Convenience wrapper for a UTF-8 Markdown file.
pub fn read_markdown(path: impl AsRef<Path>) -> Result<ContentResult, ContentError> {
    read_content(
        path,
        &ContentRequest::text(
            ContentFormat::Markdown,
            TextEncoding::Utf8,
            NewlineHandling::Detect,
        ),
    )
}

/// Convenience wrapper using automatic CSV delimiter/type inference.
pub fn read_csv(path: impl AsRef<Path>) -> Result<ContentResult, ContentError> {
    read_content(
        path,
        &ContentRequest::csv(
            TextEncoding::Utf8,
            NewlineHandling::Detect,
            CsvOptions::default(),
        ),
    )
}

/// Rechecks the native source identity, metadata version, privacy admission,
/// and complete BLAKE3 hash captured by an earlier read.
pub fn verify_source(provenance: &SourceProvenance) -> Result<(), ContentError> {
    let current = read_bounded(&provenance.path, ContentLimits::default())?;
    if current.version != provenance.version {
        return Err(ContentError::SourceChanged);
    }
    Ok(())
}

#[derive(Debug)]
struct BoundedSource {
    path: PathBuf,
    bytes: Vec<u8>,
    version: SourceVersion,
}

fn read_bounded(path: &Path, limits: ContentLimits) -> Result<BoundedSource, ContentError> {
    let canonical = admit_read(path)?;
    let admitted_identity = file_identity(&canonical)?;
    let mut file = File::open(&canonical)?;
    let before = file.metadata()?;
    let identity = file_identity(&canonical)?;
    if admitted_identity != identity {
        return Err(ContentError::SourceChanged);
    }
    if before.len() > limits.max_input_bytes {
        return Err(ContentError::InputTooLarge {
            max: limits.max_input_bytes,
        });
    }
    let mut bytes = Vec::with_capacity(before.len().min(limits.max_input_bytes) as usize);
    Read::by_ref(&mut file)
        .take(limits.max_input_bytes.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limits.max_input_bytes {
        return Err(ContentError::InputTooLarge {
            max: limits.max_input_bytes,
        });
    }
    let after = file.metadata()?;
    let after_identity = file_identity(&canonical)?;
    if identity != after_identity
        || before.len() != after.len()
        || modified_nanos(&before) != modified_nanos(&after)
    {
        return Err(ContentError::SourceChanged);
    }
    let hash = blake3::hash(&bytes).to_hex().to_string();
    Ok(BoundedSource {
        path: canonical,
        bytes,
        version: SourceVersion {
            identity,
            bytes: before.len(),
            modified_unix_nanos: modified_nanos(&before),
            blake3: hash,
        },
    })
}

fn admit_read(path: &Path) -> Result<PathBuf, ContentError> {
    if !path.is_absolute() || !is_local_volume(path) {
        return Err(ContentError::InvalidRequest(
            "offline content reads require an absolute local path".into(),
        ));
    }
    crate::privacy::check(path).map_err(|_| ContentError::PrivacyRefused)?;
    let canonical = fs::canonicalize(path)?;
    crate::privacy::check(&canonical).map_err(|_| ContentError::PrivacyRefused)?;
    let metadata = fs::symlink_metadata(&canonical)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(ContentError::InvalidRequest(
            "content source must be a regular local file".into(),
        ));
    }
    Ok(canonical)
}

fn modified_nanos(metadata: &fs::Metadata) -> Option<u128> {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos())
}

fn decode_text(
    bytes: &[u8],
    encoding: TextEncoding,
    newline: NewlineHandling,
    limits: ContentLimits,
) -> Result<(String, NewlineStyle), ContentError> {
    let text = match encoding {
        TextEncoding::Utf8 => {
            let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes);
            String::from_utf8(bytes.to_vec()).map_err(|_| ContentError::InvalidEncoding)?
        }
        TextEncoding::Utf8Bom => {
            let bytes = bytes
                .strip_prefix(&[0xef, 0xbb, 0xbf])
                .ok_or(ContentError::InvalidEncoding)?;
            String::from_utf8(bytes.to_vec()).map_err(|_| ContentError::InvalidEncoding)?
        }
        TextEncoding::Utf16Le => decode_utf16(bytes, true)?,
        TextEncoding::Utf16Be => decode_utf16(bytes, false)?,
    };
    if text.len() > limits.max_decoded_bytes {
        return Err(ContentError::DecodedTooLarge);
    }
    let detected = detect_newline(&text);
    let output_style = match newline {
        NewlineHandling::Detect => detected,
        NewlineHandling::Force(style) => style,
    };
    Ok((normalize_newlines(&text), output_style))
}

fn decode_utf16(bytes: &[u8], little_endian: bool) -> Result<String, ContentError> {
    if !bytes.len().is_multiple_of(2) {
        return Err(ContentError::InvalidEncoding);
    }
    let (expected_bom, wrong_bom) = if little_endian {
        ([0xff, 0xfe], [0xfe, 0xff])
    } else {
        ([0xfe, 0xff], [0xff, 0xfe])
    };
    if bytes.starts_with(&wrong_bom) {
        return Err(ContentError::InvalidEncoding);
    }
    let bytes = bytes.strip_prefix(&expected_bom).unwrap_or(bytes);
    let units = bytes
        .chunks_exact(2)
        .map(|chunk| {
            if little_endian {
                u16::from_le_bytes([chunk[0], chunk[1]])
            } else {
                u16::from_be_bytes([chunk[0], chunk[1]])
            }
        })
        .collect::<Vec<_>>();
    String::from_utf16(&units).map_err(|_| ContentError::InvalidEncoding)
}

fn detect_newline(text: &str) -> NewlineStyle {
    let mut lf = 0usize;
    let mut crlf = 0usize;
    let mut cr = 0usize;
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'\r' if bytes.get(index + 1) == Some(&b'\n') => {
                crlf += 1;
                index += 2;
            }
            b'\r' => {
                cr += 1;
                index += 1;
            }
            b'\n' => {
                lf += 1;
                index += 1;
            }
            _ => index += 1,
        }
    }
    let kinds = [lf > 0, crlf > 0, cr > 0]
        .into_iter()
        .filter(|present| *present)
        .count();
    if kinds == 0 {
        NewlineStyle::None
    } else if kinds > 1 {
        NewlineStyle::Mixed
    } else if crlf > 0 {
        NewlineStyle::CrLf
    } else if cr > 0 {
        NewlineStyle::Cr
    } else {
        NewlineStyle::Lf
    }
}

fn normalize_newlines(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

fn newline_bytes(style: NewlineStyle) -> &'static str {
    match style {
        NewlineStyle::CrLf => "\r\n",
        NewlineStyle::Cr => "\r",
        NewlineStyle::Lf | NewlineStyle::Mixed | NewlineStyle::None => "\n",
    }
}

fn effective_newline(style: NewlineStyle) -> NewlineStyle {
    match style {
        NewlineStyle::Mixed | NewlineStyle::None => NewlineStyle::Lf,
        other => other,
    }
}

fn parse_csv(
    text: &str,
    newline: NewlineStyle,
    encoding: TextEncoding,
    options: &CsvOptions,
    limits: ContentLimits,
) -> Result<(CsvDocument, Vec<ContentWarning>, bool), ContentError> {
    validate_delimiter(options.delimiter)?;
    if options
        .column_types
        .as_ref()
        .is_some_and(|types| types.len() > limits.max_columns)
    {
        return Err(ContentError::InvalidRequest(
            "explicit CSV types exceed the column bound".into(),
        ));
    }
    let delimiter = options.delimiter.resolve(text, limits);
    let raw = parse_raw_csv(text, delimiter, limits);
    let mut warnings = Vec::new();
    let mut partial = raw.truncated || raw.field_truncated || raw.columns_truncated;
    if raw.truncated || raw.field_truncated || raw.columns_truncated {
        warnings.push(ContentWarning {
            code: WarningCode::LimitExceeded,
            message: "CSV parsing stopped at a configured row, column, or field bound".into(),
            location: ContentLocation::WholeFile,
        });
    }
    let mut rows = raw.rows;
    let headers = if options.has_header && !rows.is_empty() {
        let header = rows.remove(0);
        if header.malformed {
            warnings.push(ContentWarning {
                code: WarningCode::MalformedRow,
                message: "CSV header has malformed quoting; recovered fields are retained".into(),
                location: ContentLocation::CsvRow { row: 1 },
            });
            partial = true;
        }
        Some(header.fields)
    } else {
        None
    };
    let width = headers.as_ref().map_or_else(
        || rows.iter().map(|row| row.fields.len()).max().unwrap_or(0),
        Vec::len,
    );
    let inferred = infer_types(&rows, width);
    let types = options
        .column_types
        .as_ref()
        .map(|explicit| {
            (0..width)
                .map(|column| explicit.get(column).copied().unwrap_or(inferred[column]))
                .collect::<Vec<_>>()
        })
        .unwrap_or(inferred);
    let expected_width = headers.as_ref().map_or(width, Vec::len);
    let mut converted = Vec::with_capacity(rows.len());
    for raw_row in rows {
        let row_number = raw_row.row;
        let location = ContentLocation::CsvRow { row: row_number };
        if raw_row.malformed {
            warnings.push(ContentWarning {
                code: WarningCode::MalformedRow,
                message: "CSV row has malformed quoting; recovered fields are retained".into(),
                location: location.clone(),
            });
            partial = true;
        }
        if raw_row.fields.len() != expected_width {
            warnings.push(ContentWarning {
                code: WarningCode::InconsistentColumns,
                message: "CSV row width differs from the declared/header width".into(),
                location: location.clone(),
            });
            partial = true;
        }
        let values = raw_row
            .fields
            .iter()
            .enumerate()
            .map(|(column, value)| {
                let kind = types.get(column).copied().unwrap_or(CsvType::Text);
                match convert_value(value, kind) {
                    Ok(value) => value,
                    Err(()) => {
                        warnings.push(ContentWarning {
                            code: WarningCode::TypeConversionFailed,
                            message:
                                "CSV value did not match its explicit/inferred type; kept as text"
                                    .into(),
                            location: ContentLocation::CsvCell {
                                row: row_number,
                                column: column as u64 + 1,
                            },
                        });
                        partial = true;
                        CsvValue::Text(value.clone())
                    }
                }
            })
            .collect();
        converted.push(CsvRow {
            row: row_number,
            values,
            location,
        });
    }
    Ok((
        CsvDocument {
            delimiter,
            has_header: options.has_header,
            encoding,
            newline,
            headers,
            column_types: types,
            rows: converted,
        },
        warnings,
        partial,
    ))
}

fn validate_delimiter(delimiter: Delimiter) -> Result<(), ContentError> {
    if let Delimiter::Character(value) = delimiter {
        if value == '"' || value == '\n' || value == '\r' || value == '\0' {
            return Err(ContentError::InvalidRequest(
                "CSV delimiter must not be a quote, newline, or NUL".into(),
            ));
        }
    }
    Ok(())
}

#[derive(Debug)]
struct RawCsv {
    rows: Vec<RawRow>,
    truncated: bool,
    field_truncated: bool,
    columns_truncated: bool,
}

#[derive(Debug)]
struct RawRow {
    row: u64,
    fields: Vec<String>,
    malformed: bool,
}

fn parse_raw_csv(text: &str, delimiter: char, limits: ContentLimits) -> RawCsv {
    let mut rows = Vec::new();
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut closed_quote = false;
    let mut malformed = false;
    let mut field_truncated = false;
    let mut any_field_truncated = false;
    let mut columns_truncated = false;
    let mut row_number = 1u64;
    let mut truncated = false;

    let push_field = |fields: &mut Vec<String>, field: &mut String, truncated: &mut bool| {
        if fields.len() < limits.max_columns {
            fields.push(std::mem::take(field));
        } else {
            field.clear();
            *truncated = true;
        }
    };
    let mut chars = text.chars().peekable();
    while let Some(character) = chars.next() {
        if in_quotes {
            if character == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    append_field(
                        &mut field,
                        '"',
                        &mut field_truncated,
                        limits.max_field_bytes,
                    );
                } else {
                    in_quotes = false;
                    closed_quote = true;
                }
            } else {
                append_field(
                    &mut field,
                    character,
                    &mut field_truncated,
                    limits.max_field_bytes,
                );
            }
            continue;
        }
        if character == delimiter {
            push_field(&mut fields, &mut field, &mut columns_truncated);
            closed_quote = false;
        } else if character == '\n' {
            push_field(&mut fields, &mut field, &mut columns_truncated);
            rows.push(RawRow {
                row: row_number,
                fields: std::mem::take(&mut fields),
                malformed,
            });
            if rows.len() >= limits.max_rows {
                any_field_truncated |= field_truncated;
                truncated = true;
                break;
            }
            row_number += 1;
            malformed = false;
            any_field_truncated |= field_truncated;
            field_truncated = false;
            closed_quote = false;
        } else if character == '"' {
            if field.is_empty() && !closed_quote {
                in_quotes = true;
            } else {
                malformed = true;
                append_field(
                    &mut field,
                    character,
                    &mut field_truncated,
                    limits.max_field_bytes,
                );
            }
        } else {
            if closed_quote {
                malformed = true;
            }
            append_field(
                &mut field,
                character,
                &mut field_truncated,
                limits.max_field_bytes,
            );
        }
    }
    if !truncated {
        if in_quotes {
            malformed = true;
        }
        any_field_truncated |= field_truncated;
        if !fields.is_empty() || !field.is_empty() || malformed || field_truncated {
            push_field(&mut fields, &mut field, &mut columns_truncated);
            rows.push(RawRow {
                row: row_number,
                fields,
                malformed,
            });
        }
    }
    RawCsv {
        rows,
        truncated,
        field_truncated: any_field_truncated,
        columns_truncated,
    }
}

fn append_field(field: &mut String, character: char, truncated: &mut bool, max: usize) {
    if field.len() < max {
        field.push(character);
    } else {
        *truncated = true;
    }
}

fn infer_delimiter(text: &str, limits: ContentLimits) -> char {
    let mut best = (0usize, ',');
    for delimiter in [',', '\t', ';', '|'] {
        let parsed = parse_raw_csv(
            text,
            delimiter,
            ContentLimits {
                max_rows: limits.max_rows.min(64),
                max_columns: limits.max_columns.min(128),
                max_field_bytes: limits.max_field_bytes.min(64 * 1024),
                ..limits
            },
        );
        let mut widths = BTreeMap::<usize, usize>::new();
        for row in parsed.rows {
            *widths.entry(row.fields.len()).or_default() += 1;
        }
        let Some((&width, &count)) = widths.iter().max_by_key(|(_, count)| **count) else {
            continue;
        };
        let score = if width > 1 { count * 100 + width } else { 0 };
        if score > best.0 {
            best = (score, delimiter);
        }
    }
    best.1
}

fn infer_types(rows: &[RawRow], width: usize) -> Vec<CsvType> {
    (0..width)
        .map(|column| {
            let values = rows
                .iter()
                .filter_map(|row| row.fields.get(column))
                .filter(|value| !value.is_empty() && !is_formula_like(value));
            let mut saw_value = false;
            let mut all_bool = true;
            let mut all_integer = true;
            let mut all_number = true;
            for value in values {
                saw_value = true;
                all_bool &= parse_bool(value).is_some();
                all_integer &= value.trim().parse::<i64>().is_ok();
                all_number &= value.trim().parse::<f64>().is_ok_and(|n| n.is_finite());
            }
            if !saw_value {
                CsvType::Text
            } else if all_bool {
                CsvType::Boolean
            } else if all_integer {
                CsvType::Integer
            } else if all_number {
                CsvType::Number
            } else {
                CsvType::Text
            }
        })
        .collect()
}

fn is_formula_like(value: &str) -> bool {
    let value = value.trim_start();
    match value.chars().next() {
        Some('=' | '@') => true,
        Some('+' | '-') => value
            .chars()
            .nth(1)
            .is_some_and(|next| !next.is_ascii_digit() && next != '.'),
        _ => false,
    }
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

fn convert_value(value: &str, kind: CsvType) -> Result<CsvValue, ()> {
    if value.is_empty() {
        return Ok(CsvValue::Null);
    }
    if kind == CsvType::Text || is_formula_like(value) {
        return Ok(CsvValue::Text(value.into()));
    }
    match kind {
        CsvType::Text => unreachable!(),
        CsvType::Integer => value
            .trim()
            .parse::<i64>()
            .map(CsvValue::Integer)
            .map_err(|_| ()),
        CsvType::Number => value
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|number| number.is_finite())
            .map(CsvValue::Number)
            .ok_or(()),
        CsvType::Boolean => parse_bool(value).map(CsvValue::Boolean).ok_or(()),
    }
}

/// A destination selected independently from document content.  This is a
/// create-new target; replacement/edit authority remains a separate capability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentWriteTarget {
    destination: PathBuf,
    policy_revision: u64,
    parent_identity: FileIdentity,
}

impl ContentWriteTarget {
    pub fn prepare_new(path: impl AsRef<Path>) -> Result<Self, ContentError> {
        let path = path.as_ref();
        if !path.is_absolute() || !is_local_volume(path) {
            return Err(ContentError::InvalidRequest(
                "offline content outputs require an absolute local path".into(),
            ));
        }
        crate::privacy::check(path).map_err(|_| ContentError::PrivacyRefused)?;
        if fs::symlink_metadata(path).is_ok() {
            return Err(ContentError::DestinationExists);
        }
        let parent = path.parent().ok_or_else(|| {
            ContentError::InvalidRequest("content output has no parent directory".into())
        })?;
        if !parent.is_dir() {
            return Err(ContentError::InvalidRequest(
                "content output parent must already exist".into(),
            ));
        }
        crate::privacy::check(parent).map_err(|_| ContentError::PrivacyRefused)?;
        let parent_identity = file_identity(parent)?;
        Ok(Self {
            destination: path.to_path_buf(),
            policy_revision: crate::privacy::current().revision(),
            parent_identity,
        })
    }

    pub fn destination(&self) -> &Path {
        &self.destination
    }
}

#[derive(Debug)]
pub struct StagedContent {
    destination: PathBuf,
    staging: Option<PathBuf>,
    expected: ContentDocument,
    policy_revision: u64,
    parent_identity: FileIdentity,
}

impl StagedContent {
    /// Reopens the staging path through the normal bounded reader and compares
    /// structured values, not just file size or the staging writer's hash.
    pub fn verify(&self) -> Result<SourceProvenance, ContentError> {
        let staging = self
            .staging
            .as_deref()
            .ok_or_else(|| ContentError::VerificationFailed("staging was published".into()))?;
        verify_output(staging, &self.expected)
    }

    pub fn destination(&self) -> &Path {
        &self.destination
    }

    /// Publishes only after a successful staged reopen and a fresh privacy and
    /// destination check.  Existing destinations are never replaced.
    pub fn publish(mut self) -> Result<PublishedContent, ContentError> {
        self.verify()?;
        if crate::privacy::current().revision() != self.policy_revision {
            return Err(ContentError::PrivacyRefused);
        }
        crate::privacy::check(&self.destination).map_err(|_| ContentError::PrivacyRefused)?;
        let parent = self.destination.parent().ok_or_else(|| {
            ContentError::InvalidRequest("content output has no parent directory".into())
        })?;
        if file_identity(parent)? != self.parent_identity {
            return Err(ContentError::SourceChanged);
        }
        if fs::symlink_metadata(&self.destination).is_ok() {
            return Err(ContentError::DestinationExists);
        }
        let staging = self
            .staging
            .take()
            .ok_or_else(|| ContentError::VerificationFailed("staging was published".into()))?;
        if let Err(error) = rename_no_replace(&staging, &self.destination) {
            let _ = fs::remove_file(&staging);
            return Err(error.into());
        }
        let verified = verify_output(&self.destination, &self.expected);
        match verified {
            Ok(provenance) => Ok(PublishedContent {
                destination: self.destination.clone(),
                provenance,
            }),
            Err(error) => Err(error),
        }
    }
}

impl Drop for StagedContent {
    fn drop(&mut self) {
        if let Some(staging) = self.staging.take() {
            let _ = fs::remove_file(staging);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedContent {
    pub destination: PathBuf,
    pub provenance: SourceProvenance,
}

/// Stages a text or Markdown document without allowing its contents to choose
/// the destination or any authority.  The target is caller-supplied separately.
pub fn stage_document(
    target: &ContentWriteTarget,
    document: &ContentDocument,
) -> Result<StagedContent, ContentError> {
    if crate::privacy::current().revision() != target.policy_revision {
        return Err(ContentError::PrivacyRefused);
    }
    crate::privacy::check(&target.destination).map_err(|_| ContentError::PrivacyRefused)?;
    let parent = target.destination.parent().ok_or_else(|| {
        ContentError::InvalidRequest("content output has no parent directory".into())
    })?;
    if file_identity(parent)? != target.parent_identity {
        return Err(ContentError::SourceChanged);
    }
    if fs::symlink_metadata(&target.destination).is_ok() {
        return Err(ContentError::DestinationExists);
    }
    let bytes = encode_document(document)?;
    if bytes.len() > MAX_OUTPUT_BYTES {
        return Err(ContentError::InvalidRequest(
            "staged output exceeds the offline output bound".into(),
        ));
    }
    let staging = target.destination.with_file_name(format!(
        ".{}.custodian-stage-{}.tmp",
        target
            .destination
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("output"),
        Uuid::new_v4().simple()
    ));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staging)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        Ok::<_, ContentError>(())
    })();
    if let Err(error) = result {
        let _ = fs::remove_file(&staging);
        return Err(error);
    }
    let staged = StagedContent {
        destination: target.destination.clone(),
        staging: Some(staging),
        expected: normalized_for_write(document),
        policy_revision: target.policy_revision,
        parent_identity: target.parent_identity,
    };
    staged.verify()?;
    Ok(staged)
}

/// Reopens an output through a fresh read path and checks structured content.
pub fn verify_output(
    path: &Path,
    expected: &ContentDocument,
) -> Result<SourceProvenance, ContentError> {
    let request = request_for_document(expected);
    let actual = read_content(path, &request)?;
    if actual.document != *expected {
        return Err(ContentError::VerificationFailed(
            "reopened content differs from the expected structured document".into(),
        ));
    }
    Ok(actual.provenance)
}

fn normalized_for_write(document: &ContentDocument) -> ContentDocument {
    match document {
        ContentDocument::Txt(value) => ContentDocument::Txt(TextDocument {
            text: value.text.clone(),
            encoding: value.encoding,
            newline: effective_newline(value.newline),
        }),
        ContentDocument::Markdown(value) => ContentDocument::Markdown(MarkdownDocument {
            text: value.text.clone(),
            encoding: value.encoding,
            newline: effective_newline(value.newline),
        }),
        ContentDocument::Csv(value) => {
            let mut copy = value.clone();
            copy.newline = effective_newline(copy.newline);
            ContentDocument::Csv(copy)
        }
    }
}

fn request_for_document(document: &ContentDocument) -> ContentRequest {
    match document {
        ContentDocument::Txt(value) => {
            ContentRequest::text(ContentFormat::Txt, value.encoding, NewlineHandling::Detect)
        }
        ContentDocument::Markdown(value) => ContentRequest::text(
            ContentFormat::Markdown,
            value.encoding,
            NewlineHandling::Detect,
        ),
        ContentDocument::Csv(value) => ContentRequest::csv(
            value.encoding,
            NewlineHandling::Detect,
            CsvOptions {
                delimiter: Delimiter::Character(value.delimiter),
                has_header: value.has_header,
                column_types: Some(value.column_types.clone()),
            },
        ),
    }
}

fn encode_document(document: &ContentDocument) -> Result<Vec<u8>, ContentError> {
    match document {
        ContentDocument::Txt(value) => encode_text(&value.text, value.encoding, value.newline),
        ContentDocument::Markdown(value) => encode_text(&value.text, value.encoding, value.newline),
        ContentDocument::Csv(value) => encode_csv(value),
    }
}

fn encode_text(
    text: &str,
    encoding: TextEncoding,
    newline: NewlineStyle,
) -> Result<Vec<u8>, ContentError> {
    let text = normalize_newlines(text).replace('\n', newline_bytes(effective_newline(newline)));
    let mut output = match encoding {
        TextEncoding::Utf8 | TextEncoding::Utf8Bom => text.into_bytes(),
        TextEncoding::Utf16Le | TextEncoding::Utf16Be => {
            let mut bytes = Vec::with_capacity(text.len() * 2);
            for unit in text.encode_utf16() {
                let encoded = if encoding == TextEncoding::Utf16Le {
                    unit.to_le_bytes()
                } else {
                    unit.to_be_bytes()
                };
                bytes.extend(encoded);
            }
            bytes
        }
    };
    if encoding == TextEncoding::Utf8Bom {
        output.splice(0..0, [0xef, 0xbb, 0xbf]);
    } else if encoding == TextEncoding::Utf16Le {
        output.splice(0..0, [0xff, 0xfe]);
    } else if encoding == TextEncoding::Utf16Be {
        output.splice(0..0, [0xfe, 0xff]);
    }
    Ok(output)
}

fn encode_csv(document: &CsvDocument) -> Result<Vec<u8>, ContentError> {
    validate_delimiter(Delimiter::Character(document.delimiter))?;
    let line_ending = newline_bytes(effective_newline(document.newline));
    let mut lines = Vec::new();
    if let Some(headers) = &document.headers {
        lines.push(
            headers
                .iter()
                .map(|value| csv_escape(value, document.delimiter))
                .collect::<Vec<_>>()
                .join(&document.delimiter.to_string()),
        );
    }
    for row in &document.rows {
        let values = row
            .values
            .iter()
            .map(|value| csv_escape(&format_csv_value(value), document.delimiter))
            .collect::<Vec<_>>();
        lines.push(values.join(&document.delimiter.to_string()));
    }
    let mut output = lines.join(line_ending);
    if !lines.is_empty() {
        output.push_str(line_ending);
    }
    encode_text(&output, document.encoding, NewlineStyle::Lf)
}

fn csv_escape(value: &str, delimiter: char) -> String {
    if value.chars().any(|character| {
        character == delimiter || character == '"' || character == '\n' || character == '\r'
    }) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

fn format_csv_value(value: &CsvValue) -> String {
    match value {
        CsvValue::Null => String::new(),
        CsvValue::Text(value) => value.clone(),
        CsvValue::Integer(value) => value.to_string(),
        CsvValue::Number(value) => value.to_string(),
        CsvValue::Boolean(value) => value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn request(format: ContentFormat) -> ContentRequest {
        ContentRequest::text(format, TextEncoding::Utf8, NewlineHandling::Detect)
    }

    #[test]
    fn unicode_txt_captures_source_and_normalizes_newlines() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("unicode.txt");
        fs::write(&path, "café 🌍\r\nsecond").unwrap();
        let result = read_content(&path, &request(ContentFormat::Txt)).unwrap();
        assert_eq!(
            result.document,
            ContentDocument::Txt(TextDocument {
                text: "café 🌍\nsecond".into(),
                encoding: TextEncoding::Utf8,
                newline: NewlineStyle::CrLf,
            })
        );
        assert_eq!(
            result.provenance.version.bytes,
            fs::metadata(&path).unwrap().len()
        );
        assert!(!result.provenance.version.blake3.is_empty());
        assert_eq!(result.provenance.format, ContentFormat::Txt);
    }

    #[test]
    fn declared_utf16_encoding_and_newline_are_honored() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("utf16.txt");
        fs::write(
            &path,
            encode_text("naïve\r\n値", TextEncoding::Utf16Le, NewlineStyle::CrLf).unwrap(),
        )
        .unwrap();
        let result = read_content(
            &path,
            &ContentRequest::text(
                ContentFormat::Txt,
                TextEncoding::Utf16Le,
                NewlineHandling::Detect,
            ),
        )
        .unwrap();
        assert_eq!(
            result.document,
            ContentDocument::Txt(TextDocument {
                text: "naïve\n値".into(),
                encoding: TextEncoding::Utf16Le,
                newline: NewlineStyle::CrLf,
            })
        );
        let wrong = ContentRequest::text(
            ContentFormat::Txt,
            TextEncoding::Utf16Be,
            NewlineHandling::Detect,
        );
        assert!(matches!(
            read_content(&path, &wrong),
            Err(ContentError::InvalidEncoding)
        ));
    }

    #[test]
    fn markdown_is_read_as_untrusted_text_with_source_provenance() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("notes.md");
        fs::write(&path, "# Heading\n\n- café\n").unwrap();
        let result = read_markdown(&path).unwrap();
        let ContentDocument::Markdown(ref document) = result.document else {
            panic!("expected Markdown document")
        };
        assert_eq!(document.text, "# Heading\n\n- café\n");
        assert_eq!(
            result.root_provenance().location,
            ContentLocation::WholeFile
        );
        assert_eq!(result.provenance.format, ContentFormat::Markdown);
    }

    #[test]
    fn csv_handles_quoted_multiline_values_and_infers_types() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rows.csv");
        fs::write(&path, "name,amount,enabled\n\"A\nB\",12,true\nC,,false\n").unwrap();
        let options = CsvOptions {
            has_header: true,
            ..CsvOptions::default()
        };
        let mut read = ContentRequest::csv(TextEncoding::Utf8, NewlineHandling::Detect, options);
        read.limits = ContentLimits::default();
        let result = read_content(&path, &read).unwrap();
        assert!(!result.is_partial());
        let ContentDocument::Csv(csv) = result.document else {
            panic!("expected CSV document")
        };
        assert_eq!(csv.delimiter, ',');
        assert_eq!(
            csv.headers,
            Some(vec!["name".into(), "amount".into(), "enabled".into()])
        );
        assert_eq!(csv.rows[0].values[0], CsvValue::Text("A\nB".into()));
        assert_eq!(csv.rows[0].values[1], CsvValue::Integer(12));
        assert_eq!(csv.rows[0].values[2], CsvValue::Boolean(true));
        assert_eq!(csv.rows[1].values[1], CsvValue::Null);
    }

    #[test]
    fn malformed_rows_are_retained_with_typed_warnings_and_partial_status() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("malformed.csv");
        fs::write(&path, "a,b\nbad\"quote,2\nshort\n").unwrap();
        let options = CsvOptions {
            has_header: true,
            ..CsvOptions::default()
        };
        let result = read_content(
            &path,
            &ContentRequest::csv(TextEncoding::Utf8, NewlineHandling::Detect, options),
        )
        .unwrap();
        assert!(result.is_partial());
        assert!(result
            .warnings
            .iter()
            .any(|warning| warning.code == WarningCode::MalformedRow));
        assert!(result
            .warnings
            .iter()
            .any(|warning| warning.code == WarningCode::InconsistentColumns));
        let ContentDocument::Csv(csv) = result.document else {
            panic!("expected CSV document")
        };
        assert_eq!(csv.rows.len(), 2);
        assert_eq!(csv.rows[0].values[0], CsvValue::Text("bad\"quote".into()));
    }

    #[test]
    fn formula_like_csv_strings_remain_data_even_with_explicit_number_type() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("formula.csv");
        fs::write(&path, "value\n=SUM(A1:A2)\n").unwrap();
        let options = CsvOptions {
            delimiter: Delimiter::Character(','),
            has_header: true,
            column_types: Some(vec![CsvType::Number]),
        };
        let result = read_content(
            &path,
            &ContentRequest::csv(TextEncoding::Utf8, NewlineHandling::Detect, options),
        )
        .unwrap();
        let ContentDocument::Csv(csv) = result.document else {
            panic!("expected CSV document")
        };
        assert_eq!(csv.rows[0].values[0], CsvValue::Text("=SUM(A1:A2)".into()));
        assert!(!result
            .warnings
            .iter()
            .any(|warning| warning.code == WarningCode::TypeConversionFailed));
    }

    #[test]
    fn changed_source_is_rejected_by_versioned_verification() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("changing.txt");
        fs::write(&path, "before").unwrap();
        let result = read_content(&path, &request(ContentFormat::Txt)).unwrap();
        fs::write(&path, "after").unwrap();
        assert!(matches!(
            verify_source(&result.provenance),
            Err(ContentError::SourceChanged)
        ));
    }

    #[test]
    fn private_source_is_refused_without_reading_content() {
        let dir = tempfile::tempdir().unwrap();
        let private = dir.path().join("private");
        fs::create_dir(&private).unwrap();
        let path = private.join("canary.txt");
        fs::write(&path, "do not read").unwrap();
        let policy = Arc::new(crate::privacy::AccessPolicy::default());
        policy.replace_private_paths(vec![private]).unwrap();
        let result = crate::privacy::with_policy(policy, || read_txt(&path));
        assert!(matches!(result, Err(ContentError::PrivacyRefused)));
    }

    #[test]
    fn staged_writer_reopens_and_verifies_structured_csv() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("input.csv");
        fs::write(&source, "name,value\nalpha,4\n").unwrap();
        let options = CsvOptions {
            has_header: true,
            ..CsvOptions::default()
        };
        let read = read_content(
            &source,
            &ContentRequest::csv(TextEncoding::Utf8, NewlineHandling::Detect, options),
        )
        .unwrap();
        let target = ContentWriteTarget::prepare_new(dir.path().join("output.csv")).unwrap();
        let staged = stage_document(&target, &read.document).unwrap();
        let staged_provenance = staged.verify().unwrap();
        assert_eq!(staged_provenance.format, ContentFormat::Csv);
        let published = staged.publish().unwrap();
        assert_eq!(published.destination, dir.path().join("output.csv"));
        assert!(published.destination.exists());
    }
}
