//! Focused, deterministic comparisons. No I/O, network, AI, or eligibility inference.
use complyph_extract::{Location, Report, Status};
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::LazyLock;

mod initial;
pub use initial::{InitialRule, RuleScope, valid_date};
pub use initial::{rules as initial_rules, sources as source_pins};
pub const ENGINE: &str = "complyph-rules-1";
/// All compiled numeric/scope logic, distinct from the additional-definition pack hash.
pub fn compiled_profile_sha256() -> String {
    let mut digest = Sha256::new();
    digest.update(include_bytes!("lib.rs"));
    digest.update(include_bytes!("initial.rs"));
    digest.update(include_bytes!("initial_rules.json"));
    format!("{:x}", digest.finalize())
}
pub const RULE_COUNT: usize = 20;
pub const PACK_ID: &str = "initial-seven-domain-v1";
pub fn rule_pack_sha256() -> String {
    format!("{:x}", Sha256::digest(include_bytes!("initial_rules.json")))
}
pub const SOURCE_KEY: &str = "labor-code-reference";
pub const SOURCE_SHA256: &str = "99fd38f1434225504d0685fc5a92c4bb305a9e95bc63875d0781423d4acfa4c0";
pub const SOURCE_TEXT_SHA256: &str =
    "c9e5a66dafa4aa6fc55c01347059c7abe15af841ac30b0ec0d74ba60823bd31f";
pub const SOURCE_URL: &str = "https://ldr.senate.gov.ph/legislative%2Bissuances/Presidential%20Decree%20No.%20442%2C%20s.%201974";
pub const LIMITATION: &str = "Twenty focused numeric terms across seven domains. New profiles require reviewer-confirmed coverage, exceptions, period and basis. These are initial coverage profiles, not full-domain assessments. This is not a compliance opinion. Eligibility, actual hours/payments/rest/leave, amendments, shortened paid meals, rest-day exceptions, SIL service/exemptions/equivalent benefits, leave valuation/commutation, special arrangements, other laws and more favourable contract/CBA terms require human review. Text and source hashes identify the snapshots used; they do not establish legal currency.";

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

