use crate::{
    ENGINE, Format, Location, MAX_TEXT_BYTES, MAX_UNITS, Report, Status, TableCell, TextUnit,
};
use quick_xml::{NsReader, events::Event, name::ResolveResult};
use std::io::{Cursor, Read};

const WORD: &[u8] = b"http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const STRICT_WORD: &[u8] = b"http://purl.oclc.org/ooxml/wordprocessingml/main";
const MAX_XML: u64 = 16 * 1024 * 1024;
const MAX_EXPANDED: u64 = 64 * 1024 * 1024;

#[derive(Clone, Copy)]
enum Failure {
    Invalid,
    Limit,
    Password,
}

type Result<T> = std::result::Result<T, Failure>;

pub(super) fn extract(bytes: &[u8]) -> Report {
    match read(bytes) {
        Ok(report) => report,
        Err(Failure::Limit) => Report::failure(
            Format::Docx,
            Status::LimitExceeded,
            "This Word file exceeds a reading limit: 2,048 ZIP entries, 64 MiB expanded data, 16 MiB per text part, 128 XML levels, 12,000 paragraphs, or 4 MiB of text.",
        ),
        Err(Failure::Password) => Report::failure(
            Format::Docx,
            Status::PasswordProtected,
            "Use a local DOCX copy without password protection. ComplyPH does not request or store passwords.",
        ),
        Err(Failure::Invalid) => Report::failure(
            Format::Docx,
            Status::Unreadable,
            "This DOCX could not be read. It may be damaged, encrypted, or use unsupported content. The original is unchanged.",
        ),
    }
}

fn read(bytes: &[u8]) -> Result<Report> {
    // Office password-protected documents use an OLE compound file, not a ZIP.
    if bytes.starts_with(&[0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1]) {
        return Err(Failure::Password);
    }
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| Failure::Invalid)?;
    if archive.len() > 2048 {
        return Err(Failure::Limit);
    }
    let mut expanded = 0u64;
    let mut parts = Vec::new();
    let mut names = std::collections::HashSet::new();
    let mut content_types = false;
    for index in 0..archive.len() {
        let file = archive.by_index(index).map_err(|_| Failure::Invalid)?;
        let name = file.name().map_err(|_| Failure::Invalid)?.into_owned();
        if !names.insert(name.clone()) {
            return Err(Failure::Invalid);
        }
        expanded = expanded.checked_add(file.size()).ok_or(Failure::Limit)?;
        if expanded > MAX_EXPANDED {
            return Err(Failure::Limit);
        }
        if name == "[Content_Types].xml" {
            content_types = true;
        }
        if name == "word/document.xml" || is_supplement(&name) {
            if file.size() > MAX_XML {
                return Err(Failure::Limit);
            }
            parts.push(name.to_owned());
        }
    }
    if !content_types || !parts.iter().any(|part| part == "word/document.xml") {
        return Err(Failure::Invalid);
    }
    parts.sort_by_key(|part| (part != "word/document.xml", part.clone()));
    let mut report = Report {
        engine: ENGINE.into(), format: Format::Docx, status: Status::Ready,
        summary: "Word text read locally. Locations identify XML parts and paragraphs; Word page numbers are not inferred.".into(),
        units: vec![],
        warnings: vec![crate::DOCX_LAYOUT_WARNING.into()],
        ocr: None,
    };
    let mut used = 0;
    for part in parts {
        let mut xml = Vec::new();
        archive
            .by_name(&part)
            .map_err(|_| Failure::Invalid)?
            .take(MAX_XML + 1)
            .read_to_end(&mut xml)
            .map_err(|_| Failure::Invalid)?;
        if xml.len() as u64 > MAX_XML {
            return Err(Failure::Limit);
        }
        read_part(&xml, &part, &mut report.units, &mut used)?;
    }
    if report.units.is_empty() {
        report.status = Status::Unreadable;
        report.summary = "No supported Word text was found. Image-only content needs OCR, which is not included yet.".into();
    }
    Ok(report)
}

fn is_supplement(name: &str) -> bool {
    if matches!(name, "word/footnotes.xml" | "word/endnotes.xml") {
        return true;
    }
    ["word/header", "word/footer"].iter().any(|prefix| {
        name.strip_prefix(prefix)
            .and_then(|tail| tail.strip_suffix(".xml"))
            .is_some_and(|digits| {
                !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
            })
    })
}

