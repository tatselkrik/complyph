//! Private, offline diagnostic surface shared with the native revision workflow.
use complyph_core::{
    Workspace, WorkspacePaths, document_comparison::CompareRequest,
    document_history::RevisionLinkRequest,
};
use serde::{Deserialize, Serialize};
use std::{
    io::Read,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
enum Request {
    History {
        document_id: i64,
    },
    AddRevision {
        document_id: i64,
        path: PathBuf,
        expected_group_id: i64,
    },
    Link {
        request: RevisionLinkRequest,
    },
    Unlink {
        document_id: i64,
        expected_group_id: i64,
    },
    Reading {
        reading_id: i64,
    },
    Compare {
        request: CompareRequest,
    },
    ExportPdf {
        request: CompareRequest,
        destination: PathBuf,
    },
    ExportJson {
        request: CompareRequest,
        destination: PathBuf,
    },
}
#[derive(Clone, Debug, Serialize)]
pub struct RevisionImport {
    pub document_id: i64,
    pub reading_id: i64,
    pub status: complyph_extract::Status,
}
pub fn add_revision(
    paths: &WorkspacePaths,
    base: i64,
    path: &Path,
    expected_group: i64,
    cancel: &AtomicBool,
) -> Result<RevisionImport, String> {
    if cancel.load(Ordering::Relaxed) {
        return Err("Revision reading cancelled.".into());
    }
    let workspace = Workspace::open(paths).map_err(|e| e.to_string())?;
    if workspace
        .document_history(base)
        .map_err(|e| e.to_string())?
        .group_id
        != expected_group
    {
        return Err("The document history changed. Refresh it before adding a revision.".into());
    }
    let document = workspace
        .register_document(path)
        .map_err(|e| e.to_string())?;
    let saved = crate::reader::read_document(paths, &document, false, cancel)?;
    if !matches!(
        saved.report.status,
        complyph_extract::Status::Ready | complyph_extract::Status::NeedsOcr
    ) {
        return Err(format!(
            "The file remains in Documents but was not linked: {}. {}",
            saved.report.status.label(),
            saved.report.summary
        ));
    }
    let history = workspace
        .document_history(document.id)
        .map_err(|e| e.to_string())?;
    if cancel.load(Ordering::Relaxed) {
        return Err("Revision reading cancelled before linking.".into());
    }
    workspace
        .link_document_revision(&RevisionLinkRequest {
            document_id: base,
            revised_document_id: document.id,
            expected_group_id: expected_group,
            expected_revision_group_id: history.group_id,
        })
        .map_err(|e| e.to_string())?;
    let reading_hash = complyph_core::document_history::reading_sha256(&saved.report)
        .map_err(|e| e.to_string())?;
    let reading_id = history
        .versions
        .iter()
        .find(|v| v.document_sha256 == saved.sha256)
        .and_then(|v| v.readings.iter().find(|r| r.reading_sha256 == reading_hash))
        .map(|r| r.id)
        .ok_or("The newly retained reading is unavailable. Refresh document history.")?;
    Ok(RevisionImport {
        document_id: document.id,
        reading_id,
        status: saved.report.status,
    })
}
pub fn run(arguments: Vec<std::ffi::OsString>) -> Result<(), String> {
    if !matches!(arguments.len(), 4 | 6)
        || arguments[0] != "--metadata"
        || arguments[2] != "--output"
        || (arguments.len() == 6 && arguments[4] != "--data-dir")
    {
        return Err(
            "Use --document-action --metadata JSON --output NEW-JSON [--data-dir PATH]".into(),
        );
    }
    let mut bytes = vec![];
    std::fs::File::open(&arguments[1])
        .map_err(|e| e.to_string())?
        .take(32 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 32 * 1024 {
        return Err("The document action exceeds its metadata limit.".into());
    }
    let request: Request = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    let output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&arguments[3])
        .map_err(|_| "Choose a new output filename; existing files are never overwritten.")?;
    let paths = if arguments.len() == 6 {
        WorkspacePaths::under(PathBuf::from(&arguments[5]))
    } else {
        WorkspacePaths::default_location().map_err(|e| e.to_string())?
    };
    let workspace = Workspace::open(&paths).map_err(|e| e.to_string())?;
    let value=(||->complyph_core::Result<serde_json::Value>{
        Ok(match request {
            Request::History{document_id}=>serde_json::to_value(workspace.document_history(document_id)?)?,
            Request::AddRevision{document_id,path,expected_group_id}=>{
                let result=add_revision(&paths,document_id,&path,expected_group_id,&AtomicBool::new(false)).map_err(complyph_core::WorkspaceError::InvalidDocument)?;
                serde_json::to_value(result)?
            },
            Request::Link{request}=>{workspace.link_document_revision(&request)?;serde_json::to_value(workspace.document_history(request.document_id)?)?},
            Request::Unlink{document_id,expected_group_id}=>{workspace.unlink_document_revision(document_id,expected_group_id)?;serde_json::to_value(workspace.document_history(document_id)?)?},
            Request::Reading{reading_id}=>serde_json::to_value(workspace.retained_reading(reading_id)?)?,
            Request::Compare{request}=>serde_json::to_value(workspace.compare_document_versions(&request)?)?,
            Request::ExportPdf{request,destination}=>{
                let comparison=workspace.compare_document_versions(&request)?;
                complyph_reports::export_comparison_pdf(&comparison,&destination).map_err(|e|complyph_core::WorkspaceError::InvalidReview(e.to_string()))?;
                serde_json::json!({"exported":true,"format":"pdf","left_version":comparison.left.reading.version_id,"right_version":comparison.right.reading.version_id})
            },
            Request::ExportJson{request,destination}=>{
                let comparison=workspace.compare_document_versions(&request)?;
                complyph_reports::export_comparison_json(&comparison,&destination).map_err(|e|complyph_core::WorkspaceError::InvalidReview(e.to_string()))?;
                serde_json::json!({"exported":true,"format":"json","left_version":comparison.left.reading.version_id,"right_version":comparison.right.reading.version_id})
            },
        })
    })().map_err(|e|e.to_string())?;
    serde_json::to_writer_pretty(output, &value).map_err(|e| e.to_string())
}
