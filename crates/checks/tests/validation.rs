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
        normal_hours: Covered,
        ordinary_overtime: Covered,
        night_differential: Covered,
    }
}
fn report(text: &str) -> Report {
    Report {
        engine: complyph_extract::ENGINE.into(),
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
        "Normal daily working hours are {hours} hours.\nOrdinary-day overtime premium is {ot}% of the regular hourly wage.\nNight shift differential is {night}% of the regular hourly wage for hours worked between 22:00 and 06:00."
    )
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
                .map(|f| f.outcome)
                .collect::<Vec<_>>(),
            expected
        );
        assert!(result.findings.iter().all(|f| f.evidence.len() == 1));
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
        normal_hours: Applicability::OutsideScope,
        ordinary_overtime: Applicability::OutsideScope,
        night_differential: Applicability::OutsideScope,
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
        review(&partial, "sha", context())
            .findings
            .iter()
            .all(|f| f.outcome == NeedsReview)
    );
    partial.warnings.clear();
    partial.status = Status::NeedsOcr;
    assert!(
        review(&partial, "sha", context())
            .findings
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
            result.findings.iter().all(|f| f.outcome == expected),
            "{name}: {:?}",
            result
                .findings
                .iter()
                .map(|f| (&f.outcome, &f.evidence))
                .collect::<Vec<_>>()
        );
    }
}
