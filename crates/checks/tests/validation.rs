use complyph_checks::{
    Applicability::{self, Covered},
    Context,
    Outcome::*,
    *,
};
use complyph_extract::{Format, Location, Report, Status, TextUnit};
fn context() -> Context {
    Context {
        operative_terms_confirmed: true,
        ocr_text_confirmed: false,
        normal_hours: Covered,
        ordinary_overtime: Covered,
        night_differential: Covered,
        meal_period: Covered,
        weekly_rest: Covered,
        service_incentive_leave: Default::default(),
    }
}
fn report(text: &str) -> Report {
    Report {
        engine: complyph_extract::ENGINE.into(),
        ocr: None,
        format: Format::Docx,
        status: Status::Ready,
        summary: "synthetic".into(),
        units: vec![TextUnit {
            location: Location::WordParagraph {
                part: "word/document.xml".into(),
                paragraph: 1,
                table_cell: None,
            },
            text: text.into(),
        }],
        warnings: vec![],
    }
}
fn clauses(hours: &str, ot: &str, night: &str) -> String {
    format!(
        "Normal daily working hours are {hours} hours.\nOrdinary-day overtime premium is {ot}% of the regular hourly wage.\nNight shift differential is {night}% of the regular hourly wage for hours worked between 22:00 and 06:00.\nRegular meal period is 60 minutes of time-off.\nWeekly rest period is 24 consecutive hours after every six consecutive normal work days."
    )
}

