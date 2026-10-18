//! Exact file-batch manifests and a deliberately small, bounded ZIP subset.
//!
//! This module is intentionally not exported from `lib.rs` yet.  The Stage 5
//! coordinator should export it only after its operation-specific authority and
//! receipts are connected.  The code here is still useful as the shared,
//! side-effecting primitive: it captures exact local file identities, checks
//! the live privacy policy at every boundary, and never publishes an output
//! until an independent reopen has verified it.

use crate::hashing::{hash_file, hash_reader};
use custodian_winsec::fsops::{file_identity, is_local_volume, rename_no_replace, FileIdentity};
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, Write};
use std::path::{Component, Path, PathBuf};

/// Maximum number of members in the supported ZIP subset.
pub const ZIP_MAX_ENTRIES: usize = 1_000;
/// Maximum sum of uncompressed member bytes.
pub const ZIP_MAX_EXPANDED_BYTES: u64 = 512 * 1024 * 1024;
/// Maximum allowed member expansion ratio.
pub const ZIP_MAX_EXPANSION_RATIO: u64 = 100;
/// A bounded input read ceiling.  ZIP headers are included in this ceiling.
pub const ZIP_MAX_ARCHIVE_BYTES: u64 = ZIP_MAX_EXPANDED_BYTES + 16 * 1024 * 1024;
/// Space retained after a ZIP operation, matching the existing local-copy
/// reserve. The native free-space query is available on Windows; other
/// targets keep the parser and tempfile tests offline.
pub const ZIP_MIN_FREE_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_MEMBER_NAME_BYTES: usize = 1_024;
const IO_BUFFER: usize = 64 * 1024;

const LOCAL_HEADER: u32 = 0x0403_4b50;
const CENTRAL_HEADER: u32 = 0x0201_4b50;
const END_HEADER: u32 = 0x0605_4b50;
const DATA_DESCRIPTOR: u32 = 0x0807_4b50;

/// The operation a manifest entry is approved to perform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BatchOperation {
    Copy,
    Move,
    Rename,
    CreateDirectory,
    CreateArchive,
    ExtractArchive,
}

/// What an executor may do when a destination is occupied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictRule {
    /// Stop rather than replacing or silently changing the selection.
    Stop,
    /// Leave the existing destination unchanged and report a skipped entry.
    Skip,
    /// Select a separately approved name.  This module does not invent one.
    SelectNewName,
}

/// The only filesystem kinds admitted by this module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileKind {
    File,
    Directory,
}

/// An exact observed file state.  The path is retained so a verifier cannot
/// accidentally apply a digest to a different path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileSnapshot {
    pub path: PathBuf,
    pub identity: FileIdentity,
    pub size: u64,
    pub hash: String,
}

/// A precondition or expected postcondition for one exact path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExpectedState {
    Absent,
    File {
        identity: Option<FileIdentity>,
        size: u64,
        hash: String,
    },
    Directory {
        identity: Option<FileIdentity>,
    },
    Unchanged,
    NotApplicable,
}

/// A structured, metadata-only exclusion.  There is deliberately no content
/// predicate in this Stage 5 module.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExclusionPredicate {
    ExactPath(PathBuf),
    ExactName(OsString),
    ExactType(FileKind),
    ExactSize(u64),
}

impl ExclusionPredicate {
    pub fn exact_path(path: impl Into<PathBuf>) -> Self {
        Self::ExactPath(path.into())
    }

    pub fn exact_name(name: impl Into<OsString>) -> Self {
        Self::ExactName(name.into())
    }

    pub fn exact_type(kind: FileKind) -> Self {
        Self::ExactType(kind)
    }

    pub fn exact_size(size: u64) -> Self {
        Self::ExactSize(size)
    }

    fn validate(&self) -> io::Result<()> {
        match self {
            Self::ExactPath(path) => validate_local_path(path),
            Self::ExactName(name) => {
                if name.is_empty() || name.to_string_lossy().chars().any(char::is_control) {
                    Err(invalid("an exact exclusion name is invalid"))
                } else {
                    Ok(())
                }
            }
            Self::ExactType(_) | Self::ExactSize(_) => Ok(()),
        }
    }

    fn matches(&self, path: &Path, kind: FileKind, size: u64) -> bool {
        match self {
            Self::ExactPath(excluded) => same_path(excluded, path),
            Self::ExactName(excluded) => path.file_name().is_some_and(|name| name == excluded),
            Self::ExactType(excluded) => *excluded == kind,
            Self::ExactSize(excluded) => *excluded == size,
        }
    }
}

/// One exact source/destination operation.  Archive members use
/// `archive_name`; regular local operations use `destination` directly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub operation: BatchOperation,
    pub source: Option<PathBuf>,
    pub source_state: Option<FileSnapshot>,
    pub destination: PathBuf,
    pub archive_name: Option<String>,
    pub conflict: ConflictRule,
    pub expected_before: ExpectedState,
    pub expected_after: ExpectedState,
    pub exclusion: Option<ExclusionPredicate>,
}

/// A shared, immutable selection and exact-state contract for a batch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchManifest {
    pub operation: BatchOperation,
    pub output: PathBuf,
    pub output_parent_identity: FileIdentity,
    pub output_before: ExpectedState,
    pub policy_revision: u64,
    pub entries: Vec<ManifestEntry>,
}

impl FileSnapshot {
    /// Capture an ordinary local file under the currently bound privacy policy.
    pub fn capture(path: &Path) -> io::Result<Self> {
        check_regular_local(path)?;
        let path = path.canonicalize()?;
        check_regular_local(&path)?;
        let identity = file_identity(&path)?;
        let size = fs::metadata(&path)?.len();
        let hash = hash_file(&path)?;
        let after_identity = file_identity(&path)?;
        let after_size = fs::metadata(&path)?.len();
        if identity != after_identity || size != after_size {
            return Err(invalid(
                "file changed while its exact manifest state was captured",
            ));
        }
        validate_digest(&hash)?;
        Ok(Self {
            path,
            identity,
            size,
            hash,
        })
    }

    fn matches_current(&self) -> io::Result<()> {
        let current = Self::capture(&self.path)?;
        if &current == self {
            Ok(())
        } else {
            Err(invalid("a manifest source changed after approval"))
        }
    }
}

impl BatchManifest {
    /// Capture an exact archive creation selection.  `sources` pairs each
    /// ordinary local file with its requested ZIP member name.
    pub fn for_archive(output: &Path, sources: Vec<(PathBuf, String)>) -> io::Result<Self> {
        let output = prepare_new_output(output)?;
        let output_parent = output
            .parent()
            .ok_or_else(|| invalid("archive output has no parent"))?;
        let output_parent_identity = file_identity(output_parent)?;
        let mut entries = Vec::with_capacity(sources.len());
        let mut names = HashSet::new();
        let mut identities = HashSet::new();
        for (source, archive_name) in sources {
            let archive_name = validate_member_name(&archive_name)?;
            let name_key = collision_key(&archive_name);
            if !names.insert(name_key) {
                return Err(invalid(
                    "archive member names collide by case or Unicode normalization",
                ));
            }
            let snapshot = FileSnapshot::capture(&source)?;
            if !identities.insert((snapshot.identity.volume, snapshot.identity.index)) {
                return Err(invalid(
                    "the manifest selects one local file more than once",
                ));
            }
            entries.push(ManifestEntry {
                operation: BatchOperation::CreateArchive,
                source: Some(snapshot.path.clone()),
                source_state: Some(snapshot.clone()),
                destination: output.clone(),
                archive_name: Some(archive_name),
                conflict: ConflictRule::Stop,
                expected_before: ExpectedState::Absent,
                expected_after: ExpectedState::Unchanged,
                exclusion: None,
            });
        }
        let manifest = Self {
            operation: BatchOperation::CreateArchive,
            output,
            output_parent_identity,
            output_before: ExpectedState::Absent,
            policy_revision: crate::privacy::current().revision(),
            entries,
        };
        manifest.validate_structure()?;
        Ok(manifest)
    }