/// Human-established facts for the standard Article 95 comparison, never inferred.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SilScope {
    pub employee_coverage: Applicability,
    pub qualifying_service: Applicability,
    /// Covered means the reviewer established that no applicable exception applies.
    pub exceptions: Applicability,
    pub basis: String,
}
impl SilScope {
    pub fn applicability(&self) -> Applicability {
        let facts = [
            self.employee_coverage,
            self.qualifying_service,
            self.exceptions,
        ];
        if facts.contains(&Applicability::OutsideScope) {
            return Applicability::OutsideScope;
        }
        let basis = self.basis.trim();
        if facts.iter().all(|f| *f == Applicability::Covered)
            && !basis.is_empty()
            && self.basis.len() <= 4000
            && !self
                .basis
                .chars()
                .any(|c| c.is_control() && c != '\n' && c != '\t')
        {
            Applicability::Covered
        } else {
            Applicability::Unknown
        }
    }
    pub fn details(&self) -> [(&'static str, &'static str); 3] {
        use Applicability::*;
        [
            (
                "Employee coverage",
                match self.employee_coverage {
                    Unknown => "Not established",
                    Covered => "Confirmed within the standard Article 95 regime",
                    OutsideScope => "Outside this standard regime / special arrangement",
                },
            ),
            (
                "Qualifying service",
                match self.qualifying_service {
                    Unknown => "Not established",
                    Covered => "At least one qualifying year confirmed",
                    OutsideScope => "Qualifying service not met for this review",
                },
            ),
            (
                "Exceptions",
                match self.exceptions {
                    Unknown => "Not reviewed / uncertain",
                    Covered => "Reviewed; no applicable exception established",
                    OutsideScope => "Exception or alternative benefit established",
                },
            ),
        ]
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Context {
    #[serde(default)]
    pub operative_terms_confirmed: bool,
    #[serde(default)]
    pub ocr_text_confirmed: bool,
    pub normal_hours: Applicability,
    pub ordinary_overtime: Applicability,
    pub night_differential: Applicability,
    /// Earlier reports never established coverage for the additional rules.
    #[serde(default)]
    pub meal_period: Applicability,
    #[serde(default)]
    pub weekly_rest: Applicability,
    /// Earlier contexts must not silently establish service or exception facts.
    #[serde(default)]
    pub service_incentive_leave: SilScope,
    #[serde(default)]
    pub additional_checks: std::collections::BTreeMap<String, RuleScope>,
}
impl Context {
    /// Resolve by stable rule ID so historical/reordered findings retain their scope.
    pub fn applicability(&self, rule_id: &str) -> Applicability {
        match rule_id {
            "PH-LAB-083-01" => self.normal_hours,
            "PH-LAB-087-01" => self.ordinary_overtime,
            "PH-LAB-086-01" => self.night_differential,
            "PH-LAB-085-01" => self.meal_period,
            "PH-LAB-091-01" => self.weekly_rest,
            "PH-LAB-095-01" => self.service_incentive_leave.applicability(),
            _ => initial_rules()
                .iter()
                .find(|r| r.id == rule_id)
                .and_then(|_| self.additional_checks.get(rule_id))
                .map(RuleScope::applicability)
                .unwrap_or_default(),
        }
    }
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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceCitation {
    pub key: String,
    pub sha256: String,
    pub text_sha256: String,
    pub url: String,
}
impl SourceCitation {
    pub fn matches(&self, raw: &[u8], text: &str) -> bool {
        format!("{:x}", Sha256::digest(raw)) == self.sha256
            && format!("{:x}", Sha256::digest(text.as_bytes())) == self.text_sha256
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvisionLocator {
    pub title: String,
    pub start: String,
    pub end: String,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceCitation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provision: Option<ProvisionLocator>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub library_review: Option<LibraryRuleReview>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LibraryRuleReview {
    pub status: String,
    pub active_source_sha256: String,
    pub decision_token: String,
    pub assessment_id: Option<i64>,
    pub reviewed_by: Option<String>,
    pub reviewed_on: Option<String>,
    pub basis: Option<String>,
    pub test_reference: Option<String>,
    pub reason: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Review {
    pub engine: String,
    pub document_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_pack_sha256: Option<String>,
    pub source_key: String,
    pub source_sha256: String,
    pub source_text_sha256: String,
    pub source_url: String,
    pub context: Context,
    pub findings: Vec<Finding>,
    pub limitation: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ocr: Option<complyph_extract::OcrInfo>,
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
const RULES: [Rule; 6] = [
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
    Rule {
        id: "PH-LAB-085-01",
        title: "Regular meal period",
        article: "Labor Code, Article 85; implementing rules, Book III, Rule I, Section 7",
        quote: "Subject to such regulations as the Secretary of Labor and Employment may prescribe, it shall be the duty of every employer to give his employees not less than sixty minutes time-off for their regular meals.",
        threshold: 6000,
        upper: false,
        patterns: [
            r"(?i)^regular meal (?:period|break) (?:is|shall be) ([0-9]{1,3}(?:\.[0-9]{1,2})?) minutes(?: of time[- ]off)?$",
            r"(?i)^time[- ]off for regular meals (?:is|shall be) ([0-9]{1,3}(?:\.[0-9]{1,2})?) minutes$",
        ],
        topic: r"(?i)\b(?:meals?|lunch|dinner|breakfast)\b",
    },
    Rule {
        id: "PH-LAB-091-01",
        title: "Consecutive weekly rest period",
        article: "Labor Code, Article 91 (read with Article 92); implementing rules, Book III, Rule III",
        quote: "It shall be the duty of every employer, whether operating for profit or not, to provide each of his employees a rest period of not less than twenty-four consecutive hours after every six consecutive normal work days.",
        threshold: 2400,
        upper: false,
        patterns: [
            r"(?i)^weekly rest period (?:is|shall be) ([0-9]{1,3}(?:\.[0-9]{1,2})?) consecutive hours after every (?:six|6) consecutive normal work ?days$",
            r"(?i)^employees (?:receive|shall receive) ([0-9]{1,3}(?:\.[0-9]{1,2})?) consecutive hours of weekly rest after every (?:six|6) consecutive normal work ?days$",
        ],
        topic: r"(?i)\bweekly\s+rest\b|\brest[- ]days?\b|\bconsecutive\s+(?:normal\s+)?work\s*days?\b",
    },
    Rule {
        id: "PH-LAB-095-01",
        title: "Annual paid service incentive leave",
        article: "Labor Code, Article 95; implementing rules, Book III, Rule V",
        quote: "Every employee who has rendered at least one year of service shall be entitled to a yearly service incentive leave of five days with pay.",
        threshold: 500,
        upper: false,
        patterns: [
            r"(?i)^(?:annual|yearly) service incentive leave(?: entitlement)? (?:is|shall be) ([0-9]{1,3}(?:\.[0-9]{1,2})?|zero|one|two|three|four|five|six|seven|eight|nine|ten) days with pay$",
            r"(?i)^paid service incentive leave(?: entitlement)? (?:is|shall be) ([0-9]{1,3}(?:\.[0-9]{1,2})?|zero|one|two|three|four|five|six|seven|eight|nine|ten) days (?:per year|annually|each year|per annum)$",
        ],
        topic: r"(?i)\bservice[- ]incentive[- ]leave\b|\bSIL\b",
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
    // Only the SIL patterns accept these words. Existing rule syntax stays numeric.
    if let Some(number) = [
        "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
    ]
    .iter()
    .position(|v| value.eq_ignore_ascii_case(v))
    {
        return Some(number as u32 * 100);
    }
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
    split_statements(text, false)
}
fn sil_sentences(text: &str) -> Vec<String> {
    split_statements(text, true)
}
fn split_statements(text: &str, preserve_qualifiers: bool) -> Vec<String> {
    let mut result = vec![];
    let mut start = 0;
    for (i, c) in text.char_indices() {
        let decimal = c == '.'
            && i > 0
            && text.as_bytes()[i - 1].is_ascii_digit()
            && text.as_bytes().get(i + 1).is_some_and(u8::is_ascii_digit);
        if (matches!(c, '.' | '!' | '?')
            || (!preserve_qualifiers && matches!(c, ';' | '\n' | '\r')))
            && !decimal
        {
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
    let ocr = report.ocr.is_some() || report.engine == complyph_extract::OCR_ENGINE;
    let readable = report.status == Status::Ready
        && ((!ocr && report.engine == complyph_extract::ENGINE)
            || (ocr && complyph_extract::valid_ocr(report) && context.ocr_text_confirmed))
        && report.warnings.iter().all(|w| {
            w == complyph_extract::PDF_LAYOUT_WARNING
                || w == complyph_extract::DOCX_LAYOUT_WARNING
                || (ocr && w == complyph_extract::OCR_WARNING)
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
    let sil_clauses: Vec<_> = report
        .units
        .iter()
        .flat_map(|unit| {
            sil_sentences(&unit.text)
                .into_iter()
                .map(move |text| (&unit.location, text))
        })
        .collect();
    // A following explicit condition cannot be detached into an apparently
    // unconditional numeric grant. Unrelated full policy interpretation is manual.
    static FOLLOWING_CONDITION: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)^(?:but\s+)?(?:if\b|unless\b|except\b|subject\s+to\b|provided\s+that\b)")
            .expect("fixed SIL condition")
    });
    let sil_condition = sil_clauses
        .windows(2)
        .any(|pair| MATCHERS[5].1.is_match(&pair[0].1) && FOLLOWING_CONDITION.is_match(&pair[1].1))
        || report.units.iter().any(|unit| {
            MATCHERS[5].1.is_match(&unit.text)
                && sil_sentences(&unit.text)
                    .iter()
                    .any(|s| FOLLOWING_CONDITION.is_match(s))
        });
    let mut findings:Vec<Finding> = RULES.iter().enumerate().map(|(i, rule)| {
        let (patterns, topic) = &MATCHERS[i];
        let mut evidence = vec![];
        let sil=rule.id=="PH-LAB-095-01";
        let mut ambiguous = sil && sil_condition;
        for (location, text) in if sil {&sil_clauses} else {&clauses} {
            if !topic.is_match(text) { continue; }
            let number = patterns.iter().find_map(|p| p.captures(text).and_then(|c| hundredths(&c[1])));
            ambiguous |= number.is_none();
            // Bound saved evidence; exceeding this bound forces an unresolved result.
            if evidence.len() < 32 { evidence.push(Evidence { location: (*location).clone(), text: text.chars().take(1500).collect(), hundredths: number }); } else { ambiguous = true; }
        }
        let values: Vec<_> = evidence.iter().filter_map(|e| e.hundredths).collect();
        let (outcome, explanation) = match context.applicability(rule.id) {
            Applicability::Unknown if rule.id=="PH-LAB-095-01" => (Outcome::NeedsContext, "Establish employee coverage, at least one qualifying year, applicable exceptions and a recorded eligibility basis before comparing annual paid service incentive leave."),
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
        Finding { rule_id: rule.id.into(), title: rule.title.into(), article: rule.article.into(), legal_quote: rule.quote.into(), outcome, explanation: explanation.into(), evidence, source:None,provision:None,library_review:None }
    }).collect();
    findings.extend(initial::evaluate(report, &context, readable));
    Review {
        engine: ENGINE.into(),
        document_sha256: document_sha256.into(),
        rule_pack_sha256: Some(rule_pack_sha256()),
        source_key: SOURCE_KEY.into(),
        source_sha256: SOURCE_SHA256.into(),
        source_text_sha256: SOURCE_TEXT_SHA256.into(),
        source_url: SOURCE_URL.into(),
        context,
        findings,
        limitation: LIMITATION.into(),
        ocr: report.ocr.clone(),
    }
}

/// Rules stop if either preserved source or derived text differs from the reviewed snapshot.
pub fn source_is_approved(raw: &[u8], text: &str) -> bool {
    format!("{:x}", Sha256::digest(raw)) == SOURCE_SHA256
        && format!("{:x}", Sha256::digest(text.as_bytes())) == SOURCE_TEXT_SHA256
}
