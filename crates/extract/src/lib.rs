//! Memory-only document readers. No networking, external programs, or file writes.
//! Run untrusted parsers in the desktop's bounded worker, never on the UI thread.

mod docx;
mod pdf;
mod processing;
pub use processing::*;

use serde::{Deserialize, Serialize};

pub const PDF_LAYOUT_WARNING: &str = "Reading order, columns, tables, handwriting, and unusual fonts may not match the original. Images are not read. No compliance assessment has been made.";
pub const LEGACY_DOCX_LAYOUT_WARNING: &str = "Tables are read cell by cell. Headers, footers, and notes appear as separate parts and may not apply to every page. Images, text boxes, comments, deleted/moved-from text, and embedded objects are omitted. Field instructions, macros, and external links are never executed or fetched. Check layout and revision history in the original. No compliance assessment has been made.";
pub const DOCX_LAYOUT_WARNING: &str = "Tables are read cell by cell; Word text boxes have separate labelled locations. Headers, footers, notes and floating text may not apply to every page or follow visual page order. Images, comments, deleted/moved-from text and embedded objects are not read. Field instructions, macros and external links are never executed or fetched. Check layout and revision history in the original. No compliance assessment has been made.";
pub const ENGINE: &str = "complyph-text-2";
pub const OCR_ENGINE: &str = "complyph-ocr-2";
pub const LEGACY_OCR_ENGINE: &str = "complyph-ocr-1";
pub const OCR_MODEL_SHA256: &str =
    "7d4322bd2a7749724879683fc3912cb542f19906c83bcc1a52132556427170b2";
pub const OCR_WARNING: &str = "Machine-read OCR text: confidence is only a recognition estimate, not proof of accuracy. Compare every page, number and operative term with the original before confirming OCR text. Handwriting, tables, stamps and poor scans may be missed.";
pub const MAX_INPUT_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_TEXT_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_UNITS: usize = 12_000;
pub const MAX_PAGES: usize = 300;
pub const MAX_REPORT_BYTES: u64 = 12 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Format {
    Pdf,
    Docx,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Ready,
    NeedsOcr,
    PasswordProtected,
    Unreadable,
    LimitExceeded,
}

impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Self::Ready => "Text available",
            Self::NeedsOcr => "Needs OCR",
            Self::PasswordProtected => "Password protected",
            Self::Unreadable => "Cannot read text",
            Self::LimitExceeded => "Reading limit reached",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableCell {
    pub table: usize,
    pub row: usize,
    pub cell: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Location {
    PdfPage {
        page: u32,
    },
    WordParagraph {
        part: String,
        paragraph: usize,
        table_cell: Option<TableCell>,
    },
    WordTextBox {
        part: String,
        text_box: usize,
        paragraph: usize,
        table_cell: Option<TableCell>,
    },
}

impl Location {
    pub fn label(&self) -> String {
        match self {
            Self::PdfPage { page } => format!("Page {page}"),
            Self::WordParagraph {
                part,
                paragraph,
                table_cell,
            } => {
                let section = word_part_label(part);
                match table_cell {
                    Some(cell) => format!(
                        "{section} · table {}, row {}, cell {} · paragraph {paragraph}",
                        cell.table, cell.row, cell.cell
                    ),
                    None => format!("{section} · paragraph {paragraph}"),
                }
            }
            Self::WordTextBox {
                part,
                text_box,
                paragraph,
                table_cell,
            } => {
                let section = word_part_label(part);
                let cell = table_cell
                    .as_ref()
                    .map(|c| format!(" · table {}, row {}, cell {}", c.table, c.row, c.cell))
                    .unwrap_or_default();
                format!("{section} · text box {text_box}{cell} · paragraph {paragraph}")
            }
        }
    }
}
pub fn word_part_label(part: &str) -> String {
    match part {
        "word/document.xml" => "Body".into(),
        "word/footnotes.xml" => "Footnotes".into(),
        "word/endnotes.xml" => "Endnotes".into(),
        _ => {
            for (prefix, label) in [("word/header", "Header"), ("word/footer", "Footer")] {
                if let Some(number) = part
                    .strip_prefix(prefix)
                    .and_then(|s| s.strip_suffix(".xml"))
                {
                    return format!("{label} {number}");
                }
            }
            part.strip_prefix("word/").unwrap_or(part).into()
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextUnit {
    pub location: Location,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OcrWord {
    /// UTF-8 byte offsets into the corresponding page's reconstructed text.
    pub start: usize,
    pub end: usize,
    /// Hundredths of a percent, from Tesseract's recognition estimate.
    pub confidence: u16,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OcrPage {
    pub page: u32,
    pub words: usize,
    pub uncertain: Vec<OcrWord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub processing: Option<OcrPageProcessing>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OcrInfo {
    pub engine_version: String,
    pub renderer: String,
    pub language: String,
    pub model_sha256: String,
    pub confidence_threshold: u16,
    pub pages: Vec<OcrPage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub processing: Option<OcrProcessing>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadingDetail {
    pub location: Location,
    pub message: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadingInfo {
    pub profile: String,
    pub details: Vec<ReadingDetail>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    pub engine: String,
    pub format: Format,
    pub status: Status,
    pub summary: String,
    pub units: Vec<TextUnit>,
    pub warnings: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ocr: Option<OcrInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reading: Option<ReadingInfo>,
}

impl Report {
    pub fn failure(format: Format, status: Status, summary: &str) -> Self {
        Self {
            engine: ENGINE.into(),
            format,
            status,
            summary: summary.into(),
            units: vec![],
            warnings: vec![],
            ocr: None,
            reading: None,
        }
    }

    pub fn text_bytes(&self) -> usize {
        self.units.iter().map(|unit| unit.text.len()).sum()
    }
}

/// Validate OCR metadata before caching it or treating it as a recognized reading method.
pub fn valid_ocr(report: &Report) -> bool {
    (report.engine == OCR_ENGINE || report.engine == LEGACY_OCR_ENGINE)
        && report.format == Format::Pdf
        && report.ocr.as_ref().is_some_and(|o| {
            o.language == "eng"
                && o.model_sha256 == OCR_MODEL_SHA256
                && o.renderer == "pdfium-8165"
                && o.confidence_threshold == 8000
                && !o.pages.is_empty()
                && o.pages.len() == report.units.len()
                && o.pages.len() <= 20
                && if report.engine == OCR_ENGINE {
                    o.processing
                        .as_ref()
                        .is_some_and(|p| p.profile == OCR_PROCESSING_PROFILE)
                        && o.pages
                            .iter()
                            .all(|p| p.processing.as_ref().is_some_and(OcrPageProcessing::valid))
                } else {
                    o.processing.is_none() && o.pages.iter().all(|p| p.processing.is_none())
                }
                && o.pages.iter().zip(&report.units).all(|(p, u)| {
                    u.location == Location::PdfPage { page: p.page }
                        && p.page > 0
                        && p.uncertain.len() <= p.words
                        && p.uncertain.iter().all(|w| {
                            w.start < w.end
                                && w.end <= u.text.len()
                                && u.text.is_char_boundary(w.start)
                                && u.text.is_char_boundary(w.end)
                                && w.confidence < 8000
                        })
                })
        })
}

/// Errors never include parser diagnostics, paths, passwords, or document excerpts.
pub fn extract(format: Format, bytes: &[u8]) -> Report {
    if bytes.len() > MAX_INPUT_BYTES {
        return Report::failure(
            format,
            Status::LimitExceeded,
            "Choose a document of 32 MiB or less.",
        );
    }
    match format {
        Format::Pdf => pdf::extract(bytes),
        Format::Docx => docx::extract(bytes),
    }
}

/// Forms/annotation appearances are not established by plain text or memory OCR.
/// Call only in the bounded parser worker; no annotation action/URL is executed.
pub fn pdf_content_cautions(doc: &lopdf::Document) -> Vec<String> {
    let mut warnings = vec![];
    if let Some(form) = doc.catalog().ok().and_then(|d| d.get(b"AcroForm").ok()) {
        let dictionary = match form {
            lopdf::Object::Reference(id) => doc.get_dictionary(*id).ok(),
            o => o.as_dict().ok(),
        };
        let unestablished = dictionary.is_none_or(|d| {
            d.has(b"XFA")
                || d.get(b"Fields")
                    .ok()
                    .and_then(|o| o.as_array().ok())
                    .is_none_or(|a| !a.is_empty())
        });
        if unestablished {
            warnings.push("PDF form/XFA fields are present. Their filled values and appearances are not established; use a flattened local PDF copy or manual review before numeric conclusions.".into());
        }
    }
    for (page, id) in doc.get_pages() {
        let Some(annotations) = doc
            .get_dictionary(id)
            .ok()
            .and_then(|d| d.get(b"Annots").ok())
        else {
            continue;
        };
        let annotations = match annotations {
            lopdf::Object::Reference(id) => doc.get_object(*id).ok(),
            o => Some(o),
        };
        let unestablished = annotations.and_then(|o| o.as_array().ok()).is_none_or(|a| {
            a.len() > 10000
                || a.iter().any(|o| {
                    let d = match o {
                        lopdf::Object::Reference(id) => doc.get_dictionary(*id).ok(),
                        o => o.as_dict().ok(),
                    };
                    d.and_then(|d| d.get(b"Subtype").ok())
                        .and_then(|o| o.as_name().ok())
                        != Some(b"Link".as_slice())
                })
        });
        if unestablished {
            warnings.push(format!("Page {page}: annotations or form appearances are not included reliably in extracted text/OCR. Use a flattened local copy or inspect the original manually."));
        }
    }
    warnings
}
