//! UI and process coordination only. The desktop has no HTTP client.
use crate::{theme::*, widgets};
use complyph_library::{
    DownloadedChange, LibraryCatalog, SourceMetadata, UpdateWatch, approved_update_url,
};
use eframe::egui::{self, RichText, TextEdit, Ui};
use egui_file_dialog::FileDialog;
use std::{
    collections::HashMap,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    time::{Duration, Instant},
};

pub struct SourceUpdates {
    watches: Vec<UpdateWatch>,
    changes: HashMap<i64, Vec<DownloadedChange>>,
    urls: HashMap<i64, String>,
    notes: HashMap<i64, String>,
    receiver: Option<Receiver<Result<String, String>>>,
    cancel: Arc<AtomicBool>,
    request: Option<bool>,
    next_due_poll: Instant,
    message: Option<String>,
    changed: bool,
    export: FileDialog,
    export_id: Option<i64>,
    compare_request: Option<i64>,
    publication_count: usize,
}
impl Default for SourceUpdates {
    fn default() -> Self {
        Self {
            watches: Vec::new(),
            changes: HashMap::new(),
            urls: HashMap::new(),
            notes: HashMap::new(),
            receiver: None,
            cancel: Arc::new(AtomicBool::new(false)),
            request: None,
            next_due_poll: Instant::now(),
            message: None,
            changed: false,
            export: FileDialog::new()
                .title("Save downloaded source copy")
                .default_size([760.0, 500.0])
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show_devices(false)
                .show_removable_devices(false)
                .show_hidden_option(false)
                .as_modal(true),
            export_id: None,
            compare_request: None,
            publication_count: 0,
        }
    }
}
impl Drop for SourceUpdates {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
fn time(value: Option<i64>) -> String {
    value
        .and_then(|v| chrono::DateTime::from_timestamp(v, 0))
        .map(|d| d.format("%Y-%m-%d %H:%M UTC").to_string())
        .unwrap_or_else(|| "Not checked".into())
}

impl SourceUpdates {
    pub fn take_compare_request(&mut self) -> Option<i64> {
        self.compare_request.take()
    }
    pub fn request_check(&mut self) {
        self.request = Some(false);
    }
    pub fn pending_for(&self, id: i64) -> usize {
        self.changes.get(&id).map_or(0, |changes| {
            changes.iter().filter(|c| c.decision.is_none()).count()
        })
    }
    pub fn load(&mut self, catalog: &LibraryCatalog) -> Result<(), complyph_library::LibraryError> {
        self.watches = catalog.update_watches()?;
        self.publication_count = catalog
            .publication_pages()?
            .iter()
            .filter(|p| p.enabled)
            .count();
        self.changes.clear();
        for source in catalog.sources()? {
            self.changes
                .insert(source.id, catalog.downloaded_changes(source.id)?);
        }
        Ok(())
    }
    pub fn overview(&mut self, ui: &mut Ui) {
        widgets::card().show(ui,|ui|{
            ui.label(RichText::new("Source update checks").font(heading(17.0)));
            let approved=self.watches.iter().filter(|w|w.enabled).count();
            let pending=self.changes.values().flatten().filter(|c|c.decision.is_none()).count();
            widgets::muted(ui,&format!("Approved source URLs: {approved} · Publication pages: {} · Downloads awaiting a decision: {pending}",self.publication_count));
            if cfg!(any(windows,target_os="linux")){widgets::muted(ui,"Only approved public sources and publication pages go online. Private documents stay on this device. Daily checks run while the app is open; missed checks run at the next launch. Unavailable operating-system protection keeps checks offline.");}
            else{widgets::muted(ui,"This platform has no enabled OS-isolated downloader and stays offline.");}
            if self.receiver.is_some(){ui.horizontal(|ui|{ui.spinner();ui.label("Checking approved public sources…");});}
            else if widgets::primary(ui,"Check approved sources now",(approved+self.publication_count)>0 && cfg!(any(windows,target_os="linux"))).on_hover_text("Checks up to 20 approved URLs/pages per batch. Document text is never used.").clicked(){self.request=Some(false);}
            if let Some(message)=&self.message{ui.label(RichText::new(message).color(AMBER));}
        });
        ui.add_space(10.0);
    }
    pub fn source(
        &mut self,
        ui: &mut Ui,
        catalog: &LibraryCatalog,
        id: i64,
        metadata: &SourceMetadata,
    ) {
        egui::CollapsingHeader::new("Source updates and downloaded history").id_salt(("source_updates",id)).default_open(self.pending_for(id)>0).show(ui,|ui|{
            let watch=self.watches.iter().find(|w|w.source_id==id).cloned();
            let suggested=watch.as_ref().map(|w|w.url.clone()).unwrap_or_else(||{
                if approved_update_url(&metadata.source_url).is_ok(){metadata.source_url.clone()}
                else{metadata.official_reference_url.clone().unwrap_or_default()}
            });
            let url=self.urls.entry(id).or_insert(suggested);
            let label=ui.label("Official publication URL to check:");
            ui.add(TextEdit::singleline(url).desired_width(f32::INFINITY)).labelled_by(label.id);
            widgets::muted(ui,"Approval allows HTTPS GET requests to this agency URL. Downloads are compared by hash and saved for review; dates and checking rules are not inferred from a byte change.");
            let busy=self.receiver.is_some();
            let mut action=None;
            ui.horizontal_wrapped(|ui|{
                if widgets::primary(ui,"Approve daily checks",!busy).clicked(){action=Some(true);}
                if watch.as_ref().is_some_and(|w|w.enabled) && widgets::secondary(ui,"Disable checks").clicked(){action=Some(false);}
            });
            if let Some(enabled)=action{
                let result=if enabled{catalog.set_update_watch(id,url,true)}else{catalog.disable_update_watch(id)};
                match result{Ok(())=>{self.changed=true;self.message=Some(if enabled && cfg!(windows){"URL approved. The next check will run while the app is open."}else if enabled{"URL approval saved. Online checks require supported operating-system confinement."}else{"Source checks disabled. Downloaded history is retained."}.into());},Err(e)=>self.message=Some(e.to_string())}
            }
            if let Some(w)=watch{
                ui.label(format!("{} · last attempt: {}",if w.enabled{"Approved"}else{"Disabled"},time(w.last_attempt)));
                ui.label(format!("Last successful check: {} · {}",time(w.last_success),w.last_status));
                if let Some(error)=w.last_error{ui.colored_label(AMBER,error);}
            }
            let changes=self.changes.get(&id).cloned().unwrap_or_default();
            if !changes.is_empty(){
                ui.colored_label(AMBER,"Downloaded changes do not replace the saved reference or activate checking rules. Compare the copies and record your review.");
            }
            for change in changes.iter().take(20){
                widgets::card().show(ui,|ui|{
                    ui.label(RichText::new(format!("Downloaded {} · {} bytes",time(Some(change.downloaded_at)),change.bytes)).font(heading(14.0)));
                    widgets::pill(ui,change.decision.as_deref().unwrap_or("Adoption / dismissal decision needed"),AMBER,AMBER_TINT,false);
                    ui.add(egui::Label::new(format!("SHA-256: {}",change.content_sha256)).wrap().selectable(true));
                    if widgets::primary(ui,"Compare and review candidate",true).clicked(){self.compare_request=Some(change.id);}
                    if widgets::secondary(ui,"Save downloaded copy…").clicked(){
                        let extension=match change.mime.as_str(){"application/pdf"=>"pdf","text/html"=>"html","text/plain"=>"txt",_=>"docx"};
                        self.export=FileDialog::new().title("Save downloaded source copy").default_size([760.0,500.0]).anchor(egui::Align2::CENTER_CENTER,[0.0,0.0]).show_devices(false).show_removable_devices(false).show_hidden_option(false).default_file_name(&format!("source-change-{}.{}",change.id,extension)).as_modal(true);
                        self.export.save_file();self.export_id=Some(change.id);
                    }
                    if let Some(note)=&change.review_note{ui.label(format!("Reviewed {}: {note}",time(change.reviewed_at)));}
                    else{
                        let note=self.notes.entry(change.id).or_default();
                        ui.add(TextEdit::multiline(note).hint_text("Record your comparison and any follow-up needed. This does not certify legal currency.").desired_rows(2).desired_width(f32::INFINITY));
                        if widgets::secondary(ui,"Record source review").clicked(){match catalog.mark_download_reviewed(change.id,note){Ok(())=>self.changed=true,Err(e)=>self.message=Some(e.to_string())}}
                    }
                });
            }
            if changes.len()>20{widgets::muted(ui,"Showing the latest 20 downloads. Earlier versions remain preserved in the catalog.");}
            if let Some(message)=&self.message{ui.colored_label(AMBER,message);}
        });
    }
    pub fn tick(
        &mut self,
        ctx: &egui::Context,
        catalog: &LibraryCatalog,
        directory: &Path,
    ) -> bool {
        self.export.update(ctx);
        if let Some(path) = self.export.take_picked()
            && let Some(id) = self.export_id.take()
        {
            self.message = Some(match catalog.export_downloaded_change(id, &path) {
                Ok(()) => {
                    "Downloaded source copy saved without changing the original reference.".into()
                }
                Err(e) => e.to_string(),
            });
        }
        if let Some(receiver) = &self.receiver {
            match receiver.try_recv() {
                Ok(result) => {
                    self.receiver = None;
                    self.message = Some(result.unwrap_or_else(|e| e));
                    self.changed = true;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.receiver = None;
                    self.message =
                        Some("The source checker stopped. Saved references are intact.".into());
                    self.changed = true;
                }
                Err(mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint_after(Duration::from_millis(150))
                }
            }
        }
        if self.receiver.is_none()
            && cfg!(any(windows, target_os = "linux"))
            && (self.watches.iter().any(|w| w.enabled) || self.publication_count > 0)
        {
            if Instant::now() >= self.next_due_poll {
                self.next_due_poll = Instant::now() + Duration::from_secs(60);
                if catalog
                    .due_source_ids(true)
                    .is_ok_and(|ids| !ids.is_empty())
                    || catalog
                        .due_publication_page_ids(true)
                        .is_ok_and(|ids| !ids.is_empty())
                {
                    self.request.get_or_insert(true);
                }
            }
            ctx.request_repaint_after(Duration::from_secs(60));
        }
        if let Some(due) = self.request.take()
            && self.receiver.is_none()
        {
            let directory = directory.to_owned();
            let ctx = ctx.clone();
            let cancel = self.cancel.clone();
            let (sender, receiver) = mpsc::channel();
            self.receiver = Some(receiver);
            std::thread::spawn(move || {
                let result = run_checker(directory, due, &cancel);
                let _ = sender.send(result);
                ctx.request_repaint();
            });
        }
        let changed = std::mem::take(&mut self.changed);
        if changed && let Err(error) = self.load(catalog) {
            self.message = Some(error.to_string());
        }
        if changed {
            ctx.request_repaint();
        }
        changed
    }
}

fn run_checker(directory: PathBuf, due: bool, cancel: &AtomicBool) -> Result<String, String> {
    let executable = std::env::current_exe()
        .map_err(|_| "The updater could not be located.")?
        .with_file_name(if cfg!(windows) {
            "complyph-updater.exe"
        } else {
            "complyph-updater"
        });
    let mut command = Command::new(executable);
    command
        .arg("--check")
        .arg("--desktop-session")
        .arg("--library-dir")
        .arg(directory);
    if due {
        command.arg("--due");
    }
    command.env_clear();
    // Userenv needs these coordinator-local profile locations for AppContainer
    // setup. They are never fields of a source HTTP request.
    for name in [
        "SystemDrive",
        "SystemRoot",
        "WINDIR",
        "TEMP",
        "TMP",
        "USERPROFILE",
        "LOCALAPPDATA",
        "APPDATA",
    ] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::process::CommandExt;
        let parent = std::process::id() as i32;
        // SAFETY: only async-signal-safe syscalls run after fork. Check the
        // parent again to close the race between parent exit and prctl.
        unsafe {
            command.pre_exec(move || {
                if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) != 0
                    || libc::getppid() != parent
                {
                    return Err(std::io::Error::other(
                        "The desktop lifetime guard could not be established.",
                    ));
                }
                Ok(())
            });
        }
    }
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .stdin(Stdio::piped())
        .spawn()
        .map_err(
            |_| "The source updater could not start. Install the complete ComplyPH package.",
        )?;
    #[cfg(windows)]
    let _lifetime_job = match lifetime_job(&child) {
        Ok(job) => job,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
    };
    {
        use std::io::Write;
        if child
            .stdin
            .take()
            .expect("piped handshake")
            .write_all(&[1])
            .is_err()
        {
            let _ = child.kill();
            let _ = child.wait();
            return Err("The updater session could not be started.".into());
        }
    }
    let output = child.stdout.take().expect("piped output");
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        output
            .take(512 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map(|_| bytes)
    });
    let start = Instant::now();
    let status = loop {
        if cancel.load(Ordering::Relaxed) || start.elapsed() > Duration::from_secs(650) {
            let _ = child.kill();
            let _ = child.wait();
            let _ = reader.join();
            return Err("Source checking stopped. Earlier references and completed downloads are preserved.".into());
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = reader.join();
                return Err("The source checker could not be monitored.".into());
            }
        }
    };
    let bytes = reader
        .join()
        .map_err(|_| "The source-check result could not be collected.")?
        .map_err(|_| "The source-check result could not be read.")?;
    if !status.success() || bytes.len() > 512 * 1024 {
        return Err(
            "The source checker could not complete. Check the source status and try again.".into(),
        );
    }
    let report: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|_| "The source checker returned an invalid result.")?;
    let checks = report["checks"]
        .as_array()
        .ok_or("The source-check report is incomplete.")?;
    let count = |name| {
        checks
            .iter()
            .filter(|c| c["outcome"].as_str() == Some(name))
            .count()
    };
    let discovery = report["discovery_checks"]
        .as_array()
        .ok_or("The publication discovery report is incomplete.")?;
    let discovered = discovery
        .iter()
        .filter_map(|r| r["new_links"].as_u64())
        .sum::<u64>();
    let discovery_failed = discovery
        .iter()
        .filter(|r| r["outcome"].as_str() == Some("failed"))
        .count();
    Ok(format!(
        "{} sources checked: {} changed · {} unchanged · {} failed. {} publication pages scanned; {discovered} newly seen citations, {discovery_failed} failed scans. Review changes and discovered citations in Library maintenance.",
        checks.len(),
        count("changed"),
        count("unchanged"),
        count("failed"),
        discovery.len()
    ))
}

#[cfg(windows)]
fn lifetime_job(child: &std::process::Child) -> Result<std::os::windows::io::OwnedHandle, String> {
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use windows_sys::Win32::System::JobObjects::*;
    // The OS closes this handle when the desktop exits, including abrupt exits.
    let raw = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
    if raw.is_null() {
        return Err("The updater lifetime guard could not be created.".into());
    }
    let job = unsafe { OwnedHandle::from_raw_handle(raw) };
    let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    if unsafe {
        SetInformationJobObject(
            raw,
            JobObjectExtendedLimitInformation,
            (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            std::mem::size_of_val(&limits) as u32,
        )
    } == 0
        || unsafe { AssignProcessToJobObject(raw, child.as_raw_handle()) } == 0
    {
        return Err("The updater could not be bound to the desktop's lifetime.".into());
    }
    Ok(job)
}
