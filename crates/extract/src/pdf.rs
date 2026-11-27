//! Position-based English/LTR PDF text. No rendering, networking or file access.
use crate::*;
use pdf_extract::{ColorSpace, MediaBox, OutputDev, OutputError, Path, PathOp, Transform};
use std::{collections::HashSet, io};

const MAX_GLYPHS: usize = 100_000;
#[derive(Clone)]
struct Glyph {
    x: f64,
    y: f64,
    end: f64,
    height: f64,
    text: String,
}
#[derive(Clone)]
struct Span {
    x: f64,
    end: f64,
    text: String,
}
struct Line {
    y: f64,
    height: f64,
    spans: Vec<Span>,
}
fn paragraphs(lines: &[&Line], gutter: Option<(f64, bool)>) -> Vec<String> {
    let mut out = Vec::<String>::new();
    let mut previous: Option<(f64, f64, f64, bool)> = None;
    for l in lines {
        let spans = l
            .spans
            .iter()
            .filter(|s| gutter.is_none_or(|(x, left)| if left { s.x < x } else { s.x >= x }))
            .collect::<Vec<_>>();
        if spans.is_empty() {
            continue;
        }
        let text = spans
            .iter()
            .map(|s| s.text.as_str())
            .collect::<Vec<_>>()
            .join("\t");
        let x = spans[0].x;
        let wraps = previous.is_some_and(|(y, height, old_x, single)| {
            single
                && spans.len() == 1
                && y - l.y > 0.0
                && y - l.y <= height.max(l.height) * 2.8
                && (old_x - x).abs() < height.max(l.height) * 2.0
        }) && out
            .last()
            .is_some_and(|s| !s.ends_with(['.', '!', '?', ';', ':']));
        if wraps {
            let p = out.last_mut().unwrap();
            p.push(' ');
            p.push_str(&text);
        } else {
            out.push(text);
        }
        previous = Some((l.y, l.height, x, spans.len() == 1));
    }
    out
}
#[derive(Default)]
struct Page {
    glyphs: Vec<Glyph>,
    bytes: usize,
    exceeded: bool,
    bad_mapping: bool,
    non_horizontal: bool,
    horizontal_rules: usize,
    vertical_rules: usize,
    width: f64,
    height: f64,
}
impl OutputDev for Page {
    fn begin_page(
        &mut self,
        _: u32,
        m: &MediaBox,
        _: Option<(f64, f64, f64, f64)>,
    ) -> Result<(), OutputError> {
        self.width = (m.urx - m.llx).abs();
        self.height = (m.ury - m.lly).abs();
        Ok(())
    }
    fn end_page(&mut self) -> Result<(), OutputError> {
        Ok(())
    }
    fn begin_word(&mut self) -> Result<(), OutputError> {
        Ok(())
    }
    fn end_word(&mut self) -> Result<(), OutputError> {
        Ok(())
    }
    fn end_line(&mut self) -> Result<(), OutputError> {
        Ok(())
    }
    fn output_character(
        &mut self,
        t: &Transform,
        width: f64,
        _: f64,
        size: f64,
        text: &str,
    ) -> Result<(), OutputError> {
        self.bytes += text.len();
        if self.bytes > MAX_TEXT_BYTES || self.glyphs.len() >= MAX_GLYPHS {
            self.exceeded = true;
            return Err(io::Error::other("glyph limit").into());
        }
        let height = size.abs() * t.m21.hypot(t.m22);
        let end = t.m31 + width * size * t.m11;
        if ![t.m31, t.m32, end, height, t.m11, t.m12, t.m21, t.m22]
            .iter()
            .all(|x| x.is_finite())
            || height <= 0.0
        {
            self.bad_mapping = true;
            return Ok(());
        }
        if t.m11 <= 0.0 || t.m12.abs() > t.m11.abs() * 0.04 || t.m21.abs() > t.m22.abs() * 0.04 {
            self.non_horizontal = true;
        }
        if text.is_empty()
            || text.chars().any(|c| {
                c == '\u{fffd}'
                    || ('\u{e000}'..='\u{f8ff}').contains(&c)
                    || (c.is_control() && !c.is_whitespace())
            })
        {
            self.bad_mapping = true;
        }
        self.glyphs.push(Glyph {
            x: t.m31,
            y: t.m32,
            end,
            height,
            text: text.into(),
        });
        Ok(())
    }
    fn stroke(
        &mut self,
        t: &Transform,
        _: &ColorSpace,
        _: &[f64],
        path: &Path,
    ) -> Result<(), OutputError> {
        let point = |x: f64, y: f64| (x * t.m11 + y * t.m21 + t.m31, x * t.m12 + y * t.m22 + t.m32);
        let mut previous = None;
        for op in &path.ops {
            match *op {
                PathOp::MoveTo(x, y) => previous = Some(point(x, y)),
                PathOp::LineTo(x, y) => {
                    let next = point(x, y);
                    if let Some((a, b)) = previous {
                        if (a - next.0).abs() < 1.0 && (b - next.1).abs() > 20.0 {
                            self.vertical_rules += 1;
                        }
                        if (b - next.1).abs() < 1.0 && (a - next.0).abs() > 20.0 {
                            self.horizontal_rules += 1;
                        }
                    }
                    previous = Some(next);
                }
                PathOp::Rect(_, _, w, h) if w.abs() > 20.0 && h.abs() > 10.0 => {
                    self.vertical_rules += 2;
                    self.horizontal_rules += 2;
                }
                _ => previous = None,
            }
        }
        Ok(())
    }
}
impl Page {
    fn lines(&mut self) -> Vec<Line> {
        self.glyphs
            .sort_by(|a, b| b.y.total_cmp(&a.y).then(a.x.total_cmp(&b.x)));
        let mut rows: Vec<Vec<Glyph>> = vec![];
        for glyph in std::mem::take(&mut self.glyphs) {
            let same = rows.last().is_some_and(|row| {
                (row[0].y - glyph.y).abs()
                    <= row[0].height.min(glyph.height).mul_add(0.3, 0.3).min(4.0)
            });
            if !same {
                rows.push(vec![]);
            }
            rows.last_mut().unwrap().push(glyph);
        }
        rows.into_iter()
            .map(|mut row| {
                row.sort_by(|a, b| a.x.total_cmp(&b.x));
                let y = row[0].y;
                let height = row.iter().map(|g| g.height).fold(0.0, f64::max);
                let mut spans: Vec<Span> = vec![];
                let mut previous: Option<Glyph> = None;
                for glyph in row {
                    if let Some(p) = &previous
                        && (glyph.x - p.x).abs() < 0.15
                        && (glyph.y - p.y).abs() < 0.15
                    {
                        if glyph.text == p.text {
                            continue;
                        }
                        if !glyph.text.trim().is_empty() && !p.text.trim().is_empty() {
                            self.bad_mapping = true;
                        }
                    }
                    let gap = previous.as_ref().map_or(0.0, |p| glyph.x - p.end);
                    if spans.is_empty() || gap > height.mul_add(1.8, 0.0).max(14.0) {
                        spans.push(Span {
                            x: glyph.x,
                            end: glyph.end,
                            text: String::new(),
                        });
                    }
                    let span = spans.last_mut().unwrap();
                    if gap > height * 0.18
                        && !span.text.ends_with(char::is_whitespace)
                        && !glyph.text.starts_with(char::is_whitespace)
                        && !span.text.is_empty()
                    {
                        span.text.push(' ');
                    }
                    span.text.push_str(&glyph.text);
                    span.end = span.end.max(glyph.end);
                    previous = Some(glyph);
                }
                for s in &mut spans {
                    s.text = s.text.trim().into();
                }
                spans.retain(|s| !s.text.is_empty());
                Line { y, height, spans }
            })
            .filter(|l| !l.spans.is_empty())
            .collect()
    }
    fn text(&mut self) -> (String, String, bool) {
        let lines = self.lines();
        let ruled = self.vertical_rules >= 2 && self.horizontal_rules >= 2;
        let gaps = lines
            .iter()
            .flat_map(|l| l.spans.windows(2).map(|s| (s[0].end, s[1].x)))
            .filter(|(a, b)| b - a >= 36.0)
            .collect::<Vec<_>>();
        let mut scores = [0usize; 80];
        if self.width > 0.0 {
            for (i, score) in scores.iter_mut().enumerate() {
                let x = self.width * (i as f64 + 0.5) / 80.0;
                *score = gaps
                    .iter()
                    .filter(|(a, b)| x > *a + 3.0 && x < *b - 3.0)
                    .count();
            }
        }
        let best = scores.iter().copied().max().unwrap_or(0);
        let mut regions: Vec<Vec<usize>> = vec![];
        if best >= 2 {
            for (i, score) in scores.iter().enumerate() {
                if *score >= best.max(2) * 3 / 4 && *score >= 2 {
                    if regions.last().is_none_or(|r| r.last().unwrap() + 1 != i) {
                        regions.push(vec![]);
                    }
                    regions.last_mut().unwrap().push(i);
                }
            }
        }
        let mut table = ruled;
        let gutter = if regions.len() == 1 {
            let r = &regions[0];
            let gutter = self.width * ((r[0] + r[r.len() - 1]) as f64 + 1.0) / 160.0;
            let left = lines
                .iter()
                .flat_map(|l| &l.spans)
                .filter(|s| s.end < gutter)
                .count();
            let right = lines
                .iter()
                .flat_map(|l| &l.spans)
                .filter(|s| s.x > gutter)
                .count();
            (left >= 3 && right >= 3).then_some(gutter)
        } else {
            None
        };
        if let Some(gutter) = gutter {
            let pairs = lines
                .iter()
                .filter(|l| l.spans.len() == 2 && l.spans[0].end < gutter && l.spans[1].x > gutter)
                .collect::<Vec<_>>();
            if pairs.len() >= 3
                && pairs
                    .iter()
                    .filter(|l| {
                        l.spans[1].end - l.spans[1].x < 100.0
                            && l.spans[0].end - l.spans[0].x < 180.0
                    })
                    .count()
                    * 2
                    >= pairs.len()
            {
                table = true;
            }
        }
        let row_text = |l: &Line| {
            l.spans
                .iter()
                .map(|s| s.text.as_str())
                .collect::<Vec<_>>()
                .join("\t")
        };
        if !table && let Some(gutter) = gutter {
            let mut out = vec![];
            let mut block = vec![];
            let flush = |block: &mut Vec<&Line>, out: &mut Vec<String>| {
                for left in [true, false] {
                    out.extend(paragraphs(block, Some((gutter, left))));
                }
                block.clear();
            };
            for l in &lines {
                if l.spans
                    .iter()
                    .any(|s| s.x < gutter - 5.0 && s.end > gutter + 5.0)
                {
                    flush(&mut block, &mut out);
                    out.push(row_text(l));
                } else {
                    block.push(l);
                }
            }
            flush(&mut block, &mut out);
            return (out.join("\n"),"Two separated text columns read top-to-bottom, left column before right, between full-width text. Verify order against the original.".into(),false);
        }
        let uncertain = table || regions.len() > 1;
        let content = if uncertain {
            lines.iter().map(row_text).collect::<Vec<_>>().join("\n")
        } else {
            paragraphs(&lines.iter().collect::<Vec<_>>(), None).join("\n")
        };
        (content,if table {"Table-like text read in positioned rows; tab separators mark gaps, not verified cell relationships."}else if regions.len()>1 {"Multiple possible column gutters; rows retained without inferring paragraph relationships."}else{"Horizontal text ordered by position; close aligned soft wraps are joined. Verify paragraph boundaries against the original."}.into(),uncertain)
    }
}