#[test]
fn ocr_numbers_require_explicit_confirmation_and_valid_reading_provenance() {
    let mut input = report(&clauses("9", "20", "5"));
    input.format = Format::Pdf;
    input.engine = complyph_extract::OCR_ENGINE.into();
    input.units[0].location = Location::PdfPage { page: 1 };
    input.warnings = vec![complyph_extract::OCR_WARNING.into()];
    input.ocr = Some(complyph_extract::OcrInfo {
        engine_version: "synthetic-validation".into(),
        renderer: "pdfium-8165".into(),
        language: "eng".into(),
        model_sha256: complyph_extract::OCR_MODEL_SHA256.into(),
        confidence_threshold: 8000,
        pages: vec![complyph_extract::OcrPage {
            page: 1,
            words: 30,
            uncertain: vec![],
        }],
    });
    assert!(
        review(&input, "synthetic", context()).findings[..5]
            .iter()
            .all(|f| f.outcome == NeedsReview)
    );
    let mut covered = context();
    covered.ocr_text_confirmed = true;
    assert!(
        review(&input, "synthetic", covered.clone()).findings[..3]
            .iter()
            .all(|f| f.outcome == PotentialIssue)
    );
    input.ocr.as_mut().unwrap().model_sha256 = "changed".into();
    assert!(
        review(&input, "synthetic", covered).findings[..5]
            .iter()
            .all(|f| f.outcome == NeedsReview)
    );
}
#[test]
fn independent_article_boundaries_exact_decimal_and_favourable_terms() {
    // Expected thresholds independently transcribed from Articles 83, 87 and 86.
    for (hours, ot, night, expected) in [
        ("8", "25", "10", [StatedTermMeetsMinimum; 3]),
        ("7.99", "25.01", "10.01", [StatedTermMeetsMinimum; 3]),
        ("8.01", "24.99", "9.99", [PotentialIssue; 3]),
        ("9", "20", "5", [PotentialIssue; 3]),
        ("7.5", "30", "15", [StatedTermMeetsMinimum; 3]),
    ] {
        let result = review(
            &report(&clauses(hours, ot, night)),
            "synthetic-sha",
            context(),
        );
        assert_eq!(
            result
                .findings
                .iter()
                .take(3)
                .map(|f| f.outcome)
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(result.findings.len(), RULE_COUNT);
        assert!(
            result.findings[3..5]
                .iter()
                .all(|f| f.outcome == StatedTermMeetsMinimum)
        );
        assert!(result.findings[..5].iter().all(|f| f.evidence.len() == 1));
        assert_eq!(result.source_sha256, SOURCE_SHA256);
        let round_trip: Review =
            serde_json::from_str(&serde_json::to_string(&result).unwrap()).unwrap();
        assert_eq!(round_trip.document_sha256, "synthetic-sha");
    }
}
#[test]
fn coverage_and_reading_integrity_are_required() {
    let input = report(&clauses("8", "25", "10"));
    assert!(
        review(&input, "sha", Context::default())
            .findings
            .iter()
            .all(|f| f.outcome == NeedsContext)
    );
    let outside = Context {
        operative_terms_confirmed: true,
        ocr_text_confirmed: false,
        normal_hours: Applicability::OutsideScope,
        ordinary_overtime: Applicability::OutsideScope,
        night_differential: Applicability::OutsideScope,
        meal_period: Applicability::OutsideScope,
        weekly_rest: Applicability::OutsideScope,
        service_incentive_leave: SilScope {
            employee_coverage: Applicability::OutsideScope,
            ..Default::default()
        },
    };
    assert!(
        review(&input, "sha", outside)
            .findings
            .iter()
            .all(|f| f.outcome == complyph_checks::Outcome::OutsideScope)
    );
    let mut partial = input.clone();
    partial.warnings.push("Unread image on page two".into());
    assert!(
        review(&partial, "sha", context()).findings[..5]
            .iter()
            .all(|f| f.outcome == NeedsReview)
    );
    partial.warnings.clear();
    partial.status = Status::NeedsOcr;
    assert!(
        review(&partial, "sha", context()).findings[..5]
            .iter()
            .all(|f| f.outcome == NeedsReview)
    );
}
#[test]
fn ambiguity_conditions_quotations_contradictions_and_missing_terms_never_pass() {
    for text in [
        "Ordinary-day overtime premium is not 25% of the regular hourly wage.",
        "Ordinary-day overtime premium may be 25% of the regular hourly wage.",
        "Ordinary-day overtime premium is 25% of the regular hourly wage unless approved otherwise.",
        "Example: Ordinary-day overtime premium is 25% of the regular hourly wage.",
        "\"Ordinary-day overtime premium is 25% of the regular hourly wage\".",
        "Overtime is paid at 125%.",
        "Ordinary-day overtime premium is 25% of the regular hourly wage. Ordinary-day overtime premium is 20% of the regular hourly wage.",
        "Holiday overtime premium is 30%.",
    ] {
        assert_eq!(
            review(&report(text), "sha", context()).findings[1].outcome,
            NeedsReview,
            "{text}"
        );
    }
    assert_eq!(
        review(&report("Forty hours per week."), "sha", context()).findings[0].outcome,
        NeedsEvidence
    );
    assert_eq!(
        review(
            &report("Normal daily working hours are 9 hours including unpaid lunch."),
            "sha",
            context()
        )
        .findings[0]
            .outcome,
        NeedsReview
    );
    assert_eq!(
        review(
            &report("Night shift differential is 10% of the regular hourly wage."),
            "sha",
            context()
        )
        .findings[2]
            .outcome,
        NeedsReview
    );
    assert_eq!(
        review(
            &report(
                "Normal daily working hours are 8 hours. Normal daily working hours are 8 hours."
            ),
            "sha",
            context()
        )
        .findings[0]
            .outcome,
        StatedTermMeetsMinimum
    );
}
#[test]
fn pinned_agency_source_contains_each_quoted_provision_and_detects_tampering() {
    let temp = tempfile::tempdir().unwrap();
    let catalog = complyph_library::LibraryCatalog::open(temp.path()).unwrap();
    let source = catalog.snapshot_by_key(SOURCE_KEY).unwrap();
    assert!(source_is_approved(&source.raw, &source.text));
    assert!(!source_is_approved(b"modified", &source.text));
    assert!(!source_is_approved(&source.raw, "modified"));
    for finding in review(&report(&clauses("8", "25", "10")), "sha", context()).findings {
        assert!(
            source.text.contains(&finding.legal_quote),
            "{}",
            finding.article
        );
    }
}

#[test]
fn meal_and_weekly_rest_boundaries_use_independent_exact_thresholds() {
    // Independently transcribed: Article 85 = 60 minutes; Article 91 = 24
    // consecutive hours after six consecutive normal work days. No conversion
    // from an inferred shift, total split breaks or another work-day cadence.
    for (minutes, hours, expected) in [
        ("59.99", "23.99", [PotentialIssue; 2]),
        ("60", "24", [StatedTermMeetsMinimum; 2]),
        ("60.01", "24.01", [StatedTermMeetsMinimum; 2]),
        ("90", "48", [StatedTermMeetsMinimum; 2]),
        ("0", "0", [PotentialIssue; 2]),
    ] {
        let text = format!(
            "Regular meal period is {minutes} minutes of time-off.\nWeekly rest period is {hours} consecutive hours after every six consecutive normal work days."
        );
        let result = review(&report(&text), "sha", context());
        assert_eq!(
            result.findings[3..5]
                .iter()
                .map(|f| f.outcome)
                .collect::<Vec<_>>(),
            expected,
            "{text}"
        );
        assert!(result.findings[3..5].iter().all(|f| f.evidence.len() == 1
            && matches!(
                f.evidence[0].location,
                Location::WordParagraph { paragraph: 1, .. }
            )));
    }
    let alternative = "Time-off for regular meals shall be 60 minutes.\nEmployees shall receive 24 consecutive hours of weekly rest after every 6 consecutive normal work days.";
    assert!(
        review(&report(alternative), "sha", context()).findings[3..5]
            .iter()
            .all(|f| f.outcome == StatedTermMeetsMinimum)
    );
}

#[test]
fn meal_and_rest_exceptions_ambiguity_and_incomplete_terms_never_pass() {
    for (index, examples) in [
        (
            3,
            vec![
                "Regular meal period is 20 minutes paid as working time.",
                "Regular meal period is not 60 minutes of time-off.",
                "Regular meal period is 60 minutes of time-off unless business requires otherwise.",
                "Example: Regular meal period is 60 minutes of time-off.",
                "\"Regular meal period is 60 minutes of time-off\".",
                "Regular meal period is 60.001 minutes of time-off.",
                "Regular meal period is 60 minutes of time-off. Regular meal period is 30 minutes of time-off.",
                "Lunch is from 12:00 to 13:00.",
                "Regular meal break is one hour.",
            ],
        ),
        (
            4,
            vec![
                "Weekly rest period is 24 hours.",
                "Weekly rest period is 24 total hours in two separate breaks.",
                "Weekly rest period is 24 consecutive hours after every seven consecutive normal work days.",
                "Weekly rest period is 24 consecutive hours after every five consecutive normal work days.",
                "Weekly rest period may be 24 consecutive hours after every six consecutive normal work days.",
                "Weekly rest period is 24 consecutive hours after every six consecutive normal work days except during emergencies.",
                "Example: Weekly rest period is 24 consecutive hours after every six consecutive normal work days.",
                "Weekly rest period is 24 consecutive hours after every six consecutive normal work days. Weekly rest period is 12 consecutive hours after every six consecutive normal work days.",
                "Sunday is a rest day.",
            ],
        ),
    ] {
        for text in examples {
            assert_eq!(
                review(&report(text), "sha", context()).findings[index].outcome,
                NeedsReview,
                "{text}"
            );
        }
    }
    let missing = review(&report("Fictional policy introduction."), "sha", context());
    assert!(
        missing.findings[3..5]
            .iter()
            .all(|f| f.outcome == NeedsEvidence)
    );
    let mut excluded = context();
    excluded.meal_period = Applicability::OutsideScope;
    excluded.weekly_rest = Applicability::OutsideScope;
    let exceptions = review(
        &report(
            "Regular meal period is 20 minutes of time-off. Weekly rest period is 12 consecutive hours after every six consecutive normal work days.",
        ),
        "sha",
        excluded,
    );
    assert!(
        exceptions.findings[3..5]
            .iter()
            .all(|f| f.outcome == complyph_checks::Outcome::OutsideScope)
    );
}

#[test]
fn earlier_context_and_reports_do_not_establish_new_rule_scope_or_gain_findings() {
    let old_context = serde_json::json!({"operative_terms_confirmed":true,"normal_hours":"covered","ordinary_overtime":"covered","night_differential":"covered"});
    let ctx: Context = serde_json::from_value(old_context.clone()).unwrap();
    assert_eq!(ctx.meal_period, Applicability::Unknown);
    assert_eq!(ctx.weekly_rest, Applicability::Unknown);
    assert_eq!(
        ctx.applicability("unrecognized-future-rule"),
        Applicability::Unknown
    );
    let current = review(&report(&clauses("8", "25", "10")), "sha", ctx);
    assert!(
        current.findings[..3]
            .iter()
            .all(|f| f.outcome == StatedTermMeetsMinimum)
    );
    assert!(
        current.findings[3..]
            .iter()
            .all(|f| f.outcome == NeedsContext)
    );
    let mut old = serde_json::to_value(&current).unwrap();
    old["engine"] = serde_json::json!("complyph-labor-1");
    old["context"] = old_context;
    old["findings"].as_array_mut().unwrap().truncate(3);
    let historical: Review = serde_json::from_value(old).unwrap();
    assert_eq!(historical.engine, "complyph-labor-1");
    assert_eq!(historical.findings.len(), 3);
    assert_eq!(
        historical
            .context
            .applicability(&historical.findings[2].rule_id),
        Covered
    );
    assert_eq!(
        historical.context.applicability("PH-LAB-085-01"),
        Applicability::Unknown
    );
}
#[test]
fn synthetic_pdf_and_docx_use_real_extraction_locations() {
    for (name, format, expected) in [
        (
            "Synthetic labour baseline.pdf",
            Format::Pdf,
            StatedTermMeetsMinimum,
        ),
        ("Synthetic labour issues.docx", Format::Docx, PotentialIssue),
        ("Synthetic labour ambiguous.docx", Format::Docx, NeedsReview),
    ] {
        let bytes = std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../examples")
                .join(name),
        )
        .unwrap();
        let extracted = complyph_extract::extract(format, &bytes);
        assert_eq!(
            extracted.status,
            Status::Ready,
            "{name}: {:?}",
            extracted.warnings
        );
        let result = review(&extracted, "synthetic", context());
        assert!(
            result.findings[..5].iter().all(|f| f.outcome == expected),
            "{name}: {:?}",
            result
                .findings
                .iter()
                .map(|f| (&f.outcome, &f.evidence))
                .collect::<Vec<_>>()
        );
    }
}
