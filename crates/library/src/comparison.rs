//! Bounded, local comparison and inert HTML reading. No browser or network I/O.
use super::*;
use scraper::{Html, Node};
use similar::{ChangeTag, TextDiff};

pub const MAX_PUBLIC_HTML_BYTES: usize = 1024 * 1024;
pub const MAX_REFERENCE_TEXT_BYTES: usize = 6 * 1024 * 1024;

pub fn html_reading_text(bytes: &[u8]) -> Result<String> {
    if bytes.len() > MAX_PUBLIC_HTML_BYTES {
        return Err(LibraryError::InvalidImport(
            "HTML references are limited to 1 MiB; use the agency PDF for larger publications."
                .into(),
        ));
    }
    let html = std::str::from_utf8(bytes).map_err(|_| LibraryError::InvalidImport("The HTML publication must use UTF-8. Preserve and inspect the original if it uses another encoding.".into()))?;
    let document = Html::parse_document(html);
    let mut text = String::new();
    for node in document.tree.nodes() {
        if node.ancestors().any(|a| matches!(a.value(), Node::Element(e) if matches!(e.name(), "script"|"style"|"noscript"|"template") || e.attr("hidden").is_some())) {
            continue;
        }
        match node.value() {
            Node::Text(value) => {
                for word in value.split_whitespace() {
                    if !text.ends_with([' ', '\n']) {
                        text.push(' ');
                    }
                    text.push_str(word);
                }
            }
            Node::Element(element)
                if matches!(
                    element.name(),
                    "p" | "div"
                        | "br"
                        | "li"
                        | "tr"
                        | "h1"
                        | "h2"
                        | "h3"
                        | "h4"
                        | "section"
                        | "article"
                ) && !text.ends_with('\n') =>
            {
                text.push('\n');
            }
            _ => {}
        }
        if text.len() > MAX_REFERENCE_TEXT_BYTES {
            return Err(LibraryError::InvalidImport(
                "The public reading text exceeds its size limit.".into(),
            ));
        }
    }
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err(LibraryError::InvalidImport(
            "The publication has no inert readable text. Scripts are never executed.".into(),
        ));
    }
    Ok(text)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DifferenceLine {
    pub kind: String,
    pub old_line: Option<usize>,
    pub new_line: Option<usize>,
    pub text: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReferenceComparison {
    pub old_sha256: String,
    pub new_sha256: String,
    pub old_text_sha256: String,
    pub new_text_sha256: String,
    pub added_lines: usize,
    pub removed_lines: usize,
    pub unchanged_lines: usize,
    pub display_truncated: bool,
    pub lines: Vec<DifferenceLine>,
}
pub fn compare_readings(
    old_hash: &str,
    old: &str,
    new_hash: &str,
    new: &str,
) -> Result<ReferenceComparison> {
    if old.len() > MAX_REFERENCE_TEXT_BYTES || new.len() > MAX_REFERENCE_TEXT_BYTES {
        return Err(LibraryError::InvalidImport(
            "The reference exceeds the comparison text limit.".into(),
        ));
    }
    let mut result = ReferenceComparison {
        old_sha256: old_hash.into(),
        new_sha256: new_hash.into(),
        old_text_sha256: sha256(old.as_bytes()),
        new_text_sha256: sha256(new.as_bytes()),
        added_lines: 0,
        removed_lines: 0,
        unchanged_lines: 0,
        display_truncated: false,
        lines: Vec::new(),
    };
    if old.lines().count() > 20_000 || new.lines().count() > 20_000 {
        result.display_truncated = true;
        return Ok(result);
    }
    let diff = TextDiff::configure()
        .timeout(Duration::from_millis(500))
        .diff_lines(old, new);
    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Insert => result.added_lines += 1,
            ChangeTag::Delete => result.removed_lines += 1,
            ChangeTag::Equal => result.unchanged_lines += 1,
        }
    }
    for group in diff.grouped_ops(2) {
        for op in group {
            for change in diff.iter_changes(&op) {
                if result.lines.len() >= 2000 {
                    result.display_truncated = true;
                    continue;
                }
                let mut chars = change.value().chars();
                let text: String = chars.by_ref().take(2000).collect();
                if chars.next().is_some() {
                    result.display_truncated = true;
                }
                result.lines.push(DifferenceLine {
                    kind: match change.tag() {
                        ChangeTag::Insert => "added",
                        ChangeTag::Delete => "removed",
                        ChangeTag::Equal => "unchanged",
                    }
                    .into(),
                    old_line: change.old_index().map(|n| n + 1),
                    new_line: change.new_index().map(|n| n + 1),
                    text,
                });
            }
        }
    }
    Ok(result)
}