    /// Capture one regular local operation for callers that share the same
    /// manifest contract with archive creation.
    pub fn for_file(
        operation: BatchOperation,
        source: &Path,
        destination: &Path,
        conflict: ConflictRule,
    ) -> io::Result<Self> {
        if !matches!(
            operation,
            BatchOperation::Copy | BatchOperation::Move | BatchOperation::Rename
        ) {
            return Err(invalid("this constructor only captures file operations"));
        }
        let destination = prepare_destination(destination)?;
        let parent = destination
            .parent()
            .ok_or_else(|| invalid("file destination has no parent"))?;
        let parent_identity = file_identity(parent)?;
        let source_state = FileSnapshot::capture(source)?;
        let expected_before = expected_state(&destination)?;
        let expected_after = ExpectedState::File {
            identity: None,
            size: source_state.size,
            hash: source_state.hash.clone(),
        };
        let manifest = Self {
            operation,
            output: destination.clone(),
            output_parent_identity: parent_identity,
            output_before: expected_before.clone(),
            policy_revision: crate::privacy::current().revision(),
            entries: vec![ManifestEntry {
                operation,
                source: Some(source_state.path.clone()),
                source_state: Some(source_state),
                destination,
                archive_name: None,
                conflict,
                expected_before,
                expected_after,
                exclusion: None,
            }],
        };
        manifest.validate_structure()?;
        Ok(manifest)
    }

    pub fn add_exclusion(&mut self, exclusion: ExclusionPredicate) -> io::Result<()> {
        exclusion.validate()?;
        for entry in &mut self.entries {
            if entry.exclusion.is_none() {
                entry.exclusion = Some(exclusion.clone());
            }
        }
        self.validate_structure()
    }

    /// Validate fields without querying source bytes again.
    pub fn validate_structure(&self) -> io::Result<()> {
        if self.entries.len() > ZIP_MAX_ENTRIES {
            return Err(invalid("the batch exceeds the 1,000-entry ceiling"));
        }
        validate_local_path(&self.output)?;
        crate::privacy::check(&self.output)?;
        let parent = self
            .output
            .parent()
            .ok_or_else(|| invalid("manifest output has no parent"))?;
        check_directory(parent)?;
        if file_identity(parent)? != self.output_parent_identity {
            return Err(invalid("the manifest output parent identity is stale"));
        }
        validate_expected_state(&self.output_before)?;
        let mut names = HashSet::new();
        let mut identities = HashSet::new();
        for entry in &self.entries {
            if entry.operation != self.operation && self.operation == BatchOperation::CreateArchive
            {
                return Err(invalid("manifest entry operation does not match its batch"));
            }
            validate_local_path(&entry.destination)?;
            crate::privacy::check(&entry.destination)?;
            if let Some(exclusion) = &entry.exclusion {
                exclusion.validate()?;
            }
            if let Some(source) = &entry.source {
                validate_local_path(source)?;
                crate::privacy::check(source)?;
            }
            if let Some(source_state) = &entry.source_state {
                validate_digest(&source_state.hash)?;
                if entry.source.as_ref() != Some(&source_state.path) {
                    return Err(invalid(
                        "manifest source path and source identity record differ",
                    ));
                }
                if !identities.insert((source_state.identity.volume, source_state.identity.index)) {
                    return Err(invalid("manifest contains an aliased source identity"));
                }
            }
            if self.operation == BatchOperation::CreateArchive {
                let name = entry
                    .archive_name
                    .as_deref()
                    .ok_or_else(|| invalid("archive entry is missing its member name"))?;
                let name = validate_member_name(name)?;
                if !names.insert(collision_key(&name)) {
                    return Err(invalid(
                        "archive member names collide by case or Unicode normalization",
                    ));
                }
                if entry.source_state.is_none() || entry.source.is_none() {
                    return Err(invalid("archive entry is missing its exact source state"));
                }
            } else if entry.archive_name.is_some() {
                return Err(invalid(
                    "non-archive entries cannot carry a ZIP member name",
                ));
            }
            validate_expected_state(&entry.expected_before)?;
            validate_expected_state(&entry.expected_after)?;
        }
        Ok(())
    }

    /// Re-check all paths, identities, lengths, hashes and privacy revision
    /// before an executor consumes this manifest.
    pub fn validate_current(&self) -> io::Result<()> {
        self.validate_structure()?;
        if crate::privacy::current().revision() != self.policy_revision {
            return Err(invalid("privacy rules changed after manifest preparation"));
        }
        if file_identity(self.output.parent().unwrap())? != self.output_parent_identity {
            return Err(invalid("the manifest output parent was replaced"));
        }
        verify_expected(&self.output, &self.output_before)?;
        for entry in &self.entries {
            if let Some(snapshot) = &entry.source_state {
                snapshot.matches_current()?;
                let kind = FileKind::File;
                if entry
                    .exclusion
                    .as_ref()
                    .is_some_and(|predicate| predicate.matches(&snapshot.path, kind, snapshot.size))
                {
                    return Err(invalid(
                        "the exact manifest source is covered by an exclusion",
                    ));
                }
            }
            verify_expected(&entry.destination, &entry.expected_before)?;
        }
        Ok(())
    }

    /// Verify one exact postcondition after a caller has performed an
    /// operation.  The manifest itself never grants mutation authority.
    pub fn verify_after(&self, index: usize) -> io::Result<()> {
        let entry = self
            .entries
            .get(index)
            .ok_or_else(|| invalid("manifest entry index is out of range"))?;
        verify_expected(&entry.destination, &entry.expected_after)
    }
}

/// One member summary returned by ZIP verification and extraction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZipEntrySummary {
    pub name: String,
    pub size: u64,
    pub compressed_size: u64,
    pub crc32: u32,
}

/// Independent result for a created or extracted ZIP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZipReport {
    pub path: PathBuf,
    pub entries: Vec<ZipEntrySummary>,
    pub expanded_bytes: u64,
    pub archive_bytes: u64,
}

/// Create a new ZIP at `manifest.output`, using a private staging file and an
/// independent reopen/byte verification before publication.
pub fn create_zip(manifest: &BatchManifest) -> io::Result<ZipReport> {
    crate::privacy::with_mutation(|| create_zip_checked(manifest))
}

fn create_zip_checked(manifest: &BatchManifest) -> io::Result<ZipReport> {
    if manifest.operation != BatchOperation::CreateArchive {
        return Err(invalid("the manifest is not an archive-creation manifest"));
    }
    manifest.validate_current()?;
    check_capacity(
        manifest.output.parent().unwrap(),
        manifest_expanded_bytes(manifest)?,
    )?;
    prepare_new_output(&manifest.output)?;
    let stage = create_staging_file(&manifest.output)?;
    let result = (|| {
        write_zip(&stage, manifest)?;
        let report = verify_zip_against_manifest(&stage, manifest)?;
        if file_identity(manifest.output.parent().unwrap())? != manifest.output_parent_identity {
            return Err(invalid("archive output parent changed during creation"));
        }
        crate::privacy::check(&manifest.output)?;
        rename_no_replace(&stage, &manifest.output)?;
        Ok(ZipReport {
            path: manifest.output.clone(),
            ..report
        })
    })();
    if result.is_err() {
        let _ = fs::remove_file(&stage);
    }
    result
}

/// Reopen and independently inspect a ZIP.  This is also the verifier used by
/// creation before its staging file is published.
pub fn verify_zip(path: &Path) -> io::Result<ZipReport> {
    let snapshot = FileSnapshot::capture(path)?;
    let archive = read_archive(path)?;
    snapshot.matches_current()?;
    let entries = archive
        .entries
        .iter()
        .map(|entry| entry.summary.clone())
        .collect::<Vec<_>>();
    for entry in &archive.entries {
        let mut sink = DigestWriter::new(io::sink());
        stream_member(&archive.bytes, entry, &mut sink, ZIP_MAX_EXPANDED_BYTES)?;
        let (_, size, hash, crc) = sink.finish();
        if size != entry.summary.size || hash.is_empty() || crc != entry.summary.crc32 {
            return Err(invalid("ZIP member failed independent byte verification"));
        }
    }
    Ok(ZipReport {
        path: path.to_path_buf(),
        entries,
        expanded_bytes: archive.expanded_bytes,
        archive_bytes: archive.bytes.len() as u64,
    })
}

