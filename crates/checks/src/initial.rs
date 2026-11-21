//! Initial cross-domain rule pack. Trusted compiled data; no I/O or legal-status inference.
use crate::{Applicability, Context, Evidence, Finding, Outcome, ProvisionLocator, SourceCitation};
use complyph_extract::Report;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::LazyLock};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RuleScope {
    pub coverage: Applicability,
    pub exceptions_reviewed: bool,
    pub review_on: String,
    pub basis: String,
}
impl RuleScope {
    pub fn applicability(&self) -> Applicability {
        if self.coverage == Applicability::OutsideScope {
            return self.coverage;
        }
        if self.coverage == Applicability::Covered
            && self.exceptions_reviewed
            && valid_date(&self.review_on)
            && !self.basis.trim().is_empty()
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
}
pub fn valid_date(date: &str) -> bool {
    let bytes = date.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || !bytes
            .iter()
            .enumerate()
            .all(|(i, b)| i == 4 || i == 7 || b.is_ascii_digit())
    {
        return false;
    }
    let (Ok(year), Ok(month), Ok(day)) = (
        date[..4].parse::<u32>(),
        date[5..7].parse::<usize>(),
        date[8..].parse::<u32>(),
    ) else {
        return false;
    };
    let leap = year.is_multiple_of(400) || (year.is_multiple_of(4) && !year.is_multiple_of(100));
    let days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    year >= 1900
        && (1..=12).contains(&month)
        && (1..=days[month.saturating_sub(1).min(11)]).contains(&day)
}
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Comparison {
    Minimum,
    Maximum,
    Exact,
}
#[derive(Clone, Debug, Deserialize)]
pub struct InitialRule {
    pub id: String,
    pub domain: String,
    pub title: String,
    pub article: String,
    pub quote: String,
    pub source: SourceCitation,
    pub provision: ProvisionLocator,
    pub pattern: String,
    pub topic: String,
    pub threshold: u32,
    pub comparison: Comparison,
    pub valid_from: String,
    pub valid_to: String,
    pub help: String,
}
static RULES: LazyLock<Vec<InitialRule>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("initial_rules.json"))
        .expect("compiled initial rule definitions")
});
static MATCHERS: LazyLock<Vec<(Regex, Regex)>> = LazyLock::new(|| {
    rules()
        .iter()
        .map(|r| {
            (
                Regex::new(&r.pattern).expect("compiled rule pattern"),
                Regex::new(&r.topic).expect("compiled rule topic"),
            )
        })
        .collect()
});
pub fn rules() -> &'static [InitialRule] {
    &RULES
}

pub(crate) fn evaluate(report: &Report, context: &Context, readable: bool) -> Vec<Finding> {
    let clauses: Vec<_> = report
        .units
        .iter()
        .flat_map(|u| {
            crate::sil_sentences(&u.text)
                .into_iter()
                .map(move |text| (&u.location, text))
        })
        .collect();
    static CONDITION: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)^(?:but\s+)?(?:if\b|unless\b|except\b|subject\s+to\b|provided\s+that\b)")
            .unwrap()
    });
    let duplicate_band = ["PH-HDMF-EE-HIGH-01", "PH-HDMF-EE-LOW-01"]
        .iter()
        .all(|id| {
            context
                .additional_checks
                .get(*id)
                .is_some_and(|s| s.coverage == Applicability::Covered)
        });
    rules().iter().zip(MATCHERS.iter()).map(|(rule,(pattern,topic))| {
        let mut evidence=Vec::new(); let mut ambiguous=false;
        for (index,(location,text)) in clauses.iter().enumerate() {
            if !topic.is_match(text) {continue;}
            let value=pattern.captures(text).and_then(|c|crate::hundredths(&c[1].replace(',',"")));
            ambiguous|=value.is_none();
            if evidence.len()<32 {evidence.push(Evidence{location:(*location).clone(),text:text.chars().take(1500).collect(),hundredths:value});} else {ambiguous=true;}
            if let Some((next_location,next))=clauses.get(index+1).filter(|(_,next)|CONDITION.is_match(next)) {
                ambiguous=true;
                if evidence.len()<32 {evidence.push(Evidence{location:(*next_location).clone(),text:next.chars().take(1500).collect(),hundredths:None});}
            }
        }
        let scope=context.additional_checks.get(&rule.id).cloned().unwrap_or_default();
        let values:Vec<_>=evidence.iter().filter_map(|e|e.hundredths).collect();
        let applicability=scope.applicability();
        let (outcome,explanation)=if applicability==Applicability::OutsideScope {
            (Outcome::OutsideScope,"The reviewer marked this check outside its supported scope. No numeric conclusion was made; rights or duties under another regime require separate review.".to_string())
        } else if applicability==Applicability::Unknown || !context.operative_terms_confirmed {
            (Outcome::NeedsContext,"Confirm coverage, review exceptions, enter a valid review-period date and record the factual basis. Confirm that the extracted statements are accurate operative terms. Eligibility is not inferred from document text.".into())
        } else if duplicate_band && rule.id.starts_with("PH-HDMF-EE-") {
            (Outcome::NeedsContext,"Both mutually exclusive Pag-IBIG employee compensation bands were marked covered. Establish the applicable band for this worker and period.".into())
        } else if scope.review_on<rule.valid_from || scope.review_on>rule.valid_to {
            (Outcome::NeedsReview,format!("This initial reference profile supports review-period dates {} through {}. The supplied date {} needs a separately assessed profile. These bounds describe supported coverage, not the law's expiry.",rule.valid_from,rule.valid_to,scope.review_on))
        } else if !readable {
            (Outcome::NeedsReview,"Incomplete text, reading warnings or unconfirmed/unsupported OCR prevent a numeric conclusion. Compare the full original.".into())
        } else if ambiguous || values.windows(2).any(|v|v[0]!=v[1]) {
            (Outcome::NeedsReview,"Relevant wording is unsupported, conditional, quoted or conflicting. Inspect the recorded passages; no numeric conclusion was inferred.".into())
        } else if values.is_empty() {
            (Outcome::NeedsEvidence,"No supported explicit statement was found. A missing match is not evidence of a legal breach.".into())
        } else {
            let issue=match rule.comparison {Comparison::Minimum=>values[0]<rule.threshold,Comparison::Maximum=>values[0]>rule.threshold,Comparison::Exact=>values[0]!=rule.threshold};
            if issue {(Outcome::PotentialIssue,"The stated term differs from the supported requirement under the confirmed scope and review-period profile. Inspect the evidence and full provision; actual implementation is not determined.".into())}
            else {(Outcome::StatedTermMeetsMinimum,"The stated term matches this supported numeric requirement under reviewer-confirmed facts and period. This does not establish full compliance, actual payment, attendance, notification or employer performance.".into())}
        };
        Finding{rule_id:rule.id.clone(),title:rule.title.clone(),article:rule.article.clone(),legal_quote:rule.quote.clone(),outcome,explanation,evidence,source:Some(rule.source.clone()),provision:Some(rule.provision.clone()),library_review:None}
    }).collect()
}

pub fn sources() -> Vec<SourceCitation> {
    let mut all = BTreeMap::new();
    all.insert(
        crate::SOURCE_KEY.to_string(),
        SourceCitation {
            key: crate::SOURCE_KEY.into(),
            sha256: crate::SOURCE_SHA256.into(),
            text_sha256: crate::SOURCE_TEXT_SHA256.into(),
            url: crate::SOURCE_URL.into(),
        },
    );
    for rule in rules() {
        all.insert(rule.source.key.clone(), rule.source.clone());
    }
    all.into_values().collect()
}
