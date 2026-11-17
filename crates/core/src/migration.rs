//! Copy-once migration. SQLite's backup API includes committed WAL content.
use crate::{Result, WorkspaceError, WorkspacePaths};
use complyph_paths::{StoragePlan, has_workspace, secure_directory};
use rusqlite::{Connection, OpenFlags};
use std::{
    fs,
    path::Path,
    time::{Duration, Instant},
};

pub fn prepare(plan: &StoragePlan) -> Result<WorkspacePaths> {
    if has_workspace(&plan.preferred) {
        return Ok(WorkspacePaths::under(&plan.preferred));
    }
    let candidates: Vec<_> = plan
        .legacy_candidates
        .iter()
        .filter(|p| has_workspace(p))
        .collect();
    match candidates.as_slice() {
        [] => {
            secure_directory(&plan.preferred)?;
            Ok(WorkspacePaths::under(&plan.preferred))
        }
        [source] => import(source, &plan.preferred),
        _ => Err(complyph_paths::LocationError::Ambiguous.into()),
    }
}

/// Explicit GUI import into an unused destination. Its empty databases are retained as a backup.
pub fn import_into_empty(source: &Path, target: &Path) -> Result<WorkspacePaths> {
    if !has_workspace(target) {
        return import(source, target);
    }
    if source.canonicalize()? == target.canonicalize()? {
        return Err(WorkspaceError::InvalidDocument(
            "This is already the active workspace.".into(),
        ));
    }
    let private = target.join("private/workspace.sqlite3");
    if !private.is_file() {
        return Err(WorkspaceError::InvalidDocument("The destination has an existing library without a private workspace. It has been preserved.".into()));
    }
    {
        let input = Connection::open_with_flags(&private, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let count: i64 = input.query_row("SELECT COUNT(*) FROM documents", [], |r| r.get(0))?;
        let version: i32 = input.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if count != 0 || version > 3 {
            return Err(WorkspaceError::InvalidDocument("The destination already has documents or a newer schema. It cannot be replaced or merged.".into()));
        }
        if version >= 3 {
            let count: i64 =
                input.query_row("SELECT COUNT(*) FROM review_runs", [], |r| r.get(0))?;
            if count != 0 {
                return Err(WorkspaceError::InvalidDocument(
                    "The destination has saved reports and cannot be replaced.".into(),
                ));
            }
        }
    }
    let backup = target.with_file_name(format!(
        "ComplyPH-empty-before-import-{}-{}",
        std::process::id(),
        crate::now()
    ));
    if backup.exists() {
        return Err(WorkspaceError::InvalidDocument(
            "An import backup already exists. Reopen ComplyPH before retrying.".into(),
        ));
    }
    fs::rename(target, &backup)?;
    match import(source, target) {
        Ok(paths) => Ok(paths),
        Err(error) => {
            if let Err(restore) = fs::rename(&backup, target) {
                return Err(WorkspaceError::InvalidDocument(format!(
                    "Import failed: {error}. The previous empty workspace is preserved at {}. Restore could not complete: {restore}",
                    backup.display()
                )));
            }
            Err(error)
        }
    }
}

/// Never merges/replaces an active workspace. Originals, references and legacy databases stay put.
pub fn import(source: &Path, target: &Path) -> Result<WorkspacePaths> {
    if !source.join("private/workspace.sqlite3").is_file() {
        return Err(WorkspaceError::InvalidDocument("Choose the workspace folder containing private/workspace.sqlite3 and library, not an installation folder.".into()));
    }
    if source.canonicalize()?
        == target
            .canonicalize()
            .unwrap_or_else(|_| target.to_path_buf())
        || has_workspace(target)
    {
        return Err(WorkspaceError::InvalidDocument(
            "The destination already contains a workspace. No data was replaced or merged.".into(),
        ));
    }
    if complyph_paths::another_desktop_running()? {
        return Err(WorkspaceError::InvalidDocument(
            "Close all other review-app windows, then reopen ComplyPH to migrate safely.".into(),
        ));
    }
    // Refuse unknown target contents. A previous successful activation always wins.
    if target.exists() && fs::read_dir(target)?.next().is_some() {
        return Err(WorkspaceError::InvalidDocument("The destination folder is not empty. Its contents have been preserved; choose another destination or resolve this folder before migration.".into()));
    }
    let parent = target.parent().ok_or_else(|| {
        WorkspaceError::InvalidDocument("Choose a destination with a parent folder.".into())
    })?;
    fs::create_dir_all(parent)?;
    let stage = parent.join(format!(
        ".ComplyPH-migration-{}-{}",
        std::process::id(),
        crate::now()
    ));
    // A left-over stage is never reused, overwritten or mistaken for an active workspace.
    if stage.exists() {
        return Err(WorkspaceError::InvalidDocument("A migration staging folder already exists. Reopen ComplyPH and try again; the original remains intact.".into()));
    }
    secure_directory(&stage)?;
    let result = (|| {
        for (relative, maximum) in [
            ("private/workspace.sqlite3", 3),
            ("library/catalog.sqlite3", 2),
        ] {
            let original = source.join(relative);
            if !original.is_file() {
                continue;
            }
            let destination = stage.join(relative);
            secure_directory(destination.parent().expect("fixed relative parent"))?;
            let input = Connection::open_with_flags(&original, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
            input.busy_timeout(Duration::from_secs(3))?;
            let version: i32 = input.query_row("PRAGMA user_version", [], |r| r.get(0))?;
            if version > maximum {
                return Err(WorkspaceError::NewerDatabase);
            }
            let integrity: String = input.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
            if integrity != "ok" {
                return Err(WorkspaceError::InvalidDocument(
                    "The existing database failed its integrity check. No data has been replaced."
                        .into(),
                ));
            }
            let before: i64 = input.query_row("PRAGMA data_version", [], |r| r.get(0))?;
            let mut output = Connection::open(&destination)?;
            {
                let backup = rusqlite::backup::Backup::new(&input, &mut output)?;
                let deadline = Instant::now() + Duration::from_secs(30);
                loop {
                    if matches!(backup.step(256)?, rusqlite::backup::StepResult::Done) {
                        break;
                    }
                    if Instant::now() > deadline {
                        return Err(WorkspaceError::InvalidDocument("Migration timed out. Close the old app and try again; its data remains intact.".into()));
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
            let after: i64 = input.query_row("PRAGMA data_version", [], |r| r.get(0))?;
            if before != after {
                return Err(WorkspaceError::InvalidDocument(
                    "The old workspace changed during migration. Close its app and retry.".into(),
                ));
            }
            let integrity: String = output.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
            if integrity != "ok" {
                return Err(WorkspaceError::InvalidDocument(
                    "The copied database failed validation. The original is intact.".into(),
                ));
            }
        }
        let marker = serde_json::json!({"copied_from":source,"copied_at":crate::now(),"original_preserved":true});
        fs::write(
            stage.join("migration.json"),
            serde_json::to_vec_pretty(&marker)?,
        )?;
        if target.exists() {
            fs::remove_dir(target)?;
        } // succeeds only while empty
        fs::rename(&stage, target)?; // same-volume activation; no partially copied active database
        Ok(WorkspacePaths::under(target))
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&stage);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Preferences, Workspace};
    #[test]
    fn copies_committed_wal_preferences_refs_and_keeps_source() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("LaborLens PH");
        let paths = WorkspacePaths::under(&source);
        let workspace = Workspace::open(&paths).unwrap();
        let original = temp.path().join("private-original.pdf");
        fs::write(&original, b"synthetic").unwrap();
        let document = workspace.register_document(&original).unwrap();
        let prefs = Preferences {
            show_full_paths: true,
            ..Preferences::default()
        };
        workspace.save_preferences(&prefs).unwrap();
        let target = temp.path().join("ProgramData/ComplyPH");
        let plan = StoragePlan {
            preferred: target.clone(),
            legacy_candidates: vec![source.clone()],
            machine_wide: true,
        };
        let copied = prepare(&plan).unwrap();
        let restored = Workspace::open(&copied).unwrap();
        assert_eq!(restored.documents().unwrap()[0].id, document.id);
        assert_eq!(
            restored.documents().unwrap()[0].path,
            original.canonicalize().unwrap()
        );
        assert!(restored.preferences().unwrap().show_full_paths);
        assert!(paths.database.is_file());
        assert_eq!(fs::read(original).unwrap(), b"synthetic");
        assert_eq!(prepare(&plan).unwrap().root, target);
        assert!(import(&source, &target).is_err());
    }
    #[test]
    fn ambiguous_corrupt_future_and_nonempty_destinations_are_preserved() {
        let temp = tempfile::tempdir().unwrap();
        let a = temp.path().join("old-a");
        let b = temp.path().join("old-b");
        let target = temp.path().join("new");
        drop(Workspace::open(&WorkspacePaths::under(&a)).unwrap());
        drop(Workspace::open(&WorkspacePaths::under(&b)).unwrap());
        assert!(
            prepare(&StoragePlan {
                preferred: target.clone(),
                legacy_candidates: vec![a.clone(), b],
                machine_wide: true
            })
            .is_err()
        );
        let db = Connection::open(a.join("private/workspace.sqlite3")).unwrap();
        db.pragma_update(None, "user_version", 99).unwrap();
        drop(db);
        assert!(import(&a, &target).is_err());
        assert!(!target.exists());
        fs::write(a.join("private/workspace.sqlite3"), b"corrupt").unwrap();
        assert!(import(&a, &target).is_err());
        assert!(!target.exists());
        fs::create_dir(&target).unwrap();
        fs::write(target.join("keep"), b"keep").unwrap();
        assert!(import(&a, &target).is_err());
        assert_eq!(fs::read(target.join("keep")).unwrap(), b"keep");
    }
    #[test]
    fn version_two_cache_survives_copy_and_explicit_empty_import_has_rollback() {
        let temp = tempfile::tempdir().unwrap();
        let source = WorkspacePaths::under(temp.path().join("old-v03"));
        let original = temp.path().join("synthetic.pdf");
        fs::write(&original, b"synthetic source").unwrap();
        let workspace = Workspace::open(&source).unwrap();
        let document = workspace.register_document(&original).unwrap();
        let snapshot = crate::document_snapshot(&original).unwrap();
        let report = complyph_extract::Report::failure(
            complyph_extract::Format::Pdf,
            complyph_extract::Status::Unreadable,
            "synthetic",
        );
        let cached = workspace
            .save_extraction(&document, &snapshot, report)
            .unwrap();
        drop(workspace);
        let db = Connection::open(&source.database).unwrap();
        db.execute_batch("DROP TABLE review_runs; PRAGMA user_version=2;")
            .unwrap();
        drop(db);
        let before = fs::read(&source.database).unwrap();
        let target = temp.path().join("new");
        drop(Workspace::open(&WorkspacePaths::under(&target)).unwrap());
        let imported = import_into_empty(&source.root, &target).unwrap();
        let workspace = Workspace::open(&imported).unwrap();
        assert_eq!(
            workspace.extraction(document.id).unwrap().unwrap().sha256,
            cached.sha256
        );
        assert_eq!(
            workspace.extraction(document.id).unwrap().unwrap().read_at,
            cached.read_at
        );
        assert_eq!(fs::read(&source.database).unwrap(), before);
        assert!(import_into_empty(&source.root, &target).is_err());
        drop(workspace);
        let empty = temp.path().join("empty");
        drop(Workspace::open(&WorkspacePaths::under(&empty)).unwrap());
        assert!(import_into_empty(&temp.path().join("missing"), &empty).is_err());
        assert!(
            Workspace::open(&WorkspacePaths::under(empty))
                .unwrap()
                .documents()
                .unwrap()
                .is_empty()
        );
    }
}
