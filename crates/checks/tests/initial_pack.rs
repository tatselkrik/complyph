use complyph_checks::{
    Applicability::{Covered, OutsideScope, Unknown},
    Context, Outcome, RuleScope,
};
use complyph_extract::{Format, Location, Report, Status, TextUnit};
fn report(text: &str) -> Report {
    Report {
        reading: None,
        engine: complyph_extract::ENGINE.into(),
        ocr: None,
        format: Format::Docx,
        status: Status::Ready,
        summary: "Synthetic policy".into(),
        warnings: vec![],
        units: vec![TextUnit {
            location: Location::WordParagraph {
                part: "word/document.xml".into(),
                paragraph: 3,
                table_cell: None,
            },
            text: text.into(),
        }],
    }
}
fn scope() -> RuleScope {
    RuleScope{coverage:Covered,exceptions_reviewed:true,review_on:"2026-11-01".into(),basis:"Fictional worker/period: standard category, applicability, amendments, exceptions and source-specific base independently reviewed.".into()}
}
fn ctx(id: &str) -> Context {
    let mut c = Context {
        operative_terms_confirmed: true,
        ..Default::default()
    };
    c.additional_checks.insert(id.into(), scope());
    c
}
fn outcome(id: &str, text: &str, c: Context) -> Outcome {
    complyph_checks::review(&report(text), "fictional", c)
        .findings
        .into_iter()
        .find(|f| f.rule_id == id)
        .unwrap()
        .outcome
}
fn cases() -> serde_json::Value {
    serde_json::from_str(include_str!("../../../examples/expected-initial.json")).unwrap()
}
#[test]
fn independent_boundaries_and_exact_rates_do_not_round_or_infer_a_base() {
    for case in cases()["boundaries"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        let fmt = case["statement"].as_str().unwrap();
        let value = case["threshold"].as_i64().unwrap();
        let kind = case["comparison"].as_str().unwrap();
        for delta in [-1, 0, 1] {
            if value + delta < 0 {
                continue;
            }
            let text = fmt.replace("{}", &(value + delta).to_string());
            let issue = match kind {
                "maximum" => delta > 0,
                "minimum" => delta < 0,
                _ => delta != 0,
            };
            assert_eq!(
                outcome(id, &text, ctx(id)),
                if issue {
                    Outcome::PotentialIssue
                } else {
                    Outcome::StatedTermMeetsMinimum
                },
                "{id}: {text}"
            );
        }
        if id != "PH-LAB-296-01" {
            for delta in [-1i64, 1] {
                let scaled = value * 100 + delta;
                if scaled < 0 {
                    continue;
                }
                let text = fmt.replace("{}", &format!("{}.{:02}", scaled / 100, scaled % 100));
                let issue = match kind {
                    "maximum" => delta > 0,
                    "minimum" => delta < 0,
                    _ => true,
                };
                assert_eq!(
                    outcome(id, &text, ctx(id)),
                    if issue {
                        Outcome::PotentialIssue
                    } else {
                        Outcome::StatedTermMeetsMinimum
                    },
                    "{id}: {text}"
                );
            }
        }
    }
    assert_eq!(
        outcome(
            "PH-BIR-024-01",
            "Annual taxable compensation income zero-rate ceiling is PHP 250,000.00.",
            ctx("PH-BIR-024-01")
        ),
        Outcome::StatedTermMeetsMinimum
    );
    assert_eq!(
        outcome(
            "PH-LAB-296-01",
            "Probationary employment period is 6.01 months.",
            ctx("PH-LAB-296-01")
        ),
        Outcome::NeedsReview
    );
    assert_eq!(
        outcome(
            "PH-BIR-024-01",
            "Annual taxable compensation income zero-rate ceiling is PHP 9999999999.",
            ctx("PH-BIR-024-01")
        ),
        Outcome::NeedsReview
    );
}
#[test]
fn every_new_check_requires_facts_period_and_basis_and_preserves_explicit_exclusions() {
    for case in cases()["boundaries"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        let text = case["statement"]
            .as_str()
            .unwrap()
            .replace("{}", &case["threshold"].to_string());
        for missing in 0..7 {
            let mut c = ctx(id);
            let s = c.additional_checks.get_mut(id).unwrap();
            match missing {
                0 => s.coverage = Unknown,
                1 => s.exceptions_reviewed = false,
                2 => s.review_on.clear(),
                3 => s.review_on = "2026-02-30".into(),
                4 => s.basis.clear(),
                5 => s.basis = "x".repeat(4001),
                _ => s.basis = "bad\0control".into(),
            };
            assert_eq!(
                outcome(id, &text, c),
                Outcome::NeedsContext,
                "{id}, missing {missing}"
            );
        }
        let mut c = ctx(id);
        c.additional_checks.get_mut(id).unwrap().coverage = OutsideScope;
        assert_eq!(outcome(id, &text, c), Outcome::OutsideScope);
        for date in ["1900-01-01", "2027-01-01"] {
            let mut c = ctx(id);
            c.additional_checks.get_mut(id).unwrap().review_on = date.into();
            assert_eq!(outcome(id, &text, c), Outcome::NeedsReview, "{id} {date}");
        }
        let mut c = ctx(id);
        c.operative_terms_confirmed = false;
        assert_eq!(outcome(id, &text, c), Outcome::NeedsContext);
    }
    for (date, expected) in [
        ("2024-02-29", true),
        ("2025-02-29", false),
        ("2026-04-31", false),
        ("2026-11-01", true),
        ("2026-1-01", false),
        ("2026-11-00", false),
        ("2026-13-01", false),
    ] {
        assert_eq!(complyph_checks::valid_date(date), expected, "{date}");
    }
}
#[test]
fn quotes_conditions_conflicts_incomplete_reading_and_missing_terms_stay_unresolved() {
    for case in cases()["boundaries"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        let fmt = case["statement"].as_str().unwrap();
        let text = fmt.replace("{}", &case["threshold"].to_string());
        for modified in [
            format!("Example: {text}"),
            format!("\"{}\".", text.trim_end_matches('.')),
            format!("{}; unless approved.", text.trim_end_matches('.')),
            format!("{text} Subject to approval."),
            format!(
                "{text} {}",
                fmt.replace("{}", &(case["threshold"].as_i64().unwrap() + 1).to_string())
            ),
        ] {
            assert_eq!(
                outcome(id, &modified, ctx(id)),
                Outcome::NeedsReview,
                "{id}: {modified}"
            );
        }
        let mut input = report(&text);
        input.warnings.push("Unread page".into());
        let r = complyph_checks::review(&input, "fictional", ctx(id));
        assert_eq!(
            r.findings.iter().find(|f| f.rule_id == id).unwrap().outcome,
            Outcome::NeedsReview
        );
        assert_eq!(
            outcome(id, "Fictional introduction only.", ctx(id)),
            Outcome::NeedsEvidence
        );
    }
    let id = "PH-NPC-038-01";
    let mut input = report(
        "Required breach notifications to the NPC and affected data subjects are sent within 72 hours of knowledge or reasonable belief.",
    );
    input.units.push(TextUnit {
        location: Location::WordParagraph {
            part: "word/document.xml".into(),
            paragraph: 4,
            table_cell: None,
        },
        text: "Unless management authorizes a delay.".into(),
    });
    let r = complyph_checks::review(&input, "fictional", ctx(id));
    let f = r.findings.iter().find(|f| f.rule_id == id).unwrap();
    assert_eq!(f.outcome, Outcome::NeedsReview);
    assert_eq!(f.evidence.len(), 2);
    assert!(f.evidence[1].text.contains("Unless"));
}
#[test]
fn source_pins_quotes_and_complete_offline_sections_are_anchored_to_seven_agency_sources() {
    let temp = tempfile::tempdir().unwrap();
    let catalog = complyph_library::LibraryCatalog::open(temp.path()).unwrap();
    assert_eq!(complyph_checks::source_pins().len(), 7);
    assert_eq!(complyph_checks::initial_rules().len(), 14);
    assert_eq!(complyph_checks::RULE_COUNT, 20);
    for rule in complyph_checks::initial_rules() {
        let saved = catalog.snapshot_by_key(&rule.source.key).unwrap();
        assert_eq!(
            saved.metadata.provenance,
            complyph_library::Provenance::AgencyDownload
        );
        assert!(rule.source.matches(&saved.raw, &saved.text));
        assert!(!rule.source.matches(b"changed", &saved.text));
        assert!(!rule.source.matches(&saved.raw, "changed"));
        let start = saved.text.find(&rule.provision.start).unwrap();
        let end = saved.text[start + rule.provision.start.len()..]
            .find(&rule.provision.end)
            .unwrap()
            + start
            + rule.provision.start.len();
        assert!(saved.text[start..end].contains(&rule.quote), "{}", rule.id);
    }
}