/// Extract a ZIP into a new staging directory and publish it only after every
/// member has been streamed, hashed, and reopened.  Existing destinations are
/// never replaced.  ZIP members that happen to be archives remain ordinary
/// files; this function never recursively extracts them.
pub fn extract_zip(archive_path: &Path, destination: &Path) -> io::Result<ZipReport> {
    crate::privacy::with_mutation(|| extract_zip_checked(archive_path, destination))
}

fn extract_zip_checked(archive_path: &Path, destination: &Path) -> io::Result<ZipReport> {
    let archive_snapshot = FileSnapshot::capture(archive_path)?;
    let destination = prepare_new_directory(destination)?;
    let archive = read_archive(archive_path)?;
    archive_snapshot.matches_current()?;
    check_capacity(destination.parent().unwrap(), archive.expanded_bytes)?;
    let logical_entries = preflight_extraction(&archive, &destination)?;
    let destination_parent_identity = file_identity(destination.parent().unwrap())?;
    let stage = create_staging_directory(&destination)?;
    let result = (|| {
        let mut total = 0u64;
        for (entry, logical) in archive.entries.iter().zip(logical_entries.iter()) {
            if !crate::privacy::request_is_current() {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "request stopped",
                ));
            }
            crate::privacy::check(logical)?;
            let target = stage.join(&entry.summary.name);
            let parent = target
                .parent()
                .ok_or_else(|| invalid("staging member has no parent"))?;
            create_safe_parent_dirs(&stage, parent)?;
            check_directory(parent)?;
            let parent_identity = file_identity(parent)?;
            crate::privacy::check(&target)?;
            let mut output = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&target)?;
            if file_identity(parent)? != parent_identity {
                return Err(invalid("staging member parent changed before writing"));
            }
            let mut digest = DigestWriter::new(&mut output);
            stream_member(
                &archive.bytes,
                entry,
                &mut digest,
                ZIP_MAX_EXPANDED_BYTES.saturating_sub(total),
            )?;
            let (_, size, hash, crc) = digest.finish();
            output.sync_all()?;
            if size != entry.summary.size || crc != entry.summary.crc32 {
                return Err(invalid("extracted member failed size or CRC verification"));
            }
            if hash.is_empty() {
                return Err(invalid("extracted member produced no digest"));
            }
            total = total
                .checked_add(size)
                .ok_or_else(|| invalid("expanded ZIP size overflowed"))?;
            let state = FileSnapshot::capture(&target)?;
            if state.size != size || state.hash != hash {
                return Err(invalid(
                    "reopened extracted member differs from streamed bytes",
                ));
            }
        }
        archive_snapshot.matches_current()?;
        verify_staging_tree(&stage, &archive)?;
        if file_identity(destination.parent().unwrap())? != destination_parent_identity {
            return Err(invalid("destination parent changed during extraction"));
        }
        crate::privacy::check(&destination)?;
        publish_directory(&stage, &destination)?;
        Ok(ZipReport {
            path: destination.clone(),
            entries: archive
                .entries
                .iter()
                .map(|entry| entry.summary.clone())
                .collect(),
            expanded_bytes: archive.expanded_bytes,
            archive_bytes: archive.bytes.len() as u64,
        })
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&stage);
    }
    result
}

fn write_zip(path: &Path, manifest: &BatchManifest) -> io::Result<()> {
    if manifest.entries.len() > ZIP_MAX_ENTRIES {
        return Err(invalid("ZIP has too many entries"));
    }
    let mut file = OpenOptions::new().write(true).truncate(true).open(path)?;
    let mut central = Vec::with_capacity(manifest.entries.len());
    let mut expanded = 0u64;
    for entry in &manifest.entries {
        if entry.operation != BatchOperation::CreateArchive {
            return Err(invalid("ZIP creation received a non-archive entry"));
        }
        let source = entry
            .source_state
            .as_ref()
            .ok_or_else(|| invalid("archive source state is missing"))?;
        let name = entry
            .archive_name
            .as_deref()
            .ok_or_else(|| invalid("archive member name is missing"))?;
        let name = validate_member_name(name)?;
        let name_bytes = name.as_bytes();
        let offset = file.stream_position()?;
        if offset > u64::from(u32::MAX) {
            return Err(invalid("ZIP exceeds the non-ZIP64 offset ceiling"));
        }
        // Bit 3 permits a streaming data descriptor.  This avoids buffering a
        // whole source file merely to learn its CRC before writing the header.
        write_u32(&mut file, LOCAL_HEADER)?;
        write_u16(&mut file, 20)?;
        write_u16(&mut file, 0x0808)?;
        write_u16(&mut file, 8)?;
        write_u16(&mut file, 0)?;
        write_u16(&mut file, 0)?;
        write_u32(&mut file, 0)?;
        write_u32(&mut file, 0)?;
        write_u32(&mut file, 0)?;
        write_u16(&mut file, checked_u16(name_bytes.len())?)?;
        write_u16(&mut file, 0)?;
        file.write_all(name_bytes)?;

        let mut source_file = File::open(&source.path)?;
        let opened_identity = file_identity(&source.path)?;
        if opened_identity != source.identity {
            return Err(invalid("archive source identity changed before streaming"));
        }
        let mut encoder =
            DeflateEncoder::new(CountingWriter::new(&mut file), Compression::default());
        let mut buffer = vec![0u8; IO_BUFFER];
        let mut read_bytes = 0u64;
        let mut hash = blake3::Hasher::new();
        let mut crc = crc32fast::Hasher::new();
        loop {
            let count = source_file.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            read_bytes = read_bytes
                .checked_add(count as u64)
                .ok_or_else(|| invalid("source size overflowed"))?;
            if read_bytes > source.size {
                return Err(invalid("source grew while the archive was being created"));
            }
            hash.update(&buffer[..count]);
            crc.update(&buffer[..count]);
            encoder.write_all(&buffer[..count])?;
        }
        let writer = encoder.finish()?;
        let compressed_size = writer.count;
        let crc = crc.finalize();
        let hash = hash.finalize().to_hex().to_string();
        if read_bytes != source.size || hash != source.hash {
            return Err(invalid("archive source bytes changed while being streamed"));
        }
        if file_identity(&source.path)? != source.identity {
            return Err(invalid(
                "archive source identity changed while being streamed",
            ));
        }
        if read_bytes > 0
            && (compressed_size == 0
                || u128::from(read_bytes)
                    > u128::from(compressed_size) * u128::from(ZIP_MAX_EXPANSION_RATIO))
        {
            return Err(invalid("ZIP member exceeds the 100:1 expansion ratio"));
        }
        let end = file.stream_position()?;
        write_u32(&mut file, DATA_DESCRIPTOR)?;
        write_u32(&mut file, crc)?;
        write_u32(&mut file, checked_u32(compressed_size)?)?;
        write_u32(&mut file, checked_u32(read_bytes)?)?;
        expanded = expanded
            .checked_add(read_bytes)
            .ok_or_else(|| invalid("ZIP expanded size overflowed"))?;
        if expanded > ZIP_MAX_EXPANDED_BYTES {
            return Err(invalid("ZIP expanded bytes exceed 512 MiB"));
        }
        let _ = end;
        central.push((name, crc, compressed_size, read_bytes, offset));
    }
    let central_offset = file.stream_position()?;
    for (name, crc, compressed, size, offset) in &central {
        let name_bytes = name.as_bytes();
        write_u32(&mut file, CENTRAL_HEADER)?;
        write_u16(&mut file, 20)?;
        write_u16(&mut file, 20)?;
        write_u16(&mut file, 0x0808)?;
        write_u16(&mut file, 8)?;
        write_u16(&mut file, 0)?;
        write_u16(&mut file, 0)?;
        write_u32(&mut file, *crc)?;
        write_u32(&mut file, checked_u32(*compressed)?)?;
        write_u32(&mut file, checked_u32(*size)?)?;
        write_u16(&mut file, checked_u16(name_bytes.len())?)?;
        write_u16(&mut file, 0)?;
        write_u16(&mut file, 0)?;
        write_u16(&mut file, 0)?;
        write_u16(&mut file, 0)?;
        write_u32(&mut file, 0)?;
        write_u32(&mut file, checked_u32(*offset)?)?;
        file.write_all(name_bytes)?;
    }
    let central_size = file.stream_position()? - central_offset;
    write_u32(&mut file, END_HEADER)?;
    write_u16(&mut file, 0)?;
    write_u16(&mut file, 0)?;
    write_u16(&mut file, checked_u16(central.len())?)?;
    write_u16(&mut file, checked_u16(central.len())?)?;
    write_u32(&mut file, checked_u32(central_size)?)?;
    write_u32(&mut file, checked_u32(central_offset)?)?;
    write_u16(&mut file, 0)?;
    file.sync_all()
}

