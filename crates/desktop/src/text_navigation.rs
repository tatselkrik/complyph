//! Exact location and whitespace-tolerant anchors. No fuzzy or cross-document guesses.
use complyph_checks::Evidence;
use complyph_core::SavedExtraction;
use std::ops::Range;

pub fn matching_ranges(text: &str, quote: &str) -> Vec<Range<usize>> {
    fn normalize(text: &str) -> (String, Vec<usize>) {
        let mut folded = String::new();
        let mut positions = Vec::new();
        let mut whitespace = false;
        for (byte, c) in text.char_indices() {
            if c.is_whitespace() {
                if whitespace {
                    continue;
                }
                folded.push(' ');
                positions.push(byte);
                whitespace = true;
            } else {
                folded.push(c);
                positions.extend(std::iter::repeat_n(byte, c.len_utf8()));
                whitespace = false;
            }
        }
        positions.push(text.len());
        (folded, positions)
    }
    let (needle, _) = normalize(quote.trim());
    if needle.is_empty() {
        return vec![];
    }
    let (haystack, positions) = normalize(text);
    haystack
        .match_indices(&needle)
        .map(|(start, _)| positions[start]..positions[start + needle.len()])
        .collect()
}

#[derive(Clone, Debug)]
pub struct Passage {
    pub unit: usize,
    pub ranges: Vec<Range<usize>>,
    pub active: usize,
}
pub fn locate(
    saved: &SavedExtraction,
    expected_hash: &str,
    evidence: &Evidence,
) -> Result<Passage, String> {
    if saved.sha256 != expected_hash {
        return Err("This document differs from the saved review. Passage navigation is disabled; the saved evidence and legal source remain available.".into());
    }
    let units: Vec<_> = saved
        .report
        .units
        .iter()
        .enumerate()
        .filter(|(_, u)| u.location == evidence.location)
        .collect();
    let [(unit, text)] = units.as_slice() else {
        return Err(
            "The saved original location is unavailable or ambiguous. No passage was guessed."
                .into(),
        );
    };
    let ranges = matching_ranges(&text.text, &evidence.text);
    if ranges.is_empty() {
        return Err("The saved passage could not be located in this text. No approximate match was highlighted.".into());
    }
    Ok(Passage {
        unit: *unit,
        ranges,
        active: 0,
    })
}
pub fn chunk_boundaries(text: &str) -> Vec<usize> {
    if text.is_empty() {
        return vec![0, 0];
    }
    text.char_indices()
        .step_by(8_000)
        .map(|(i, _)| i)
        .chain(std::iter::once(text.len()))
        .collect()
}
pub fn chunk_at(boundaries: &[usize], byte: usize) -> usize {
    boundaries
        .windows(2)
        .position(|pair| byte >= pair[0] && byte < pair[1])
        .unwrap_or(0)
}

/// Clip source-byte anchors into a display section. Non-overlapping anchors stay absent.
pub fn clip_ranges(ranges: &[Range<usize>], window: Range<usize>) -> Vec<Range<usize>> {
    ranges
        .iter()
        .filter_map(|range| {
            let start = range.start.max(window.start);
            let end = range.end.min(window.end);
            if start < end {
                Some(start - window.start..end - window.start)
            } else {
                None
            }
        })
        .collect()
}

