//! Local memory-only PDF rendering/recognition. Call only inside the bounded worker.
//! No URL/path is passed to the renderer or recognizer: the input and model are bytes.
mod native;
use complyph_extract::{Format, Location, OcrInfo, OcrPage, OcrWord, Report, Status, TextUnit};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub const MAX_PAGES: usize = 20;
pub const MAX_SIDE: u32 = 2800;
pub const THRESHOLD: u16 = 8000;
pub const MODEL_SHA256: &str = "7d4322bd2a7749724879683fc3912cb542f19906c83bcc1a52132556427170b2";
pub const RENDERER: &str = "pdfium-8165";

pub fn runtime_directory() -> Result<PathBuf, String> {
    // Developer override is a local directory, never a URL or document-derived value.
    #[cfg(debug_assertions)]
    if let Some(path) = std::env::var_os("COMPLYPH_OCR_DIR") {
        return Ok(PathBuf::from(path));
    }
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.join("OCR")))
        .ok_or_else(|| {
            "The bundled OCR folder could not be located. Reinstall the complete app.".into()
        })
}

pub fn recognize(bytes: &[u8], directory: &Path) -> Report {
    let preflight = complyph_extract::extract(Format::Pdf, bytes);
    if !matches!(preflight.status, Status::Ready | Status::NeedsOcr) {
        return preflight;
    }
    if preflight.units.len() > MAX_PAGES {
        return Report::failure(
            Format::Pdf,
            Status::LimitExceeded,
            "Local OCR supports up to 20 PDF pages per run. Split a longer scanned document locally first.",
        );
    }
    let result = recognize_inner(bytes, directory);
    match result {
        Ok(report) => report,
        Err(message) => Report::failure(Format::Pdf, Status::Unreadable, &message),
    }
}
fn recognize_inner(bytes: &[u8], directory: &Path) -> Result<Report, String> {
    let model = std::fs::read(directory.join("eng.traineddata"))
        .map_err(|_| "The bundled English OCR data is missing. Reinstall the complete app; no data is downloaded automatically.")?;
    if format!("{:x}", Sha256::digest(&model)) != MODEL_SHA256 {
        return Err(
            "The OCR model does not match the packaged version. Reinstall the complete app.".into(),
        );
    }
    let mut engine = native::Engine::new(directory, bytes, &model)?;
    let count = engine.pages();
    if count == 0 || count > MAX_PAGES {
        return Ok(Report::failure(
            Format::Pdf,
            Status::LimitExceeded,
            "Local OCR supports up to 20 PDF pages per run. Split a longer scanned document locally before using OCR.",
        ));
    }
    let mut units = Vec::new();
    let mut pages = Vec::new();
    let mut used = 0;
    for index in 0..count {
        let tsv = engine.page(index)?;
        let (text, quality) = parse_tsv(&tsv, index as u32 + 1)?;
        used += text.len();
        if used > complyph_extract::MAX_TEXT_BYTES {
            return Err("The OCR text exceeds the 4 MiB reading limit.".into());
        }
        units.push(TextUnit {
            location: Location::PdfPage {
                page: index as u32 + 1,
            },
            text,
        });
        pages.push(quality);
    }
    let any_text = units.iter().any(|u| !u.text.is_empty());
    let empty = units.iter().any(|u| u.text.is_empty());
    let mut warnings = vec![complyph_extract::OCR_WARNING.into()];
    if empty {
        warnings.push("One or more pages produced no OCR text. Their contents are unestablished; automatic numeric conclusions are blocked.".into());
    }
    Ok(Report {
        engine: complyph_extract::OCR_ENGINE.into(), format: Format::Pdf,
        status: if any_text { Status::Ready } else { Status::NeedsOcr },
        summary: "Pages recognized locally with English OCR. Physical PDF page numbers are preserved. Confirm all text against the original before checking.".into(),
        units, warnings,
        ocr: Some(OcrInfo { engine_version: engine.version(), renderer: RENDERER.into(), language: "eng".into(), model_sha256: MODEL_SHA256.into(), confidence_threshold: THRESHOLD, pages }),
    })
}

fn parse_tsv(tsv: &str, page: u32) -> Result<(String, OcrPage), String> {
    let mut text = String::new();
    let mut uncertain = Vec::new();
    let mut previous = None;
    let mut words = 0;
    for row in tsv.lines().skip(1) {
        let fields: Vec<_> = row.splitn(12, '\t').collect();
        if fields.len() != 12 {
            return Err("The local OCR engine returned invalid word locations.".into());
        }
        if fields[0] != "5" {
            continue;
        }
        let word = fields[11].trim();
        if word.is_empty() {
            continue;
        }
        let line = (
            fields[2].to_owned(),
            fields[3].to_owned(),
            fields[4].to_owned(),
        );
        if !text.is_empty() {
            text.push(if previous.as_ref() == Some(&line) {
                ' '
            } else {
                '\n'
            });
        }
        let start = text.len();
        text.push_str(word);
        words += 1;
        let score: f64 = fields[10]
            .parse()
            .map_err(|_| "The local OCR confidence could not be read.")?;
        if !score.is_finite() || !(0.0..=100.0).contains(&score) {
            return Err("The local OCR confidence is out of range.".into());
        }
        let confidence = (score * 100.0).round() as u16;
        if confidence < THRESHOLD {
            uncertain.push(OcrWord {
                start,
                end: text.len(),
                confidence,
            });
        }
        if text.len() > complyph_extract::MAX_TEXT_BYTES || uncertain.len() > 20_000 {
            return Err("This OCR page exceeds its safe text or uncertainty limit.".into());
        }
        previous = Some(line);
    }
    Ok((
        text,
        OcrPage {
            page,
            words,
            uncertain,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tsv_preserves_lines_utf8_and_flags_recognition_estimates_without_certifying_numbers() {
        let (text, page) = parse_tsv("header\n5\t1\t1\t1\t1\t1\t0\t0\t5\t5\t95.1\tJosé\n5\t1\t1\t1\t1\t2\t0\t0\t5\t5\t42\t8.01\n5\t1\t1\t1\t2\t1\t0\t0\t5\t5\t99\thours", 7).unwrap();
        assert_eq!(text, "José 8.01\nhours");
        assert_eq!(page.page, 7);
        assert_eq!(page.words, 3);
        assert_eq!(
            &text[page.uncertain[0].start..page.uncertain[0].end],
            "8.01"
        );
        assert_eq!(page.uncertain[0].confidence, 4200);
        assert!(parse_tsv("header\ninvalid", 1).is_err());
        assert!(parse_tsv("header\n5\t1\t1\t1\t1\t1\t0\t0\t5\t5\tNaN\tx", 1).is_err());
    }
}