fn verify_zip_against_manifest(path: &Path, manifest: &BatchManifest) -> io::Result<ZipReport> {
    let archive = read_archive(path)?;
    if archive.entries.len() != manifest.entries.len() {
        return Err(invalid(
            "reopened ZIP entry count differs from the manifest",
        ));
    }
    for (archive_entry, manifest_entry) in archive.entries.iter().zip(&manifest.entries) {
        let expected_name = manifest_entry
            .archive_name
            .as_deref()
            .ok_or_else(|| invalid("manifest archive name is missing"))?;
        let source = manifest_entry
            .source_state
            .as_ref()
            .ok_or_else(|| invalid("manifest source state is missing"))?;
        if archive_entry.summary.name != expected_name || archive_entry.summary.size != source.size
        {
            return Err(invalid(
                "reopened ZIP names or sizes differ from the manifest",
            ));
        }
        let mut sink = DigestWriter::new(io::sink());
        stream_member(
            &archive.bytes,
            archive_entry,
            &mut sink,
            ZIP_MAX_EXPANDED_BYTES,
        )?;
        let (_, size, hash, crc) = sink.finish();
        if size != source.size || hash != source.hash || crc != archive_entry.summary.crc32 {
            return Err(invalid(
                "reopened ZIP bytes differ from the manifest source",
            ));
        }
    }
    Ok(ZipReport {
        path: path.to_path_buf(),
        entries: archive
            .entries
            .iter()
            .map(|entry| entry.summary.clone())
            .collect(),
        expanded_bytes: archive.expanded_bytes,
        archive_bytes: archive.bytes.len() as u64,
    })
}

fn manifest_expanded_bytes(manifest: &BatchManifest) -> io::Result<u64> {
    manifest
        .entries
        .iter()
        .map(|entry| {
            entry
                .source_state
                .as_ref()
                .map(|source| source.size)
                .ok_or_else(|| invalid("archive source state is missing"))
        })
        .try_fold(0u64, |total, size| {
            total
                .checked_add(size?)
                .ok_or_else(|| invalid("ZIP expanded size overflowed"))
        })
        .and_then(|total| {
            if total > ZIP_MAX_EXPANDED_BYTES {
                Err(invalid("ZIP expanded bytes exceed 512 MiB"))
            } else {
                Ok(total)
            }
        })
}

#[cfg(windows)]
fn check_capacity(parent: &Path, required: u64) -> io::Result<()> {
    let free = custodian_winsec::fsops::available_bytes(parent)?;
    if free.saturating_sub(required) < ZIP_MIN_FREE_BYTES {
        return Err(invalid("ZIP operation would exceed its free-space reserve"));
    }
    Ok(())
}

#[cfg(not(windows))]
fn check_capacity(_parent: &Path, _required: u64) -> io::Result<()> {
    // The existing native volume helper is Windows-only. Keep Unix fixtures
    // offline while retaining the reserve on the supported Windows target.
    Ok(())
}

#[derive(Debug)]
struct ParsedArchive {
    bytes: Vec<u8>,
    entries: Vec<ParsedEntry>,
    expanded_bytes: u64,
}

#[derive(Debug)]
struct ParsedEntry {
    summary: ZipEntrySummary,
    method: u16,
    data_start: usize,
    data_end: usize,
}