fn read_part(xml: &[u8], part: &str, units: &mut Vec<TextUnit>, used: &mut usize) -> Result<()> {
    let mut reader = NsReader::from_reader(xml);
    reader.config_mut().expand_empty_elements = true;
    let mut depth = 0usize;
    let mut skip = None;
    let mut capture = false;
    let mut root_seen = false;
    let mut paragraph = 0;
    let mut active: Option<TextUnit> = None;
    let mut table_count = 0;
    let mut tables = Vec::<TableCell>::new();
    loop {
        let (namespace, event) = reader.read_resolved_event().map_err(|_| Failure::Invalid)?;
        let word = matches!(namespace, ResolveResult::Bound(ns) if ns.as_ref() == WORD || ns.as_ref() == STRICT_WORD);
        match event {
            Event::Start(tag) => {
                depth += 1;
                if depth > 128 {
                    return Err(Failure::Limit);
                }
                if depth == 1 {
                    if root_seen || !word {
                        return Err(Failure::Invalid);
                    }
                    let expected: &[u8] = if part == "word/document.xml" {
                        b"document"
                    } else if part.contains("header") {
                        b"hdr"
                    } else if part.contains("footer") {
                        b"ftr"
                    } else if part.contains("footnotes") {
                        b"footnotes"
                    } else {
                        b"endnotes"
                    };
                    if tag.local_name().as_ref() != expected {
                        return Err(Failure::Invalid);
                    }
                    root_seen = true;
                }
                // Count original XML elements even when their content is omitted.
                // Later visible passages must retain their actual part indices.
                if word {
                    match tag.local_name().as_ref() {
                        b"p" => {
                            paragraph += 1;
                            if paragraph > MAX_UNITS {
                                return Err(Failure::Limit);
                            }
                        }
                        b"tbl" => table_count += 1,
                        _ => (),
                    }
                }
                if skip.is_some() || !word {
                    continue;
                }
                match tag.local_name().as_ref() {
                    b"del" | b"moveFrom" | b"txbxContent" => {
                        skip = Some(depth);
                    }
                    b"tbl" => {
                        tables.push(TableCell {
                            table: table_count,
                            row: 0,
                            cell: 0,
                        });
                    }
                    b"tr" => {
                        if let Some(table) = tables.last_mut() {
                            table.row += 1;
                            table.cell = 0;
                        }
                    }
                    b"tc" => {
                        if let Some(table) = tables.last_mut() {
                            table.cell += 1;
                        }
                    }
                    b"p" => {
                        if active.is_some() {
                            return Err(Failure::Invalid);
                        }
                        active = Some(TextUnit {
                            location: Location::WordParagraph {
                                part: part.into(),
                                paragraph,
                                table_cell: tables.last().cloned(),
                            },
                            text: String::new(),
                        });
                    }
                    b"t" => capture = active.is_some(),
                    b"tab" => append(&mut active, "\t", used)?,
                    b"br" | b"cr" => append(&mut active, "\n", used)?,
                    _ => (),
                }
            }
            Event::End(tag) => {
                if depth == 0 {
                    return Err(Failure::Invalid);
                }
                if skip == Some(depth) {
                    skip = None;
                } else if skip.is_none() && word {
                    match tag.local_name().as_ref() {
                        b"t" => capture = false,
                        b"tbl" => {
                            tables.pop();
                        }
                        b"p" => {
                            if let Some(mut unit) = active.take() {
                                unit.text = unit.text.trim().to_owned();
                                if !unit.text.is_empty() {
                                    if units.len() >= MAX_UNITS {
                                        return Err(Failure::Limit);
                                    }
                                    units.push(unit);
                                }
                            }
                            capture = false;
                        }
                        _ => (),
                    }
                }
                depth -= 1;
            }
            Event::Text(text) if capture && skip.is_none() => {
                let text = text.decode().map_err(|_| Failure::Invalid)?;
                append(&mut active, &text, used)?;
            }
            Event::CData(text) if capture && skip.is_none() => {
                let text = text.decode().map_err(|_| Failure::Invalid)?;
                append(&mut active, &text, used)?;
            }
            Event::GeneralRef(reference) if capture && skip.is_none() => {
                if let Some(character) =
                    reference.resolve_char_ref().map_err(|_| Failure::Invalid)?
                {
                    append(&mut active, character.encode_utf8(&mut [0; 4]), used)?;
                } else {
                    let name = reference.decode().map_err(|_| Failure::Invalid)?;
                    let value = quick_xml::escape::resolve_predefined_entity(&name)
                        .ok_or(Failure::Invalid)?;
                    append(&mut active, value, used)?;
                }
            }
            Event::DocType(_) => return Err(Failure::Invalid),
            Event::Eof => break,
            _ => (),
        }
    }
    if !root_seen || depth != 0 || active.is_some() {
        return Err(Failure::Invalid);
    }
    Ok(())
}

fn append(active: &mut Option<TextUnit>, text: &str, used: &mut usize) -> Result<()> {
    if let Some(unit) = active {
        *used = used.checked_add(text.len()).ok_or(Failure::Limit)?;
        if *used > MAX_TEXT_BYTES {
            return Err(Failure::Limit);
        }
        unit.text.push_str(text);
    }
    Ok(())
}
