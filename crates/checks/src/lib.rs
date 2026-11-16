//! Three narrow, deterministic comparisons. No I/O, network, AI, or eligibility inference.
use complyph_extract::{Location, Report, Status};
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::LazyLock;

pub const ENGINE: &str = "complyph-labor-1";
pub const SOURCE_KEY: &str = "labor-code-reference";
pub const SOURCE_SHA256: &str = "99fd38f1434225504d0685fc5a92c4bb305a9e95bc63875d0781423d4acfa4c0";
pub const SOURCE_TEXT_SHA256: &str =
    "c9e5a66dafa4aa6fc55c01347059c7abe15af841ac30b0ec0d74ba60823bd31f";
pub const SOURCE_URL: &str = "https://ldr.senate.gov.ph/legislative%2Bissuances/Presidential%20Decree%20No.%20442%2C%20s.%201974";
pub const LIMITATION: &str = "Three stated numeric terms only. This is not a compliance opinion. Eligibility, actual hours/payments, amendments, special arrangements, other laws and more favourable contract/CBA terms require human review. Text and source hashes identify the snapshots used; they do not establish legal currency.";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Applicability {
    #[default]
    Unknown,
    Covered,
    OutsideScope,
}
impl Applicability {
    pub fn label(self) -> &'static str {
        match self {
            Self::Unknown => "Not established",
            Self::Covered => "Confirmed within this rule's scope",
            Self::OutsideScope => "Outside this rule's scope",
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Context {
    #[serde(default)]
    pub operative_terms_confirmed: bool,
    pub normal_hours: Applicability,
    pub ordinary_overtime: Applicability,
    pub night_differential: Applicability,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    PotentialIssue,
    StatedTermMeetsMinimum,
    NeedsContext,
    NeedsEvidence,
    NeedsReview,
    OutsideScope,
}
impl Outcome {
    pub fn label(self) -> &'static str {
        match self {
            Self::PotentialIssue => "Potential issue",
            Self::StatedTermMeetsMinimum => "Stated term meets this numeric threshold",
            Self::NeedsContext => "Confirm applicability",
            Self::NeedsEvidence => "No supported statement found",
            Self::NeedsReview => "Needs human review",
            Self::OutsideScope => "Outside scope",
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Evidence {
    pub location: Location,
    pub text: String,
    pub hundredths: Option<u32>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Finding {
    pub rule_id: String,
    pub title: String,
    pub article: String,
    pub legal_quote: String,
    pub outcome: Outcome,
    pub explanation: String,
    pub evidence: Vec<Evidence>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Review {
    pub engine: String,
    pub document_sha256: String,
    pub source_key: String,
    pub source_sha256: String,
    pub source_text_sha256: String,
    pub source_url: String,
    pub context: Context,
    pub findings: Vec<Finding>,
    pub limitation: String,
}
struct Rule {
    id: &'static str,
    title: &'static str,
    article: &'static str,
    quote: &'static str,
    threshold: u32,
    upper: bool,
    patterns: [&'static str; 2],
    topic: &'static str,
}
const RULES: [Rule; 3] = [
    Rule {
        id: "PH-LAB-083-01",
        title: "Normal daily working hours",
        article: "Labor Code, Article 83 (read with Articles 82, 84–85)",
        quote: "The normal hours of work of any employee shall not exceed eight in a day.",
        threshold: 800,
        upper: true,
        patterns: [
            r"(?i)^normal daily working hours (?:are|shall be|is) ([0-9]{1,3}(?:\.[0-9]{1,2})?) hours(?: per day)?$",
            r"(?i)^normal working hours (?:are|shall be|is) ([0-9]{1,3}(?:\.[0-9]{1,2})?) hours per day$",
        ],
        topic: r"(?i)normal.*(?:hours|workday)|daily working hours",
    },
    Rule {
        id: "PH-LAB-087-01",
        title: "Ordinary-day overtime additional premium",
        article: "Labor Code, Article 87 (ordinary working days only)",
        quote: "Work may be performed beyond eight hours a day provided that the employee is paid for the overtime work an additional compensation equivalent to his regular wage plus at least twenty-five percent thereof.",
        threshold: 2500,
        upper: false,
        patterns: [
            r"(?i)^ordinary[- ]day overtime (?:additional )?premium (?:is|shall be) ([0-9]{1,3}(?:\.[0-9]{1,2})?)% (?:of|in addition to) (?:the )?regular hourly wage$",
            r"(?i)^overtime on ordinary working days (?:is|shall be) paid (?:at )?(?:the )?regular hourly wage plus ([0-9]{1,3}(?:\.[0-9]{1,2})?)%$",
        ],
        topic: r"(?i)overtime|\bOT\b",
    },
    Rule {
        id: "PH-LAB-086-01",
        title: "Night shift differential additional premium",
        article: "Labor Code, Article 86; implementing rules, Book III, Rule II",
        quote: "Every employee shall be paid a night shift differential of not less than ten percent of his regular wage for each hour of work performed between ten o’clock in the evening and six o’clock in the morning.",
        threshold: 1000,
        upper: false,
        patterns: [
            r"(?i)^night[- ]shift differential (?:is|shall be) ([0-9]{1,3}(?:\.[0-9]{1,2})?)% (?:of|in addition to) (?:the )?regular hourly wage for (?:each hour|hours) worked between 22:00 and 06:00$",
            r"(?i)^night differential (?:is|shall be) ([0-9]{1,3}(?:\.[0-9]{1,2})?)% of (?:the )?regular hourly wage for hours worked between 10 pm and 6 am$",
        ],
        topic: r"(?i)night.*(?:shift|differential)|\bNSD\b",
    },
];
static MATCHERS: LazyLock<Vec<(Vec<Regex>, Regex)>> = LazyLock::new(|| {
    RULES
        .iter()
        .map(|r| {
            (
                r.patterns
                    .iter()
                    .map(|s| Regex::new(s).expect("fixed pattern"))
                    .collect(),
                Regex::new(r.topic).expect("fixed topic"),
            )
        })
        .collect()
});

/// Exact decimal comparison; no floating-point rounding of legal boundaries.
fn hundredths(value: &str) -> Option<u32> {
    let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
    let scale = match fraction.len() {
        0 => 0,
        1 => fraction.parse::<u32>().ok()? * 10,
        2 => fraction.parse().ok()?,
        _ => return None,
    };
    whole
        .parse::<u32>()
        .ok()?
        .checked_mul(100)?
        .checked_add(scale)
}
fn segments(text: &str) -> Vec<String> {
    let mut result = vec![];
    let mut start = 0;
    for (i, c) in text.char_indices() {
        let decimal = c == '.'
            && i > 0
            && text.as_bytes()[i - 1].is_ascii_digit()
            && text.as_bytes().get(i + 1).is_some_and(u8::is_ascii_digit);
        if matches!(c, '.' | '!' | '?' | ';' | '\n' | '\r') && !decimal {
            if start < i {
                result.push(
                    text[start..i]
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" "),
                );
            }
            start = i + c.len_utf8();
        }
    }
    if start < text.len() {
        result.push(
            text[start..]
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" "),
        );
    }
    result
}
/// Applicability is asserted by a human. Never inferred from a job title or document.
pub fn review(report: &Report, document_sha256: &str, context: Context) -> Review {
    let coverage = [
        context.normal_hours,
        context.ordinary_overtime,
        context.night_differential,
    ];
    let readable = report.status == Status::Ready
        && report.warnings.iter().all(|w| {
            w == complyph_extract::PDF_LAYOUT_WARNING || w == complyph_extract::DOCX_LAYOUT_WARNING
        })
        && !report.units.is_empty()
        && report.units.iter().all(|u| !u.text.trim().is_empty())
        && report.units.iter().map(|u| u.text.len()).sum::<usize>()
            <= complyph_extract::MAX_TEXT_BYTES;
    let clauses: Vec<_> = report
        .units
        .iter()
        .flat_map(|unit| {
            segments(&unit.text)
                .into_iter()
                .map(move |text| (&unit.location, text))
        })
        .collect();
    let findings = RULES.iter().enumerate().map(|(i, rule)| {
        let (patterns, topic) = &MATCHERS[i];
        let mut evidence = vec![];
        let mut ambiguous = false;
        for (location, text) in &clauses {
            if !topic.is_match(text) { continue; }
            let number = patterns.iter().find_map(|p| p.captures(text).and_then(|c| hundredths(&c[1])));
            ambiguous |= number.is_none();
            // Bound saved evidence; exceeding this bound forces an unresolved result.
            if evidence.len() < 32 { evidence.push(Evidence { location: (*location).clone(), text: text.chars().take(1500).collect(), hundredths: number }); } else { ambiguous = true; }
        }
        let values: Vec<_> = evidence.iter().filter_map(|e| e.hundredths).collect();
        let (outcome, explanation) = match coverage[i] {
            Applicability::Unknown => (Outcome::NeedsContext, "Confirm this rule's coverage, review period and exceptions before interpreting a numeric comparison."),
            Applicability::OutsideScope => (Outcome::OutsideScope, "The reviewer marked this rule outside scope; no threshold conclusion was made."),
            Applicability::Covered if !context.operative_terms_confirmed => (Outcome::NeedsContext, "Confirm that extracted statements are accurate, operative terms rather than examples or quotations."),
            Applicability::Covered if !readable => (Outcome::NeedsReview, "The text is incomplete, has reading warnings or is unavailable. Compare every page with the original before a manual assessment."),
            Applicability::Covered if ambiguous => (Outcome::NeedsReview, "A relevant clause is unsupported, conditional, quoted or ambiguous. Read the passages; no numeric conclusion was inferred."),
            Applicability::Covered if values.is_empty() => (Outcome::NeedsEvidence, "No supported explicit numeric statement was found. Absence of a match is not evidence of a legal breach."),
            Applicability::Covered if values.iter().any(|v| *v != values[0]) => (Outcome::NeedsReview, "Different stated values were found. Their applicability must be resolved by a human."),
            Applicability::Covered if (rule.upper && values[0] > rule.threshold) || (!rule.upper && values[0] < rule.threshold) => (Outcome::PotentialIssue, "The explicit stated term is outside this rule's numeric threshold, subject to the confirmed scope. Check the cited evidence and legal context."),
            Applicability::Covered => (Outcome::StatedTermMeetsMinimum, "The explicit stated term meets this numeric threshold under the reviewer-confirmed scope. This does not establish full legal compliance or actual payment."),
        };
        Finding { rule_id: rule.id.into(), title: rule.title.into(), article: rule.article.into(), legal_quote: rule.quote.into(), outcome, explanation: explanation.into(), evidence }
    }).collect();
    Review {
        engine: ENGINE.into(),
        document_sha256: document_sha256.into(),
        source_key: SOURCE_KEY.into(),
        source_sha256: SOURCE_SHA256.into(),
        source_text_sha256: SOURCE_TEXT_SHA256.into(),
        source_url: SOURCE_URL.into(),
        context,
        findings,
        limitation: LIMITATION.into(),
    }
}

/// Rules stop if either preserved source or derived text differs from the reviewed snapshot.
pub fn source_is_approved(raw: &[u8], text: &str) -> bool {
    format!("{:x}", Sha256::digest(raw)) == SOURCE_SHA256
        && format!("{:x}", Sha256::digest(text.as_bytes())) == SOURCE_TEXT_SHA256
}
