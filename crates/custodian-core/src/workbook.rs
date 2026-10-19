//! Conservative, read-only XLSX admission and independent cell verification.
//! No extraction, filesystem writes, macro evaluation, links or general formulas.
use custodian_winsec::excel::{parse_sum, Cell, CellValue, Snapshot};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
};

fn invalid() -> &'static str {
    "Workbook is outside the qualified macro-free single-sheet subset"
}
fn u16le(b: &[u8], p: usize) -> Result<u16, &'static str> {
    Ok(u16::from_le_bytes(
        b.get(p..p + 2)
            .ok_or_else(invalid)?
            .try_into()
            .map_err(|_| invalid())?,
    ))
}
fn u32le(b: &[u8], p: usize) -> Result<u32, &'static str> {
    Ok(u32::from_le_bytes(
        b.get(p..p + 4)
            .ok_or_else(invalid)?
            .try_into()
            .map_err(|_| invalid())?,
    ))
}

fn archive(bytes: &[u8]) -> Result<BTreeMap<String, Vec<u8>>, &'static str> {
    if bytes.len() < 22 || bytes.len() > 8 * 1024 * 1024 {
        return Err(invalid());
    }
    let end = bytes.len() - 22;
    if u32le(bytes, end)? != 0x06054b50
        || u16le(bytes, end + 4)? != 0
        || u16le(bytes, end + 6)? != 0
        || u16le(bytes, end + 20)? != 0
    {
        return Err(invalid());
    }
    let count = u16le(bytes, end + 10)? as usize;
    if count == 0 || count > 32 || count != u16le(bytes, end + 8)? as usize {
        return Err(invalid());
    }
    let start = u32le(bytes, end + 16)? as usize;
    let size = u32le(bytes, end + 12)? as usize;
    if start.checked_add(size) != Some(end) {
        return Err(invalid());
    }
    let allowed = BTreeSet::from([
        "[Content_Types].xml",
        "_rels/.rels",
        "docProps/core.xml",
        "docProps/app.xml",
        "xl/workbook.xml",
        "xl/_rels/workbook.xml.rels",
        "xl/worksheets/sheet1.xml",
        "xl/styles.xml",
        "xl/theme/theme1.xml",
        "xl/sharedStrings.xml",
        "xl/calcChain.xml",
    ]);
    let mut cursor = start;
    let mut files = BTreeMap::new();
    let mut spans = Vec::new();
    let mut expanded = 0usize;
    for _ in 0..count {
        if u32le(bytes, cursor)? != 0x02014b50 {
            return Err(invalid());
        }
        let flags = u16le(bytes, cursor + 8)?;
        let method = u16le(bytes, cursor + 10)?;
        if flags & !0x080e != 0 || ![0, 8].contains(&method) {
            return Err(invalid());
        }
        let crc = u32le(bytes, cursor + 16)?;
        let compressed = u32le(bytes, cursor + 20)? as usize;
        let length = u32le(bytes, cursor + 24)? as usize;
        let name_len = u16le(bytes, cursor + 28)? as usize;
        let extra = u16le(bytes, cursor + 30)? as usize;
        let comment = u16le(bytes, cursor + 32)? as usize;
        if extra != 0
            || comment != 0
            || u16le(bytes, cursor + 34)? != 0
            || length > 2 * 1024 * 1024
            || length > compressed.saturating_mul(256).saturating_add(1024)
        {
            return Err(invalid());
        }
        expanded = expanded.checked_add(length).ok_or_else(invalid)?;
        if expanded > 8 * 1024 * 1024 {
            return Err(invalid());
        }
        let name = std::str::from_utf8(
            bytes
                .get(cursor + 46..cursor + 46 + name_len)
                .ok_or_else(invalid)?,
        )
        .map_err(|_| invalid())?;
        if !allowed.contains(name) || files.contains_key(name) {
            return Err(invalid());
        }
        let local = u32le(bytes, cursor + 42)? as usize;
        if u32le(bytes, local)? != 0x04034b50
            || u16le(bytes, local + 6)? != flags
            || u16le(bytes, local + 8)? != method
            || u16le(bytes, local + 26)? as usize != name_len
        {
            return Err(invalid());
        }
        let name_end = local.checked_add(30 + name_len).ok_or_else(invalid)?;
        if bytes.get(local + 30..name_end) != Some(name.as_bytes()) {
            return Err(invalid());
        }
        let extra_len = u16le(bytes, local + 28)? as usize;
        let data_start = name_end.checked_add(extra_len).ok_or_else(invalid)?;
        office_padding(bytes.get(name_end..data_start).ok_or_else(invalid)?)?;
        let data_end = data_start.checked_add(compressed).ok_or_else(invalid)?;
        if data_end > start {
            return Err(invalid());
        }
        let input = bytes.get(data_start..data_end).ok_or_else(invalid)?;
        let data = if method == 0 {
            if compressed != length {
                return Err(invalid());
            }
            input.to_vec()
        } else {
            let mut decoder = flate2::read::DeflateDecoder::new(input);
            let mut output = Vec::new();
            decoder
                .by_ref()
                .take(length as u64 + 1)
                .read_to_end(&mut output)
                .map_err(|_| invalid())?;
            if decoder.total_in() != compressed as u64 {
                return Err(invalid());
            }
            output
        };
        if data.len() != length || crc32fast::hash(&data) != crc {
            return Err(invalid());
        }
        let block_end = if flags & 8 != 0 {
            let descriptor = if u32le(bytes, data_end)? == 0x08074b50 {
                data_end + 4
            } else {
                data_end
            };
            if u32le(bytes, descriptor)? != crc
                || u32le(bytes, descriptor + 4)? as usize != compressed
                || u32le(bytes, descriptor + 8)? as usize != length
            {
                return Err(invalid());
            }
            descriptor + 12
        } else {
            if u32le(bytes, local + 14)? != crc
                || u32le(bytes, local + 18)? as usize != compressed
                || u32le(bytes, local + 22)? as usize != length
            {
                return Err(invalid());
            }
            data_end
        };
        if block_end > start {
            return Err(invalid());
        }
        spans.push((local, block_end));
        files.insert(name.to_owned(), data);
        cursor += 46 + name_len + extra + comment;
    }
    if cursor != end {
        return Err(invalid());
    }
    spans.sort_unstable();
    let mut next = 0;
    for (first, last) in spans {
        if first != next {
            return Err(invalid());
        }
        next = last
    }
    if next != start {
        return Err(invalid());
    }
    Ok(files)
}