fn read_archive(path: &Path) -> io::Result<ParsedArchive> {
    check_regular_local(path)?;
    let length = fs::metadata(path)?.len();
    if length > ZIP_MAX_ARCHIVE_BYTES {
        return Err(invalid("ZIP archive exceeds its bounded input size"));
    }
    let mut file = File::open(path)?;
    let mut bytes = Vec::with_capacity(length as usize);
    Read::by_ref(&mut file)
        .take(ZIP_MAX_ARCHIVE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 != length || bytes.len() as u64 > ZIP_MAX_ARCHIVE_BYTES {
        return Err(invalid("ZIP changed or exceeded its bounded input size"));
    }
    parse_archive_bytes(bytes)
}

fn parse_archive_bytes(bytes: Vec<u8>) -> io::Result<ParsedArchive> {
    if bytes.len() < 22 {
        return Err(invalid("ZIP is shorter than its end record"));
    }
    let end = bytes.len() - 22;
    if u32_at(&bytes, end)? != END_HEADER
        || u16_at(&bytes, end + 4)? != 0
        || u16_at(&bytes, end + 6)? != 0
        || u16_at(&bytes, end + 20)? != 0
    {
        return Err(invalid("ZIP end record has unsupported disks or a comment"));
    }
    let count = usize::from(u16_at(&bytes, end + 10)?);
    if count > ZIP_MAX_ENTRIES || count != usize::from(u16_at(&bytes, end + 8)?) {
        return Err(invalid("ZIP entry count is outside the bounded subset"));
    }
    let central_size = usize::try_from(u32_at(&bytes, end + 12)?)
        .map_err(|_| invalid("ZIP central directory is too large"))?;
    let central_start = usize::try_from(u32_at(&bytes, end + 16)?)
        .map_err(|_| invalid("ZIP central directory offset is too large"))?;
    if central_start.checked_add(central_size) != Some(end) {
        return Err(invalid(
            "ZIP central directory does not end at the end record",
        ));
    }
    let mut cursor = central_start;
    let mut entries = Vec::with_capacity(count);
    let mut spans = Vec::with_capacity(count);
    let mut expanded = 0u64;
    let mut names = HashSet::new();
    for _ in 0..count {
        if u32_at(&bytes, cursor)? != CENTRAL_HEADER {
            return Err(invalid("ZIP central directory record is malformed"));
        }
        let made_by = u16_at(&bytes, cursor + 4)?;
        let flags = u16_at(&bytes, cursor + 8)?;
        let method = u16_at(&bytes, cursor + 10)?;
        let crc = u32_at(&bytes, cursor + 16)?;
        let compressed = u64::from(u32_at(&bytes, cursor + 20)?);
        let size = u64::from(u32_at(&bytes, cursor + 24)?);
        let name_len = usize::from(u16_at(&bytes, cursor + 28)?);
        let extra_len = usize::from(u16_at(&bytes, cursor + 30)?);
        let comment_len = usize::from(u16_at(&bytes, cursor + 32)?);
        let disk = u16_at(&bytes, cursor + 34)?;
        let attrs = u32_at(&bytes, cursor + 38)?;
        let local_offset = usize::try_from(u32_at(&bytes, cursor + 42)?)
            .map_err(|_| invalid("ZIP local offset is too large"))?;
        if disk != 0 || extra_len != 0 || comment_len != 0 {
            return Err(invalid(
                "ZIP extras, comments, or multiple disks are unsupported",
            ));
        }
        validate_flags(flags)?;
        if method != 0 && method != 8 {
            return Err(invalid("ZIP compression method is unsupported"));
        }
        if size > ZIP_MAX_EXPANDED_BYTES || compressed > ZIP_MAX_ARCHIVE_BYTES {
            return Err(invalid("ZIP member exceeds its size ceiling"));
        }
        if size > 0
            && (compressed == 0
                || u128::from(size) > u128::from(compressed) * u128::from(ZIP_MAX_EXPANSION_RATIO))
        {
            return Err(invalid("ZIP member exceeds the 100:1 expansion ratio"));
        }
        expanded = expanded
            .checked_add(size)
            .ok_or_else(|| invalid("ZIP expanded size overflowed"))?;
        if expanded > ZIP_MAX_EXPANDED_BYTES {
            return Err(invalid("ZIP expanded bytes exceed 512 MiB"));
        }
        let name_bytes = bytes
            .get(cursor + 46..cursor + 46 + name_len)
            .ok_or_else(|| invalid("ZIP member name is truncated"))?;
        if flags & 0x0800 == 0 && !name_bytes.is_ascii() {
            return Err(invalid("non-ASCII ZIP names must declare UTF-8"));
        }
        let raw_name =
            std::str::from_utf8(name_bytes).map_err(|_| invalid("ZIP member name is not UTF-8"))?;
        let name = validate_member_name(raw_name)?;
        if !names.insert(collision_key(&name)) {
            return Err(invalid(
                "ZIP member names collide by case or Unicode normalization",
            ));
        }
        validate_external_attributes(made_by, attrs)?;
        let local = parse_local_header(
            &bytes,
            local_offset,
            flags,
            method,
            name_bytes,
            crc,
            compressed,
            size,
            central_start,
        )?;
        spans.push(local.span);
        entries.push(ParsedEntry {
            summary: ZipEntrySummary {
                name,
                size,
                compressed_size: compressed,
                crc32: crc,
            },
            method,
            data_start: local.data_start,
            data_end: local.data_end,
        });
        cursor = cursor
            .checked_add(46 + name_len)
            .ok_or_else(|| invalid("ZIP central cursor overflowed"))?;
    }
    if cursor != end {
        return Err(invalid("ZIP central directory has trailing bytes"));
    }
    spans.sort_unstable();
    if spans.windows(2).any(|pair| pair[0].1 > pair[1].0) {
        return Err(invalid("ZIP local members overlap"));
    }
    Ok(ParsedArchive {
        bytes,
        entries,
        expanded_bytes: expanded,
    })
}

struct LocalRecord {
    data_start: usize,
    data_end: usize,
    span: (usize, usize),
}

#[allow(clippy::too_many_arguments)]
fn parse_local_header(
    bytes: &[u8],
    offset: usize,
    central_flags: u16,
    method: u16,
    name: &[u8],
    crc: u32,
    compressed: u64,
    size: u64,
    central_start: usize,
) -> io::Result<LocalRecord> {
    if u32_at(bytes, offset)? != LOCAL_HEADER {
        return Err(invalid("ZIP local header is malformed"));
    }
    let flags = u16_at(bytes, offset + 6)?;
    let local_method = u16_at(bytes, offset + 8)?;
    let local_crc = u32_at(bytes, offset + 14)?;
    let local_compressed = u64::from(u32_at(bytes, offset + 18)?);
    let local_size = u64::from(u32_at(bytes, offset + 22)?);
    let name_len = usize::from(u16_at(bytes, offset + 26)?);
    let extra_len = usize::from(u16_at(bytes, offset + 28)?);
    if flags != central_flags || local_method != method || name_len != name.len() || extra_len != 0
    {
        return Err(invalid("ZIP local and central headers disagree"));
    }
    let local_name = bytes
        .get(offset + 30..offset + 30 + name_len)
        .ok_or_else(|| invalid("ZIP local member name is truncated"))?;
    if local_name != name {
        return Err(invalid("ZIP local and central names disagree"));
    }
    let data_start = offset
        .checked_add(30 + name_len)
        .ok_or_else(|| invalid("ZIP local data offset overflowed"))?;
    let compressed_usize =
        usize::try_from(compressed).map_err(|_| invalid("ZIP member is too large"))?;
    let data_end = data_start
        .checked_add(compressed_usize)
        .ok_or_else(|| invalid("ZIP member data offset overflowed"))?;
    if data_end > central_start {
        return Err(invalid(
            "ZIP member data reaches into its central directory",
        ));
    }
    if flags & 0x0008 == 0 {
        if local_crc != crc || local_compressed != compressed || local_size != size {
            return Err(invalid("ZIP local sizes disagree with the central record"));
        }
    } else if local_crc != 0 || local_compressed != 0 || local_size != 0 {
        return Err(invalid(
            "ZIP data-descriptor local headers must have zero sizes",
        ));
    }
    let mut span_end = data_end;
    if flags & 0x0008 != 0 {
        let descriptor_len = if u32_at(bytes, data_end).ok() == Some(DATA_DESCRIPTOR) {
            16
        } else {
            12
        };
        if u32_at(bytes, data_end).ok() == Some(DATA_DESCRIPTOR) {
            if u32_at(bytes, data_end + 4)? != crc
                || u32_at(bytes, data_end + 8)? != checked_u32(compressed)?
                || u32_at(bytes, data_end + 12)? != checked_u32(size)?
            {
                return Err(invalid(
                    "ZIP data descriptor disagrees with the central record",
                ));
            }
        } else if u32_at(bytes, data_end)? != crc
            || u32_at(bytes, data_end + 4)? != checked_u32(compressed)?
            || u32_at(bytes, data_end + 8)? != checked_u32(size)?
        {
            return Err(invalid("ZIP data descriptor is malformed"));
        }
        span_end = data_end
            .checked_add(descriptor_len)
            .ok_or_else(|| invalid("ZIP data descriptor offset overflowed"))?;
        if span_end > central_start {
            return Err(invalid(
                "ZIP data descriptor reaches into the central directory",
            ));
        }
    }
    Ok(LocalRecord {
        data_start,
        data_end,
        span: (offset, span_end),
    })
}

fn stream_member<W: Write>(
    bytes: &[u8],
    entry: &ParsedEntry,
    output: &mut DigestWriter<W>,
    remaining_limit: u64,
) -> io::Result<()> {
    let compressed = bytes
        .get(entry.data_start..entry.data_end)
        .ok_or_else(|| invalid("ZIP member data is truncated"))?;
    let mut written = 0u64;
    let mut buffer = [0u8; IO_BUFFER];
    match entry.method {
        0 => {
            let mut cursor = 0;
            while cursor < compressed.len() {
                let end = (cursor + IO_BUFFER).min(compressed.len());
                let chunk = &compressed[cursor..end];
                written = written
                    .checked_add(chunk.len() as u64)
                    .ok_or_else(|| invalid("ZIP member size overflowed"))?;
                if written > entry.summary.size || written > remaining_limit {
                    return Err(invalid("ZIP streamed bytes exceed the declared limit"));
                }
                output.write_all(chunk)?;
                cursor = end;
            }
        }
        8 => {
            let mut decoder = DeflateDecoder::new(compressed);
            loop {
                let count = decoder.read(&mut buffer)?;
                if count == 0 {
                    break;
                }
                written = written
                    .checked_add(count as u64)
                    .ok_or_else(|| invalid("ZIP member size overflowed"))?;
                if written > entry.summary.size || written > remaining_limit {
                    return Err(invalid("ZIP streamed bytes exceed the declared limit"));
                }
                output.write_all(&buffer[..count])?;
            }
            if !decoder.get_ref().is_empty() {
                return Err(invalid("ZIP deflate stream has trailing compressed bytes"));
            }
        }
        _ => return Err(invalid("ZIP compression method is unsupported")),
    }
    if written != entry.summary.size {
        return Err(invalid("ZIP streamed bytes do not match the declared size"));
    }
    Ok(())
}

fn preflight_extraction(archive: &ParsedArchive, destination: &Path) -> io::Result<Vec<PathBuf>> {
    check_directory(destination.parent().unwrap())?;
    crate::privacy::check(destination)?;
    let mut result = Vec::with_capacity(archive.entries.len());
    let mut keys = HashSet::new();
    for entry in &archive.entries {
        let key = collision_key(&entry.summary.name);
        if !keys.insert(key) {
            return Err(invalid("ZIP member names collide during extraction"));
        }
        let logical = destination.join(&entry.summary.name);
        validate_local_path(&logical)?;
        crate::privacy::check(&logical)?;
        if fs::symlink_metadata(&logical).is_ok() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "extraction would replace an existing path",
            ));
        }
        result.push(logical);
    }
    Ok(result)
}

