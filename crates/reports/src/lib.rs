//! Offline PDF reports. No browser, printer service, network or original-file access.
use complyph_core::{ReviewEvent, ReviewWorkflow};
use lopdf::{
    Dictionary, Document, Object, Stream,
    content::{Content, Operation},
    dictionary,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Write,
    path::Path,
};
use thiserror::Error;

pub const ENGINE: &str = "complyph-pdf-1";
pub const MAX_PAGES: usize = 200;
const MAX_TEXT_BYTES: usize = 8 * 1024 * 1024;
const WIDTH: f32 = 595.28;
const HEIGHT: f32 = 841.89;
const LEFT: f32 = 48.0;
const RIGHT: f32 = WIDTH - 48.0;
const REGULAR: &[u8] = include_bytes!("../../../assets/fonts/Lato-Regular.ttf");
const BOLD: &[u8] = include_bytes!("../../../assets/fonts/Lato-Bold.ttf");

#[derive(Debug, Error)]
pub enum ExportError {
    #[error("{0}")]
    Invalid(String),
    #[error("The local PDF could not be written: {0}")]
    Io(#[from] std::io::Error),
    #[error("The PDF could not be encoded: {0}")]
    Pdf(#[from] lopdf::Error),
}
type Result<T> = std::result::Result<T, ExportError>;

fn time(value: i64) -> String {
    chrono::DateTime::from_timestamp(value, 0)
        .map(|d| d.format("%Y-%m-%d %H:%M:%S UTC").to_string())
        .unwrap_or_else(|| "Unrecorded time".into())
}
fn face(bytes: &'static [u8]) -> Result<ttf_parser::Face<'static>> {
    ttf_parser::Face::parse(bytes, 0)
        .map_err(|_| ExportError::Invalid("The bundled PDF font could not be read.".into()))
}
fn advance(face: &ttf_parser::Face<'_>, c: char, size: f32) -> Result<f32> {
    let glyph=face.glyph_index(c).ok_or_else(||ExportError::Invalid(format!("The bundled PDF font cannot display U+{:04X}. Copy the full report JSON to retain text in unsupported scripts.",c as u32)))?;
    Ok(face.glyph_hor_advance(glyph).unwrap_or(0) as f32 * size / face.units_per_em() as f32)
}
#[derive(Clone)]
struct Line {
    text: String,
    x: f32,
    y: f32,
    size: f32,
    bold: bool,
    color: [f32; 3],
}
struct Page {
    lines: Vec<Line>,
}
struct Layout {
    pages: Vec<Page>,
    y: f32,
    bytes: usize,
    regular: ttf_parser::Face<'static>,
    bold: ttf_parser::Face<'static>,
}
impl Layout {
    fn new() -> Result<Self> {
        let mut layout = Self {
            pages: Vec::new(),
            y: 0.0,
            bytes: 0,
            regular: face(REGULAR)?,
            bold: face(BOLD)?,
        };
        layout.page()?;
        Ok(layout)
    }
    fn page(&mut self) -> Result<()> {
        if self.pages.len() >= MAX_PAGES {
            return Err(ExportError::Invalid("This report exceeds the 200-page PDF limit. Copy the full report JSON for its complete history.".into()));
        }
        self.pages.push(Page {
            lines: vec![
                Line {
                    text: "ComplyPH".into(),
                    x: LEFT,
                    y: HEIGHT - 34.0,
                    size: 16.0,
                    bold: true,
                    color: [0.98, 0.96, 0.91],
                },
                Line {
                    text: "LOCAL DOCUMENT REVIEW".into(),
                    x: RIGHT - 145.0,
                    y: HEIGHT - 32.0,
                    size: 8.0,
                    bold: false,
                    color: [0.98, 0.96, 0.91],
                },
            ],
        });
        self.y = HEIGHT - 86.0;
        Ok(())
    }
    fn ensure(&mut self, height: f32) -> Result<()> {
        if self.y - height < 64.0 {
            self.page()?;
        }
        Ok(())
    }
    fn paragraph(&mut self, text: &str, size: f32, bold: bool, color: [f32; 3]) -> Result<()> {
        self.bytes = self
            .bytes
            .checked_add(text.len())
            .ok_or_else(|| ExportError::Invalid("The report is too large.".into()))?;
        if self.bytes > MAX_TEXT_BYTES {
            return Err(ExportError::Invalid(
                "This report exceeds the 8 MiB PDF text limit. Copy the full report JSON instead."
                    .into(),
            ));
        }
        let font = if bold { &self.bold } else { &self.regular };
        let normalized = text
            .replace("\r\n", "\n")
            .replace('\r', "\n")
            .replace('\t', "    ");
        let mut lines = Vec::new();
        for paragraph in normalized.split('\n') {
            let mut line = String::new();
            let mut width = 0.0;
            for c in paragraph.chars() {
                let next = advance(font, c, size)?;
                if width + next > RIGHT - LEFT - 2.0 && !line.is_empty() {
                    if let Some(space) = line.rfind(' ') {
                        let tail = line[space + 1..].to_owned();
                        lines.push(line[..space].to_owned());
                        line = tail;
                        width = line
                            .chars()
                            .map(|c| advance(font, c, size))
                            .collect::<Result<Vec<_>>>()?
                            .iter()
                            .sum();
                    } else {
                        lines.push(std::mem::take(&mut line));
                        width = 0.0;
                    }
                    if width + next > RIGHT - LEFT - 2.0 && !line.is_empty() {
                        lines.push(std::mem::take(&mut line));
                        width = 0.0;
                    }
                }
                line.push(c);
                width += next;
            }
            lines.push(line);
        }
        for text in lines {
            self.ensure(size * 1.4)?;
            self.pages.last_mut().expect("page").lines.push(Line {
                text,
                x: LEFT,
                y: self.y,
                size,
                bold,
                color,
            });
            self.y -= size * 1.4;
        }
        self.y -= 5.0;
        Ok(())
    }
    fn body(&mut self, text: &str) -> Result<()> {
        self.paragraph(text, 10.0, false, [0.12, 0.16, 0.20])
    }
    fn muted(&mut self, text: &str) -> Result<()> {
        self.paragraph(text, 8.5, false, [0.32, 0.37, 0.41])
    }
    fn heading(&mut self, text: &str) -> Result<()> {
        self.ensure(58.0)?;
        self.y -= 10.0;
        self.paragraph(text, 13.0, true, [0.08, 0.17, 0.29])
    }
    fn event(&mut self, event: &ReviewEvent) -> Result<()> {
        let change = match (event.previous_status, event.new_status) {
            (Some(a), Some(b)) => format!("{} to {}", a.label(), b.label()),
            _ => "Note added".into(),
        };
        self.muted(&format!(
            "Event #{} | {} | {} | {}",
            event.id,
            time(event.created_at),
            event.reviewer,
            change
        ))?;
        self.body(&event.note)
    }
}

/// Render only the captured historical review and its captured human history.
pub fn render_pdf(workflow: &ReviewWorkflow) -> Result<Vec<u8>> {
    let mut layout = Layout::new()?;
    layout.paragraph(&workflow.document_name, 20.0, true, [0.08, 0.17, 0.29])?;
    layout.body(&format!("Document review #{}", workflow.saved.id))?;
    layout.muted(&format!(
        "Automated review: {}",
        time(workflow.saved.created_at)
    ))?;
    layout.muted(&format!(
        "Export snapshot: {} | Tracking through event #{}",
        time(workflow.captured_at),
        workflow.last_event_id()
    ))?;
    layout.body("This report preserves a saved automated review and separate human follow-up. A resolved follow-up status does not change the automated result or certify legal compliance.")?;
    layout.muted("Contains document excerpts and reviewer notes. Reviewer names are self-reported, not verified identities or digital signatures. Export does not recheck the current original or current law.")?;
    layout.heading("Document review notes")?;
    let document_notes: Vec<_> = workflow
        .events
        .iter()
        .filter(|e| e.rule_id.is_none())
        .collect();
    if document_notes.is_empty() {
        layout.body("No document review notes recorded.")?;
    }
    for event in document_notes {
        layout.event(event)?;
    }
    for (index, finding) in workflow.saved.review.findings.iter().enumerate() {
        layout.heading(&format!("{}. {}", index + 1, finding.title))?;
        layout.body(&format!("Automated result: {}", finding.outcome.label()))?;
        layout.body(&finding.explanation)?;
        layout.body(&format!(
            "Confirmed scope: {}",
            workflow
                .saved
                .review
                .context
                .applicability(&finding.rule_id)
                .label()
        ))?;
        layout.body(&format!(
            "Human follow-up status: {}",
            workflow.status(&finding.rule_id).label()
        ))?;
        layout.muted(&format!("Rule: {}", finding.rule_id))?;
        if finding.rule_id == "PH-LAB-095-01" {
            for (label, value) in workflow
                .saved
                .review
                .context
                .service_incentive_leave
                .details()
            {
                layout.body(&format!("{label}: {value}"))?;
            }
            let basis = workflow
                .saved
                .review
                .context
                .service_incentive_leave
                .basis
                .trim();
            layout.body(&format!(
                "Reviewer-provided eligibility basis: {}",
                if basis.is_empty() {
                    "Not recorded"
                } else {
                    basis
                }
            ))?;
        }
        if finding.evidence.is_empty() {
            layout.body("No supported document passage was recorded.")?;
        }
        for evidence in &finding.evidence {
            layout.muted(&format!(
                "Recorded document passage | {}",
                evidence.location.label()
            ))?;
            layout.body(&evidence.text)?;
        }
        layout.body(&format!("Supporting provision: {}", finding.article))?;
        layout.body(&finding.legal_quote)?;
        let history: Vec<_> = workflow
            .events
            .iter()
            .filter(|e| e.rule_id.as_deref() == Some(&finding.rule_id))
            .collect();
        if history.is_empty() {
            layout.muted("No reviewer actions recorded for this finding.")?;
        }
        for event in history {
            layout.event(event)?;
        }
    }
    layout.heading("Preserved scope and limitations")?;
    layout.body(&workflow.saved.review.limitation)?;
    layout.body(&format!(
        "Operative-term accuracy confirmation: {}",
        if workflow.saved.review.context.operative_terms_confirmed {
            "recorded by the original reviewer"
        } else {
            "not established"
        }
    ))?;
    layout.heading("Snapshot and source identifiers")?;
    if let Some(ocr) = &workflow.saved.review.ocr {
        layout.body(&format!(
            "OCR reading: Tesseract {} | {} | {}",
            ocr.engine_version, ocr.renderer, ocr.language
        ))?;
        layout.body(&format!("{} OCR pages; {} low-confidence words. All text still requires comparison with the original.",ocr.pages.len(),ocr.pages.iter().map(|p|p.uncertain.len()).sum::<usize>()))?;
        layout.body(&format!(
            "OCR accuracy confirmation: {}",
            if workflow.saved.review.context.ocr_text_confirmed {
                "recorded by the original reviewer"
            } else {
                "not established"
            }
        ))?;
        layout.muted(&format!("OCR model SHA-256: {}", ocr.model_sha256))?;
    }
    for line in [
        format!("Rule engine: {}", workflow.saved.review.engine),
        format!(
            "Document SHA-256: {}",
            workflow.saved.review.document_sha256
        ),
        format!("Source key: {}", workflow.saved.review.source_key),
        format!("Source SHA-256: {}", workflow.saved.review.source_sha256),
        format!(
            "Reading-text SHA-256: {}",
            workflow.saved.review.source_text_sha256
        ),
        format!("Recorded source URL: {}", workflow.saved.review.source_url),
        format!(
            "Original review fingerprint: {}",
            workflow.review_fingerprint
        ),
        format!(
            "PDF engine: {ENGINE} | ComplyPH {}",
            env!("CARGO_PKG_VERSION")
        ),
    ] {
        layout.muted(&line)?;
    }
    let count = layout.pages.len();
    for (index, page) in layout.pages.iter_mut().enumerate() {
        page.lines.push(Line {
            text: format!(
                "Review #{} | Page {} of {} | Historical snapshot",
                workflow.saved.id,
                index + 1,
                count
            ),
            x: LEFT,
            y: 32.0,
            size: 8.0,
            bold: false,
            color: [0.32, 0.37, 0.41],
        });
    }
    encode(&layout.pages, &layout.regular, &layout.bold, workflow)
}

fn unicode(value: &str) -> Object {
    let mut bytes = vec![0xfe, 0xff];
    for unit in value.encode_utf16() {
        bytes.extend(unit.to_be_bytes());
    }
    Object::String(bytes, lopdf::StringFormat::Hexadecimal)
}
fn embed_font(
    doc: &mut Document,
    bytes: &'static [u8],
    face: &ttf_parser::Face<'_>,
    chars: &BTreeMap<char, u16>,
    name: &str,
) -> Result<lopdf::ObjectId> {
    let scale = 1000.0 / face.units_per_em() as f32;
    let font_file = doc.add_object(Stream::new(
        dictionary! {"Length1"=>bytes.len() as i64},
        bytes.to_vec(),
    ));
    let bbox = face.global_bounding_box();
    let descriptor=doc.add_object(dictionary!{
        "Type"=>"FontDescriptor","FontName"=>Object::Name(name.as_bytes().to_vec()),"Flags"=>32,
        "FontBBox"=>[bbox.x_min as f32*scale,bbox.y_min as f32*scale,bbox.x_max as f32*scale,bbox.y_max as f32*scale].into_iter().map(Object::Real).collect::<Vec<_>>(),
        "ItalicAngle"=>0,"Ascent"=>face.ascender() as f32*scale,"Descent"=>face.descender() as f32*scale,
        "CapHeight"=>face.capital_height().unwrap_or(face.ascender()) as f32*scale,"StemV"=>80,"FontFile2"=>font_file
    });
    let mut glyphs = vec![0u8; 2 * (chars.len() + 1)];
    let mut widths = Vec::new();
    let mut mappings = Vec::new();
    for (&c, &cid) in chars {
        let glyph = face.glyph_index(c).ok_or_else(|| {
            ExportError::Invalid(format!("The PDF font cannot display U+{:04X}.", c as u32))
        })?;
        glyphs[cid as usize * 2..cid as usize * 2 + 2].copy_from_slice(&glyph.0.to_be_bytes());
        widths.push(Object::Integer(
            (face.glyph_hor_advance(glyph).unwrap_or(0) as f32 * scale).round() as i64,
        ));
        let code = c
            .encode_utf16(&mut [0u16; 2])
            .iter()
            .map(|u| format!("{u:04X}"))
            .collect::<String>();
        mappings.push(format!("<{cid:04X}> <{code}>\n"));
    }
    let gid_map = doc.add_object(Stream::new(Dictionary::new(), glyphs));
    let mut cmap = String::from(
        "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n/CMapName /ComplyPHUnicode def\n/CMapType 2 def\n1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
    );
    for chunk in mappings.chunks(100) {
        cmap.push_str(&format!("{} beginbfchar\n", chunk.len()));
        for line in chunk {
            cmap.push_str(line);
        }
        cmap.push_str("endbfchar\n");
    }
    cmap.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
    let to_unicode = doc.add_object(Stream::new(Dictionary::new(), cmap.into_bytes()));
    let descendant=doc.add_object(dictionary!{
        "Type"=>"Font","Subtype"=>"CIDFontType2","BaseFont"=>Object::Name(name.as_bytes().to_vec()),
        "CIDSystemInfo"=>dictionary!{"Registry"=>Object::string_literal("Adobe"),"Ordering"=>Object::string_literal("Identity"),"Supplement"=>0},
        "FontDescriptor"=>descriptor,"CIDToGIDMap"=>gid_map,"DW"=>1000,"W"=>vec![Object::Integer(1),Object::Array(widths)]
    });
    Ok(doc.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type0","BaseFont"=>Object::Name(name.as_bytes().to_vec()),"Encoding"=>"Identity-H","DescendantFonts"=>vec![Object::Reference(descendant)],"ToUnicode"=>to_unicode}))
}
fn encode(
    pages: &[Page],
    regular: &ttf_parser::Face<'_>,
    bold: &ttf_parser::Face<'_>,
    workflow: &ReviewWorkflow,
) -> Result<Vec<u8>> {
    let all: BTreeSet<char> = pages
        .iter()
        .flat_map(|p| p.lines.iter().flat_map(|l| l.text.chars()))
        .collect();
    if all.len() >= u16::MAX as usize {
        return Err(ExportError::Invalid(
            "The PDF contains too many distinct characters.".into(),
        ));
    }
    let chars: BTreeMap<_, _> = all
        .into_iter()
        .enumerate()
        .map(|(i, c)| (c, (i + 1) as u16))
        .collect();
    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();
    let f1 = embed_font(&mut doc, REGULAR, regular, &chars, "Lato-Regular")?;
    let f2 = embed_font(&mut doc, BOLD, bold, &chars, "Lato-Bold")?;
    let mut children = Vec::new();
    for page in pages {
        let mut operations = vec![
            Operation::new("rg", vec![0.08.into(), 0.17.into(), 0.29.into()]),
            Operation::new(
                "re",
                vec![0.into(), (HEIGHT - 52.0).into(), WIDTH.into(), 52.into()],
            ),
            Operation::new("f", vec![]),
        ];
        for line in &page.lines {
            let text: Vec<u8> = line
                .text
                .chars()
                .flat_map(|c| chars[&c].to_be_bytes())
                .collect();
            operations.extend([
                Operation::new("BT", vec![]),
                Operation::new(
                    "Tf",
                    vec![
                        Object::Name(if line.bold { b"F2" } else { b"F1" }.to_vec()),
                        line.size.into(),
                    ],
                ),
                Operation::new("rg", line.color.iter().map(|v| Object::Real(*v)).collect()),
                Operation::new(
                    "Tm",
                    vec![
                        1.into(),
                        0.into(),
                        0.into(),
                        1.into(),
                        line.x.into(),
                        line.y.into(),
                    ],
                ),
                Operation::new(
                    "Tj",
                    vec![Object::String(text, lopdf::StringFormat::Hexadecimal)],
                ),
                Operation::new("ET", vec![]),
            ]);
        }
        let content = doc.add_object(Stream::new(
            Dictionary::new(),
            Content { operations }.encode()?,
        ));
        let id=doc.add_object(dictionary!{"Type"=>"Page","Parent"=>pages_id,"MediaBox"=>vec![0.into(),0.into(),WIDTH.into(),HEIGHT.into()],"Contents"=>content,"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>f1,"F2"=>f2}}});
        children.push(Object::Reference(id));
    }
    doc.objects.insert(
        pages_id,
        dictionary! {"Type"=>"Pages","Kids"=>children,"Count"=>pages.len() as i64}.into(),
    );
    let root = doc.add_object(
        dictionary! {"Type"=>"Catalog","Pages"=>pages_id,"Lang"=>Object::string_literal("en-PH")},
    );
    let info=doc.add_object(dictionary!{"Title"=>unicode(&format!("ComplyPH review #{}: {}",workflow.saved.id,workflow.document_name)),"Creator"=>Object::string_literal(format!("ComplyPH {} / {ENGINE}",env!("CARGO_PKG_VERSION"))),"Subject"=>unicode("Historical technical review and separate human follow-up"),"FontLicense"=>Object::string_literal("Lato fonts: SIL Open Font License 1.1")});
    doc.trailer.set("Root", root);
    doc.trailer.set("Info", info);
    doc.compress();
    let mut output = Vec::new();
    doc.save_to(&mut output)?;
    Ok(output)
}

/// Complete the PDF before atomically creating a new destination. Never overwrite originals.
pub fn export_pdf(workflow: &ReviewWorkflow, destination: &Path) -> Result<()> {
    if destination.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "Choose a new PDF filename; the destination already exists.",
        )
        .into());
    }
    let bytes = render_pdf(workflow)?;
    let parent = destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(&bytes)?;
    temporary.as_file().sync_all()?;
    temporary
        .persist_noclobber(destination)
        .map_err(|e| ExportError::Io(e.error))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use complyph_core::{ReviewActionRequest, ReviewStatus, Workspace, WorkspacePaths};
    use std::fs;

    fn setup() -> (tempfile::TempDir, Workspace, ReviewWorkflow) {
        let temp = tempfile::tempdir().unwrap();
        let workspace =
            Workspace::open(&WorkspacePaths::under(temp.path().join("private data"))).unwrap();
        let original = temp.path().join("Fictional José policy.docx");
        let bytes = include_bytes!("../../../examples/Synthetic labour issues.docx");
        fs::write(&original, bytes).unwrap();
        let document = workspace.register_document(&original).unwrap();
        let report = complyph_extract::extract(complyph_extract::Format::Docx, bytes);
        use complyph_checks::Applicability::Covered;
        let review = complyph_checks::review(
            &report,
            "synthetic-document-hash",
            complyph_checks::Context {
                operative_terms_confirmed: true,
                ocr_text_confirmed: false,
                normal_hours: Covered,
                ordinary_overtime: Covered,
                night_differential: Covered,
                meal_period: Covered,
                weekly_rest: Covered,
                service_incentive_leave: Default::default(),
            },
        );
        assert!(
            review.findings[..5]
                .iter()
                .all(|f| f.outcome == complyph_checks::Outcome::PotentialIssue)
        );
        let saved = workspace.save_review(document.id, review).unwrap();
        let state = workspace.review_workflow(saved.id).unwrap();
        (temp, workspace, state)
    }
    fn add(
        workspace: &Workspace,
        state: &ReviewWorkflow,
        rule: Option<&str>,
        status: Option<ReviewStatus>,
        note: &str,
    ) -> ReviewWorkflow {
        workspace
            .append_review_action(
                state.saved.id,
                &ReviewActionRequest {
                    review_fingerprint: state.review_fingerprint.clone(),
                    rule_id: rule.map(str::to_owned),
                    expected_revision: state.revision(rule),
                    reviewer: "José Example".into(),
                    note: note.into(),
                    status,
                },
            )
            .unwrap();
        workspace.review_workflow(state.saved.id).unwrap()
    }
    fn text(bytes: &[u8]) -> String {
        pdf_extract::extract_text_from_mem(bytes)
            .unwrap()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn pdf_preserves_findings_evidence_citations_unicode_and_complete_separate_history() {
        let (temp, workspace, state) = setup();
        let original = serde_json::to_string(&state.saved).unwrap();
        let state = add(
            &workspace,
            &state,
            None,
            None,
            "Fictional ₱500 example; private notes remain local.",
        );
        let state = add(
            &workspace,
            &state,
            Some("PH-LAB-083-01"),
            Some(ReviewStatus::Acknowledged),
            "Assigned for manual follow-up.",
        );
        let state = add(
            &workspace,
            &state,
            Some("PH-LAB-083-01"),
            Some(ReviewStatus::Resolved),
            "Follow-up complete. Original evidence is retained.",
        );
        let output = temp.path().join("report.pdf");
        export_pdf(&state, &output).unwrap();
        let bytes = fs::read(output).unwrap();
        let rendered = text(&bytes);
        for expected in [
            "José",
            "₱500",
            "Unresolved to Acknowledged",
            "Acknowledged to Resolved",
            "Human follow-up status: Resolved",
            "Automated result: Potential issue",
            "Original evidence is retained.",
            "complyph-labor-4",
            "complyph-pdf-1",
            "Document review notes",
            "does not change the automated result",
        ] {
            assert!(
                rendered.contains(expected),
                "missing {expected}: {rendered}"
            );
        }
        for finding in &state.saved.review.findings {
            assert!(rendered.contains(&finding.rule_id));
            assert!(rendered.contains(&finding.article));
            assert!(
                rendered.contains(
                    &finding
                        .legal_quote
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ")
                )
            );
            for evidence in &finding.evidence {
                assert!(
                    rendered.contains(
                        &evidence
                            .text
                            .split_whitespace()
                            .collect::<Vec<_>>()
                            .join(" ")
                    )
                );
            }
        }
        assert!(!rendered.contains(temp.path().to_str().unwrap()));
        assert_eq!(
            serde_json::to_string(&workspace.review_workflow(state.saved.id).unwrap().saved)
                .unwrap(),
            original
        );
    }

    #[test]
    fn long_history_paginates_without_clipping_and_note_content_cannot_create_pdf_actions() {
        let (_temp, workspace, mut state) = setup();
        for i in 0..8 {
            state = add(
                &workspace,
                &state,
                None,
                None,
                &format!(
                    "Entry {i}: {} END-{i}",
                    "A note with q /JS (untrusted) Q /OpenAction text. ".repeat(80)
                ),
            );
        }
        state.saved.review.source_url = format!("https://example.gov.ph/{}", "x".repeat(300));
        let bytes = render_pdf(&state).unwrap();
        let rendered = text(&bytes);
        for i in 0..8 {
            assert!(rendered.contains(&format!("END-{i}")));
        }
        let document = Document::load_mem(&bytes).unwrap();
        assert!(document.get_pages().len() >= 8);
        let mut font_files = 0;
        for object in document.objects.values() {
            if let Ok(dictionary) = object.as_dict() {
                for forbidden in [
                    b"OpenAction".as_slice(),
                    b"AA",
                    b"JS",
                    b"JavaScript",
                    b"EmbeddedFiles",
                    b"Annots",
                ] {
                    assert!(!dictionary.has(forbidden));
                }
                if dictionary.has(b"FontFile2") {
                    font_files += 1;
                }
            }
        }
        assert_eq!(font_files, 2);
        let mut layout = Layout::new().unwrap();
        layout.body(&format!("A {} END", "x".repeat(500))).unwrap();
        for line in layout
            .pages
            .iter()
            .flat_map(|p| &p.lines)
            .filter(|l| l.y < HEIGHT - 52.0)
        {
            let width: f32 = line
                .text
                .chars()
                .map(|c| advance(&layout.regular, c, line.size).unwrap())
                .sum();
            assert!(line.x + width <= RIGHT, "line extends past margin");
            assert!(line.y >= 64.0);
        }
    }

    #[test]
    fn exports_are_atomic_never_overwrite_and_explicitly_reject_unsupported_or_oversize_text() {
        let (temp, workspace, state) = setup();
        let original = temp.path().join("Fictional José policy.docx");
        let before = fs::read(&original).unwrap();
        assert!(export_pdf(&state, &original).is_err());
        assert_eq!(fs::read(&original).unwrap(), before);
        let mut unsupported = state.clone();
        unsupported.document_name = "Unsupported 🧪 script".into();
        let failed = temp.path().join("failed.pdf");
        assert!(
            export_pdf(&unsupported, &failed)
                .unwrap_err()
                .to_string()
                .contains("U+1F9EA")
        );
        assert!(!failed.exists());
        let mut huge = state.clone();
        huge.document_name = "Word ".repeat(300_000);
        assert!(
            export_pdf(&huge, &failed)
                .unwrap_err()
                .to_string()
                .contains("200-page")
        );
        assert!(!failed.exists());
        fs::remove_file(&original).unwrap();
        let historical = workspace.review_workflow(state.saved.id).unwrap();
        export_pdf(&historical, &temp.path().join("historical.pdf")).unwrap();
        // A historical three-rule report is preserved without manufacturing newer findings.
        let mut old = historical;
        old.saved.review.findings.truncate(3);
        old.saved.review.engine = "complyph-labor-1".into();
        let text = text(&render_pdf(&old).unwrap());
        assert!(text.contains("complyph-labor-1"));
        assert!(!text.contains("PH-LAB-085-01"));
    }
}
