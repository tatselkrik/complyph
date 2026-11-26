use crate::library_view::LibraryView;
use crate::theme::*;
use crate::widgets::{self, Icon};
use chrono::{DateTime, Local};
use complyph_core::{Document, Preferences, Workspace, WorkspaceError, WorkspacePaths};
use complyph_library::{LibraryCatalog, LibrarySummary};
use eframe::egui::{
    self, Align, Align2, FontId, Frame, Layout, Margin, Rect, RichText, ScrollArea, Sense, Stroke,
    StrokeKind, Ui, UiBuilder, pos2, vec2,
};
use egui_file_dialog::{FileDialog, Filter};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Page {
    Review,
    Documents,
    Library,
    Reports,
    Settings,
}

impl Page {
    const ALL: [Self; 5] = [
        Self::Review,
        Self::Documents,
        Self::Library,
        Self::Reports,
        Self::Settings,
    ];
    fn key(self) -> &'static str {
        match self {
            Self::Review => "review",
            Self::Documents => "documents",
            Self::Library => "library",
            Self::Reports => "reports",
            Self::Settings => "settings",
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Review => "Review",
            Self::Documents => "Documents",
            Self::Library => "Legal library",
            Self::Reports => "Reports",
            Self::Settings => "Settings",
        }
    }
    fn icon(self) -> Icon {
        match self {
            Self::Review => Icon::Review,
            Self::Documents => Icon::Document,
            Self::Library => Icon::Library,
            Self::Reports => Icon::Reports,
            Self::Settings => Icon::Settings,
        }
    }
    fn parse(key: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|p| p.key() == key)
            .unwrap_or(Self::Review)
    }
}

enum Message {
    Imported { ids: Vec<i64>, errors: Vec<String> },
    Failed(String),
}

struct Notice {
    text: String,
    error: bool,
}

pub struct ComplyApp {
    paths: Option<WorkspacePaths>,
    workspace: Option<Workspace>,
    catalog: Option<LibraryCatalog>,
    library: LibrarySummary,
    library_error: Option<String>,
    library_view: LibraryView,
    reader_view: crate::reader_view::ReaderView,
    check_view: crate::check_view::CheckView,
    review_tools: crate::review_tools::ReviewTools,
    document_versions: crate::document_versions::DocumentVersions,
    folder_watch: crate::folder_watch::FolderWatch,
    provision_view: crate::provision_view::ProvisionView,
    workspace_picker: FileDialog,
    import_source: Option<PathBuf>,
    startup_error: Option<String>,
    preferences: Preferences,
    page: Page,
    documents: Vec<Document>,
    selected: Option<i64>,
    search: String,
    sender: Sender<Message>,
    receiver: Receiver<Message>,
    choosing: bool,
    notice: Option<Notice>,
    pending_remove: Option<i64>,
    file_dialog: FileDialog,
}

impl ComplyApp {
    pub fn new(paths: Result<WorkspacePaths, WorkspaceError>) -> Self {
        let (sender, receiver) = mpsc::channel();
        let mut app = Self {
            paths: None,
            workspace: None,
            catalog: None,
            library: LibrarySummary::default(),
            library_error: None,
            library_view: LibraryView::default(),
            reader_view: crate::reader_view::ReaderView::default(),
            check_view: crate::check_view::CheckView::default(),
            review_tools: crate::review_tools::ReviewTools::default(),
            document_versions: crate::document_versions::DocumentVersions::default(),
            folder_watch: crate::folder_watch::FolderWatch::default(),
            provision_view: crate::provision_view::ProvisionView::default(),
            workspace_picker: FileDialog::new()
                .title("Choose existing workspace folder")
                .as_modal(true),
            import_source: None,
            startup_error: None,
            preferences: Preferences::default(),
            page: Page::Review,
            documents: Vec::new(),
            selected: None,
            search: String::new(),
            sender,
            receiver,
            choosing: false,
            notice: None,
            pending_remove: None,
            file_dialog: FileDialog::new()
                .title("Choose documents")
                .add_file_filter(
                    "PDF and Word documents",
                    Filter::new(|path: &std::path::Path| {
                        complyph_core::DocumentKind::from_path(path).is_some()
                    }),
                )
                .default_file_filter("PDF and Word documents")
                .default_size([760.0, 500.0])
                .show_new_folder_button(false)
                .show_pinned_folders(false)
                .show_devices(false)
                .show_removable_devices(false)
                .show_hidden_option(false)
                .show_system_files_option(false)
                .show_all_files_filter(false)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .as_modal(true),
        };
        match paths.and_then(|paths| Workspace::open(&paths).map(|workspace| (paths, workspace))) {
            Ok((paths, workspace)) => {
                match workspace.preferences() {
                    Ok(preferences) => app.preferences = preferences,
                    Err(error) => app.startup_error = Some(error.to_string()),
                }
                match workspace.documents() {
                    Ok(documents) => app.documents = documents,
                    Err(error) => app.startup_error = Some(error.to_string()),
                }
                match LibraryCatalog::open(&paths.library_dir) {
                    Ok(catalog) => {
                        if let Err(error) = app.library_view.load(&catalog) {
                            app.library_error = Some(error.to_string());
                        }
                        match catalog.summary() {
                            Ok(summary) => app.library = summary,
                            Err(error) => app.library_error = Some(error.to_string()),
                        }
                        app.catalog = Some(catalog);
                    }
                    Err(error) => app.library_error = Some(error.to_string()),
                }
                app.paths = Some(paths);
                app.workspace = Some(workspace);
                if app.preferences.remember_page {
                    app.page = Page::parse(&app.preferences.last_page);
                }
            }
            Err(error) => app.startup_error = Some(error.to_string()),
        }
        app
    }