// Excel reserves local-header growth space using one A220/A028 padding field.
// Accept that exact inert representation, not arbitrary ZIP extensions or ZIP64.
fn office_padding(extra: &[u8]) -> Result<(), &'static str> {
    if extra.is_empty() {
        return Ok(());
    }
    if ![264, 520].contains(&extra.len())
        || u16le(extra, 0)? != 0xa220
        || u16le(extra, 2)? as usize != extra.len() - 4
        || u16le(extra, 4)? != 0xa028
        || u16le(extra, 6)? as usize != extra.len() - 8
        || extra[8..].iter().any(|b| *b != 0)
    {
        return Err(invalid());
    }
    Ok(())
}

#[derive(Default)]
struct Node {
    name: String,
    attributes: BTreeMap<String, String>,
    text: String,
    children: Vec<Node>,
}
impl Node {
    fn child(&self, name: &str) -> Option<&Self> {
        self.children.iter().find(|n| n.name == name)
    }
    fn attribute(&self, name: &str) -> Result<&str, &'static str> {
        self.attributes
            .get(name)
            .map(String::as_str)
            .ok_or_else(invalid)
    }
    fn text(&self) -> String {
        let mut text = self.text.clone();
        for child in &self.children {
            text.push_str(&child.text())
        }
        text
    }
}
fn xml(bytes: &[u8]) -> Result<Node, &'static str> {
    use quick_xml::{events::Event, Reader, XmlVersion};
    let content = std::str::from_utf8(bytes).map_err(|_| invalid())?;
    let mut reader = Reader::from_str(content);
    let mut stack = vec![Node::default()];
    let mut nodes = 0;
    loop {
        let event = reader.read_event().map_err(|_| invalid())?;
        let empty = matches!(&event, Event::Empty(_));
        match event {
            Event::Start(element) | Event::Empty(element) => {
                let name = element
                    .name()
                    .as_ref()
                    .rsplit(':')
                    .next()
                    .ok_or_else(invalid)?
                    .to_string();
                let mut node = Node {
                    name,
                    ..Default::default()
                };
                for attr in element.attributes() {
                    let attr = attr.map_err(|_| invalid())?;
                    let name = attr.key.as_ref().to_string();
                    if node
                        .attributes
                        .insert(
                            name,
                            attr.normalized_value(XmlVersion::Implicit1_0)
                                .map_err(|_| invalid())?
                                .into_owned(),
                        )
                        .is_some()
                    {
                        return Err(invalid());
                    }
                }
                nodes += 1;
                if nodes > 40000 || stack.len() > 32 {
                    return Err(invalid());
                }
                if empty {
                    stack.last_mut().ok_or_else(invalid)?.children.push(node)
                } else {
                    stack.push(node)
                }
            }
            Event::End(_) => {
                if stack.len() < 2 {
                    return Err(invalid());
                }
                let node = stack.pop().ok_or_else(invalid)?;
                stack.last_mut().ok_or_else(invalid)?.children.push(node)
            }
            Event::Text(text) => stack
                .last_mut()
                .ok_or_else(invalid)?
                .text
                .push_str(&text.xml_content(XmlVersion::Implicit1_0)),
            Event::CData(text) => stack
                .last_mut()
                .ok_or_else(invalid)?
                .text
                .push_str(text.as_ref()),
            Event::GeneralRef(reference) => {
                let value =
                    if let Some(value) = reference.resolve_char_ref().map_err(|_| invalid())? {
                        value
                    } else {
                        match reference.as_ref() {
                            "amp" => '&',
                            "lt" => '<',
                            "gt" => '>',
                            "apos" => '\'',
                            "quot" => '"',
                            _ => return Err(invalid()),
                        }
                    };
                stack.last_mut().ok_or_else(invalid)?.text.push(value);
            }
            Event::DocType(_) | Event::PI(_) => return Err(invalid()),
            Event::Decl(decl) => {
                if decl
                    .encoding()
                    .transpose()
                    .map_err(|_| invalid())?
                    .is_some_and(|e| !e.eq_ignore_ascii_case("utf-8"))
                {
                    return Err(invalid());
                }
            }
            Event::Eof => break,
            Event::Comment(_) => {}
        }
    }
    if stack.len() != 1 || stack[0].children.len() != 1 {
        return Err(invalid());
    }
    Ok(stack.remove(0).children.remove(0))
}
fn collect<'a>(node: &'a Node, name: &str, output: &mut Vec<&'a Node>) {
    if node.name == name {
        output.push(node)
    }
    for child in &node.children {
        collect(child, name, output)
    }
}
fn root(files: &BTreeMap<String, Vec<u8>>, path: &str, name: &str) -> Result<Node, &'static str> {
    let node = xml(files.get(path).ok_or_else(invalid)?)?;
    if node.name != name {
        return Err(invalid());
    }
    Ok(node)
}
fn reference(value: &str) -> Result<(u16, u16), &'static str> {
    let first = *value.as_bytes().first().ok_or_else(invalid)?;
    if !(b'A'..=b'P').contains(&first) {
        return Err(invalid());
    }
    let row = value[1..].parse::<u16>().map_err(|_| invalid())?;
    if !(1..=16).contains(&row) || row.to_string() != value[1..] {
        return Err(invalid());
    }
    Ok((row, (first - b'A' + 1) as u16))
}
fn number(value: &str) -> Result<f64, &'static str> {
    let number = value.parse::<f64>().map_err(|_| invalid())?;
    if !number.is_finite() || number.abs() > 1e12 {
        return Err(invalid());
    }
    Ok(number)
}

