//! Local exports of one captured document comparison, including both review histories.
use super::*;
use complyph_core::document_comparison::{ComparisonSide, DocumentComparison};

fn side_header(layout: &mut Layout, label: &str, side: &ComparisonSide) -> Result<()> {
    layout.heading(label)?;
    layout.body(&side.reading.name)?;
    for text in [
        format!(
            "Document version #{} | Retained reading #{}",
            side.reading.version_id, side.reading.id
        ),
        format!("File SHA-256: {}", side.reading.saved.sha256),
        format!("Reading SHA-256: {}", side.reading.reading_sha256),
        format!(
            "Reading captured: {} | {} | {}",
            time(side.reading.saved.read_at),
            side.reading.saved.report.engine,
            side.reading.saved.report.status.label()
        ),
        side.review
            .as_ref()
            .map(|r| {
                format!(
                    "Review #{} | Exact reading recorded: {} | Tracking through event #{}",
                    r.workflow.saved.id,
                    r.exact_reading,
                    r.workflow.last_event_id()
                )
            })
            .unwrap_or_else(|| "No saved review selected.".into()),
    ] {
        layout.muted(&text)?;
    }
    for warning in &side.reading.saved.report.warnings {
        layout.body(&format!("Reading warning: {warning}"))?;
    }
    Ok(())
}
pub fn render_comparison_pdf(comparison: &DocumentComparison) -> Result<Vec<u8>> {
    let mut layout = Layout::new()?;
    layout.paragraph(
        "Document version comparison",
        20.0,
        true,
        [0.08, 0.17, 0.29],
    )?;
    layout.muted(&format!(
        "Captured: {} | {}",
        time(comparison.captured_at),
        comparison.engine
    ))?;
    layout.body(&comparison.limitation)?;
    layout.body("Earlier and revised are the selected sides, not a claim about signing dates or legal effect. This report contains private excerpts and saved reviewer notes.")?;
    if comparison.reading_only_change {
        layout.body("READING CHANGE ONLY: original file bytes are identical. An OCR/reader difference does not establish a document edit.")?;
    } else if comparison.same_file_bytes {
        layout.body(
            "FILE BYTES IDENTICAL: review or follow-up changes do not establish a document edit.",
        )?;
    }
    side_header(&mut layout, "Earlier selected version", &comparison.left)?;
    side_header(&mut layout, "Revised selected version", &comparison.right)?;
    layout.heading("Text changes")?;
    layout.body(&comparison.text.detail)?;
    if comparison.text.available {
        layout.body(&format!(
            "{} added lines | {} removed lines | {} unchanged lines",
            comparison.text.added_lines.unwrap_or(0),
            comparison.text.removed_lines.unwrap_or(0),
            comparison.text.unchanged_lines.unwrap_or(0)
        ))?;
    }
    if comparison.text.display_truncated {
        layout.body("DIFFERENCE DISPLAY TRUNCATED: the complete retained readings are included in comparison JSON. The PDF does not contain all original document text.")?;
    }
    for row in &comparison.text.lines {
        let location = row
            .new_location
            .as_ref()
            .or(row.old_location.as_ref())
            .map(|l| l.label())
            .unwrap_or_default();
        let prefix = match row.kind.as_str() {
            "added" => "Added",
            "removed" => "Removed",
            "gap" => "Context gap",
            _ => "Context",
        };
        let color = match row.kind.as_str() {
            "added" => [0.15, 0.35, 0.25],
            "removed" => [0.63, 0.15, 0.15],
            _ => [0.32, 0.37, 0.41],
        };
        layout.paragraph(
            &format!("{prefix} [{location}] {}", row.text),
            10.0,
            false,
            color,
        )?;
    }
    layout.heading("Finding comparison and separate human follow-up")?;
    if comparison.findings.is_empty() {
        layout.body("Select a saved review on each side to compare findings. Text changes alone do not establish that an issue was corrected.")?;
    }
    for finding in &comparison.findings {
        layout.heading(&format!("{} | {}", finding.rule_id, finding.title))?;
        layout.body(finding.change.label())?;
        layout.body(&format!(
            "Earlier result: {} | Follow-up: {}",
            finding
                .left_outcome
                .map(|o| o.label())
                .unwrap_or("Not assessed"),
            finding
                .left_status
                .map(|s| s.label())
                .unwrap_or("Not recorded")
        ))?;
        layout.body(&format!(
            "Revised result: {} | Follow-up: {}",
            finding
                .right_outcome
                .map(|o| o.label())
                .unwrap_or("Not assessed"),
            finding
                .right_status
                .map(|s| s.label())
                .unwrap_or("Not recorded")
        ))?;
        for reason in &finding.reasons {
            layout.body(reason)?;
        }
    }
    for (label, side) in [
        (
            "Earlier saved review and complete follow-up history",
            &comparison.left,
        ),
        (
            "Revised saved review and complete follow-up history",
            &comparison.right,
        ),
    ] {
        if let Some(review) = &side.review {
            layout.page()?;
            layout.heading(label)?;
            append_review(&mut layout, &review.workflow)?;
        }
    }
    let count = layout.pages.len();
    for (index, page) in layout.pages.iter_mut().enumerate() {
        page.lines.push(Line {
            text: format!(
                "Version comparison | Page {} of {} | Captured snapshot",
                index + 1,
                count
            ),
            x: LEFT,
            y: 32.0,
            size: 8.0,
            bold: false,
            color: [0.32, 0.37, 0.41],
        });
    }
    encode(
        &layout.pages,
        &layout.regular,
        &layout.bold,
        &format!(
            "ComplyPH version comparison: {} / {}",
            comparison.left.reading.name, comparison.right.reading.name
        ),
        "Retained document versions, selected historical findings and separate human follow-up",
    )
}
pub fn export_comparison_pdf(comparison: &DocumentComparison, destination: &Path) -> Result<()> {
    let bytes = render_comparison_pdf(comparison)?;
    write_new(destination, |file| file.write_all(&bytes))
}
pub fn export_comparison_json(comparison: &DocumentComparison, destination: &Path) -> Result<()> {
    write_new(destination, |file| {
        serde_json::to_writer_pretty(file, comparison).map_err(std::io::Error::other)
    })
}
fn write_new(
    destination: &Path,
    write: impl FnOnce(&mut std::fs::File) -> std::io::Result<()>,
) -> Result<()> {
    if destination.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "Choose a new filename; existing files are never overwritten.",
        )
        .into());
    }
    let parent = destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    write(temporary.as_file_mut())?;
    temporary.as_file().sync_all()?;
    temporary
        .persist_noclobber(destination)
        .map_err(|e| ExportError::Io(e.error))?;
    Ok(())
}