#[test]
fn every_initial_rule_requires_separate_ocr_confirmation_and_supported_provenance() {
    for case in cases()["boundaries"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        let text = case["statement"]
            .as_str()
            .unwrap()
            .replace("{}", &case["threshold"].to_string());
        let mut input = report(&text);
        input.engine = complyph_extract::OCR_ENGINE.into();
        input.format = Format::Pdf;
        input.units[0].location = Location::PdfPage { page: 1 };
        input.warnings = vec![complyph_extract::OCR_WARNING.into()];
        input.ocr = Some(complyph_extract::OcrInfo {
            processing: Some(Default::default()),
            engine_version: "synthetic-validation".into(),
            renderer: "pdfium-8165".into(),
            language: "eng".into(),
            model_sha256: complyph_extract::OCR_MODEL_SHA256.into(),
            confidence_threshold: 8000,
            pages: vec![complyph_extract::OcrPage {
                processing: Some(Default::default()),
                page: 1,
                words: 20,
                uncertain: vec![],
            }],
        });
        let result = |input: &Report, c: Context| {
            complyph_checks::review(input, "fictional", c)
                .findings
                .into_iter()
                .find(|f| f.rule_id == id)
                .unwrap()
                .outcome
        };
        assert_eq!(result(&input, ctx(id)), Outcome::NeedsReview);
        let mut c = ctx(id);
        c.ocr_text_confirmed = true;
        assert_eq!(result(&input, c.clone()), Outcome::StatedTermMeetsMinimum);
        input.ocr.as_mut().unwrap().model_sha256 = "modified model".into();
        assert_eq!(result(&input, c), Outcome::NeedsReview);
    }
}
#[test]
fn mutually_exclusive_pagibig_bands_and_old_six_rule_contexts_never_gain_eligibility() {
    let mut c = ctx("PH-HDMF-EE-HIGH-01");
    c.additional_checks
        .insert("PH-HDMF-EE-LOW-01".into(), scope());
    for id in ["PH-HDMF-EE-HIGH-01", "PH-HDMF-EE-LOW-01"] {
        assert_eq!(
            outcome(
                id,
                "Pag-IBIG employee mandatory savings rate is 2% of applicable fund salary.",
                c.clone()
            ),
            Outcome::NeedsContext
        );
    }
    let old = serde_json::json!({"operative_terms_confirmed":true,"normal_hours":"covered","ordinary_overtime":"covered","night_differential":"covered"});
    let c: Context = serde_json::from_value(old.clone()).unwrap();
    assert!(c.additional_checks.is_empty());
    let mut r = serde_json::to_value(complyph_checks::review(
        &report("Fictional policy."),
        "old",
        c,
    ))
    .unwrap();
    r.as_object_mut().unwrap().remove("rule_pack_sha256");
    r["context"] = old;
    r["engine"] = "complyph-labor-4".into();
    r["findings"].as_array_mut().unwrap().truncate(6);
    let old: complyph_checks::Review = serde_json::from_value(r).unwrap();
    assert_eq!(old.findings.len(), 6);
    assert!(old.rule_pack_sha256.is_none());
    assert!(
        old.findings
            .iter()
            .all(|f| f.source.is_none() && f.provision.is_none())
    );
}
#[test]
fn real_pdf_word_baselines_issues_and_ambiguity_follow_independent_expected_outcomes() {
    let value = cases();
    for (name, case) in value["cases"].as_object().unwrap() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples")
            .join(name);
        let bytes = std::fs::read(path).unwrap();
        let format = if name.ends_with("pdf") {
            Format::Pdf
        } else {
            Format::Docx
        };
        let input = complyph_extract::extract(format, &bytes);
        assert_eq!(input.status, Status::Ready, "{name}: {:?}", input.warnings);
        let c = serde_json::from_value(case["context"].clone()).unwrap();
        let r = complyph_checks::review(&input, "fictional", c);
        let actual =
            serde_json::to_value(r.findings.iter().map(|f| f.outcome).collect::<Vec<_>>()).unwrap();
        assert_eq!(actual, case["outcomes"], "{name}");
    }
}
