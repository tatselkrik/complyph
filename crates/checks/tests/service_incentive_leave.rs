use complyph_checks::{
    Applicability::{Covered, OutsideScope, Unknown},
    Context, Outcome, SilScope,
};
use complyph_extract::{Format, Location, Report, Status, TextUnit};

fn scope() -> SilScope {
    SilScope {employee_coverage:Covered,qualifying_service:Covered,exceptions:Covered,
        basis:"Synthetic review period: 2026. Covered staff; qualifying service/absences, regular establishment headcount and existing paid benefits reviewed. No applicable exemption established.".into()}
}
fn context() -> Context {
    Context {
        operative_terms_confirmed: true,
        service_incentive_leave: scope(),
        ..Default::default()
    }
}
fn input(text: &str) -> Report {
    Report {
        engine: complyph_extract::ENGINE.into(),
        format: Format::Docx,
        status: Status::Ready,
        summary: "Fictional SIL case".into(),
        warnings: vec![],
        ocr: None,
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
fn outcome(text: &str, ctx: Context) -> Outcome {
    complyph_checks::review(&input(text), "synthetic-sil-hash", ctx).findings[5].outcome
}

#[test]
fn independently_transcribed_five_paid_day_boundary_and_supported_wording() {
    use Outcome::*;
    // Article 95(a) and Book III Rule V section 2: FIVE days WITH PAY yearly.
    for (days, expected) in [
        ("0", PotentialIssue),
        ("4", PotentialIssue),
        ("4.99", PotentialIssue),
        ("5", StatedTermMeetsMinimum),
        ("5.00", StatedTermMeetsMinimum),
        ("5.01", StatedTermMeetsMinimum),
        ("10", StatedTermMeetsMinimum),
        ("four", PotentialIssue),
        ("five", StatedTermMeetsMinimum),
        ("six", StatedTermMeetsMinimum),
    ] {
        for statement in [
            format!("Annual service incentive leave is {days} days with pay."),
            format!("Paid service incentive leave entitlement shall be {days} days per year."),
        ] {
            assert_eq!(outcome(&statement, context()), expected, "{statement}");
        }
    }
    assert_eq!(
        outcome(
            "Paid service incentive leave is 5\ndays per year.",
            context()
        ),
        StatedTermMeetsMinimum
    );
    let review = complyph_checks::review(
        &input("Yearly service incentive leave is five days with pay."),
        "synthetic",
        context(),
    );
    let finding = &review.findings[5];
    assert_eq!(finding.rule_id, "PH-LAB-095-01");
    assert_eq!(finding.evidence[0].hundredths, Some(500));
    assert_eq!(finding.evidence[0].location, input("").units[0].location);
    assert!(
        include_str!("../../../library/starter/text/labor-code-reference.txt")
            .contains(&finding.legal_quote)
    );
    assert_eq!(review.context.service_incentive_leave, scope());
}

#[test]
fn sil_ocr_entitlement_needs_separate_accuracy_confirmation_and_valid_provenance() {
    let mut report = input("Paid service incentive leave is 4 days per year.");
    report.engine = complyph_extract::OCR_ENGINE.into();
    report.format = Format::Pdf;
    report.units[0].location = Location::PdfPage { page: 1 };
    report.warnings = vec![complyph_extract::OCR_WARNING.into()];
    report.ocr = Some(complyph_extract::OcrInfo {
        engine_version: "synthetic-validation".into(),
        renderer: "pdfium-8165".into(),
        language: "eng".into(),
        model_sha256: complyph_extract::OCR_MODEL_SHA256.into(),
        confidence_threshold: 8000,
        pages: vec![complyph_extract::OcrPage {
            page: 1,
            words: 11,
            uncertain: vec![],
        }],
    });
    assert_eq!(
        complyph_checks::review(&report, "synthetic", context()).findings[5].outcome,
        Outcome::NeedsReview
    );
    let mut confirmed = context();
    confirmed.ocr_text_confirmed = true;
    assert_eq!(
        complyph_checks::review(&report, "synthetic", confirmed.clone()).findings[5].outcome,
        Outcome::PotentialIssue
    );
    report.ocr.as_mut().unwrap().model_sha256 = "changed".into();
    assert_eq!(
        complyph_checks::review(&report, "synthetic", confirmed).findings[5].outcome,
        Outcome::NeedsReview
    );
}

#[test]
fn every_eligibility_fact_and_recorded_basis_is_required_without_inference() {
    let statement = "Paid service incentive leave is 4 days per year.";
    for dimension in 0..3 {
        let mut ctx = context();
        let facts = &mut ctx.service_incentive_leave;
        match dimension {
            0 => facts.employee_coverage = Unknown,
            1 => facts.qualifying_service = Unknown,
            _ => facts.exceptions = Unknown,
        }
        assert_eq!(outcome(statement, ctx), Outcome::NeedsContext);
    }
    for basis in [
        "".into(),
        "   \n".into(),
        "x".repeat(4001),
        format!(" {}", "x".repeat(4000)),
        "\u{000c}basis".into(),
        "unsupported\0control".into(),
    ] {
        let mut ctx = context();
        ctx.service_incentive_leave.basis = basis;
        assert_eq!(outcome(statement, ctx), Outcome::NeedsContext);
    }
    let ctx = Context {
        operative_terms_confirmed: true,
        ..Default::default()
    };
    assert_eq!(
        outcome(
            "Employee has worked 15 years. Establishment employs 20 people. Paid service incentive leave is 4 days per year.",
            ctx
        ),
        Outcome::NeedsContext
    );
    let mut ctx = context();
    ctx.operative_terms_confirmed = false;
    assert_eq!(outcome(statement, ctx), Outcome::NeedsContext);
}

#[test]
fn explicit_exclusion_special_regime_or_service_not_met_never_produces_a_threshold_verdict() {
    for dimension in 0..3 {
        let mut ctx = context();
        let facts = &mut ctx.service_incentive_leave;
        match dimension {
            0 => facts.employee_coverage = OutsideScope,
            1 => facts.qualifying_service = OutsideScope,
            _ => facts.exceptions = OutsideScope,
        }
        assert_eq!(
            outcome("Annual service incentive leave is 0 days with pay.", ctx),
            Outcome::OutsideScope
        );
    }
    // Labels/pay methods do not automatically establish or exclude coverage.
    for label in [
        "Probationary employee",
        "Piece-rate worker",
        "Commission-paid worker",
        "Manager",
        "Field personnel",
    ] {
        assert_eq!(
            outcome(
                &format!("{label}. Paid service incentive leave is 4 days per year."),
                Context {
                    operative_terms_confirmed: true,
                    ..Default::default()
                }
            ),
            Outcome::NeedsContext
        );
    }
    let mut ctx = context();
    ctx.service_incentive_leave.exceptions = OutsideScope;
    ctx.service_incentive_leave.basis =
        "Fictional existing paid vacation benefit reviewed as an applicable Article 95 exception."
            .into();
    assert_eq!(
        outcome("Paid service incentive leave is 0 days per year.", ctx),
        Outcome::OutsideScope
    );
}

#[test]
fn uncertain_payment_period_conditions_pools_quotes_and_conflicts_stay_unresolved() {
    for statement in [
        "Service incentive leave is 5 days per year.",
        "Annual service incentive leave is 5 days unpaid.",
        "Paid service incentive leave is 5 days per month.",
        "Annual service incentive leave is up to 5 days with pay.",
        "Paid service incentive leave is 5 calendar days per year.",
        "Combined vacation and service incentive leave is 10 days per year.",
        "Paid service incentive leave is 5 days per year subject to approval.",
        "Paid service incentive leave is 4 days per year; if approved by management.",
        "Paid service incentive leave is 4 days per year\nif approved by management.",
        "Paid service incentive leave is 4 days per year. Unless a separate benefit applies.",
        "Example: Paid service incentive leave is 4 days per year.",
        "\"Paid service incentive leave is 4 days per year\".",
        "Paid service incentive leave is 4 days per year. Paid service incentive leave is 5 days per year.",
        "Paid SIL is 5 days per year.",
        "Paid service incentive leave is 4.999 days per year.",
    ] {
        assert_eq!(
            outcome(statement, context()),
            Outcome::NeedsReview,
            "{statement}"
        );
    }
    assert_eq!(
        outcome("Vacation leave is 15 days per year.", context()),
        Outcome::NeedsEvidence
    );
    assert_eq!(
        outcome("Fictional policy introduction.", context()),
        Outcome::NeedsEvidence
    );
    let mut report = input("Paid service incentive leave is 5 days per year.");
    report.warnings.push("unread page".into());
    assert_eq!(
        complyph_checks::review(&report, "synthetic", context()).findings[5].outcome,
        Outcome::NeedsReview
    );
}

#[test]
fn an_explicit_following_condition_in_the_next_paragraph_prevents_a_numeric_verdict() {
    let mut report = input("Paid service incentive leave is 5 days per year.");
    report.units.push(TextUnit {
        location: Location::WordParagraph {
            part: "word/document.xml".into(),
            paragraph: 4,
            table_cell: None,
        },
        text: "Subject to management's sole discretion.".into(),
    });
    assert_eq!(
        complyph_checks::review(&report, "synthetic", context()).findings[5].outcome,
        Outcome::NeedsReview
    );
}

#[test]
fn five_rule_contexts_and_saved_reviews_keep_unknown_sil_scope_and_original_findings() {
    let old = serde_json::json!({"operative_terms_confirmed":true,"normal_hours":"covered","ordinary_overtime":"covered","night_differential":"covered","meal_period":"covered","weekly_rest":"covered"});
    let ctx: Context = serde_json::from_value(old.clone()).unwrap();
    assert_eq!(ctx.applicability("PH-LAB-095-01"), Unknown);
    let review = complyph_checks::review(
        &input("Paid service incentive leave is 0 days per year."),
        "old-context",
        ctx,
    );
    assert_eq!(review.findings[5].outcome, Outcome::NeedsContext);
    let mut json = serde_json::to_value(review).unwrap();
    json["context"] = old;
    json["engine"] = "complyph-labor-3".into();
    json["findings"].as_array_mut().unwrap().truncate(5);
    let saved: complyph_checks::Review = serde_json::from_value(json).unwrap();
    assert_eq!(saved.findings.len(), 5);
    assert_eq!(saved.engine, "complyph-labor-3");
    assert_eq!(saved.context.applicability("PH-LAB-095-01"), Unknown);
}