    fn navigate(&mut self, page: Page) {
        self.page = page;
        self.preferences.last_page = page.key().to_owned();
        self.save_preferences();
    }

    fn save_preferences(&mut self) {
        if self.startup_error.is_some() {
            return;
        }
        if let Some(workspace) = &self.workspace
            && let Err(error) = workspace.save_preferences(&self.preferences)
        {
            self.notice = Some(Notice {
                text: error.to_string(),
                error: true,
            });
        }
    }

    fn choose_documents(&mut self, ctx: &egui::Context) {
        if self.choosing || self.workspace.is_none() || self.startup_error.is_some() {
            return;
        }
        self.file_dialog.pick_multiple();
        ctx.request_repaint();
    }

    fn accept_files(&mut self, files: Vec<PathBuf>, ctx: &egui::Context) {
        if files.is_empty() || self.choosing || self.startup_error.is_some() {
            return;
        }
        let Some(paths) = self.paths.clone() else {
            return;
        };
        self.choosing = true;
        let sender = self.sender.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            import_files(paths, files, &sender);
            ctx.request_repaint();
        });
    }

    fn try_sample(&mut self, name: &str, ctx: &egui::Context) {
        let packaged = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|path| path.join("Samples").join(name)));
        #[cfg(debug_assertions)]
        let packaged = packaged.filter(|path| path.is_file()).or_else(|| {
            Some(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../examples")
                    .join(name),
            )
        });
        if let Some(path) = packaged.filter(|path| path.is_file()) {
            self.accept_files(vec![path], ctx);
        } else {
            self.notice = Some(Notice { text: "The synthetic Samples folder is missing. Reinstall the app or choose a PDF/DOCX file.".into(), error: true });
        }
    }

    fn select_document(&mut self, id: i64, ctx: &egui::Context) {
        self.selected = Some(id);
        self.check_view.clear();
        if let Some(workspace) = &self.workspace
            && let Ok(reviews) = workspace.reviews()
            && let Some(review) = reviews.into_iter().find(|r| r.document_id == id)
        {
            self.check_view.set_review(review);
        }
        if let Some(paths) = self.paths.clone()
            && let Some(document) = self
                .documents
                .iter()
                .find(|document| document.id == id)
                .cloned()
        {
            self.reader_view.start(paths, document, false, ctx);
        }
    }

    fn open_saved_review(&mut self, review: complyph_core::SavedReview, ctx: &egui::Context) {
        let reading = self
            .workspace
            .as_ref()
            .map(|w| w.reading_for_review(review.id));
        match reading {
            Some(Ok(Some(reading))) => {
                if reading.saved.sha256 != review.review.document_sha256
                    || reading.document_id != review.document_id
                {
                    self.notice = Some(Notice {
                        text: "The saved review changed. Refresh Reports before reopening it."
                            .into(),
                        error: true,
                    });
                    return;
                }
                self.selected = Some(review.document_id);
                self.reader_view.open_retained(reading.saved);
                self.check_view.set_review(review);
                self.notice=Some(Notice{text:"Showing the exact retained reading used by this saved review. Choose Review current file to prepare a new assessment.".into(),error:false});
            }
            Some(Ok(None)) => {
                self.select_document(review.document_id, ctx);
                self.check_view.set_review(review);
                self.notice=Some(Notice{text:"This earlier review did not record its complete reading. The current original is opened if available; passage navigation requires its matching file hash and evidence. No old text is reconstructed from a changed file.".into(),error:false});
            }
            Some(Err(error)) => {
                self.notice = Some(Notice {
                    text: error.to_string(),
                    error: true,
                })
            }
            None => return,
        }
        self.navigate(Page::Review);
    }

    fn messages(&mut self, ctx: &egui::Context) {
        while let Ok(message) = self.receiver.try_recv() {
            self.choosing = false;
            match message {
                Message::Imported { ids, errors } => {
                    if let Some(workspace) = &self.workspace {
                        match workspace.documents() {
                            Ok(documents) => self.documents = documents,
                            Err(error) => {
                                self.notice = Some(Notice {
                                    text: error.to_string(),
                                    error: true,
                                })
                            }
                        }
                    }
                    if let Some(id) = ids.last() {
                        self.select_document(*id, ctx);
                        self.navigate(Page::Review);
                    }
                    let count = ids.len();
                    let text = if errors.is_empty() {
                        format!(
                            "{} document{} saved to your local list. Original files are unchanged.",
                            count,
                            if count == 1 { "" } else { "s" }
                        )
                    } else {
                        format!(
                            "{} document{} added. {}",
                            count,
                            if count == 1 { "" } else { "s" },
                            errors.join(" ")
                        )
                    };
                    self.notice = Some(Notice {
                        text,
                        error: !errors.is_empty(),
                    });
                }
                Message::Failed(text) => self.notice = Some(Notice { text, error: true }),
            }
        }
    }

    fn sidebar(&mut self, ui: &mut Ui, rect: Rect) {
        ui.painter().rect_filled(rect, 0, NAVY_DEEP);
        let brand = Rect::from_min_size(rect.min + vec2(22.0, 28.0), vec2(38.0, 38.0));
        widgets::logo(ui.painter(), brand, CREAM);
        ui.painter().text(
            pos2(rect.left() + 73.0, rect.top() + 36.0),
            Align2::LEFT_CENTER,
            "ComplyPH",
            heading(24.0),
            IVORY,
        );
        ui.painter().text(
            pos2(rect.left() + 74.0, rect.top() + 59.0),
            Align2::LEFT_CENTER,
            "P H I L I P P I N E S",
            FontId::proportional(9.5),
            NAV_MUTED,
        );

        let nav = Rect::from_min_max(
            rect.min + vec2(14.0, 128.0),
            pos2(rect.right() - 14.0, rect.bottom() - 200.0),
        );
        ui.scope_builder(UiBuilder::new().max_rect(nav), |ui| {
            ui.add_space(2.0);
            ui.label(
                RichText::new("    WORKSPACE")
                    .font(heading(10.5))
                    .color(NAV_MUTED),
            );
            ui.add_space(8.0);
            for page in Page::ALL {
                let count = if page == Page::Documents {
                    Some(self.documents.len())
                } else {
                    None
                };
                if widgets::navigation(ui, page.label(), page.icon(), self.page == page, count)
                    .clicked()
                {
                    self.navigate(page);
                }
                ui.add_space(3.0);
            }
        });
        let privacy_rect = Rect::from_min_max(
            pos2(rect.left() + 18.0, rect.bottom() - 172.0),
            pos2(rect.right() - 18.0, rect.bottom() - 62.0),
        );
        ui.painter().rect_filled(privacy_rect, 10, NAVY);
        widgets::paint_icon(
            ui.painter(),
            Rect::from_min_size(privacy_rect.min + vec2(14.0, 15.0), vec2(20.0, 20.0)),
            Icon::Lock,
            CREAM,
        );
        ui.painter().text(
            privacy_rect.min + vec2(14.0, 53.0),
            Align2::LEFT_CENTER,
            "Private by design",
            heading(13.0),
            CREAM,
        );
        ui.painter().text(
            privacy_rect.min + vec2(14.0, 76.0),
            Align2::LEFT_CENTER,
            "Your documents stay here.",
            FontId::proportional(11.5),
            NAV_MUTED,
        );
        ui.painter().text(
            pos2(rect.left() + 25.0, rect.bottom() - 30.0),
            Align2::LEFT_CENTER,
            concat!("REVIEW TRACKING / ", env!("CARGO_PKG_VERSION")),
            FontId::proportional(10.0),
            NAV_MUTED,
        );
    }

    fn topbar(&self, ui: &mut Ui, rect: Rect) {
        ui.painter().rect_filled(rect, 0, IVORY);
        ui.painter().line_segment(
            [rect.left_bottom(), rect.right_bottom()],
            Stroke::new(1.0, BORDER),
        );
        ui.painter().text(
            pos2(rect.left() + 32.0, rect.center().y),
            Align2::LEFT_CENTER,
            format!("Workspace   /   {}", self.page.label()),
            FontId::proportional(13.0),
            MUTED,
        );
        let pill_rect = Rect::from_min_size(
            pos2(rect.right() - 181.0, rect.center().y - 15.0),
            vec2(150.0, 30.0),
        );
        ui.scope_builder(UiBuilder::new().max_rect(pill_rect), |ui| {
            widgets::pill(ui, "Local workspace", GREEN, GREEN_TINT, true);
        });
    }

    fn page_header(&self, ui: &mut Ui, eyebrow: &str, title: &str, subtitle: &str) {
        widgets::eyebrow(ui, eyebrow);
        ui.add_space(2.0);
        widgets::title(ui, title);
        ui.add_space(1.0);
        widgets::muted(ui, subtitle);
        ui.add_space(20.0);
    }

    fn stats(&self, ui: &mut Ui) {
        ui.columns(3, |columns| {
            for (index, label, value, description) in [
                (
                    0,
                    "DOCUMENTS",
                    self.documents.len().to_string(),
                    "In your local workspace",
                ),
                (
                    1,
                    "LEGAL SOURCES",
                    self.library.source_count.to_string(),
                    "Preserved references saved locally",
                ),
                (
                    2,
                    "DOCUMENT STORAGE",
                    "On this device".to_owned(),
                    "Original files stay in place",
                ),
            ] {
                widgets::card()
                    .inner_margin(Margin::same(18))
                    .show(&mut columns[index], |ui| {
                        ui.set_min_width(ui.available_width());
                        widgets::eyebrow(ui, label);
                        ui.add_space(2.0);
                        ui.label(RichText::new(value).font(heading(if index == 2 {
                            23.0
                        } else {
                            29.0
                        })));
                        ui.label(RichText::new(description).size(12.0).color(MUTED));
                    });
            }
        });
    }

    fn chooser_card(&mut self, ui: &mut Ui) {
        let hovering = ui.ctx().input(|i| !i.raw.hovered_files.is_empty());
        let stroke = if hovering {
            Stroke::new(2.0, NAVY)
        } else {
            Stroke::new(1.0, BORDER)
        };
        widgets::card().stroke(stroke).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.set_min_height(252.0);
            ui.with_layout(Layout::top_down(Align::Center), |ui| {
                ui.add_space(6.0);
                let (rect, _) = ui.allocate_exact_size(vec2(60.0, 60.0), Sense::hover());
                ui.painter().circle_filled(rect.center(), 30.0, CREAM);
                widgets::paint_icon(ui.painter(), rect.shrink(13.0), Icon::Document, NAVY);
                ui.add_space(10.0);
                ui.label(RichText::new("Bring a document into focus").font(heading(21.0)));
                ui.label(
                    RichText::new("Choose a file, or drop it into this window.")
                        .color(MUTED)
                        .size(14.0),
                );
                ui.add_space(8.0);
                let label = if self.choosing {
                    "Adding documents..."
                } else {
                    "Choose document"
                };
                if widgets::primary(ui, label, !self.choosing).clicked() {
                    self.choose_documents(ui.ctx());
                }
                ui.add_space(4.0);
                ui.label(
                    RichText::new("PDF  ·  DOCX")
                        .font(heading(11.0))
                        .color(MUTED),
                );
                ui.label(
                    RichText::new("Originals are kept exactly where they are.")
                        .size(11.5)
                        .color(MUTED),
                );
                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    widgets::muted(ui, "Try a synthetic sample:");
                    for (label, name) in [
                        ("PDF", "Synthetic employment.pdf"),
                        ("Word", "Synthetic workplace policy.docx"),
                        ("Scan", "Synthetic scan.pdf"),
                    ] {
                        if ui
                            .add_enabled(!self.choosing, egui::Button::new(label).small())
                            .clicked()
                        {
                            self.try_sample(name, ui.ctx());
                        }
                    }
                });
            });
        });
    }

    fn workflow_card(&self, ui: &mut Ui) {
        widgets::card().show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.set_min_height(252.0);
            ui.label(RichText::new("Your review workflow").font(heading(18.0)));
            ui.add_space(10.0);
            for (number, title, subtitle, available) in [
                ("01", "Read your documents", "Local PDF and Word text", true),
                (
                    "02",
                    "Browse legal references",
                    "19 saved references · 7 domains",
                    true,
                ),
                (
                    "03",
                    "Review with citations",
                    "20 focused checks · 7 domains",
                    true,
                ),
            ] {
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(vec2(30.0, 30.0), Sense::hover());
                    ui.painter()
                        .rect_filled(rect, 7, if available { NAVY } else { CREAM });
                    ui.painter().text(
                        rect.center(),
                        Align2::CENTER_CENTER,
                        number,
                        heading(11.0),
                        if available { CREAM } else { MUTED },
                    );
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 3.0;
                        ui.label(RichText::new(title).font(heading(13.5)));
                        ui.label(RichText::new(subtitle).size(11.5).color(MUTED));
                    });
                });
                ui.add_space(6.0);
            }
            ui.add_space(4.0);
            widgets::pill(
                ui,
                "Evidence stays on this computer",
                GREEN,
                GREEN_TINT,
                false,
            );
        });
    }

    fn review(&mut self, ui: &mut Ui) {
        if let Some(document) = self
            .selected
            .and_then(|id| self.documents.iter().find(|d| d.id == id))
            .cloned()
        {
            self.document_details(ui, &document);
            return;
        }
        self.page_header(
            ui,
            "DOCUMENT REVIEW",
            "Clarity starts with a document.",
            "A private space for your contracts, policies, and handbooks.",
        );
        if self.selected.is_none() {
            self.stats(ui);
            ui.add_space(20.0);
        }
        let selected = self
            .selected
            .and_then(|id| self.documents.iter().find(|d| d.id == id))
            .cloned();
        if let Some(document) = selected {
            self.document_details(ui, &document);
        } else if ui.available_width() >= 790.0 {
            let width = ui.available_width();
            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(
                    vec2(width * 0.63 - 8.0, 300.0),
                    Layout::top_down(Align::Min),
                    |ui| self.chooser_card(ui),
                );
                ui.allocate_ui_with_layout(
                    vec2(width * 0.37 - 8.0, 300.0),
                    Layout::top_down(Align::Min),
                    |ui| self.workflow_card(ui),
                );
            });
        } else {
            self.chooser_card(ui);
            ui.add_space(14.0);
            self.workflow_card(ui);
        }
        ui.add_space(20.0);
        widgets::card().show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new("Recent documents").font(heading(18.0)));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .link(RichText::new("View all").font(heading(12.5)))
                        .clicked()
                    {
                        self.navigate(Page::Documents);
                    }
                });
            });
            ui.add_space(10.0);
            if self.documents.is_empty() {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    widgets::icon(ui, Icon::Document, 22.0, MUTED);
                    widgets::muted(ui, "Your documents will appear here once you add them.");
                });
                ui.add_space(8.0);
            } else {
                let counts = self.workspace.as_ref().and_then(|w| w.review_counts().ok());
                for document in self.documents.iter().take(3).cloned().collect::<Vec<_>>() {
                    if document_row(
                        ui,
                        &document,
                        self.selected == Some(document.id),
                        false,
                        counts
                            .as_ref()
                            .map(|c| c.get(&document.id).copied().unwrap_or(0)),
                    )
                    .clicked()
                    {
                        self.select_document(document.id, ui.ctx());
                    }
                }
            }
        });
    }

    fn document_details(&mut self, ui: &mut Ui, document: &Document) {
        widgets::card().show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                widgets::icon(ui, Icon::Document, 22.0, NAVY);
                ui.label(RichText::new(&document.name).font(heading(19.0)));
                widgets::pill(ui, "Connected review · local", GREEN, GREEN_TINT, false);
            });
            ui.horizontal_wrapped(|ui| {
                if widgets::primary(
                    ui,
                    "Check document",
                    self.reader_view
                        .current()
                        .is_some_and(|s| s.report.status == complyph_extract::Status::Ready)
                        && !self.reader_view.is_retained()
                        && self.catalog.is_some()
                        && self.library_error.is_none(),
                )
                .clicked()
                {
                    self.check_view.edit_scope();
                }
                if widgets::secondary(ui, "Choose another").clicked() {
                    self.choose_documents(ui.ctx());
                }
                if ui.button("Version history / compare…").clicked()
                    && let Some(workspace) = &self.workspace
                {
                    self.document_versions.open(workspace, document.id);
                }
                if self.reader_view.is_retained() && ui.button("Review current file").clicked() {
                    self.select_document(document.id, ui.ctx());
                    self.check_view.edit_scope();
                }
                if ui.button("Back to overview").clicked() {
                    self.selected = None;
                    self.reader_view.clear();
                    self.check_view.clear();
                }
                if ui.button("Remove entry").clicked() {
                    self.pending_remove = Some(document.id);
                }
            });
            if self.preferences.show_full_paths {
                ui.add(
                    egui::Label::new(RichText::new(document.path.display().to_string()).small())
                        .wrap()
                        .selectable(true),
                );
            }
        });
        if self.selected.is_none() {
            return;
        }
        ui.add_space(12.0);
        let pane_height = ui.available_height().max(240.0);
        self.check_view
            .historical_reading(self.reader_view.is_retained());
        let mut reread = false;
        let mut action = None;
        ui.columns(2, |columns| {
            ScrollArea::vertical()
                .id_salt(("document-pane", document.id))
                .max_height(pane_height)
                .auto_shrink([false, false])
                .show(&mut columns[0], |ui| {
                    reread = self.reader_view.show(ui, pane_height);
                });
            ScrollArea::vertical()
                .id_salt(("findings-pane", document.id))
                .max_height(pane_height)
                .auto_shrink([false, false])
                .show(&mut columns[1], |ui| {
                    if let (Some(workspace), Some(catalog)) = (&self.workspace, &self.catalog) {
                        action = self.check_view.show(
                            ui,
                            workspace,
                            catalog,
                            document,
                            self.reader_view.current(),
                            &mut self.review_tools,
                        );
                    } else {
                        widgets::muted(
                            ui,
                            "Resolve the local library/workspace error before reviewing.",
                        );
                    }
                });
        });
        if let Some(action) = action {
            self.review_action(action);
        }
        if self.reader_view.take_ocr_request()
            && let Some(paths) = self.paths.clone()
        {
            self.check_view.clear();
            self.reader_view
                .start_ocr(paths, document.clone(), ui.ctx());
        } else if reread && let Some(paths) = self.paths.clone() {
            self.check_view.clear();
            self.reader_view
                .start(paths, document.clone(), true, ui.ctx());
        }
    }
    fn review_action(&mut self, action: crate::check_view::ReviewAction) {
        match action {
            crate::check_view::ReviewAction::Passage { hash, evidence } => {
                self.reader_view.jump_to(&hash, &evidence)
            }
            crate::check_view::ReviewAction::Provision { review, finding } => {
                if let Some(catalog) = &self.catalog {
                    self.provision_view.open(catalog, &review, finding);
                } else {
                    self.notice = Some(Notice {
                        text: "The local library is unavailable. No online request was made."
                            .into(),
                        error: true,
                    });
                }
            }
        }
    }

    fn documents_page(&mut self, ui: &mut Ui) {
        self.page_header(
            ui,
            "LOCAL FILES",
            "Your documents",
            "An organized workspace, with your original files kept in place.",
        );
        ui.horizontal(|ui| {
            widgets::icon(ui, Icon::Search, 18.0, MUTED);
            let width = (ui.available_width() - 205.0).max(170.0);
            ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .hint_text("Search by filename...")
                    .desired_width(width)
                    .margin(vec2(10.0, 10.0)),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if widgets::primary(ui, "Add documents", !self.choosing).clicked() {
                    self.choose_documents(ui.ctx());
                }
            });
        });
        ui.add_space(14.0);
        let query = self.search.to_lowercase();
        let filtered = self
            .documents
            .iter()
            .filter(|d| d.name.to_lowercase().contains(&query))
            .cloned()
            .collect::<Vec<_>>();
        ui.label(
            RichText::new(format!(
                "{} document{}",
                filtered.len(),
                if filtered.len() == 1 { "" } else { "s" }
            ))
            .size(12.0)
            .color(MUTED),
        );
        ui.add_space(6.0);
        if self.documents.is_empty() {
            self.chooser_card(ui);
            return;
        }
        widgets::card()
            .inner_margin(Margin::same(10))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                if filtered.is_empty() {
                    ui.add_space(20.0);
                    ui.vertical_centered(|ui| {
                        ui.label(RichText::new("No matching documents").font(heading(20.0)));
                        widgets::muted(ui, "Try another filename or clear your search.");
                        if widgets::secondary(ui, "Clear search").clicked() {
                            self.search.clear();
                        }
                    });
                    ui.add_space(20.0);
                }
                let counts = self.workspace.as_ref().and_then(|w| w.review_counts().ok());
                for document in filtered {
                    if document_row(
                        ui,
                        &document,
                        self.selected == Some(document.id),
                        self.preferences.show_full_paths,
                        counts
                            .as_ref()
                            .map(|c| c.get(&document.id).copied().unwrap_or(0)),
                    )
                    .clicked()
                    {
                        self.select_document(document.id, ui.ctx());
                        self.navigate(Page::Review);
                    }
                }
            });
    }

    fn library_page(&mut self, ui: &mut Ui) {
        self.page_header(
            ui,
            "OFFICIAL REFERENCE",
            "A library you can trace.",
            "Browse saved official publications across Philippine compliance domains.",
        );
        if let Some(error) = &self.library_error {
            widgets::card().show(ui, |ui| {
                ui.colored_label(RED, error);
            });
            return;
        }
        if let Some(catalog) = &self.catalog {
            self.library_view.show(ui, catalog, &self.library);
        }
    }

    fn reports_page(&mut self, ui: &mut Ui) {
        self.page_header(ui,"REVIEW HISTORY","Your findings, kept together.","Open a version's saved review, retain its notes and statuses, compare revisions, or export a local PDF report.");
        let mut open = None;
        let mut action = None;
        let mut history = None;
        if let Some(workspace) = &self.workspace {
            match workspace.reviews() {
                Ok(reviews) if reviews.is_empty() => self.empty_state(
                    ui,
                    Icon::Reports,
                    "Your first report will appear here.",
                    "Open a document in Review, confirm scope and run the focused labour checks.",
                ),
                Ok(reviews) => {
                    for saved in reviews {
                        let name = self
                            .documents
                            .iter()
                            .find(|d| d.id == saved.document_id)
                            .map(|d| d.name.as_str())
                            .unwrap_or("Document");
                        egui::CollapsingHeader::new(format!(
                            "{} · {} · Review #{} · File {}",
                            name,
                            format_date(saved.created_at),
                            saved.id,
                            saved
                                .review
                                .document_sha256
                                .get(..12)
                                .unwrap_or(&saved.review.document_sha256)
                        ))
                        .id_salt(("report", saved.id))
                        .show(ui, |ui| {
                            if ui.button("Open connected review").clicked() {
                                open = Some(saved.clone());
                            }
                            if ui.button("Version history / compare…").clicked() {
                                history = Some(saved.document_id);
                            }
                            if let Some(event) = crate::check_view::show_review(
                                ui,
                                &saved,
                                None,
                                workspace,
                                &mut self.review_tools,
                            ) {
                                action = Some(event);
                            }
                        });
                    }
                }
                Err(error) => {
                    ui.colored_label(RED, error.to_string());
                }
            }
        }
        if let Some(action) = action {
            self.review_action(action);
        }
        if let Some(review) = open {
            self.open_saved_review(review, ui.ctx());
        }
        if let Some(id) = history
            && let Some(workspace) = &self.workspace
        {
            self.document_versions.open(workspace, id);
        }
    }

    fn empty_state(&self, ui: &mut Ui, icon: Icon, title: &str, text: &str) {
        widgets::card().show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.add_space(32.0);
            ui.vertical_centered(|ui| {
                let (rect, _) = ui.allocate_exact_size(vec2(66.0, 66.0), Sense::hover());
                ui.painter().circle_filled(rect.center(), 33.0, CREAM);
                widgets::paint_icon(ui.painter(), rect.shrink(16.0), icon, NAVY);
                ui.add_space(12.0);
                ui.label(RichText::new(title).font(heading(23.0)));
                ui.add_space(3.0);
                ui.label(RichText::new(text).size(14.0).color(MUTED));
            });
            ui.add_space(36.0);
        });
    }

    fn settings_page(&mut self, ui: &mut Ui) {
        self.page_header(
            ui,
            "PREFERENCES",
            "Make yourself at home.",
            "A few thoughtful settings for this local workspace.",
        );
        let mut changed = false;
        widgets::card().show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(RichText::new("Workspace preferences").font(heading(19.0)));
            ui.add_space(12.0);
            changed |= ui
                .checkbox(
                    &mut self.preferences.remember_page,
                    "Remember where I left off",
                )
                .changed();
            widgets::muted(ui, "Open the last section you used when ComplyPH starts.");
            ui.add_space(12.0);
            ui.separator();
            ui.add_space(12.0);
            changed |= ui
                .checkbox(
                    &mut self.preferences.show_full_paths,
                    "Show full document paths",
                )
                .changed();
            widgets::muted(
                ui,
                "Display the original file location in document details and lists.",
            );
        });
        if changed {
            self.save_preferences();
        }
        ui.add_space(18.0);
        if let Some(workspace) = &self.workspace {
            self.folder_watch.show(ui, workspace);
        }
        ui.add_space(18.0);
        widgets::card().show(ui,|ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| { widgets::icon(ui,Icon::Lock,22.0,NAVY); ui.label(RichText::new("Stored on this computer").font(heading(19.0))); });
            ui.add_space(8.0);
            widgets::muted(ui,"The workspace stores document references, extracted text, preferences and saved findings locally. Original files stay in place; no document content is uploaded.");
            if widgets::secondary(ui,"Import an existing workspace…").clicked() {self.workspace_picker.pick_directory();}
            widgets::muted(ui,"For a workspace stored elsewhere: choose the folder containing private and library. Import only into an empty workspace; originals and old databases are preserved.");
            if let Some(paths) = &self.paths {
                ui.add_space(10.0);
                widgets::eyebrow(ui,"WORKSPACE LOCATION");
                ui.add(egui::Label::new(RichText::new(paths.root.display().to_string()).size(12.0).color(MUTED)).wrap().selectable(true));
                if widgets::secondary(ui,"Copy location").clicked() {
                    ui.ctx().copy_text(paths.root.display().to_string());
                    self.notice=Some(Notice {text:"Workspace location copied.".into(),error:false});
                }
            }
        });
        ui.add_space(18.0);
        widgets::card().show(ui,|ui| {
            ui.set_min_width(ui.available_width());
            ui.label(RichText::new("ComplyPH").font(heading(20.0)));
            widgets::muted(ui,"A private workspace for Philippine compliance document review.");
            ui.label(RichText::new(concat!("Version ", env!("CARGO_PKG_VERSION"), " · Reviewer notes and PDF reports")).size(12.5).color(MUTED));
            ui.add_space(8.0);
            ui.label(RichText::new("Twenty source-cited checks across seven domains are available. Each requires confirmed applicability and evidence. Approved source URLs can be checked daily while the app is open.").size(12.5).color(AMBER));
        });
    }

    fn show_notice(&mut self, ui: &mut Ui) {
        let Some(notice) = &self.notice else {
            return;
        };
        let mut close = false;
        Frame::new()
            .fill(if notice.error { RED_TINT } else { GREEN_TINT })
            .corner_radius(8)
            .inner_margin(Margin::same(12))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.add(
                        egui::Label::new(
                            RichText::new(&notice.text)
                                .size(13.0)
                                .color(if notice.error { RED } else { GREEN }),
                        )
                        .wrap(),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        close = ui.small_button("Dismiss").clicked();
                    });
                });
            });
        ui.add_space(12.0);
        if close {
            self.notice = None;
        }
    }

    fn removal_dialog(&mut self, ctx: &egui::Context) {
        let Some(id) = self.pending_remove else {
            return;
        };
        let document = self.documents.iter().find(|d| d.id == id).cloned();
        let Some(document) = document else {
            self.pending_remove = None;
            return;
        };
        let mut remove = false;
        let mut cancel = false;
        egui::Window::new("Remove this workspace entry?").collapsible(false).resizable(false)
            .anchor(Align2::CENTER_CENTER,[0.0,0.0]).default_width(420.0).show(ctx,|ui| {
                ui.label(RichText::new(&document.name).font(heading(17.0)));
                ui.label("Its workspace entry, retained readings, saved reviews and follow-up history will be removed. Other linked files keep their own history. The original file stays where it is.");
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    remove=widgets::primary(ui,"Remove entry",true).clicked();
                    cancel=widgets::secondary(ui,"Keep document").clicked();
                });
            });
        if remove && let Some(workspace) = &self.workspace {
            match workspace
                .remove_document(id)
                .and_then(|()| workspace.documents())
            {
                Ok(documents) => {
                    self.document_versions.clear();
                    self.documents = documents;
                    if self.selected == Some(id) {
                        self.reader_view.clear();
                        self.check_view.clear();
                        self.selected = None;
                    }
                    self.notice = Some(Notice {
                        text: "Workspace entry and cached text removed. The original file is unchanged.".into(),
                        error: false,
                    });
                }
                Err(error) => {
                    self.notice = Some(Notice {
                        text: error.to_string(),
                        error: true,
                    })
                }
            }
        }
        if remove || cancel || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.pending_remove = None;
        }
    }
}