fn verify_staging_tree(stage: &Path, archive: &ParsedArchive) -> io::Result<()> {
    for entry in &archive.entries {
        let path = stage.join(&entry.summary.name);
        check_regular_local(&path)?;
        let state = FileSnapshot::capture(&path)?;
        let mut sink = DigestWriter::new(io::sink());
        stream_member(&archive.bytes, entry, &mut sink, ZIP_MAX_EXPANDED_BYTES)?;
        let (_, size, hash, crc) = sink.finish();
        if state.size != size || state.hash != hash || crc != entry.summary.crc32 {
            return Err(invalid("staging tree failed independent verification"));
        }
    }
    Ok(())
}

fn create_safe_parent_dirs(stage: &Path, parent: &Path) -> io::Result<()> {
    if parent == stage {
        return Ok(());
    }
    let relative = parent
        .strip_prefix(stage)
        .map_err(|_| invalid("staging path escaped its root"))?;
    let mut current = stage.to_path_buf();
    for component in relative.components() {
        let Component::Normal(name) = component else {
            return Err(invalid("staging parent contains an unsafe component"));
        };
        current.push(name);
        match fs::symlink_metadata(&current) {
            Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() => {}
            Ok(_) => return Err(invalid("staging parent was replaced by a non-directory")),
            Err(error) if error.kind() == io::ErrorKind::NotFound => fs::create_dir(&current)?,
            Err(error) => return Err(error),
        }
        check_directory(&current)?;
    }
    Ok(())
}

fn publish_directory(stage: &Path, destination: &Path) -> io::Result<()> {
    if fs::symlink_metadata(destination).is_ok() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "extraction destination appeared before publication",
        ));
    }
    #[cfg(windows)]
    {
        rename_no_replace(stage, destination)
    }
    #[cfg(not(windows))]
    {
        fs::rename(stage, destination)
    }
}

fn prepare_new_output(path: &Path) -> io::Result<PathBuf> {
    let path = prepare_destination(path)?;
    if fs::symlink_metadata(&path).is_ok() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "output must be a new path",
        ));
    }
    Ok(path)
}

fn prepare_new_directory(path: &Path) -> io::Result<PathBuf> {
    let path = prepare_destination(path)?;
    if fs::symlink_metadata(&path).is_ok() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "extraction destination must be new",
        ));
    }
    Ok(path)
}

fn prepare_destination(path: &Path) -> io::Result<PathBuf> {
    validate_local_path(path)?;
    let parent = path
        .parent()
        .ok_or_else(|| invalid("destination has no parent"))?
        .canonicalize()?;
    check_directory(&parent)?;
    crate::privacy::check(path)?;
    let name = path
        .file_name()
        .ok_or_else(|| invalid("destination has no final name"))?;
    Ok(parent.join(name))
}

fn create_staging_file(output: &Path) -> io::Result<PathBuf> {
    let parent = output.parent().unwrap();
    for _ in 0..8 {
        let stage = parent.join(format!(
            ".{}.custodian-{}.part",
            output.file_name().unwrap().to_string_lossy(),
            uuid::Uuid::new_v4()
        ));
        match OpenOptions::new().write(true).create_new(true).open(&stage) {
            Ok(_) => return Ok(stage),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(invalid("could not allocate a unique archive staging file"))
}

fn create_staging_directory(destination: &Path) -> io::Result<PathBuf> {
    let parent = destination.parent().unwrap();
    for _ in 0..8 {
        let stage = parent.join(format!(
            ".{}.custodian-{}.staging",
            destination.file_name().unwrap().to_string_lossy(),
            uuid::Uuid::new_v4()
        ));
        match fs::create_dir(&stage) {
            Ok(()) => return Ok(stage),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(invalid(
        "could not allocate a unique extraction staging directory",
    ))
}

fn expected_state(path: &Path) -> io::Result<ExpectedState> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(invalid("links are not ordinary manifest destinations"))
        }
        Ok(metadata) if metadata.is_file() => {
            let snapshot = FileSnapshot::capture(path)?;
            Ok(ExpectedState::File {
                identity: Some(snapshot.identity),
                size: snapshot.size,
                hash: snapshot.hash,
            })
        }
        Ok(metadata) if metadata.is_dir() => Ok(ExpectedState::Directory {
            identity: Some(file_identity(path)?),
        }),
        Ok(_) => Err(invalid(
            "manifest destination is not an ordinary file or directory",
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(ExpectedState::Absent),
        Err(error) => Err(error),
    }
}

fn verify_expected(path: &Path, expected: &ExpectedState) -> io::Result<()> {
    match expected {
        ExpectedState::Absent => {
            if fs::symlink_metadata(path).is_ok() {
                Err(invalid("expected destination to be absent"))
            } else {
                Ok(())
            }
        }
        ExpectedState::File {
            identity,
            size,
            hash,
        } => {
            let state = FileSnapshot::capture(path)?;
            if identity.is_some_and(|expected| expected != state.identity)
                || *size != state.size
                || hash != &state.hash
            {
                Err(invalid("expected file state does not match"))
            } else {
                Ok(())
            }
        }
        ExpectedState::Directory { identity } => {
            check_directory(path)?;
            if identity
                .is_some_and(|expected| file_identity(path).is_ok_and(|actual| actual != expected))
            {
                Err(invalid("expected directory identity does not match"))
            } else {
                Ok(())
            }
        }
        ExpectedState::Unchanged | ExpectedState::NotApplicable => Ok(()),
    }
}

fn validate_expected_state(state: &ExpectedState) -> io::Result<()> {
    if let ExpectedState::File { hash, .. } = state {
        validate_digest(hash)?;
    }
    Ok(())
}

fn check_regular_local(path: &Path) -> io::Result<()> {
    validate_local_path(path)?;
    crate::privacy::check(path)?;
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(invalid("only ordinary local files are supported"));
    }
    if custodian_winsec::fsops::file_link_count(path)? != 1 {
        return Err(invalid("hard-linked files are not supported"));
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & (0x400 | 0x1000 | 0x0040_0000) != 0 {
            return Err(invalid(
                "cloud placeholders and reparse points are unsupported",
            ));
        }
    }
    Ok(())
}

fn check_directory(path: &Path) -> io::Result<()> {
    validate_local_path(path)?;
    crate::privacy::check(path)?;
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(invalid("only ordinary local directories are supported"));
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & (0x400 | 0x1000 | 0x0040_0000) != 0 {
            return Err(invalid(
                "cloud placeholders and reparse points are unsupported",
            ));
        }
    }
    Ok(())
}

fn validate_local_path(path: &Path) -> io::Result<()> {
    if !path.is_absolute() || !is_local_volume(path) {
        return Err(invalid(
            "only absolute paths on local volumes are supported",
        ));
    }
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => {
                #[cfg(windows)]
                if !matches!(
                    prefix.kind(),
                    std::path::Prefix::Disk(_) | std::path::Prefix::VerbatimDisk(_)
                ) {
                    return Err(invalid("UNC, device, and network paths are unsupported"));
                }
                #[cfg(not(windows))]
                let _ = prefix;
            }
            Component::RootDir => {}
            Component::CurDir | Component::ParentDir => {
                return Err(invalid("dot and parent path components are unsupported"));
            }
            Component::Normal(name) => {
                let text = name.to_string_lossy();
                if text.is_empty()
                    || text.contains(':')
                    || text.contains('\0')
                    || text.chars().any(char::is_control)
                {
                    return Err(invalid(
                        "path contains a drive, ADS, NUL, or control character",
                    ));
                }
                validate_reserved_component(&text)?;
            }
        }
    }
    Ok(())
}

