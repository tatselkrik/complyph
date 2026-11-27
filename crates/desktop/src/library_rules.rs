//! Connect compiled checks to exact, reviewed public-library versions.
use complyph_checks::{LibraryRuleReview, Outcome, Review, SourceCitation};
use complyph_library::{LibraryCatalog, ProvisionBoundary, RuleDependency, RuleReadiness};
use sha2::{Digest, Sha256};

pub(crate) fn register(catalog: &LibraryCatalog) -> Result<(), String> {
    let template = complyph_checks::review(
        &complyph_extract::Report {
            reading: None,
            engine: complyph_extract::ENGINE.into(),
            ocr: None,
            format: complyph_extract::Format::Docx,
            status: complyph_extract::Status::Ready,
            summary: "Compiled source-dependency registration".into(),
            warnings: vec![],
            units: vec![],
        },
        "",
        Default::default(),
    );
    let mut profiles = Vec::new();
    let mut snapshots = std::collections::BTreeMap::new();
    for finding in &template.findings {
        let source = finding.source.clone().unwrap_or_else(|| SourceCitation {
            key: template.source_key.clone(),
            sha256: template.source_sha256.clone(),
            text_sha256: template.source_text_sha256.clone(),
            url: template.source_url.clone(),
        });
        let key = (
            source.key.clone(),
            source.sha256.clone(),
            source.text_sha256.clone(),
        );
        let snapshot = match snapshots.entry(key) {
            std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::btree_map::Entry::Vacant(entry) => entry.insert(
                catalog
                    .snapshot_for_review(&source.key, &source.sha256, &source.text_sha256)
                    .map_err(|e| e.to_string())?,
            ),
        };
        let provisions = if let Some(locator) = &finding.provision {
            let mut matches = snapshot.text.match_indices(&locator.start);
            let (start, _) = matches.next().ok_or("Missing compiled provision start.")?;
            if matches.next().is_some() {
                return Err("Ambiguous compiled provision start.".into());
            }
            let end = snapshot.text[start + locator.start.len()..]
                .find(&locator.end)
                .map(|n| start + locator.start.len() + n)
                .ok_or("Missing compiled provision ending.")?;
            vec![ProvisionBoundary {
                title: locator.title.clone(),
                start: locator.start.clone(),
                end: locator.end.clone(),
                text_sha256: format!(
                    "{:x}",
                    Sha256::digest(&snapshot.text.as_bytes()[start..end])
                ),
            }]
        } else {
            crate::provision_view::dependency_boundaries(
                &snapshot.text,
                &finding.rule_id,
                &finding.legal_quote,
            )?
        };
        profiles.push(RuleDependency {
            rule_id: finding.rule_id.clone(),
            title: finding.title.clone(),
            source_key: source.key,
            raw_sha256: source.sha256,
            text_sha256: source.text_sha256,
            provisions,
            engine: complyph_checks::ENGINE.into(),
            pack_sha256: complyph_checks::compiled_profile_sha256(),
        });
    }
    catalog
        .register_rule_profiles(&profiles)
        .map_err(|e| e.to_string())
}
pub(crate) fn apply(catalog: &LibraryCatalog, mut review: Review) -> Result<Review, String> {
    let states = catalog.rule_library_states().map_err(|e| e.to_string())?;
    if states.len() != complyph_checks::RULE_COUNT {
        return Err(
            "The current compiled rule dependencies are unavailable. Reopen the library.".into(),
        );
    }
    for finding in &mut review.findings {
        let state = states
            .iter()
            .find(|s| s.dependency.rule_id == finding.rule_id)
            .ok_or("A checking rule has no library assessment.")?;
        let before_effective = state
            .source_effective_from
            .as_deref()
            .zip(review.context.additional_checks.get(&finding.rule_id))
            .is_some_and(|(effective, scope)| {
                chrono::NaiveDate::parse_from_str(effective, "%Y-%m-%d")
                    .ok()
                    .zip(chrono::NaiveDate::parse_from_str(&scope.review_on, "%Y-%m-%d").ok())
                    .is_some_and(|(effective, period)| period < effective)
            });
        if state.status == RuleReadiness::Current && !before_effective {
            continue;
        }
        finding.library_review = Some(LibraryRuleReview {
            status: if before_effective {
                "Source effective after document period".into()
            } else {
                state.status.label().into()
            },
            active_source_sha256: state.active_sha256.clone(),
            decision_token: state.token.clone(),
            assessment_id: state.review_id,
            reviewed_by: state.reviewer.as_ref().map(|r| r.actor.clone()),
            reviewed_on: state.reviewer.as_ref().map(|r| r.review_on.clone()),
            basis: state.reviewer.as_ref().map(|r| r.basis.clone()),
            test_reference: state.test_reference.clone(),
            reason: if before_effective {
                format!(
                    "The adopted source is recorded effective from {}, later than this document's selected review period. Review the appropriate preserved version.",
                    state.source_effective_from.as_deref().unwrap_or("")
                )
            } else {
                state.reason.clone()
            },
        });
        if !state.status.usable() || before_effective {
            if finding.outcome != Outcome::OutsideScope {
                finding.outcome = Outcome::NeedsReview;
                finding.explanation = format!(
                    "Library maintenance hold: {} No numeric conclusion is activated for this rule. Review Legal library → Rule revalidation. The preserved compiled citation remains available.",
                    finding
                        .library_review
                        .as_ref()
                        .map(|r| r.reason.as_str())
                        .unwrap_or(&state.reason)
                );
            }
            continue;
        }
        let active = catalog
            .snapshot_for_review(
                &state.dependency.source_key,
                &state.active_sha256,
                &state.active_text_sha256,
            )
            .map_err(|e| e.to_string())?;
        for provision in &state.dependency.provisions {
            provision.text(&active.text).map_err(|e| e.to_string())?;
        }
        finding.source = Some(SourceCitation {
            key: active.metadata.key,
            sha256: active.metadata.raw_sha256,
            text_sha256: active.metadata.text_sha256,
            url: active.metadata.source_url,
        });
    }
    Ok(review)
}