impl eframe::App for ComplyApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.messages(ctx);
        self.reader_view.poll();
        if self
            .folder_watch
            .tick(ctx, self.workspace.as_ref(), self.paths.as_ref())
            && let Some(workspace) = &self.workspace
        {
            if let Ok(documents) = workspace.documents() {
                self.documents = documents;
            }
            self.notice=Some(Notice {text:"Folder intake added or updated local document entries. Open them from Documents and confirm reading accuracy and scope before review.".into(),error:false});
        }
        self.review_tools.tick(ctx, self.workspace.as_ref());
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::O)) {
            self.choose_documents(ctx);
        }
        let files = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .collect::<Vec<_>>()
        });
        self.accept_files(files, ctx);
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let rect = ui.available_rect_before_wrap();
        ui.painter().rect_filled(rect, 0, CREAM);
        let sidebar_width = if rect.width() < 1100.0 { 208.0 } else { 232.0 };
        let sidebar_rect =
            Rect::from_min_max(rect.min, pos2(rect.left() + sidebar_width, rect.bottom()));
        let header_rect = Rect::from_min_max(
            pos2(sidebar_rect.right(), rect.top()),
            pos2(rect.right(), rect.top() + 66.0),
        );
        self.sidebar(ui, sidebar_rect);
        self.topbar(ui, header_rect);
        let content_rect = Rect::from_min_max(
            pos2(sidebar_rect.right() + 30.0, header_rect.bottom() + 28.0),
            pos2(rect.right() - 30.0, rect.bottom() - 20.0),
        );
        ui.scope_builder(UiBuilder::new().max_rect(content_rect).layout(Layout::top_down(Align::Min)),|ui| {
            if self.page==Page::Review && self.selected.is_some() && self.startup_error.is_none() {
                self.show_notice(ui);self.review(ui);return;
            }
            ScrollArea::vertical().id_salt(("main",self.page.key())).auto_shrink([false,false]).show(ui,|ui| {
                if let Some(error) = &self.startup_error {
                    widgets::title(ui,"Your workspace needs attention.");
                    ui.add_space(15.0);
                    widgets::card().show(ui,|ui| {
                        ui.colored_label(RED,error);
                        ui.label("Your existing files have been preserved. Close all other review-app windows before migration.");
                        if ui.button("Choose existing workspace folder…").clicked(){self.workspace_picker.pick_directory();}
                    });
                    return;
                }
                self.show_notice(ui);
                match self.page {
                    Page::Review=>self.review(ui),
                    Page::Documents=>self.documents_page(ui),
                    Page::Library=>self.library_page(ui),
                    Page::Reports=>self.reports_page(ui),
                    Page::Settings=>self.settings_page(ui),
                }
                ui.add_space(18.0);
            });
        });
        self.provision_view.show(ui.ctx());
        if let (Some(workspace), Some(paths)) = (&self.workspace, &self.paths) {
            let (changed, action) = self.document_versions.show(ui.ctx(), workspace, paths);
            if changed && let Ok(documents) = workspace.documents() {
                self.documents = documents;
            }
            if let Some(action) = action {
                match action {
                    crate::document_versions::VersionAction::Reading(reading) => {
                        self.selected = Some(reading.document_id);
                        self.reader_view.open_retained(reading.saved);
                        self.check_view.clear();
                        self.navigate(Page::Review);
                    }
                    crate::document_versions::VersionAction::Review(review) => {
                        self.open_saved_review(*review, ui.ctx())
                    }
                    crate::document_versions::VersionAction::CurrentFile {
                        document_id,
                        context,
                    } => {
                        self.select_document(document_id, ui.ctx());
                        if let Some(context) = context {
                            self.check_view.start_with_scope(context);
                        } else {
                            self.check_view.edit_scope();
                        }
                        self.notice=Some(Notice{text:"Reviewing the current original file. Reconfirm text accuracy, applicability and any OCR before saving a fresh review. Earlier notes and statuses remain with their original review.".into(),error:false});
                        self.navigate(Page::Review);
                    }
                }
            }
        }
        self.removal_dialog(ui.ctx());
        self.workspace_picker.update(ui.ctx());
        if let Some(source) = self.workspace_picker.take_picked() {
            self.import_source = Some(source);
        }
        if let Some(source) = self.import_source.clone() {
            egui::Window::new("Import existing workspace").collapsible(false).resizable(false).show(ui.ctx(),|ui| {
                ui.label(format!("Copy from: {}",source.display()));
                let target=self.paths.as_ref().map(|p|p.root.clone()).map(Ok).unwrap_or_else(complyph_core::default_workspace_target);
                if let Ok(target)=&target {ui.label(format!("Activate at: {}",target.display()));}
                ui.label("Copy databases only. Original documents and the old workspace remain in place as a backup. A destination with documents or reports cannot be replaced or merged.");
                ui.horizontal(|ui| {
                    if ui.button("Copy and activate workspace").clicked() {
                        self.reader_view.clear();
                        self.workspace=None;
                        self.catalog=None;
                        let result=target.and_then(|target|complyph_core::migration::import_into_empty(&source,&target));
                        match result {
                            Ok(paths)=>*self=Self::new(Ok(paths)),
                            Err(error)=>{
                                let text=error.to_string();
                                if let Some(paths)=self.paths.clone(){*self=Self::new(Ok(paths));self.notice=Some(Notice{text,error:true});}
                                else{self.startup_error=Some(text);}
                            }
                        }
                        self.import_source=None;
                    }
                    if ui.button("Cancel").clicked(){self.import_source=None;}
                });
            });
        }
        self.file_dialog.update(ui.ctx());
        if let (Some(catalog), Some(paths)) = (&self.catalog, &self.paths)
            && self
                .library_view
                .update_dialog(ui.ctx(), catalog, &paths.library_dir)
        {
            match catalog.summary() {
                Ok(summary) => self.library = summary,
                Err(error) => self.library_error = Some(error.to_string()),
            }
        }
        if let Some(files) = self.file_dialog.take_picked_multiple() {
            self.accept_files(files, ui.ctx());
        }
    }
}