pub fn inspect(bytes: &[u8]) -> Result<Snapshot, &'static str> {
    let files = archive(bytes)?;
    // Validate every XML entry, including metadata not returned as cell data.
    for (name, data) in &files {
        let doc = xml(data)?;
        let forbidden = [
            "externalReferences",
            "pivotCaches",
            "fileSharing",
            "webPublishing",
            "hyperlinks",
            "oleObjects",
            "controls",
            "drawing",
            "legacyDrawing",
            "tableParts",
            "connections",
            "ddeLink",
            "oleLink",
        ];
        for element in forbidden {
            let mut nodes = Vec::new();
            collect(&doc, element, &mut nodes);
            if !nodes.is_empty() {
                return Err(invalid());
            }
        }
        if name.ends_with(".rels") {
            if doc.name != "Relationships"
                || doc.attribute("xmlns")?
                    != "http://schemas.openxmlformats.org/package/2006/relationships"
            {
                return Err(invalid());
            }
            let mut ids = BTreeSet::new();
            for relationship in &doc.children {
                if relationship.name != "Relationship"
                    || !ids.insert(relationship.attribute("Id")?)
                    || relationship
                        .attributes
                        .get("TargetMode")
                        .is_some_and(|s| s != "Internal")
                {
                    return Err(invalid());
                }
                let kind = relationship.attribute("Type")?;
                let target = relationship.attribute("Target")?;
                let suffix = if kind == "http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" {
                    "metadata/core-properties"
                } else { kind.strip_prefix(
                    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/"
                ).ok_or_else(invalid)? };
                let expected = match (name.as_str(), suffix) {
                    ("_rels/.rels", "officeDocument") => "xl/workbook.xml",
                    ("_rels/.rels", "metadata/core-properties") => "docProps/core.xml",
                    ("_rels/.rels", "extended-properties") => "docProps/app.xml",
                    ("xl/_rels/workbook.xml.rels", "worksheet") => "worksheets/sheet1.xml",
                    ("xl/_rels/workbook.xml.rels", "styles") => "styles.xml",
                    ("xl/_rels/workbook.xml.rels", "theme") => "theme/theme1.xml",
                    ("xl/_rels/workbook.xml.rels", "sharedStrings") => "sharedStrings.xml",
                    ("xl/_rels/workbook.xml.rels", "calcChain") => "calcChain.xml",
                    _ => return Err(invalid()),
                };
                let resolved = if name == "_rels/.rels" {
                    expected.to_string()
                } else {
                    format!("xl/{expected}")
                };
                if target != expected || !files.contains_key(&resolved) {
                    return Err(invalid());
                }
            }
        }
    }
    let types = root(&files, "[Content_Types].xml", "Types")?;
    for item in &types.children {
        let kind = item.attribute("ContentType")?;
        if ![
            "application/xml",
            "application/vnd.openxmlformats-package.relationships+xml",
            "application/vnd.openxmlformats-package.core-properties+xml",
            "application/vnd.openxmlformats-officedocument.extended-properties+xml",
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml",
            "application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml",
            "application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml",
            "application/vnd.openxmlformats-officedocument.theme+xml",
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml",
            "application/vnd.openxmlformats-officedocument.spreadsheetml.calcChain+xml",
        ]
        .contains(&kind)
        {
            return Err(invalid());
        }
    }
    let workbook = root(&files, "xl/workbook.xml", "workbook")?;
    if workbook.attribute("xmlns")? != "http://schemas.openxmlformats.org/spreadsheetml/2006/main" {
        return Err(invalid());
    }
    for child in &workbook.children {
        if ![
            "fileVersion",
            "workbookPr",
            "AlternateContent",
            "revisionPtr",
            "bookViews",
            "sheets",
            "calcPr",
            "extLst",
            "definedNames",
        ]
        .contains(&child.name.as_str())
        {
            return Err(invalid());
        }
        if child.name == "definedNames"
            && (!child.children.is_empty() || !child.text.trim().is_empty())
        {
            return Err(invalid());
        }
        if child.name == "AlternateContent"
            && (child.children.len() != 1
                || child.children[0].name != "Choice"
                || child.children[0].children.len() != 1
                || child.children[0].children[0].name != "absPath")
        {
            return Err(invalid());
        }
        if child.name == "extLst" {
            for extension in &child.children {
                if extension.name != "ext"
                    || ![
                        "{140A7094-0E35-4892-8432-C4D2E57EDEB5}",
                        "{B58B0392-4F1F-4190-BB64-5DF3571DCE5F}",
                        "{D14903EA-33C4-47F7-8F05-3474C54BE107}",
                    ]
                    .contains(&extension.attribute("uri")?)
                {
                    return Err(invalid());
                }
                fn harmless(node: &Node) -> bool {
                    ["ext", "workbookPr", "calcFeatures", "feature", "version"]
                        .contains(&node.name.as_str())
                        && node.children.iter().all(harmless)
                }
                if !harmless(extension) {
                    return Err(invalid());
                }
            }
        }
    }
    let sheets = workbook.child("sheets").ok_or_else(invalid)?;
    if sheets.children.len() != 1
        || sheets.children[0].name != "sheet"
        || sheets.children[0]
            .attributes
            .get("state")
            .is_some_and(|s| s != "visible")
    {
        return Err(invalid());
    }
    let name = sheets.children[0].attribute("name")?.to_string();
    let relations = root(&files, "xl/_rels/workbook.xml.rels", "Relationships")?;
    let sheet_id = sheets.children[0].attribute("r:id")?;
    if relations
        .children
        .iter()
        .filter(|r| {
            r.attributes.get("Id").is_some_and(|id| id == sheet_id)
                && r.attributes
                    .get("Target")
                    .is_some_and(|t| t == "worksheets/sheet1.xml")
        })
        .count()
        != 1
    {
        return Err(invalid());
    }
    if name.is_empty() || name.len() > 128 || name.chars().any(char::is_control) {
        return Err(invalid());
    }
    let mut strings = Vec::new();
    if let Some(data) = files.get("xl/sharedStrings.xml") {
        let sst = xml(data)?;
        if sst.name != "sst" || sst.children.len() > 256 {
            return Err(invalid());
        }
        for string in &sst.children {
            if string.name != "si" {
                return Err(invalid());
            }
            strings.push(string.text());
        }
    }
    let mut formats = vec![false];
    if files.contains_key("xl/styles.xml") {
        let styles = root(&files, "xl/styles.xml", "styleSheet")?;
        let xfs = styles.child("cellXfs").ok_or_else(invalid)?;
        formats.clear();
        for xf in &xfs.children {
            let format = xf
                .attribute("numFmtId")?
                .parse::<u32>()
                .map_err(|_| invalid())?;
            formats.push(match format {
                0 => false,
                49 => true,
                _ => return Err(invalid()),
            });
        }
    }
    let sheet = root(&files, "xl/worksheets/sheet1.xml", "worksheet")?;
    if sheet.children.iter().any(|node| {
        ![
            "dimension",
            "sheetViews",
            "sheetFormatPr",
            "cols",
            "sheetData",
            "pageMargins",
            "printOptions",
            "pageSetup",
            "headerFooter",
        ]
        .contains(&node.name.as_str())
    }) {
        return Err(invalid());
    }
    if let Some(dimension) = sheet.child("dimension") {
        let span = dimension.attribute("ref")?;
        for corner in span.split(':') {
            reference(corner)?;
        }
    }
    let mut cells = BTreeMap::new();
    let mut seen_rows = BTreeSet::new();
    let mut seen_cells = BTreeSet::new();
    for row in &sheet.child("sheetData").ok_or_else(invalid)?.children {
        if row.name != "row" {
            return Err(invalid());
        }
        let row_number = row.attribute("r")?.parse::<u16>().map_err(|_| invalid())?;
        if !(1..=16).contains(&row_number) || !seen_rows.insert(row_number) {
            return Err(invalid());
        }
        for cell in &row.children {
            if cell.name != "c" {
                return Err(invalid());
            }
            let (r, col) = reference(cell.attribute("r")?)?;
            if r != row_number || !seen_cells.insert((r, col)) {
                return Err(invalid());
            }
            let mut children = BTreeSet::new();
            for child in &cell.children {
                if !["v", "f", "is"].contains(&child.name.as_str())
                    || !children.insert(child.name.as_str())
                {
                    return Err(invalid());
                }
            }
            let style = cell
                .attributes
                .get("s")
                .map(|s| s.parse::<usize>())
                .transpose()
                .map_err(|_| invalid())?
                .unwrap_or(0);
            let text_format = *formats.get(style).ok_or_else(invalid)?;
            let kind = cell.attributes.get("t").map(String::as_str).unwrap_or("n");
            let text = cell.child("v").map(Node::text).unwrap_or_default();
            let (value, calculated) = if let Some(formula) = cell.child("f") {
                if kind != "n" || !formula.attributes.is_empty() {
                    return Err(invalid());
                }
                (
                    parse_sum(&format!("={}", formula.text())).ok_or_else(invalid)?,
                    Some(number(&text)?),
                )
            } else {
                match kind {
                    "n" if text.is_empty() => (CellValue::Empty, None),
                    "n" => (
                        CellValue::Number {
                            value: number(&text)?,
                        },
                        None,
                    ),
                    "s" => {
                        let index = text.parse::<usize>().map_err(|_| invalid())?;
                        (
                            CellValue::Text {
                                value: strings.get(index).ok_or_else(invalid)?.clone(),
                            },
                            None,
                        )
                    }
                    "inlineStr" => (
                        CellValue::Text {
                            value: cell.child("is").ok_or_else(invalid)?.text(),
                        },
                        None,
                    ),
                    _ => return Err(invalid()),
                }
            };
            if let CellValue::Text { value } = &value {
                if value.len() > 4096
                    || value
                        .chars()
                        .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
                {
                    return Err(invalid());
                }
            }
            if value != CellValue::Empty || text_format {
                cells.insert(
                    (r, col),
                    Cell {
                        row: r,
                        col,
                        value,
                        calculated,
                        text_format,
                    },
                );
            }
        }
    }
    Ok(Snapshot {
        sheet: name,
        cells: cells.into_values().collect(),
    })
}