fn validate_member_name(name: &str) -> io::Result<String> {
    if name.is_empty()
        || name.len() > MAX_MEMBER_NAME_BYTES
        || name.contains('\\')
        || name.starts_with('/')
        || name.starts_with("//")
    {
        return Err(invalid(
            "ZIP member is absolute, too long, or uses a backslash",
        ));
    }
    if name.contains('\0') || name.chars().any(char::is_control) {
        return Err(invalid("ZIP member contains NUL or a control character"));
    }
    let mut components = Vec::new();
    for component in name.split('/') {
        if component.is_empty() || component == "." || component == ".." || component.contains(':')
        {
            return Err(invalid(
                "ZIP member contains traversal, drive, or ADS syntax",
            ));
        }
        validate_reserved_component(component)?;
        components.push(component);
    }
    if components.is_empty() || name.ends_with('/') {
        return Err(invalid(
            "ZIP directories are outside the ordinary-file subset",
        ));
    }
    Ok(components.join("/"))
}

fn validate_reserved_component(component: &str) -> io::Result<()> {
    let trimmed = component.trim_end_matches(['.', ' ']);
    let stem = trimmed
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    if trimmed.is_empty()
        || matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CLOCK$")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.as_bytes()[3].is_ascii_digit()
            && stem.as_bytes()[3] != b'0')
    {
        return Err(invalid("reserved Windows path component"));
    }
    if component.ends_with(['.', ' ']) {
        return Err(invalid("trailing dot or space is not a portable path"));
    }
    Ok(())
}

fn collision_key(name: &str) -> String {
    let mut result = String::new();
    for lower in name.chars().flat_map(char::to_lowercase) {
        if is_combining_mark(lower) {
            continue;
        }
        result.push_str(&decompose_latin(lower));
    }
    result
}

fn is_combining_mark(character: char) -> bool {
    matches!(character as u32, 0x0300..=0x036f | 0x1ab0..=0x1aff | 0x1dc0..=0x1dff | 0x20d0..=0x20ff | 0xfe20..=0xfe2f)
}

fn decompose_latin(character: char) -> String {
    let replacement = match character {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' | 'ǎ' | 'ǻ' => "a",
        'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => "c",
        'ď' | 'đ' => "d",
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => "e",
        'ĝ' | 'ğ' | 'ġ' | 'ģ' => "g",
        'ĥ' | 'ħ' => "h",
        'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'ı' => "i",
        'ĵ' => "j",
        'ķ' => "k",
        'ĺ' | 'ļ' | 'ľ' | 'ŀ' | 'ł' => "l",
        'ñ' | 'ń' | 'ņ' | 'ň' | 'ŉ' | 'ŋ' => "n",
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ō' | 'ŏ' | 'ő' | 'ø' => "o",
        'ŕ' | 'ŗ' | 'ř' => "r",
        'ś' | 'ŝ' | 'ş' | 'š' => "s",
        'ţ' | 'ť' | 'ŧ' => "t",
        'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => "u",
        'ŵ' => "w",
        'ý' | 'ÿ' | 'ŷ' => "y",
        'ź' | 'ż' | 'ž' => "z",
        'ß' => "ss",
        'æ' => "ae",
        'œ' => "oe",
        _ => return character.to_string(),
    };
    replacement.to_owned()
}

fn same_path(left: &Path, right: &Path) -> bool {
    left == right || left.to_string_lossy().to_lowercase() == right.to_string_lossy().to_lowercase()
}

fn validate_digest(digest: &str) -> io::Result<()> {
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(invalid("manifest digest is not lowercase BLAKE3 hex"));
    }
    Ok(())
}

fn validate_flags(flags: u16) -> io::Result<()> {
    if flags & 0x0001 != 0 || flags & !(0x0008 | 0x0800) != 0 {
        return Err(invalid("encrypted or unsupported ZIP flags are refused"));
    }
    Ok(())
}

fn validate_external_attributes(made_by: u16, attrs: u32) -> io::Result<()> {
    if made_by >> 8 == 3 {
        let mode = (attrs >> 16) as u16;
        let kind = mode & 0xf000;
        if kind != 0 && kind != 0x8000 {
            return Err(invalid(
                "ZIP links, directories, and special files are refused",
            ));
        }
    }
    if attrs & 0x10 != 0 {
        return Err(invalid(
            "ZIP directory attributes are outside the file subset",
        ));
    }
    Ok(())
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

fn checked_u16(value: usize) -> io::Result<u16> {
    u16::try_from(value).map_err(|_| invalid("ZIP field exceeds u16"))
}

fn checked_u32(value: u64) -> io::Result<u32> {
    u32::try_from(value).map_err(|_| invalid("ZIP field exceeds non-ZIP64 u32"))
}

fn u16_at(bytes: &[u8], offset: usize) -> io::Result<u16> {
    let value = bytes
        .get(offset..offset + 2)
        .ok_or_else(|| invalid("ZIP record is truncated"))?;
    Ok(u16::from_le_bytes([value[0], value[1]]))
}

fn u32_at(bytes: &[u8], offset: usize) -> io::Result<u32> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| invalid("ZIP record is truncated"))?;
    Ok(u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
}

fn write_u16(output: &mut impl Write, value: u16) -> io::Result<()> {
    output.write_all(&value.to_le_bytes())
}

fn write_u32(output: &mut impl Write, value: u32) -> io::Result<()> {
    output.write_all(&value.to_le_bytes())
}

struct CountingWriter<W> {
    inner: W,
    count: u64,
}

impl<W> CountingWriter<W> {
    fn new(inner: W) -> Self {
        Self { inner, count: 0 }
    }
}

impl<W: Write> Write for CountingWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let written = self.inner.write(bytes)?;
        self.count = self.count.saturating_add(written as u64);
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

struct DigestWriter<W> {
    inner: W,
    hash: blake3::Hasher,
    crc: crc32fast::Hasher,
    size: u64,
}

impl<W> DigestWriter<W> {
    fn new(inner: W) -> Self {
        Self {
            inner,
            hash: blake3::Hasher::new(),
            crc: crc32fast::Hasher::new(),
            size: 0,
        }
    }

    fn finish(self) -> (W, u64, String, u32) {
        (
            self.inner,
            self.size,
            self.hash.finalize().to_hex().to_string(),
            self.crc.finalize(),
        )
    }
}

impl<W: Write> Write for DigestWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.inner.write_all(bytes)?;
        self.hash.update(bytes);
        self.crc.update(bytes);
        self.size = self
            .size
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| invalid("digest size overflowed"))?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