fn import_files(paths: WorkspacePaths, files: Vec<PathBuf>, sender: &Sender<Message>) {
    let workspace = match Workspace::open(&paths) {
        Ok(workspace) => workspace,
        Err(error) => {
            let _ = sender.send(Message::Failed(error.to_string()));
            return;
        }
    };
    let mut ids = Vec::new();
    let mut errors = Vec::new();
    for file in files {
        match workspace.register_document(&file) {
            Ok(document) => {
                if !ids.contains(&document.id) {
                    ids.push(document.id);
                }
            }
            Err(error) => errors.push(error.to_string()),
        }
    }
    let _ = sender.send(Message::Imported { ids, errors });
}

fn document_row(
    ui: &mut Ui,
    document: &Document,
    selected: bool,
    show_path: bool,
    reviews: Option<u64>,
) -> egui::Response {
    let width = ui.available_width();
    let height = if show_path { 91.0 } else { 72.0 };
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::click());
    if selected || response.hovered() {
        ui.painter().rect_filled(rect, 8, CREAM);
    }
    if response.has_focus() {
        ui.painter()
            .rect_stroke(rect, 8, Stroke::new(1.5, NAVY), StrokeKind::Inside);
    }
    let icon_rect = Rect::from_min_size(rect.min + vec2(12.0, 16.0), vec2(38.0, 40.0));
    ui.painter().rect_filled(icon_rect, 7, CREAM);
    widgets::paint_icon(ui.painter(), icon_rect.shrink(8.0), Icon::Document, NAVY);
    let name_rect = Rect::from_min_max(
        rect.min + vec2(65.0, 11.0),
        pos2(rect.right() - 156.0, rect.top() + 36.0),
    );
    {
        let mut name_ui = ui.new_child(
            UiBuilder::new()
                .id_salt(("document-name", document.id))
                .max_rect(name_rect),
        );
        name_ui.add(
            egui::Label::new(RichText::new(&document.name).font(heading(15.0)))
                .truncate()
                .selectable(false),
        );
    }
    ui.painter().text(
        rect.min + vec2(65.0, 45.0),
        Align2::LEFT_CENTER,
        format!(
            "{}   ·   {}   ·   {}",
            document.kind.label(),
            format_size(document.bytes),
            format_date(document.added_at)
        ),
        FontId::proportional(11.5),
        MUTED,
    );
    if show_path {
        let path_rect = Rect::from_min_max(
            rect.min + vec2(65.0, 61.0),
            pos2(rect.right() - 20.0, rect.bottom()),
        );
        {
            let mut path_ui = ui.new_child(
                UiBuilder::new()
                    .id_salt(("document-path", document.id))
                    .max_rect(path_rect),
            );
            path_ui.add(
                egui::Label::new(
                    RichText::new(document.path.display().to_string())
                        .size(11.0)
                        .color(MUTED),
                )
                .truncate()
                .selectable(false),
            );
        }
    }
    ui.painter().text(
        pos2(rect.right() - 55.0, rect.top() + 35.0),
        Align2::RIGHT_CENTER,
        match reviews {
            Some(0) => "No saved reviews".to_owned(),
            Some(1) => "1 saved review".to_owned(),
            Some(count) => format!("{count} saved reviews"),
            None => "Review status unavailable".to_owned(),
        },
        FontId::proportional(10.5),
        if reviews.is_some_and(|n| n > 0) {
            GREEN
        } else {
            MUTED
        },
    );
    widgets::paint_icon(
        ui.painter(),
        Rect::from_center_size(
            pos2(rect.right() - 22.0, rect.top() + 35.0),
            vec2(18.0, 18.0),
        ),
        Icon::Arrow,
        MUTED,
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), &document.name)
    });
    response
        .on_hover_text(&document.name)
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

pub(crate) fn format_date(timestamp: i64) -> String {
    DateTime::from_timestamp(timestamp, 0)
        .map(|d| d.with_timezone(&Local).format("%b %-d, %Y").to_string())
        .unwrap_or_else(|| "Unknown date".to_owned())
}
