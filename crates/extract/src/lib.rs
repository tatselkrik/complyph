//! Memory-only document readers. No networking, external programs, or file writes.
//! Run untrusted parsers in the desktop's bounded worker, never on the UI thread.

mod docx;

use serde::{Deserialize, Serialize};
use std::io::{self, Write};

pub const PDF_LAYOUT_WARNING: &str = "Reading order, columns, tables, handwriting, and unusual fonts may not match the original. Images are not read. No compliance assessment has been made.";
pub const DOCX_LAYOUT_WARNING: &str = "Tables are read cell by cell. Headers, footers, and notes appear as separate parts and may not apply to every page. Images, text boxes, comments, deleted/moved-from text, and embedded objects are omitted. Field instructions, macros, and external links are never executed or fetched. Check layout and revision history in the original. No compliance assessment has been made.";
pub const ENGINE: &str = "complyph-text-1";
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
                let section = if part == "word/document.xml" {
                    "Body".to_owned()
                } else {
                    part.strip_prefix("word/").unwrap_or(part).to_owned()
                };
                match table_cell {
                    Some(cell) => format!(
                        "{section} · table {}, row {}, cell {} · paragraph {paragraph}",
                        cell.table, cell.row, cell.cell
                    ),
                    None => format!("{section} · paragraph {paragraph}"),
                }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextUnit {
    pub location: Location,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    pub engine: String,
    pub format: Format,
    pub status: Status,
    pub summary: String,
    pub units: Vec<TextUnit>,
    pub warnings: Vec<String>,
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
        }
    }

    pub fn text_bytes(&self) -> usize {
        self.units.iter().map(|unit| unit.text.len()).sum()
    }
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
        Format::Pdf => pdf(bytes),
        Format::Docx => docx::extract(bytes),
    }
}

struct LimitedText {
    text: String,
    remaining: usize,
    exceeded: bool,
}

impl Write for LimitedText {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.remaining {
            self.exceeded = true;
            return Err(io::Error::other("text limit"));
        }
        let text = std::str::from_utf8(bytes).map_err(io::Error::other)?;
        self.text.push_str(text);
        self.remaining -= bytes.len();
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn pdf(bytes: &[u8]) -> Report {
    let failed = || {
        Report::failure(
            Format::Pdf,
            Status::Unreadable,
            "This PDF could not be read. It may be damaged or use unsupported content. The original is unchanged.",
        )
    };
    let Ok(document) = lopdf::Document::load_mem(bytes) else {
        return failed();
    };
    if document.is_encrypted() {
        return Report::failure(
            Format::Pdf,
            Status::PasswordProtected,
            "Open a local copy without password protection. ComplyPH does not request or store passwords.",
        );
    }
    let pages = document.get_pages();
    if pages.is_empty() {
        return failed();
    }
    if pages.len() > MAX_PAGES {
        return Report::failure(
            Format::Pdf,
            Status::LimitExceeded,
            "This reader supports PDFs up to 300 pages.",
        );
    }
    let mut report = Report {
        engine: ENGINE.into(),
        format: Format::Pdf,
        status: Status::Ready,
        summary: "PDF text read locally. Page numbers refer to the original PDF.".into(),
        units: vec![],
        warnings: vec![PDF_LAYOUT_WARNING.into()],
    };
    let mut used = 0;
    let mut empty = Vec::new();
    for page in pages.keys().copied() {
        let mut text = LimitedText {
            text: String::new(),
            remaining: MAX_TEXT_BYTES - used,
            exceeded: false,
        };
        let result = {
            let writer: &mut dyn Write = &mut text;
            let mut output = pdf_extract::PlainTextOutput::new(writer);
            pdf_extract::output_doc_page(&document, &mut output, page)
        };
        if text.exceeded {
            return Report::failure(
                Format::Pdf,
                Status::LimitExceeded,
                "The extracted text exceeds the 4 MiB reading limit.",
            );
        }
        if result.is_err() {
            return failed();
        }
        let content = text.text.trim().to_owned();
        used += content.len();
        if content.is_empty() {
            empty.push(page.to_string());
        }
        report.units.push(TextUnit {
            location: Location::PdfPage { page },
            text: content,
        });
    }
    if empty.len() == report.units.len() {
        report.status = Status::NeedsOcr;
        report.summary = "No selectable text was found. This may be a scanned or image-only PDF. Offline OCR is not included yet.".into();
    } else if !empty.is_empty() {
        report.warnings.push(format!(
            "No text on page(s) {}. These may be blank or need OCR; they have not been read.",
            empty.join(", ")
        ));
    }
    report
}