pub fn highlighted_label(
    ui: &mut eframe::egui::Ui,
    text: &str,
    ranges: &[Range<usize>],
    scroll_to: Option<usize>,
) {
    use crate::theme::{AMBER_TINT, NAVY};
    use eframe::egui::{
        self, FontId, TextFormat,
        text::{CCursor, LayoutJob},
    };
    let base = TextFormat {
        font_id: FontId::proportional(15.0),
        color: NAVY,
        ..Default::default()
    };
    let mut job = LayoutJob::default();
    job.wrap.max_width = ui.available_width();
    let mut cursor = 0;
    for range in ranges {
        if range.start < cursor
            || range.end > text.len()
            || !text.is_char_boundary(range.start)
            || !text.is_char_boundary(range.end)
        {
            continue;
        }
        job.append(&text[cursor..range.start], 0.0, base.clone());
        job.append(
            &text[range.clone()],
            0.0,
            TextFormat {
                background: AMBER_TINT,
                ..base.clone()
            },
        );
        cursor = range.end;
    }
    job.append(&text[cursor..], 0.0, base);
    let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
    let response = ui.add(egui::Label::new(galley.clone()).selectable(true));
    if let Some(byte) = scroll_to.filter(|byte| *byte <= text.len() && text.is_char_boundary(*byte))
    {
        let position = galley.pos_from_cursor(CCursor::new(text[..byte].chars().count()));
        ui.scroll_to_rect(
            position.translate(response.rect.min.to_vec2()),
            Some(egui::Align::Center),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use complyph_extract::{Format, Location, Report, Status, TextUnit};
    fn fixture() -> (SavedExtraction, Evidence) {
        let location = Location::PdfPage { page: 2 };
        (SavedExtraction{sha256:"original".into(),read_at:0,report:Report{engine:complyph_extract::ENGINE.into(),format:Format::Pdf,status:Status::Ready,summary:String::new(),units:vec![TextUnit{location:location.clone(),text:"José’s normal daily\nworking  hours are 9 hours.\nJosé’s normal daily working hours are 9 hours.".into()}],warnings:vec![]}},Evidence{location,text:"José’s normal daily working hours are 9 hours".into(),hundredths:Some(900)})
    }
    #[test]
    fn unicode_wrapped_and_duplicate_passages_preserve_actual_byte_spans() {
        let (saved, evidence) = fixture();
        let result = locate(&saved, "original", &evidence).unwrap();
        assert_eq!(result.ranges.len(), 2);
        assert_eq!(
            &saved.report.units[0].text[result.ranges[0].clone()],
            "José’s normal daily\nworking  hours are 9 hours"
        );
        assert_eq!(
            &saved.report.units[0].text[result.ranges[1].clone()],
            evidence.text
        );
    }
    #[test]
    fn changed_hash_wrong_location_and_absent_passage_never_guess() {
        let (saved, mut evidence) = fixture();
        assert!(locate(&saved, "changed", &evidence).is_err());
        evidence.location = Location::PdfPage { page: 1 };
        assert!(locate(&saved, "original", &evidence).is_err());
        evidence.location = Location::PdfPage { page: 2 };
        evidence.text = "working hours are 8".into();
        assert!(locate(&saved, "original", &evidence).is_err());
        assert!(matching_ranges("", "").is_empty());
    }
    #[test]
    fn word_tables_and_parts_are_distinct_and_late_unicode_chunks_are_safe() {
        let (mut saved, mut evidence) = fixture();
        evidence.location = Location::WordParagraph {
            part: "word/header1.xml".into(),
            paragraph: 1,
            table_cell: None,
        };
        saved.report.units[0].location = Location::WordParagraph {
            part: "word/document.xml".into(),
            paragraph: 1,
            table_cell: None,
        };
        assert!(locate(&saved, "original", &evidence).is_err());
        let text = format!("{}target", "é".repeat(8_100));
        let boundaries = chunk_boundaries(&text);
        let found = matching_ranges(&text, "target");
        assert_eq!(chunk_at(&boundaries, found[0].start), 1);
        assert!(boundaries.iter().all(|&b| text.is_char_boundary(b)));
    }
    #[test]
    fn highlighting_does_not_underflow_in_related_sections_or_later_display_chunks() {
        let anchors = vec![10..20, 100..120];
        assert_eq!(clip_ranges(&anchors, 30..90), Vec::<Range<usize>>::new());
        assert_eq!(clip_ranges(&anchors, 110..200), vec![0..10]);
        assert_eq!(clip_ranges(&anchors, 0..15), vec![10..15]);
        assert_eq!(clip_ranges(&anchors, 200..300), Vec::<Range<usize>>::new());
    }
}