fn dictionary<'a>(doc: &'a lopdf::Document, o: &'a lopdf::Object) -> Option<&'a lopdf::Dictionary> {
    match o {
        lopdf::Object::Reference(id) => doc.get_dictionary(*id).ok(),
        _ => o.as_dict().ok(),
    }
}
fn resources(doc: &lopdf::Document, page: lopdf::ObjectId) -> Option<&lopdf::Dictionary> {
    let (direct, parents) = doc.get_page_resources(page).ok()?;
    direct.or_else(|| {
        parents
            .into_iter()
            .find_map(|id| doc.get_dictionary(id).ok())
    })
}
fn images(
    doc: &lopdf::Document,
    content: &[u8],
    res: Option<&lopdf::Dictionary>,
    scale: f64,
    depth: usize,
    visited: &mut HashSet<lopdf::ObjectId>,
    calls: &mut usize,
) -> Result<f64, ()> {
    *calls += 1;
    if depth > 8 || *calls > 128 || content.len() > 16 * 1024 * 1024 {
        return Err(());
    }
    let content = lopdf::content::Content::decode(content).map_err(|_| ())?;
    if content.operations.len() > 100_000 {
        return Err(());
    }
    let mut scale = scale;
    let mut stack = vec![];
    let mut area = 0.0;
    for op in content.operations {
        match op.operator.as_str() {
            "q" => stack.push(scale),
            "Q" => scale = stack.pop().unwrap_or(1.0),
            "cm" if op.operands.len() == 6 => {
                let n = op
                    .operands
                    .iter()
                    .map(|o| o.as_float().map(f64::from))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|_| ())?;
                scale *= (n[0] * n[3] - n[1] * n[2]).abs();
                if !scale.is_finite() {
                    return Err(());
                }
            }
            "BI" => area += scale,
            "Do" => {
                let name = op
                    .operands
                    .first()
                    .and_then(|o| o.as_name().ok())
                    .ok_or(())?;
                let Some(objects) = res
                    .and_then(|r| r.get(b"XObject").ok())
                    .and_then(|o| dictionary(doc, o))
                else {
                    continue;
                };
                let object = objects.get(name).map_err(|_| ())?;
                let id = object.as_reference().ok();
                let object = if let Some(id) = id {
                    doc.get_object(id).map_err(|_| ())?
                } else {
                    object
                };
                let stream = object.as_stream().map_err(|_| ())?;
                match stream
                    .dict
                    .get(b"Subtype")
                    .and_then(lopdf::Object::as_name)
                    .unwrap_or(b"")
                {
                    b"Image" => area += scale,
                    b"Form" => {
                        if id.is_some_and(|id| !visited.insert(id)) {
                            return Err(());
                        }
                        let form_scale = stream
                            .dict
                            .get(b"Matrix")
                            .ok()
                            .and_then(|o| o.as_array().ok())
                            .and_then(|a| {
                                if a.len() != 6 {
                                    return None;
                                }
                                let n = a
                                    .iter()
                                    .map(|o| o.as_float().map(f64::from))
                                    .collect::<Result<Vec<_>, _>>()
                                    .ok()?;
                                Some((n[0] * n[3] - n[1] * n[2]).abs())
                            })
                            .unwrap_or(1.0);
                        let bytes = stream.get_plain_content().map_err(|_| ())?;
                        area += images(
                            doc,
                            &bytes,
                            stream
                                .dict
                                .get(b"Resources")
                                .ok()
                                .and_then(|o| dictionary(doc, o))
                                .or(res),
                            scale * form_scale,
                            depth + 1,
                            visited,
                            calls,
                        )?;
                        if let Some(id) = id {
                            visited.remove(&id);
                        }
                    }
                    _ => (),
                }
            }
            _ => (),
        }
    }
    Ok(area)
}

