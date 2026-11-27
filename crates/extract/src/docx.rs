use crate::{
    ENGINE, Format, Location, MAX_TEXT_BYTES, MAX_UNITS, ReadingDetail, ReadingInfo, Report,
    Status, TableCell, TextUnit,
};
use quick_xml::{NsReader, events::Event, name::ResolveResult};
use std::io::{Cursor, Read};

const WORD: &[u8] = b"http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const STRICT_WORD: &[u8] = b"http://purl.oclc.org/ooxml/wordprocessingml/main";
const MC: &[u8] = b"http://schemas.openxmlformats.org/markup-compatibility/2006";
const DRAWING: &[u8] = b"http://schemas.openxmlformats.org/drawingml/2006/main";
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
        reading:Some(ReadingInfo{profile:"word-xml-2".into(),details:vec![]}),
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
        read_part(&xml, &part, &mut report, &mut used)?;
    }
    if report.units.is_empty() {
        report.status = Status::Unreadable;
        report.summary = "No supported Word text was found. Export an image-only Word document to PDF locally, then use Run local OCR. Embedded objects are not opened.".into();
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

#[derive(Default)]
struct Alternatives {
    fallback: std::collections::HashSet<usize>,
    uncertain: std::collections::HashSet<usize>,
}
struct Branch {
    id: usize,
    depth: usize,
    text: String,
}
fn fallbacks(xml: &[u8]) -> Result<Alternatives> {
    let mut reader = NsReader::from_reader(xml);
    reader.config_mut().expand_empty_elements = true;
    let mut result = Alternatives::default();
    let mut id = 0;
    let mut stack = vec![];
    let mut depth = 0usize;
    let mut branches = Vec::<Branch>::new();
    let mut texts = std::collections::HashMap::<usize, Vec<String>>::new();
    let mut capture = None;
    let mut compared = 0usize;
    loop {
        let (namespace, event) = reader.read_resolved_event().map_err(|_| Failure::Invalid)?;
        let mc = matches!(&namespace,ResolveResult::Bound(ns) if ns.as_ref()==MC);
        let word = matches!(&namespace,ResolveResult::Bound(ns) if ns.as_ref()==WORD || ns.as_ref()==STRICT_WORD);
        match event {
            Event::Start(tag) => {
                depth += 1;
                if depth > 128 {
                    return Err(Failure::Limit);
                }
                let name = tag.local_name();
                let name = name.as_ref();
                if mc && name == b"AlternateContent" {
                    id += 1;
                    stack.push((id, depth));
                }
                if mc
                    && matches!(name, b"Fallback" | b"Choice")
                    && let Some((id, parent)) = stack.last()
                    && *parent + 1 == depth
                {
                    if name == b"Fallback" {
                        result.fallback.insert(*id);
                    }
                    branches.push(Branch {
                        id: *id,
                        depth,
                        text: String::new(),
                    });
                }
                if word && name == b"t" {
                    capture = Some(depth);
                }
                if matches!(
                    name,
                    b"blip" | b"imagedata" | b"object" | b"altChunk" | b"sym"
                ) {
                    for branch in &branches {
                        result.uncertain.insert(branch.id);
                    }
                }
            }
            Event::End(tag) => {
                if depth == 0 {
                    return Err(Failure::Invalid);
                }
                if capture == Some(depth) {
                    capture = None;
                }
                if word && tag.local_name().as_ref() == b"p" {
                    for branch in &mut branches {
                        branch.text.push('\n');
                    }
                }
                if let Some(branch) = branches.pop_if(|b| b.depth == depth) {
                    texts
                        .entry(branch.id)
                        .or_default()
                        .push(branch.text.split_whitespace().collect::<Vec<_>>().join(" "));
                }
                if mc && tag.local_name().as_ref() == b"AlternateContent" {
                    stack.pop();
                }
                depth -= 1;
            }
            Event::Text(t) if capture.is_some() => {
                let text = t.decode().map_err(|_| Failure::Invalid)?;
                for b in &mut branches {
                    compared += text.len();
                    if compared > MAX_TEXT_BYTES * 2 {
                        return Err(Failure::Limit);
                    }
                    b.text.push_str(&text);
                }
            }
            Event::CData(t) if capture.is_some() => {
                let text = t.decode().map_err(|_| Failure::Invalid)?;
                for b in &mut branches {
                    compared += text.len();
                    if compared > MAX_TEXT_BYTES * 2 {
                        return Err(Failure::Limit);
                    }
                    b.text.push_str(&text);
                }
            }
            Event::GeneralRef(r) if capture.is_some() => {
                let text = if let Some(c) = r.resolve_char_ref().map_err(|_| Failure::Invalid)? {
                    c.to_string()
                } else {
                    quick_xml::escape::resolve_predefined_entity(
                        &r.decode().map_err(|_| Failure::Invalid)?,
                    )
                    .ok_or(Failure::Invalid)?
                    .into()
                };
                for b in &mut branches {
                    compared += text.len();
                    if compared > MAX_TEXT_BYTES * 2 {
                        return Err(Failure::Limit);
                    }
                    b.text.push_str(&text);
                }
            }
            Event::DocType(_) => return Err(Failure::Invalid),
            Event::Eof => break,
            _ => (),
        }
    }
    for (id, branches) in texts {
        if branches.windows(2).any(|b| b[0] != b[1]) {
            result.uncertain.insert(id);
        }
    }
    Ok(result)
}

struct BoxFrame {
    active: Option<TextUnit>,
    capture: bool,
    tables: Vec<TableCell>,
    text_box: Option<usize>,
}
fn note(report: &mut Report, part: &str, message: &str) {
    let warning = format!("{}: {message}", crate::word_part_label(part));
    if !report.warnings.contains(&warning) {
        report.warnings.push(warning);
    }
}
fn emit(active: &mut Option<TextUnit>, units: &mut Vec<TextUnit>) -> Result<()> {
    if let Some(mut unit) = active.take() {
        unit.text = unit.text.trim().into();
        if !unit.text.is_empty() {
            if units.len() >= MAX_UNITS {
                return Err(Failure::Limit);
            }
            units.push(unit);
        }
    }
    Ok(())
}
fn read_part(xml: &[u8], part: &str, report: &mut Report, used: &mut usize) -> Result<()> {
    let fallback = fallbacks(xml)?;
    let first_unit = report.units.len();
    let mut reader = NsReader::from_reader(xml);
    reader.config_mut().expand_empty_elements = true;
    let (mut depth, mut paragraph, mut table_count, mut box_count, mut alternate) =
        (0usize, 0, 0, 0, 0);
    let mut skip = None;
    let mut capture = false;
    let mut root_seen = false;
    let mut active: Option<TextUnit> = None;
    let mut tables = Vec::<TableCell>::new();
    let mut text_box = None;
    let mut frames = Vec::<BoxFrame>::new();
    let mut alternates = vec![];
    let mut chosen = std::collections::HashSet::new();
    loop {
        let (namespace, event) = reader.read_resolved_event().map_err(|_| Failure::Invalid)?;
        let word = matches!(&namespace,ResolveResult::Bound(ns) if ns.as_ref()==WORD || ns.as_ref()==STRICT_WORD);
        let mc = matches!(&namespace,ResolveResult::Bound(ns) if ns.as_ref()==MC);
        let drawing = matches!(&namespace,ResolveResult::Bound(ns) if ns.as_ref()==DRAWING);
        match event {
            Event::Start(tag) => {
                depth += 1;
                if depth > 128 {
                    return Err(Failure::Limit);
                }
                let name = tag.local_name();
                let name = name.as_ref();
                if depth == 1 {
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
                    if root_seen || !word || name != expected {
                        return Err(Failure::Invalid);
                    }
                    root_seen = true;
                }
                // Raw XML paragraph/table/box numbers include omitted branches and revisions.
                if word {
                    match name {
                        b"p" => {
                            paragraph += 1;
                            if paragraph > MAX_UNITS {
                                return Err(Failure::Limit);
                            }
                        }
                        b"tbl" => table_count += 1,
                        b"txbxContent" => box_count += 1,
                        _ => (),
                    }
                }
                if mc && name == b"AlternateContent" {
                    alternate += 1;
                    alternates.push(alternate);
                    if skip.is_none() && fallback.uncertain.contains(&alternate) {
                        note(
                            report,
                            part,
                            "Alternative Word representations contain different text or unread embedded content. Establish the operative version manually before numeric conclusions.",
                        );
                    }
                }
                if skip.is_some() {
                    continue;
                }
                if mc
                    && name == b"Choice"
                    && let Some(id) = alternates.last()
                    && (fallback.fallback.contains(id) || !chosen.insert(*id))
                {
                    skip = Some(depth);
                }
                if drawing && name == b"blip" {
                    note(
                        report,
                        part,
                        "Image content is not read in Word. Export to a local PDF and use OCR or review the complete original manually before making numeric conclusions.",
                    );
                }
                if drawing && name == b"t" {
                    note(
                        report,
                        part,
                        "DrawingML/chart text outside a supported Word text box is omitted. Embedded-content relationships need manual review.",
                    );
                }
                if !word {
                    continue;
                }
                match name {
                    b"del" | b"moveFrom" => {
                        note(
                            report,
                            part,
                            "Tracked changes are present; deleted/moved-from text is omitted. Establish the operative revision manually before numeric review.",
                        );
                        skip = Some(depth);
                    }
                    b"ins" | b"moveTo" => note(
                        report,
                        part,
                        "Tracked changes are present; deleted/moved-from text is omitted. Establish the operative revision manually before numeric review.",
                    ),
                    b"altChunk" | b"object" | b"subDoc" => note(
                        report,
                        part,
                        "An embedded object or external content part was not opened or read. Review that content manually; no external target is fetched.",
                    ),
                    b"fldChar" | b"instrText" | b"fldSimple" => note(
                        report,
                        part,
                        "Displayed field results may be stale. Field instructions are not evaluated; confirm the full original manually.",
                    ),
                    b"strike" | b"dstrike" | b"vanish" | b"webHidden" => note(
                        report,
                        part,
                        "Hidden or struck-through formatting is present. Plain extracted text cannot establish its operative meaning; inspect the original manually.",
                    ),
                    b"sym" => note(
                        report,
                        part,
                        "A symbol-font character could not be mapped safely. Inspect the original; no numeric interpretation is established.",
                    ),
                    b"txbxContent" => {
                        let outer = active.take();
                        frames.push(BoxFrame {
                            active: outer,
                            capture,
                            tables: std::mem::take(&mut tables),
                            text_box,
                        });
                        capture = false;
                        text_box = Some(box_count);
                    }
                    b"tbl" => {
                        if !tables.is_empty() {
                            note(
                                report,
                                part,
                                "Nested table relationships need manual review; text remains labelled by its innermost cell.",
                            );
                        }
                        tables.push(TableCell {
                            table: table_count,
                            row: 0,
                            cell: 0,
                        });
                    }
                    b"tr" => {
                        if let Some(t) = tables.last_mut() {
                            t.row += 1;
                            t.cell = 0;
                        }
                    }
                    b"tc" => {
                        if let Some(t) = tables.last_mut() {
                            t.cell += 1;
                        }
                    }
                    b"gridSpan" | b"vMerge" | b"hMerge" => {
                        let mut merged = name != b"gridSpan";
                        for a in tag.attributes() {
                            let a = a.map_err(|_| Failure::Invalid)?;
                            if a.key.local_name().as_ref() == b"val" {
                                merged |= a
                                    .decoded_and_normalized_value(
                                        quick_xml::XmlVersion::Implicit1_0,
                                        reader.decoder(),
                                    )
                                    .map_err(|_| Failure::Invalid)?
                                    .as_ref()
                                    != "1";
                            }
                        }
                        if merged {
                            note(
                                report,
                                part,
                                "Merged table cells need manual review; cell text does not establish header/value relationships.",
                            );
                        }
                    }
                    b"p" => {
                        if active.is_some() {
                            return Err(Failure::Invalid);
                        }
                        let location = match text_box {
                            Some(text_box) => Location::WordTextBox {
                                part: part.into(),
                                text_box,
                                paragraph,
                                table_cell: tables.last().cloned(),
                            },
                            None => Location::WordParagraph {
                                part: part.into(),
                                paragraph,
                                table_cell: tables.last().cloned(),
                            },
                        };
                        if text_box.is_some() {
                            report.reading.as_mut().unwrap().details.push(ReadingDetail{location:location.clone(),message:"Word text box read at its own XML location. Floating position, page placement and applicability are not inferred.".into()});
                        }
                        active = Some(TextUnit {
                            location,
                            text: String::new(),
                        });
                    }
                    b"t" => capture = active.is_some(),
                    b"tab" => append(&mut active, "\t", used)?,
                    b"br" | b"cr" | b"lastRenderedPageBreak" => append(&mut active, "\n", used)?,
                    b"noBreakHyphen" => append(&mut active, "\u{2011}", used)?,
                    b"softHyphen" => append(&mut active, "\u{00ad}", used)?,
                    _ => (),
                }
            }
            Event::End(tag) => {
                if depth == 0 {
                    return Err(Failure::Invalid);
                }
                let name = tag.local_name();
                let name = name.as_ref();
                if skip == Some(depth) {
                    skip = None;
                } else if skip.is_none() && word {
                    match name {
                        b"t" => capture = false,
                        b"tbl" => {
                            tables.pop();
                        }
                        b"p" => {
                            emit(&mut active, &mut report.units)?;
                            capture = false;
                        }
                        b"txbxContent" => {
                            if active.is_some() {
                                return Err(Failure::Invalid);
                            }
                            let frame = frames.pop().ok_or(Failure::Invalid)?;
                            active = frame.active;
                            capture = frame.capture;
                            tables = frame.tables;
                            text_box = frame.text_box;
                        }
                        _ => (),
                    }
                }
                if mc && name == b"AlternateContent" {
                    alternates.pop();
                }
                depth -= 1;
            }
            Event::Text(t) if capture && skip.is_none() => {
                let s = t.decode().map_err(|_| Failure::Invalid)?;
                append(&mut active, &s, used)?;
            }
            Event::CData(t) if capture && skip.is_none() => {
                let s = t.decode().map_err(|_| Failure::Invalid)?;
                append(&mut active, &s, used)?;
            }
            Event::GeneralRef(r) if capture && skip.is_none() => {
                if let Some(c) = r.resolve_char_ref().map_err(|_| Failure::Invalid)? {
                    append(&mut active, c.encode_utf8(&mut [0; 4]), used)?;
                } else {
                    let name = r.decode().map_err(|_| Failure::Invalid)?;
                    let text = quick_xml::escape::resolve_predefined_entity(&name)
                        .ok_or(Failure::Invalid)?;
                    append(&mut active, text, used)?;
                }
            }
            Event::DocType(_) => return Err(Failure::Invalid),
            Event::Eof => break,
            _ => (),
        }
    }
    if !root_seen || depth != 0 || active.is_some() || !frames.is_empty() {
        return Err(Failure::Invalid);
    }
    // One unit per original XML paragraph: a box never splits its containing
    // paragraph into duplicate evidence locations. Floating page order is unknown.
    report.units[first_unit..].sort_by_key(|u| match &u.location {
        Location::WordParagraph { paragraph, .. } | Location::WordTextBox { paragraph, .. } => {
            *paragraph
        }
        _ => 0,
    });
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