/// Predict the exact permitted cell result without invoking Office. Unselected
/// cells and formats survive; formula-like text is never parsed as a formula.
pub fn apply_edits(
    snapshot: &Snapshot,
    edits: &[crate::app_adapter::CellEdit],
) -> Result<Snapshot, &'static str> {
    crate::app_adapter::validate_cell_edits("native-bound-sheet", edits)?;
    let mut result = snapshot.clone();
    for edit in edits {
        let value = edit.value.native()?;
        let text_format = match &value {
            CellValue::Text { .. } => true,
            CellValue::Empty => result
                .cells
                .iter()
                .find(|c| (c.row, c.col) == (edit.row, edit.col))
                .is_some_and(|c| c.text_format),
            _ => false,
        };
        result
            .cells
            .retain(|c| (c.row, c.col) != (edit.row, edit.col));
        if value != CellValue::Empty || text_format {
            result.cells.push(Cell {
                row: edit.row,
                col: edit.col,
                value,
                calculated: None,
                text_format,
            });
        }
    }
    result.cells.sort_by_key(|c| (c.row, c.col));
    recalculate(&result)
}

pub fn recalculate(snapshot: &Snapshot) -> Result<Snapshot, &'static str> {
    if snapshot.cells.len() > 256 {
        return Err(invalid());
    }
    let mut cells = BTreeMap::new();
    for cell in &snapshot.cells {
        if !(1..=16).contains(&cell.row) || !(1..=16).contains(&cell.col) {
            return Err(invalid());
        }
        match &cell.value {
            CellValue::Number { value } if !value.is_finite() || value.abs() > 1e12 => {
                return Err(invalid());
            }
            CellValue::Text { value }
                if value.len() > 4096
                    || value
                        .chars()
                        .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t')) =>
            {
                return Err(invalid());
            }
            CellValue::Sum {
                first_row,
                first_col,
                last_row,
                last_col,
            } if !(1..=16).contains(first_row)
                || !(1..=16).contains(first_col)
                || !(1..=16).contains(last_row)
                || !(1..=16).contains(last_col)
                || first_row > last_row
                || first_col > last_col =>
            {
                return Err(invalid());
            }
            _ => {}
        }
        if cells.insert((cell.row, cell.col), cell.clone()).is_some() {
            return Err(invalid());
        }
    }
    fn evaluate(
        key: (u16, u16),
        cells: &BTreeMap<(u16, u16), Cell>,
        visiting: &mut BTreeSet<(u16, u16)>,
        memo: &mut BTreeMap<(u16, u16), f64>,
        budget: &mut usize,
    ) -> Result<f64, &'static str> {
        *budget = budget.checked_sub(1).ok_or_else(invalid)?;
        if let Some(value) = memo.get(&key) {
            return Ok(*value);
        }
        let Some(cell) = cells.get(&key) else {
            return Ok(0.0);
        };
        let value = match &cell.value {
            CellValue::Empty | CellValue::Text { .. } => Ok(0.0),
            CellValue::Number { value } => Ok(*value),
            CellValue::Sum {
                first_row,
                first_col,
                last_row,
                last_col,
            } => {
                if !visiting.insert(key) {
                    return Err(invalid());
                }
                let mut sum = 0.0;
                for r in *first_row..=*last_row {
                    for c in *first_col..=*last_col {
                        sum += evaluate((r, c), cells, visiting, memo, budget)?
                    }
                }
                visiting.remove(&key);
                if !sum.is_finite() || sum.abs() > 1e12 {
                    return Err(invalid());
                }
                Ok(sum)
            }
        }?;
        memo.insert(key, value);
        Ok(value)
    }
    let mut result = snapshot.clone();
    let mut memo = BTreeMap::new();
    let mut budget = 131072;
    for cell in &mut result.cells {
        if matches!(cell.value, CellValue::Sum { .. }) {
            cell.calculated = Some(evaluate(
                (cell.row, cell.col),
                &cells,
                &mut BTreeSet::new(),
                &mut memo,
                &mut budget,
            )?)
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(row: u16, col: u16, value: CellValue) -> Cell {
        Cell {
            row,
            col,
            value,
            calculated: None,
            text_format: false,
        }
    }
    fn snapshot(cells: Vec<Cell>) -> Snapshot {
        Snapshot {
            sheet: "Synthetic".into(),
            cells,
        }
    }
    fn sum(first_row: u16, last_row: u16) -> CellValue {
        CellValue::Sum {
            first_row,
            first_col: 1,
            last_row,
            last_col: 1,
        }
    }
    #[test]
    fn independent_calculation_ignores_text_and_detects_cycles_and_overflow() {
        let source = snapshot(vec![
            cell(1, 1, CellValue::Number { value: 7.0 }),
            cell(
                2,
                1,
                CellValue::Text {
                    value: "=WEBSERVICE(\"private\")".into(),
                },
            ),
            cell(3, 1, sum(1, 2)),
            cell(4, 1, sum(1, 3)),
        ]);
        let result = recalculate(&source).unwrap();
        assert_eq!(result.cells[2].calculated, Some(7.0));
        assert_eq!(result.cells[3].calculated, Some(14.0));
        assert!(recalculate(&snapshot(vec![cell(1, 1, sum(1, 1))])).is_err());
        assert!(recalculate(&snapshot(vec![
            cell(1, 1, sum(2, 2)),
            cell(2, 1, sum(1, 1))
        ]))
        .is_err());
        assert!(recalculate(&snapshot(vec![
            cell(1, 1, CellValue::Number { value: 1e12 }),
            cell(2, 1, CellValue::Number { value: 1e12 }),
            cell(3, 1, sum(1, 2))
        ]))
        .is_err());
    }
    #[test]
    fn public_recalculator_refuses_unbounded_or_ambiguous_coordinates() {
        for source in [
            snapshot(vec![cell(1, 1, sum(0, u16::MAX))]),
            snapshot(vec![cell(0, 1, CellValue::Empty)]),
            snapshot(vec![
                cell(1, 1, CellValue::Empty),
                cell(1, 1, CellValue::Number { value: 1.0 }),
            ]),
            snapshot(vec![cell(1, 1, CellValue::Number { value: f64::NAN })]),
            snapshot(vec![cell(
                1,
                1,
                CellValue::Text {
                    value: "x".repeat(4097),
                },
            )]),
        ] {
            assert!(recalculate(&source).is_err());
        }
        let mut cells = vec![cell(1, 1, CellValue::Number { value: 1.0 })];
        for row in 2..=16 {
            cells.push(cell(row, 1, sum(1, row - 1)));
        }
        assert_eq!(
            recalculate(&snapshot(cells)).unwrap().cells[15].calculated,
            Some(16384.0)
        );
    }
    // Small synthetic ZIP builder: no Office usernames, timestamps or local paths.
    fn zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        fn short(v: &mut Vec<u8>, n: u16) {
            v.extend(n.to_le_bytes());
        }
        fn long(v: &mut Vec<u8>, n: u32) {
            v.extend(n.to_le_bytes());
        }
        let mut bytes = vec![];
        let mut central = vec![];
        for (name, data) in entries {
            let offset = bytes.len() as u32;
            let crc = crc32fast::hash(data);
            long(&mut bytes, 0x04034b50);
            for n in [20, 0, 0, 0, 0] {
                short(&mut bytes, n);
            }
            for n in [crc, data.len() as u32, data.len() as u32] {
                long(&mut bytes, n);
            }
            short(&mut bytes, name.len() as u16);
            short(&mut bytes, 0);
            bytes.extend(name.as_bytes());
            bytes.extend(*data);
            long(&mut central, 0x02014b50);
            for n in [20, 20, 0, 0, 0, 0] {
                short(&mut central, n);
            }
            for n in [crc, data.len() as u32, data.len() as u32] {
                long(&mut central, n);
            }
            short(&mut central, name.len() as u16);
            for _ in 0..4 {
                short(&mut central, 0);
            }
            long(&mut central, 0);
            long(&mut central, offset);
            central.extend(name.as_bytes());
        }
        let start = bytes.len() as u32;
        let size = central.len() as u32;
        bytes.extend(central);
        long(&mut bytes, 0x06054b50);
        short(&mut bytes, 0);
        short(&mut bytes, 0);
        short(&mut bytes, entries.len() as u16);
        short(&mut bytes, entries.len() as u16);
        long(&mut bytes, size);
        long(&mut bytes, start);
        short(&mut bytes, 0);
        bytes
    }
    #[test]
    fn archive_rejects_unknown_parts_duplicates_corruption_and_appended_content() {
        let valid = zip(&[("xl/workbook.xml", b"<workbook/>")]);
        assert!(archive(&valid).is_ok());
        for entries in [
            vec![("../escape.xml", b"x".as_slice())],
            vec![("xl/vbaProject.bin", b"x".as_slice())],
            vec![
                ("xl/workbook.xml", b"x".as_slice()),
                ("xl/workbook.xml", b"y".as_slice()),
            ],
        ] {
            assert!(archive(&zip(&entries)).is_err());
        }
        let mut appended = valid.clone();
        appended.push(0);
        assert!(archive(&appended).is_err());
        let mut corrupt = valid;
        corrupt[45] ^= 1;
        assert!(archive(&corrupt).is_err());
    }
    #[test]
    fn xml_refuses_entities_multiple_roots_and_excessive_depth() {
        for bytes in [
            b"<!DOCTYPE a [<!ENTITY x SYSTEM 'file:///private'>]><a>&x;</a>".as_slice(),
            b"<a/><b/>",
            b"<?read private?><a/>",
            b"<a>&unknown;</a>",
            b"<a><b></a>",
        ] {
            assert!(xml(bytes).is_err());
        }
        assert!(xml(format!("{}{}", "<a>".repeat(40), "</a>".repeat(40)).as_bytes()).is_err());
        assert_eq!(xml(b"<a>&amp;&#65;&#x1f600;</a>").unwrap().text(), "&A😀");
    }
    #[test]
    fn only_exact_office_local_padding_is_admitted() {
        let mut extra = vec![0; 264];
        extra[..8].copy_from_slice(&[0x20, 0xa2, 4, 1, 0x28, 0xa0, 0, 1]);
        assert!(office_padding(&extra).is_ok());
        extra[20] = 1;
        assert!(office_padding(&extra).is_err());
        extra[20] = 0;
        extra[0] = 1;
        assert!(office_padding(&extra).is_err());
    }

    fn fixture(sheet: &str, relations: &str) -> Vec<u8> {
        let types = r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" ContentType="application/xml"/><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/></Types>"#;
        let root_rels = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#;
        let workbook = r#"<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Synthetic" sheetId="1" r:id="rId1"/></sheets></workbook>"#;
        zip(&[
            ("[Content_Types].xml", types.as_bytes()),
            ("_rels/.rels", root_rels.as_bytes()),
            ("xl/workbook.xml", workbook.as_bytes()),
            ("xl/_rels/workbook.xml.rels", relations.as_bytes()),
            ("xl/worksheets/sheet1.xml", sheet.as_bytes()),
        ])
    }
    const RELATIONS: &str = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/></Relationships>"#;
    const SHEET: &str = r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><dimension ref="A1:A3"/><sheetData><row r="1"><c r="A1"><v>7</v></c></row><row r="2"><c r="A2"><v>8</v></c></row><row r="3"><c r="A3"><f>SUM(A1:A2)</f><v>15</v></c></row></sheetData></worksheet>"#;
    #[test]
    fn qualified_archive_is_independently_read_and_calculated() {
        let actual = inspect(&fixture(SHEET, RELATIONS)).unwrap();
        assert_eq!(actual.cells.len(), 3);
        assert_eq!(actual.cells[2].calculated, Some(15.0));
        assert_eq!(actual, recalculate(&actual).unwrap());
    }
    #[test]
    fn links_forged_relationships_outside_cells_and_active_formulas_are_refused() {
        for relations in [
            RELATIONS.replace("Target=", "TargetMode=\"External\" Target="),
            RELATIONS.replace("worksheets/sheet1.xml", "https://invalid.example/private"),
            RELATIONS.replace(
                "http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet",
                "https://evil.example/worksheet",
            ),
            RELATIONS.replace("worksheets/sheet1.xml", "styles.xml"),
            RELATIONS.replace("rId1", "rId2"),
        ] {
            assert!(inspect(&fixture(SHEET, &relations)).is_err());
        }
        for sheet in [
            SHEET.replace(
                "SUM(A1:A2)",
                "WEBSERVICE(&quot;https://invalid.example&quot;)",
            ),
            SHEET.replace("SUM(A1:A2)", "SUM([other.xlsx]Sheet1!A1:A2)"),
            SHEET.replace("<f>", "<f t=\"shared\" si=\"0\">"),
            SHEET.replace("A3", "Q3"),
            SHEET.replace("</worksheet>", "<hyperlinks/></worksheet>"),
            SHEET.replace("<v>7</v>", "<v>NaN</v>"),
        ] {
            assert!(inspect(&fixture(&sheet, RELATIONS)).is_err());
        }
    }
}