pub(super) fn extract(bytes: &[u8]) -> Report {
    let fail = || {
        Report::failure(
            Format::Pdf,
            Status::Unreadable,
            "This PDF could not be read. It may be damaged or use unsupported content. The original is unchanged.",
        )
    };
    let Ok(doc) = lopdf::Document::load_mem(bytes) else {
        return fail();
    };
    if doc.is_encrypted() {
        return Report::failure(
            Format::Pdf,
            Status::PasswordProtected,
            "Open a local copy without password protection. ComplyPH does not request or store passwords.",
        );
    }
    let pages = doc.get_pages();
    if pages.is_empty() {
        return fail();
    }
    if pages.len() > MAX_PAGES {
        return Report::failure(
            Format::Pdf,
            Status::LimitExceeded,
            "This reader supports PDFs up to 300 pages.",
        );
    }
    let mut report=Report{engine:ENGINE.into(),format:Format::Pdf,status:Status::Ready,
        summary:"PDF text read locally from character positions. Page numbers refer to the original PDF.".into(),units:vec![],warnings:vec![PDF_LAYOUT_WARNING.into()],ocr:None,
        reading:Some(ReadingInfo{profile:"spatial-pdf-1".into(),details:vec![]})};
    report.warnings.extend(crate::pdf_content_cautions(&doc));
    let mut empty = vec![];
    let mut used = 0;
    for (number, id) in pages {
        let Ok(content) = doc.get_page_content(id) else {
            return fail();
        };
        let coverage = images(
            &doc,
            &content,
            resources(&doc, id),
            1.0,
            0,
            &mut HashSet::new(),
            &mut 0,
        );
        if coverage.is_err() {
            return Report::failure(
                Format::Pdf,
                Status::Unreadable,
                "PDF content nesting, images or drawing operations could not be bounded safely. Use a simpler local PDF copy or manual review.",
            );
        }
        let mut page = Page::default();
        let result = pdf_extract::output_doc_page(&doc, &mut page, number);
        if page.exceeded {
            return Report::failure(
                Format::Pdf,
                Status::LimitExceeded,
                "This PDF exceeds the per-page 100,000-character or 4 MiB reading limit.",
            );
        }
        if result.is_err() {
            return fail();
        }
        let (text, detail, ambiguous) = page.text();
        used += text.len();
        if used > MAX_TEXT_BYTES {
            return Report::failure(
                Format::Pdf,
                Status::LimitExceeded,
                "The extracted text exceeds the 4 MiB reading limit.",
            );
        }
        if text.is_empty() {
            empty.push(number.to_string());
        }
        let location = Location::PdfPage { page: number };
        report
            .reading
            .as_mut()
            .unwrap()
            .details
            .push(ReadingDetail {
                location: location.clone(),
                message: detail,
            });
        if ambiguous {
            report.warnings.push(format!("Page {number}: table or column relationships need manual review. Positioned text does not establish which value applies to which clause."));
        }
        if page.bad_mapping {
            report.warnings.push(format!("Page {number}: missing, overlapping or undecodable font characters. Do not trust numeric terms from this reading; try local OCR or manual review."));
        }
        if page.non_horizontal {
            report.warnings.push(format!("Page {number}: rotated, mirrored or non-horizontal selectable text is outside the supported spatial layout. Use local OCR or manual review."));
        }
        if coverage.unwrap_or(0.0) > page.width * page.height * 0.05 {
            report.warnings.push(format!("Page {number}: substantial image content was not read, even though selectable text may be present. Run local OCR on every page or review the full original manually."));
        }
        report.units.push(TextUnit { location, text });
    }
    if empty.len() == report.units.len() {
        report.status = Status::NeedsOcr;
        report.summary="No selectable text was found. This may be a scanned or image-only PDF. Choose Run local OCR to recognize its pages offline.".into();
    } else if !empty.is_empty() {
        report.warnings.push(format!(
            "No text on page(s) {}. These may be blank or need OCR; they have not been read.",
            empty.join(", ")
        ));
    }
    report
}
