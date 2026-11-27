//! Offline navigation into the report's exact preserved source version.
use crate::{text_navigation, theme::*, widgets};
use complyph_core::SavedReview;
use complyph_library::LibraryCatalog;
use eframe::egui::{self, RichText};
use std::ops::Range;

struct Section {
    title: String,
    range: Range<usize>,
}
struct OpenSource {
    title: String,
    provenance: String,
    note: String,
    url: String,
    hash: String,
    text: String,
    quote: String,
    sections: Vec<Section>,
    section: usize,
    full: bool,
    chunk: usize,
    scroll: bool,
}
#[derive(Default)]
pub struct ProvisionView {
    open: bool,
    source: Option<OpenSource>,
    error: Option<String>,
}

fn article_range(text: &str, article: u16) -> Result<Range<usize>, String> {
    let marker = format!("\nArticle {article}.\n");
    let matches: Vec<_> = text.match_indices(&marker).collect();
    let [(start, _)] = matches.as_slice() else {
        return Err(format!(
            "Article {article} is unavailable or ambiguous in this saved source."
        ));
    };
    let start = start + 1;
    let end = text[start..]
        .find(&format!("\nArticle {}.\n", article + 1))
        .map(|end| start + end)
        .ok_or_else(|| "The exact provision boundary could not be established.".to_string())?;
    Ok(start..end)
}
fn sections(text: &str, rule: &str, quote: &str) -> Result<Vec<Section>, String> {
    let numbers: &[u16] = match rule {
        "PH-LAB-083-01" => &[83, 82, 84, 85],
        "PH-LAB-087-01" => &[87, 82, 90],
        "PH-LAB-086-01" => &[86, 82],
        "PH-LAB-085-01" => &[85, 82, 84],
        "PH-LAB-091-01" => &[91, 82, 92],
        "PH-LAB-095-01" => &[95, 82],
        _ => return Err("This saved rule has no supported offline provision mapping.".into()),
    };
    let mut sections: Vec<_> = numbers
        .iter()
        .map(|&n| {
            Ok(Section {
                title: format!("Article {n}"),
                range: article_range(text, n)?,
            })
        })
        .collect::<Result<_, String>>()?;
    if !text[sections[0].range.clone()].contains(quote) || quote.trim().is_empty() {
        return Err("The report's legal quote does not match its mapped saved provision. No substitute was opened.".into());
    }
    let related_rule = match rule {
        "PH-LAB-085-01" => Some((
            "\nRule 1\nHOURS OF WORK\n",
            "\nRule I-A\n",
            "Book III · Rule I",
        )),
        "PH-LAB-086-01" => Some((
            "\nRule II\nNIGHT SHIFT DIFFERENTIAL\n",
            "\nRule III\n",
            "Book III · Rule II",
        )),
        "PH-LAB-091-01" => Some((
            "\nRule III\nWEEKLY REST PERIODS\n",
            "\nRule IV\n",
            "Book III · Rule III",
        )),
        "PH-LAB-095-01" => Some((
            "\nRule V\nSERVICE INCENTIVE LEAVE\n",
            "\nRule VI\n",
            "Book III · Rule V",
        )),
        _ => None,
    };
    if let Some((marker, next, title)) = related_rule {
        let matches: Vec<_> = text.match_indices(marker).collect();
        let [(start, _)] = matches.as_slice() else {
            return Err("The supporting implementing rule is unavailable or ambiguous.".into());
        };
        let end = text[start + 1..]
            .find(next)
            .map(|end| start + 1 + end)
            .ok_or_else(|| {
                "The implementing-rule boundary could not be established.".to_string()
            })?;
        sections.push(Section {
            title: title.into(),
            range: start + 1..end,
        });
    }
    Ok(sections)
}
pub(crate) fn dependency_boundaries(
    text: &str,
    rule: &str,
    quote: &str,
) -> Result<Vec<complyph_library::ProvisionBoundary>, String> {
    use sha2::{Digest, Sha256};
    sections(text, rule, quote)?
        .into_iter()
        .map(|section| {
            let body = &text[section.range.clone()];
            let end: String = text[section.range.end..].chars().take(96).collect();
            if body.is_empty() || end.is_empty() {
                return Err("The complete supporting boundary is missing.".into());
            }
            Ok(complyph_library::ProvisionBoundary {
                title: section.title,
                start: body.chars().take(96).collect(),
                end,
                text_sha256: format!("{:x}", Sha256::digest(body.as_bytes())),
            })
        })
        .collect()
}
impl ProvisionView {
    pub fn open(&mut self, catalog: &LibraryCatalog, saved: &SavedReview, index: usize) {
        self.open = true;
        self.source = None;
        self.error = None;
        let result = (|| {
            let finding = saved
                .review
                .findings
                .get(index)
                .ok_or("This saved finding is unavailable.")?;
            let (key, raw_hash, text_hash) = if let Some(source) = &finding.source {
                (&source.key, &source.sha256, &source.text_sha256)
            } else {
                (
                    &saved.review.source_key,
                    &saved.review.source_sha256,
                    &saved.review.source_text_sha256,
                )
            };
            let snapshot = catalog
                .snapshot_for_review(key, raw_hash, text_hash)
                .map_err(|e| e.to_string())?;
            let sections = if let Some(locator) = &finding.provision {
                let markers: Vec<_> = snapshot.text.match_indices(&locator.start).collect();
                let [(start, _)] = markers.as_slice() else {
                    return Err("The saved provision boundary is unavailable or ambiguous; no substitute was opened.".into());
                };
                let end = snapshot.text[start + locator.start.len()..]
                    .find(&locator.end)
                    .map(|n| start + locator.start.len() + n)
                    .ok_or("The saved provision ending is unavailable.")?;
                if finding.legal_quote.trim().is_empty()
                    || !snapshot.text[*start..end].contains(&finding.legal_quote)
                {
                    return Err(
                        "The saved quote does not match the exact preserved section.".into(),
                    );
                }
                vec![Section {
                    title: locator.title.clone(),
                    range: *start..end,
                }]
            } else {
                sections(&snapshot.text, &finding.rule_id, &finding.legal_quote)?
            };
            Ok(OpenSource {
                title: snapshot.metadata.title,
                provenance: snapshot.metadata.provenance.label().into(),
                note: snapshot.metadata.applicability_note,
                url: snapshot.metadata.source_url,
                hash: snapshot.metadata.raw_sha256,
                text: snapshot.text,
                quote: finding.legal_quote.clone(),
                sections,
                section: 0,
                full: false,
                chunk: 0,
                scroll: true,
            })
        })();
        match result {
            Ok(source) => self.source = Some(source),
            Err(error) => self.error = Some(error),
        }
    }
    pub fn show(&mut self, ctx: &egui::Context) {
        if !self.open {
            return;
        }
        egui::Window::new("Offline legal library").id(egui::Id::unique("supporting-provision")).open(&mut self.open)
            .default_size([820.0,620.0]).min_width(520.0).resizable(true).collapsible(false).show(ctx,|ui| {
                if let Some(error)=&self.error {ui.colored_label(RED,error);widgets::muted(ui,"The report is unchanged. Its recorded citation and excerpt remain available; no online request or latest-version substitution was made.");return;}
                let Some(source)=&mut self.source else{return;};
                ui.label(RichText::new(&source.title).font(heading(20.0)));
                widgets::pill(ui,"Saved source · offline",GREEN,GREEN_TINT,false);
                widgets::muted(ui,&source.provenance);
                ui.horizontal_wrapped(|ui| {
                    for (index,section) in source.sections.iter().enumerate() {
                        if ui.selectable_label(!source.full && source.section==index,&section.title).clicked(){source.section=index;source.full=false;source.scroll=true;}
                    }
                    if ui.selectable_label(source.full,"Full saved source").clicked(){
                        source.full=true;
                        let bounds=text_navigation::chunk_boundaries(&source.text);
                        let position=text_navigation::matching_ranges(&source.text[source.sections[source.section].range.clone()],&source.quote).first().map(|r|source.sections[source.section].range.start+r.start).unwrap_or(source.sections[source.section].range.start);
                        source.chunk=text_navigation::chunk_at(&bounds,position);source.scroll=true;
                    }
                });
                ui.add_space(6.0);
                let range=if source.full {
                    let bounds=text_navigation::chunk_boundaries(&source.text);let count=bounds.len()-1;
                    ui.horizontal_wrapped(|ui| {
                        if ui.add_enabled(source.chunk>0,egui::Button::new("Previous section")).clicked(){source.chunk-=1;source.scroll=true;}
                        if ui.add_enabled(source.chunk+1<count,egui::Button::new("Next section")).clicked(){source.chunk+=1;source.scroll=true;}
                        ui.label(format!("Reading section {} / {count}",source.chunk+1));
                    });
                    widgets::muted(ui,"Reading sections are display chunks, not original page numbers.");
                    bounds[source.chunk]..bounds[source.chunk+1]
                }else{source.sections[source.section].range.clone()};
                let text=&source.text[range.clone()];
                let matches=text_navigation::clip_ranges(&text_navigation::matching_ranges(&source.text,&source.quote),range.clone());
                let scroll=source.scroll.then(||matches.first().map(|r|r.start).unwrap_or(0));source.scroll=false;
                egui::ScrollArea::vertical().id_salt(("legal-provision",source.full,source.section,source.chunk)).max_height((ui.available_height()-90.0).max(220.0)).min_scrolled_height(180.0).auto_shrink([false,false]).show(ui,|ui| {
                    text_navigation::highlighted_label(ui,text,&matches,scroll);
                    ui.add_space(10.0);
                    widgets::muted(ui,&source.note);
                });
                ui.collapsing("Preserved source details",|ui| {
                    widgets::muted(ui,"This is the source version recorded by the saved review. A checksum establishes preserved integrity, not legal currency.");
                    ui.add(egui::Label::new(&source.url).wrap().selectable(true));
                    ui.add(egui::Label::new(format!("SHA-256: {}",source.hash)).wrap().selectable(true));
                });
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_initial_finding_opens_and_renders_its_own_exact_preserved_source() {
        let temp = tempfile::tempdir().unwrap();
        let catalog = LibraryCatalog::open(temp.path()).unwrap();
        let input = complyph_extract::Report {
            reading: None,
            engine: complyph_extract::ENGINE.into(),
            ocr: None,
            format: complyph_extract::Format::Docx,
            status: complyph_extract::Status::Ready,
            summary: "Synthetic".into(),
            warnings: vec![],
            units: vec![],
        };
        let review = complyph_checks::review(&input, "synthetic", Default::default());
        let mut saved = SavedReview {
            id: 1,
            document_id: 1,
            created_at: 0,
            review,
        };
        let ctx = egui::Context::default();
        for index in 6..saved.review.findings.len() {
            let finding = &saved.review.findings[index];
            let mut view = ProvisionView::default();
            view.open(&catalog, &saved, index);
            assert!(
                view.error.is_none(),
                "{}: {:?}",
                finding.rule_id,
                view.error
            );
            let source = view.source.as_ref().unwrap();
            assert_eq!(source.hash, finding.source.as_ref().unwrap().sha256);
            assert!(source.text[source.sections[0].range.clone()].contains(&finding.legal_quote));
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| view.show(ui.ctx()));
            output.textures_delta.clear();
        }
        saved.review.findings[7].legal_quote = "tampered quote".into();
        let mut view = ProvisionView::default();
        view.open(&catalog, &saved, 7);
        assert!(view.error.is_some() && view.source.is_none());
    }
    #[test]
    fn exact_articles_and_related_rules_preserve_full_provisions_and_boundaries() {
        let text = include_str!("../../../library/starter/text/labor-code-reference.txt");
        for (rule, quote, article, count) in [
            (
                "PH-LAB-083-01",
                "The normal hours of work of any employee shall not exceed eight in a day.",
                83,
                4,
            ),
            (
                "PH-LAB-087-01",
                "Work may be performed beyond eight hours a day",
                87,
                3,
            ),
            (
                "PH-LAB-086-01",
                "Every employee shall be paid a night shift differential",
                86,
                3,
            ),
            (
                "PH-LAB-085-01",
                "not less than sixty minutes time-off for their regular meals",
                85,
                4,
            ),
            (
                "PH-LAB-091-01",
                "twenty-four consecutive hours after every six consecutive normal work days",
                91,
                4,
            ),
            (
                "PH-LAB-095-01",
                "yearly service incentive leave of five days with pay",
                95,
                3,
            ),
        ] {
            let found = sections(text, rule, quote).unwrap();
            assert_eq!(found.len(), count);
            let selected = &text[found[0].range.clone()];
            assert!(selected.starts_with(&format!("Article {article}.\n")));
            assert!(selected.contains(quote));
            assert!(!selected.contains(&format!("Article {}.", article + 1)));
            if article == 83 {
                assert!(selected.contains("Health personnel"));
            }
            if article == 87 {
                assert!(selected.contains("holiday or rest day"));
            }
            if article == 86 {
                let irr = &text[found[2].range.clone()];
                assert!(irr.contains("not more than five (5) workers"));
                assert!(irr.contains("Relation to agreements"));
                assert!(!irr.contains("WEEKLY REST PERIODS"));
            }
            if article == 85 {
                let irr = &text[found[3].range.clone()];
                assert!(irr.contains("shorter meal period is credited as compensable hours"));
                assert!(irr.contains("twenty (20) minutes"));
                assert!(!irr.contains("HOURS OF WORK OF HOSPITAL"));
            }
            if article == 91 {
                assert!(text[found[2].range.clone()].contains("emergencies"));
                let irr = &text[found[3].range.clone()];
                assert!(irr.contains("When work on rest day authorized"));
                assert!(!irr.contains("HOLIDAYS WITH PAY"));
            }
            if article == 95 {
                assert!(selected.contains("regularly employing less than ten employees"));
                let irr = &text[found[2].range.clone()];
                for expected in [
                    "whether continuous or broken",
                    "authorized absences and paid regular holidays",
                    "vacation leave with pay of at least five days",
                    "commutable to its money equivalent",
                    "Relation to agreements",
                ] {
                    assert!(irr.contains(expected));
                }
                assert!(!irr.contains("SERVICE CHARGES"));
            }
        }
        assert!(sections(text, "PH-LAB-083-01", "incorrect quote").is_err());
        assert!(sections(text, "unknown-rule", "quote").is_err());
        assert!(article_range("Article 83. unavailable", 83).is_err());
    }
    #[test]
    fn every_related_provision_and_full_source_chunk_renders_without_substituting_a_quote() {
        let text = include_str!("../../../library/starter/text/labor-code-reference.txt");
        let quote = "Every employee shall be paid a night shift differential";
        let ctx = egui::Context::default();
        let mut view = ProvisionView {
            open: true,
            source: Some(OpenSource {
                title: "Synthetic UI test using public legal text".into(),
                provenance: "Agency snapshot".into(),
                note: "Test".into(),
                url: "https://example.invalid".into(),
                hash: "synthetic".into(),
                text: text.into(),
                quote: quote.into(),
                sections: sections(text, "PH-LAB-086-01", quote).unwrap(),
                section: 0,
                full: false,
                chunk: 0,
                scroll: true,
            }),
            error: None,
        };
        let count = view.source.as_ref().unwrap().sections.len();
        for section in 0..count {
            view.source.as_mut().unwrap().section = section;
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| view.show(ui.ctx()));
            output.textures_delta.clear();
        }
        for (rule, quote) in [
            (
                "PH-LAB-085-01",
                "not less than sixty minutes time-off for their regular meals",
            ),
            (
                "PH-LAB-091-01",
                "twenty-four consecutive hours after every six consecutive normal work days",
            ),
            (
                "PH-LAB-095-01",
                "yearly service incentive leave of five days with pay",
            ),
        ] {
            let source = view.source.as_mut().unwrap();
            source.sections = sections(text, rule, quote).unwrap();
            source.quote = quote.into();
            for section in 0..view.source.as_ref().unwrap().sections.len() {
                view.source.as_mut().unwrap().section = section;
                let mut output = ctx.run_ui(egui::RawInput::default(), |ui| view.show(ui.ctx()));
                output.textures_delta.clear();
            }
        }
        view.source.as_mut().unwrap().full = true;
        let count = text_navigation::chunk_boundaries(text).len() - 1;
        for chunk in 0..count {
            view.source.as_mut().unwrap().chunk = chunk;
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| view.show(ui.ctx()));
            output.textures_delta.clear();
        }
    }
}