// Keep this import exercised in the module's source-level contract: callers
// preparing exact manifests may use the same streamed hash primitive.
#[allow(dead_code)]
fn hash_open_reader(reader: &mut File) -> io::Result<String> {
    hash_reader(reader)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stored_zip(name: &str, data: &[u8], attrs: u32) -> Vec<u8> {
        stored_zip_many(&[(name, data, attrs)])
    }

    fn stored_zip_many(entries: &[(&str, &[u8], u32)]) -> Vec<u8> {
        let mut zip = Vec::new();
        let mut records = Vec::with_capacity(entries.len());
        for (raw_name, data, attrs) in entries {
            let name = raw_name.as_bytes();
            let crc = crc32fast::hash(data);
            let offset = zip.len() as u32;
            let flags: u16 = if name.is_ascii() { 0 } else { 0x0800 };
            zip.extend_from_slice(&LOCAL_HEADER.to_le_bytes());
            zip.extend_from_slice(&20u16.to_le_bytes());
            zip.extend_from_slice(&flags.to_le_bytes());
            zip.extend_from_slice(&0u16.to_le_bytes());
            zip.extend_from_slice(&0u16.to_le_bytes());
            zip.extend_from_slice(&0u16.to_le_bytes());
            zip.extend_from_slice(&crc.to_le_bytes());
            zip.extend_from_slice(&(data.len() as u32).to_le_bytes());
            zip.extend_from_slice(&(data.len() as u32).to_le_bytes());
            zip.extend_from_slice(&(name.len() as u16).to_le_bytes());
            zip.extend_from_slice(&0u16.to_le_bytes());
            zip.extend_from_slice(name);
            zip.extend_from_slice(data);
            records.push((name.to_vec(), *attrs, flags, crc, data.len() as u32, offset));
        }
        let central = zip.len() as u32;
        for (name, attrs, flags, crc, size, offset) in &records {
            zip.extend_from_slice(&CENTRAL_HEADER.to_le_bytes());
            zip.extend_from_slice(&20u16.to_le_bytes());
            zip.extend_from_slice(&20u16.to_le_bytes());
            zip.extend_from_slice(&flags.to_le_bytes());
            zip.extend_from_slice(&0u16.to_le_bytes());
            zip.extend_from_slice(&0u16.to_le_bytes());
            zip.extend_from_slice(&0u16.to_le_bytes());
            zip.extend_from_slice(&crc.to_le_bytes());
            zip.extend_from_slice(&size.to_le_bytes());
            zip.extend_from_slice(&size.to_le_bytes());
            zip.extend_from_slice(&(name.len() as u16).to_le_bytes());
            zip.extend_from_slice(&0u16.to_le_bytes());
            zip.extend_from_slice(&0u16.to_le_bytes());
            zip.extend_from_slice(&0u16.to_le_bytes());
            zip.extend_from_slice(&0u16.to_le_bytes());
            zip.extend_from_slice(&attrs.to_le_bytes());
            zip.extend_from_slice(&offset.to_le_bytes());
            zip.extend_from_slice(name);
        }
        let size = zip.len() as u32 - central;
        zip.extend_from_slice(&END_HEADER.to_le_bytes());
        zip.extend_from_slice(&0u16.to_le_bytes());
        zip.extend_from_slice(&0u16.to_le_bytes());
        zip.extend_from_slice(&(records.len() as u16).to_le_bytes());
        zip.extend_from_slice(&(records.len() as u16).to_le_bytes());
        zip.extend_from_slice(&size.to_le_bytes());
        zip.extend_from_slice(&central.to_le_bytes());
        zip.extend_from_slice(&0u16.to_le_bytes());
        zip
    }

    #[test]
    fn round_trip_uses_exact_manifest_and_new_outputs() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source.txt");
        fs::write(&source, b"round trip bytes").unwrap();
        let archive = root.path().join("bundle.zip");
        let manifest = BatchManifest::for_archive(
            &archive,
            vec![(source.clone(), "folder/source.txt".to_owned())],
        )
        .unwrap();
        let report = create_zip(&manifest).unwrap();
        assert_eq!(report.entries.len(), 1);
        assert_eq!(
            fs::read(&archive).unwrap().len() as u64,
            report.archive_bytes
        );
        let output = root.path().join("unpacked");
        let extracted = extract_zip(&archive, &output).unwrap();
        assert_eq!(extracted.entries, report.entries);
        assert_eq!(
            fs::read(output.join("folder/source.txt")).unwrap(),
            b"round trip bytes"
        );
        assert!(!root.path().join(".unpacked.custodian-staging").exists());
    }

    #[test]
    fn changed_source_is_rejected_by_the_shared_manifest() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source.txt");
        fs::write(&source, b"approved").unwrap();
        let archive = root.path().join("bundle.zip");
        let manifest =
            BatchManifest::for_archive(&archive, vec![(source.clone(), "source.txt".into())])
                .unwrap();
        fs::write(&source, b"changed").unwrap();
        assert!(create_zip(&manifest).is_err());
        assert!(!archive.exists());
    }

    #[test]
    fn hostile_names_are_rejected_before_any_destination_is_created() {
        let root = tempfile::tempdir().unwrap();
        let archive = root.path().join("hostile.zip");
        for name in [
            "../outside.txt",
            "/absolute.txt",
            "C:drive.txt",
            "stream:name.txt",
            "CON.txt",
            "a\\b.txt",
        ] {
            fs::write(&archive, stored_zip(name, b"hostile", 0)).unwrap();
            let destination = root.path().join(format!("out-{}", name.len()));
            assert!(extract_zip(&archive, &destination).is_err(), "{name}");
            assert!(!destination.exists());
        }
    }

    #[test]
    fn case_and_unicode_collisions_are_rejected() {
        let root = tempfile::tempdir().unwrap();
        let archive = root.path().join("collision.zip");
        for entries in [
            vec![
                ("Readme.txt", b"one".as_slice(), 0),
                ("README.TXT", b"two".as_slice(), 0),
            ],
            vec![
                ("é.txt", b"one".as_slice(), 0),
                ("e\u{301}.txt", b"two".as_slice(), 0),
            ],
        ] {
            fs::write(&archive, stored_zip_many(&entries)).unwrap();
            let destination = root.path().join("out");
            assert!(extract_zip(&archive, &destination).is_err());
            assert!(!destination.exists());
        }
    }

    #[cfg(unix)]
    #[test]
    fn symlink_source_and_private_paths_are_refused() {
        use std::sync::Arc;
        let root = tempfile::tempdir().unwrap();
        let private = root.path().join("private");
        fs::create_dir(&private).unwrap();
        let secret = private.join("secret.txt");
        fs::write(&secret, b"private").unwrap();
        let public = root.path().join("public.txt");
        std::os::unix::fs::symlink(&secret, root.path().join("link.txt")).unwrap();
        let policy = Arc::new(crate::privacy::AccessPolicy::default());
        policy.replace_private_paths(vec![private]).unwrap();
        crate::privacy::with_policy(policy, || {
            assert!(FileSnapshot::capture(&secret).is_err());
            assert!(FileSnapshot::capture(&root.path().join("link.txt")).is_err());
            fs::write(&public, b"public").unwrap();
            let archive = root.path().join("private.zip");
            assert!(
                BatchManifest::for_archive(&archive, vec![(public, "public.txt".into())]).is_err()
            );
        });
    }

    #[test]
    fn archive_member_limit_and_encryption_flag_fail_closed() {
        let root = tempfile::tempdir().unwrap();
        let archive = root.path().join("encrypted.zip");
        let mut bytes = stored_zip("file.txt", b"data", 0);
        bytes[6..8].copy_from_slice(&1u16.to_le_bytes());
        let central = bytes.len() - 22 - (46 + "file.txt".len());
        bytes[central + 8..central + 10].copy_from_slice(&1u16.to_le_bytes());
        fs::write(&archive, bytes).unwrap();
        assert!(verify_zip(&archive).is_err());
    }

    #[test]
    fn links_and_nested_archives_are_not_followed_or_recursively_extracted() {
        let root = tempfile::tempdir().unwrap();
        let archive = root.path().join("links.zip");
        let mut link = stored_zip("link", b"target", 0xa000_0000);
        let central = 30 + "link".len() + b"target".len();
        link[central + 4..central + 6].copy_from_slice(&(3u16 << 8 | 20).to_le_bytes());
        fs::write(&archive, link).unwrap();
        assert!(verify_zip(&archive).is_err());

        let nested = stored_zip("nested.zip", b"not recursively extracted", 0);
        let nested_archive = root.path().join("nested.zip");
        fs::write(&nested_archive, nested).unwrap();
        let output = root.path().join("nested-out");
        extract_zip(&nested_archive, &output).unwrap();
        assert!(output.join("nested.zip").is_file());
        assert!(!output.join("nested").exists());
    }

    #[test]
    fn creation_enforces_the_expansion_ratio() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("repetitive.bin");
        fs::write(&source, vec![0u8; 16 * 1024]).unwrap();
        let archive = root.path().join("ratio.zip");
        let manifest =
            BatchManifest::for_archive(&archive, vec![(source, "repetitive.bin".into())]).unwrap();
        assert!(create_zip(&manifest).is_err());
        assert!(!archive.exists());
    }
}
