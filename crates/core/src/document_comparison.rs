//! Bounded local text/finding comparison. Results never transfer a human resolution.
use crate::{
    document_history::{self, RetainedReading},
    *,
};
use complyph_checks::{Finding, Outcome};
use complyph_extract::{Location, Status};
use similar::{ChangeTag, TextDiff};
use std::collections::{BTreeMap, BTreeSet};

pub const ENGINE: &str = "complyph-document-comparison-1";
pub const MAX_DIFF_LINES: usize = 20_000;
pub const MAX_DISPLAY_ROWS: usize = 2_000;
pub const MAX_ROW_CHARS: usize = 2_000;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompareRequest {
    pub left_reading_id: i64,
    pub right_reading_id: i64,
    pub left_review_id: Option<i64>,
    pub right_review_id: Option<i64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ComparedReview {
    pub workflow: ReviewWorkflow,
    pub exact_reading: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ComparisonSide {
    pub reading: RetainedReading,
    pub review: Option<ComparedReview>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DifferenceLine {
    pub kind: String,
    pub old_line: Option<usize>,
    pub new_line: Option<usize>,
    pub old_location: Option<Location>,
    pub new_location: Option<Location>,
    pub text: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TextComparison {
    pub available: bool,
    pub detail: String,
    pub added_lines: Option<usize>,
    pub removed_lines: Option<usize>,
    pub unchanged_lines: Option<usize>,
    pub display_truncated: bool,
    pub lines: Vec<DifferenceLine>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingChange {
    Unchanged,
    Changed,
    StillFlagged,
    NewlyFlagged,
    NoLongerFlagged,
    RuleAdded,
    RuleRemoved,
    NotComparable,
}
impl FindingChange {
    pub fn label(self) -> &'static str {
        match self {
            Self::Unchanged => "Unchanged",
            Self::Changed => "Result or evidence changed",
            Self::StillFlagged => "Still flagged",
            Self::NewlyFlagged => "Newly flagged",
            Self::NoLongerFlagged => "Numeric issue no longer flagged",
            Self::RuleAdded => "Check absent from earlier review",
            Self::RuleRemoved => "Check absent from revised review",
            Self::NotComparable => "Review basis differs",
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FindingComparison {
    pub rule_id: String,
    pub title: String,
    pub change: FindingChange,
    pub evidence_changed: bool,
    pub reasons: Vec<String>,
    pub left_outcome: Option<Outcome>,
    pub right_outcome: Option<Outcome>,
    pub left_status: Option<ReviewStatus>,
    pub right_status: Option<ReviewStatus>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DocumentComparison {
    pub engine: String,
    pub captured_at: i64,
    pub group_id: i64,
    pub same_file_bytes: bool,
    pub reading_only_change: bool,
    pub left: ComparisonSide,
    pub right: ComparisonSide,
    pub text: TextComparison,
    pub findings: Vec<FindingComparison>,
    pub limitation: String,
}

struct Lines<'a> {
    text: String,
    entries: Vec<(&'a str, &'a Location)>,
}
fn lines(reading: &RetainedReading) -> Lines<'_> {
    let entries = reading
        .saved
        .report
        .units
        .iter()
        .flat_map(|u| u.text.lines().map(move |line| (line, &u.location)))
        .collect::<Vec<_>>();
    Lines {
        text: entries
            .iter()
            .map(|(t, _)| *t)
            .collect::<Vec<_>>()
            .join("\n"),
        entries,
    }
}
fn text_comparison(left: &RetainedReading, right: &RetainedReading) -> TextComparison {
    let mut result = TextComparison {
        available: false,
        detail: String::new(),
        added_lines: None,
        removed_lines: None,
        unchanged_lines: None,
        display_truncated: false,
        lines: vec![],
    };
    if left.saved.report.status != Status::Ready || right.saved.report.status != Status::Ready {
        result.detail="Both versions need readable text. Unavailable or incomplete text is not treated as a deletion; open the original and run local OCR if appropriate.".into();
        return result;
    }
    let left = lines(left);
    let right = lines(right);
    if left.entries.len() > MAX_DIFF_LINES || right.entries.len() > MAX_DIFF_LINES {
        result.display_truncated = true;
        result.detail="This comparison exceeds the 20,000-line computation limit. Inspect the complete retained readings and originals; no difference totals were inferred.".into();
        return result;
    }
    let diff = TextDiff::configure()
        .timeout(Duration::from_millis(500))
        .diff_lines(&left.text, &right.text);
    let (mut added, mut removed, mut unchanged) = (0, 0, 0);
    for row in diff.iter_all_changes() {
        match row.tag() {
            ChangeTag::Insert => added += 1,
            ChangeTag::Delete => removed += 1,
            ChangeTag::Equal => unchanged += 1,
        }
    }
    result.available = true;
    result.added_lines = Some(added);
    result.removed_lines = Some(removed);
    result.unchanged_lines = Some(unchanged);
    result.detail="Extracted-text comparison with two context lines. Layout, images, signatures and legal meaning are not compared. A 500 ms edit-search budget can produce a nonminimal diff.".into();
    'groups: for (group, ops) in diff.grouped_ops(2).iter().enumerate() {
        if group > 0 {
            if result.lines.len() >= MAX_DISPLAY_ROWS {
                result.display_truncated = true;
                break;
            }
            result.lines.push(DifferenceLine {
                kind: "gap".into(),
                old_line: None,
                new_line: None,
                old_location: None,
                new_location: None,
                text: "… unchanged text omitted …".into(),
            });
        }
        for op in ops {
            for change in diff.iter_changes(op) {
                if result.lines.len() >= MAX_DISPLAY_ROWS {
                    result.display_truncated = true;
                    break 'groups;
                }
                let value = change.value().trim_end_matches(['\r', '\n']);
                let cut = value.chars().take(MAX_ROW_CHARS).collect::<String>();
                if cut.len() != value.len() {
                    result.display_truncated = true;
                }
                result.lines.push(DifferenceLine {
                    kind: match change.tag() {
                        ChangeTag::Insert => "added",
                        ChangeTag::Delete => "removed",
                        ChangeTag::Equal => "equal",
                    }
                    .into(),
                    old_line: change.old_index().map(|n| n + 1),
                    new_line: change.new_index().map(|n| n + 1),
                    old_location: change
                        .old_index()
                        .and_then(|n| left.entries.get(n))
                        .map(|(_, l)| (*l).clone()),
                    new_location: change
                        .new_index()
                        .and_then(|n| right.entries.get(n))
                        .map(|(_, l)| (*l).clone()),
                    text: cut,
                });
            }
        }
    }
    result
}
fn scope(context: &complyph_checks::Context, id: &str) -> Result<serde_json::Value> {
    use serde_json::json;
    Ok(match id {
        "PH-LAB-083-01" => json!(context.normal_hours),
        "PH-LAB-087-01" => json!(context.ordinary_overtime),
        "PH-LAB-086-01" => json!(context.night_differential),
        "PH-LAB-085-01" => json!(context.meal_period),
        "PH-LAB-091-01" => json!(context.weekly_rest),
        "PH-LAB-095-01" => serde_json::to_value(&context.service_incentive_leave)?,
        _ => serde_json::to_value(context.additional_checks.get(id))?,
    })
}
fn source(workflow: &ReviewWorkflow, finding: &Finding) -> Result<serde_json::Value> {
    let review = &workflow.saved.review;
    Ok(serde_json::json!({
        "citation":finding.source.clone().unwrap_or(complyph_checks::SourceCitation{
            key:review.source_key.clone(),sha256:review.source_sha256.clone(),
            text_sha256:review.source_text_sha256.clone(),url:review.source_url.clone(),
        }),
        "provision":finding.provision,"article":finding.article,"quote":finding.legal_quote,
        "library_assessment":finding.library_review,
    }))
}
fn method(reading: &RetainedReading) -> serde_json::Value {
    let report = &reading.saved.report;
    serde_json::json!({"engine":report.engine,"format":report.format,"reading_profile":report.reading.as_ref().map(|r|&r.profile),"ocr":report.ocr.as_ref().map(|o|serde_json::json!({
        "engine_version":o.engine_version,"renderer":o.renderer,"language":o.language,
        "model_sha256":o.model_sha256,"confidence_threshold":o.confidence_threshold,"processing":o.processing,
    }))})
}
fn bases(
    left: &ComparisonSide,
    right: &ComparisonSide,
    a: &Finding,
    b: &Finding,
) -> Result<Vec<String>> {
    let (Some(l), Some(r)) = (&left.review, &right.review) else {
        return Ok(vec!["Select a saved review for each reading.".into()]);
    };
    let (lr, rr) = (&l.workflow.saved.review, &r.workflow.saved.review);
    let mut reasons = vec![];
    if !l.exact_reading || !r.exact_reading {
        reasons.push(
            "At least one historical review is not bound to this exact saved reading.".into(),
        );
    }
    if lr.engine != rr.engine || lr.rule_pack_sha256 != rr.rule_pack_sha256 {
        reasons.push("The tested rule engine or rule pack changed.".into());
    }
    if source(&l.workflow, a)? != source(&r.workflow, b)? {
        reasons
            .push("The legal source, supporting provision or library assessment changed.".into());
    }
    if scope(&lr.context, &a.rule_id)? != scope(&rr.context, &b.rule_id)?
        || lr.context.operative_terms_confirmed != rr.context.operative_terms_confirmed
        || lr.context.ocr_text_confirmed != rr.context.ocr_text_confirmed
    {
        reasons.push(
            "The recorded coverage, period, factual basis or text confirmations changed.".into(),
        );
    }
    if method(&left.reading) != method(&right.reading) {
        reasons.push("The reading/OCR method or model changed.".into());
    }
    if left.reading.saved.sha256 == right.reading.saved.sha256
        && left.reading.reading_sha256 != right.reading.reading_sha256
    {
        reasons.push(
            "The file bytes are unchanged; a different reading is not evidence of a document edit."
                .into(),
        );
    }
    if [left, right].iter().any(|s| {
        s.reading.saved.report.status != Status::Ready
            || s.reading.saved.report.warnings.iter().any(|w| {
                w != complyph_extract::PDF_LAYOUT_WARNING
                    && w != complyph_extract::DOCX_LAYOUT_WARNING
                    && w != complyph_extract::LEGACY_DOCX_LAYOUT_WARNING
                    && !(s.reading.saved.report.ocr.is_some() && w == complyph_extract::OCR_WARNING)
            })
    }) {
        reasons.push("An extraction is incomplete or has reading warnings.".into());
    }
    Ok(reasons)
}
fn finding_comparisons(
    left: &ComparisonSide,
    right: &ComparisonSide,
) -> Result<Vec<FindingComparison>> {
    fn map(side: &ComparisonSide) -> Result<BTreeMap<&str, &Finding>> {
        let mut result = BTreeMap::new();
        if let Some(review) = &side.review {
            for finding in &review.workflow.saved.review.findings {
                if result.insert(finding.rule_id.as_str(), finding).is_some() {
                    return Err(WorkspaceError::InvalidReview(
                        "A saved review contains duplicate check IDs. It cannot be matched safely."
                            .into(),
                    ));
                }
            }
        }
        Ok(result)
    }
    let a = map(left)?;
    let b = map(right)?;
    let ids = a.keys().chain(b.keys()).copied().collect::<BTreeSet<_>>();
    ids.into_iter().map(|id|{
        let (l,r)=(a.get(id).copied(),b.get(id).copied());
        let evidence_changed=match(l,r){(Some(a),Some(b))=>serde_json::to_value(&a.evidence)?!=serde_json::to_value(&b.evidence)?,_=>true};
        let mut reasons=vec![];
        let change=match(l,r) {
            (Some(a),Some(b))=>{
                reasons=bases(left,right,a,b)?;
                if !reasons.is_empty(){FindingChange::NotComparable}
                else if a.outcome==Outcome::PotentialIssue && b.outcome==Outcome::PotentialIssue {FindingChange::StillFlagged}
                else if a.outcome==Outcome::PotentialIssue && b.outcome==Outcome::StatedTermMeetsMinimum {FindingChange::NoLongerFlagged}
                else if a.outcome!=Outcome::PotentialIssue && b.outcome==Outcome::PotentialIssue {FindingChange::NewlyFlagged}
                else if a.outcome==b.outcome && !evidence_changed {FindingChange::Unchanged}
                else {FindingChange::Changed}
            },
            (None,Some(_)) if left.review.is_some()=>FindingChange::RuleAdded,
            (Some(_),None) if right.review.is_some()=>FindingChange::RuleRemoved,
            _=>{reasons.push("Select a saved review on both sides to compare findings.".into());FindingChange::NotComparable},
        };
        if change==FindingChange::Changed && l.is_some_and(|f|f.outcome==Outcome::PotentialIssue) {
            reasons.push("A missing statement, uncertain result or exclusion does not establish that the earlier issue was corrected.".into());
        }
        Ok(FindingComparison{rule_id:id.into(),title:r.or(l).map(|f|f.title.clone()).unwrap_or_default(),change,evidence_changed,reasons,
            left_outcome:l.map(|f|f.outcome),right_outcome:r.map(|f|f.outcome),
            left_status:left.review.as_ref().and_then(|s|l.map(|_|s.workflow.status(id))),
            right_status:right.review.as_ref().and_then(|s|r.map(|_|s.workflow.status(id))),
        })
    }).collect()
}

fn side(conn: &Connection, reading_id: i64, review_id: Option<i64>) -> Result<ComparisonSide> {
    let reading = document_history::retained_reading(conn, reading_id)?;
    let review = review_id
        .map(|id| {
            let workflow = review_workflow::capture(conn, id)?;
            if workflow.saved.document_id != reading.document_id
                || workflow.saved.review.document_sha256 != reading.saved.sha256
            {
                return Err(WorkspaceError::InvalidReview(
                    "The selected review belongs to another file version.".into(),
                ));
            }
            let exact_reading = document_history::review_reading_id(conn, id)? == Some(reading.id);
            Ok(ComparedReview {
                workflow,
                exact_reading,
            })
        })
        .transpose()?;
    Ok(ComparisonSide { reading, review })
}
impl Workspace {
    /// Capture both immutable readings and both follow-up streams in one SQLite snapshot.
    pub fn compare_document_versions(
        &self,
        request: &CompareRequest,
    ) -> Result<DocumentComparison> {
        let tx = self.connection.unchecked_transaction()?;
        let left = side(&tx, request.left_reading_id, request.left_review_id)?;
        let right = side(&tx, request.right_reading_id, request.right_review_id)?;
        let group_id = document_history::group_id(&tx, left.reading.document_id)?;
        if group_id != document_history::group_id(&tx, right.reading.document_id)? {
            return Err(WorkspaceError::InvalidDocument(
                "Link these documents explicitly as revisions before comparing them.".into(),
            ));
        }
        tx.commit()?;
        let same_file_bytes = left.reading.saved.sha256 == right.reading.saved.sha256;
        let reading_only_change =
            same_file_bytes && left.reading.reading_sha256 != right.reading.reading_sha256;
        let text = text_comparison(&left.reading, &right.reading);
        let findings = finding_comparisons(&left, &right)?;
        Ok(DocumentComparison{engine:ENGINE.into(),captured_at:now(),group_id,same_file_bytes,reading_only_change,left,right,text,findings,
            limitation:"This is a comparison of retained extracted text and selected historical reviews, not a new legal review. Differences may reflect extraction, OCR, scope, rule or source changes. Notes and human statuses remain attached to their original review; no resolution is transferred. Missing old readings are never reconstructed from a changed original.".into()})
    }
}
