//! Offline acceptance of the shipped UI, through egui's real input/accessibility output.
//! This diagnostic requires a new fixture directory and never opens the default workspace.
use super::*;
use crate::theme;
use egui::accesskit::{Node, NodeId, Role};
use serde::Serialize;
use std::time::{Duration, Instant};
use std::{collections::HashMap, ffi::OsString, fs, path::Path};

pub const PROFILE: &str = "initial-desktop-quality-v1";

#[derive(Serialize)]
struct CaseResult {
    name: &'static str,
    passed: bool,
}
fn passed(name: &'static str) -> CaseResult {
    CaseResult { name, passed: true }
}
fn require(ok: bool, message: &str) -> Result<(), String> {
    if ok { Ok(()) } else { Err(message.into()) }
}

#[derive(Serialize)]
struct LayoutResult {
    physical_width: u32,
    physical_height: u32,
    scale: f32,
    logical_width: f32,
    logical_height: f32,
    scenarios: usize,
    passed: bool,
}

struct Harness {
    ctx: egui::Context,
    screen: Rect,
    scale: f32,
    time: f64,
}
impl Harness {
    fn new(width: u32, height: u32, scale: f32) -> Self {
        let ctx = egui::Context::default();
        theme::install(&ctx);
        ctx.enable_accesskit();
        ctx.set_zoom_factor(1.0);
        Self {
            ctx,
            screen: Rect::from_min_size(
                egui::Pos2::ZERO,
                vec2(width as f32 / scale, height as f32 / scale),
            ),
            scale,
            time: 0.0,
        }
    }
    fn frame(&mut self, app: &mut ComplyApp, events: Vec<egui::Event>) -> egui::FullOutput {
        self.time += 0.1;
        let mut input = egui::RawInput {
            screen_rect: Some(self.screen),
            time: Some(self.time),
            events,
            ..Default::default()
        };
        let modifiers = input
            .events
            .iter()
            .filter_map(|e| match e {
                egui::Event::Key { modifiers, .. } => Some(*modifiers),
                _ => None,
            })
            .next_back()
            .unwrap_or_default();
        input
            .events
            .insert(0, egui::Event::ModifiersChanged(modifiers));
        input
            .viewports
            .entry(egui::ViewportId::ROOT)
            .or_default()
            .native_pixels_per_point = Some(self.scale);
        let mut output = self.ctx.run_ui(input, |ui| {
            app.messages(ui.ctx());
            app.reader_view.poll();
            app.shortcuts(ui.ctx());
            app.render(ui);
        });
        // The acceptance assertions inspect actual widget bounds, not raster textures.
        output.textures_delta.clear();
        output
    }
    fn settle(&mut self, app: &mut ComplyApp) -> egui::FullOutput {
        self.frame(app, vec![]);
        self.frame(app, vec![]);
        self.frame(app, vec![])
    }
    fn inspect(
        &self,
        output: &egui::FullOutput,
        scenario: &str,
        navigation: bool,
    ) -> Result<(), String> {
        inspect(
            output,
            self.screen,
            self.scale,
            scenario,
            navigation,
            &self.ctx,
        )
    }
    fn key(
        &mut self,
        app: &mut ComplyApp,
        key: egui::Key,
        modifiers: egui::Modifiers,
    ) -> egui::FullOutput {
        let event = |pressed| egui::Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers,
        };
        self.frame(app, vec![event(true)]);
        self.frame(app, vec![event(false)]);
        self.settle(app)
    }
    fn click(&mut self, app: &mut ComplyApp, name: &str) -> Result<egui::FullOutput, String> {
        for step in 0..32 {
            let output = self.settle(app);
            let tree = nodes(&output)?;
            let target = tree.iter().find(|(_, n)| {
                label(n, &tree) == name
                    && !n.is_disabled()
                    && n.supports_action(egui::accesskit::Action::Click)
            });
            if let Some((target, rect)) = target
                .and_then(|(id, node)| {
                    widget_response(&self.ctx, *id)
                        .map(|r| r.interact_rect)
                        .or_else(|| bounds(node))
                        .map(|r| (*id, r.intersect(self.screen)))
                })
                .filter(|(_, b)| b.is_positive())
            {
                self.frame(
                    app,
                    vec![
                        egui::Event::PointerMoved(rect.center()),
                        egui::Event::AccessKitActionRequest(egui::accesskit::ActionRequest {
                            action: egui::accesskit::Action::Click,
                            target_tree: egui::accesskit::TreeId::ROOT,
                            target_node: target,
                            data: None,
                        }),
                    ],
                );
                return Ok(self.settle(app));
            }
            // Move toward the actual control without jumping over a short row
            // between two viewport positions on a compact display.
            let delta = target
                .and_then(|(_, n)| bounds(n))
                .map(|b| {
                    let visible_top = 74.0;
                    let visible_bottom = self.screen.bottom() - 16.0;
                    if b.bottom() > visible_bottom {
                        -(b.bottom() - visible_bottom + 40.0)
                    } else {
                        visible_top - b.top() + 40.0
                    }
                })
                .unwrap_or(if step < 16 { 400.0 } else { -400.0 });
            self.frame(
                app,
                vec![
                    egui::Event::PointerMoved(self.screen.right_bottom() - vec2(35.0, 70.0)),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: vec2(0.0, delta.clamp(-400.0, 400.0)),
                        phase: egui::TouchPhase::Move,
                        source: egui::MouseWheelSource::Unknown,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
        let output = self.settle(app);
        let tree = nodes(&output)?;
        let windows = tree
            .values()
            .filter(|n| n.role() == Role::Window)
            .map(|n| (label(n, &tree), bounds(n)))
            .collect::<Vec<_>>();
        let controls = tree
            .iter()
            .filter(|(_, n)| n.role() == Role::Button)
            .map(|(id, n)| {
                (
                    label(n, &tree),
                    bounds(n),
                    widget_response(&self.ctx, *id).map(|r| r.interact_rect),
                )
            })
            .collect::<Vec<_>>();
        Err(format!(
            "The control {name:?} could not be reached by scrolling. Page: {}; error: {:?}; windows: {windows:?}; size {:?}; controls: {controls:?}.",
            app.page.label(),
            app.library_error,
            self.screen.size()
        ))
    }
}
fn focused_label(output: &egui::FullOutput) -> Result<String, String> {
    let tree = nodes(output)?;
    let id = output
        .platform_output
        .accesskit_update
        .as_ref()
        .unwrap()
        .focus;
    Ok(tree.get(&id).map(|n| label(n, &tree)).unwrap_or_default())
}
fn keyboard_checks(
    app: &mut ComplyApp,
    doc: &Document,
    saved: &complyph_core::SavedExtraction,
    review: &complyph_core::SavedReview,
) -> Result<Vec<CaseResult>, String> {
    let mut results = vec![];
    let mut h = Harness::new(1280, 960, 2.0);
    app.selected = None;
    app.page = Page::Review;
    h.settle(app);
    let command = egui::Modifiers {
        ctrl: true,
        command: true,
        ..Default::default()
    };
    for (page, key) in Page::ALL.into_iter().zip([
        egui::Key::Num1,
        egui::Key::Num2,
        egui::Key::Num3,
        egui::Key::Num4,
        egui::Key::Num5,
    ]) {
        let output = h.key(app, key, command);
        require(app.page == page, "A page shortcut did not navigate.")?;
        require(
            focused_label(&output)? == page.label(),
            "Page navigation did not retain visible keyboard focus.",
        )?;
    }
    results.push(passed("page_shortcuts_and_focus"));
    h.key(app, egui::Key::Num2, command);
    let output = h.key(app, egui::Key::F, command);
    require(
        focused_label(&output)? == "Search documents by filename",
        "Document search was not labelled/focused.",
    )?;
    h.frame(app, vec![egui::Event::Text("not-a-real-file".into())]);
    require(
        app.search == "not-a-real-file",
        "Typing did not reach the filename search.",
    )?;
    app.search.clear();
    app.selected = Some(doc.id);
    app.page = Page::Review;
    app.review_pane = ReviewPane::Findings;
    app.reader_view.open_retained(saved.clone());
    app.check_view.set_review(review.clone());
    let output = h.key(app, egui::Key::F, command);
    require(
        app.review_pane == ReviewPane::Document && focused_label(&output)? == "Find text",
        "Connected document search did not switch/focus the document tab.",
    )?;
    h.key(app, egui::Key::Num3, command);
    let output = h.key(app, egui::Key::F, command);
    require(
        focused_label(&output)? == "Search legal sources",
        "Legal-source search was not labelled/focused.",
    )?;
    results.push(passed("scoped_search_and_typing"));
    h.key(app, egui::Key::O, command);
    require(
        app.file_dialog.state() == &egui_file_dialog::DialogState::Open,
        "Open-document shortcut did not open its picker.",
    )?;
    let page = app.page;
    h.key(app, egui::Key::Num5, command);
    require(
        app.page == page,
        "A global shortcut navigated behind the file picker.",
    )?;
    for _ in 0..24 {
        let output = h.key(app, egui::Key::Tab, egui::Modifiers::NONE);
        let id = output
            .platform_output
            .accesskit_update
            .as_ref()
            .unwrap()
            .focus;
        require(
            !Page::ALL
                .into_iter()
                .any(|p| egui::Id::unique(("workspace-page", p.label())).accesskit_id() == id),
            "Keyboard focus escaped the modal file picker.",
        )?;
    }
    h.key(app, egui::Key::Escape, egui::Modifiers::NONE);
    require(
        app.file_dialog.state() != &egui_file_dialog::DialogState::Open,
        "Escape failed to dismiss the file picker.",
    )?;
    results.push(passed("file_picker_focus_and_dismissal"));
    app.pending_remove = Some(doc.id);
    let output = h.settle(app);
    require(
        focused_label(&output)? == "Keep document",
        "Removal did not focus its safe default.",
    )?;
    let page = app.page;
    h.key(app, egui::Key::Num1, command);
    require(
        app.page == page,
        "A global shortcut navigated behind a removal confirmation.",
    )?;
    h.key(app, egui::Key::Enter, egui::Modifiers::NONE);
    require(
        app.pending_remove.is_none() && app.documents.iter().any(|d| d.id == doc.id),
        "Enter on the default confirmation removed a document.",
    )?;
    app.pending_remove = Some(doc.id);
    h.settle(app);
    h.key(app, egui::Key::Escape, egui::Modifiers::NONE);
    require(
        app.pending_remove.is_none() && doc.path.exists(),
        "Escape did not preserve the document.",
    )?;
    results.push(passed("safe_confirmation_default_and_escape"));
    let mut h = Harness::new(1280, 960, 2.0);
    app.page = Page::Review;
    app.selected = None;
    h.settle(app);
    let mut found = false;
    for _ in 0..40 {
        let output = h.key(app, egui::Key::Tab, egui::Modifiers::NONE);
        if focused_label(&output)? == "Documents" {
            found = true;
            break;
        }
    }
    require(found, "Tab could not reach document navigation.")?;
    h.key(app, egui::Key::Enter, egui::Modifiers::NONE);
    require(
        app.page == Page::Documents,
        "Enter did not activate navigation.",
    )?;
    let mut found = false;
    for _ in 0..60 {
        let output = h.key(app, egui::Key::Tab, egui::Modifiers::NONE);
        if focused_label(&output)? == doc.name {
            found = true;
            break;
        }
    }
    require(found, "Tab could not reach the document row.")?;
    h.key(app, egui::Key::Space, egui::Modifiers::NONE);
    require(
        app.selected == Some(doc.id) && app.page == Page::Review,
        "Space did not open the focused document row.",
    )?;
    finish_reading(&mut h, app)?;
    // Opening a historical reading from Findings must reveal the document,
    // rather than leave the compact view on the now-cleared findings pane.
    app.review_pane = ReviewPane::Findings;
    app.document_versions
        .open(app.workspace.as_ref().unwrap(), doc.id);
    h.settle(app);
    h.click(app, "Read saved text")?;
    require(
        app.reader_view.is_retained() && app.review_pane == ReviewPane::Document,
        "Opening a saved version did not select its document tab.",
    )?;
    results.push(passed("tab_enter_and_space_activation"));
    Ok(results)
}
fn finish_reading(h: &mut Harness, app: &mut ComplyApp) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(30);
    while app.reader_view.is_reading() {
        if Instant::now() > deadline {
            return Err("The bounded reader did not finish its synthetic case.".into());
        }
        h.frame(app, vec![]);
        std::thread::sleep(Duration::from_millis(10));
    }
    h.settle(app);
    Ok(())
}
fn recovery_checks(
    app: &mut ComplyApp,
    doc: &Document,
    review: &complyph_core::SavedReview,
    bytes: &[u8],
) -> Result<Vec<CaseResult>, String> {
    let mut results = vec![];
    let mut h = Harness::new(1280, 960, 2.0);
    app.page = Page::Review;
    app.selected = Some(doc.id);
    app.review_pane = ReviewPane::Document;
    let paths = app.paths.clone().ok_or("Missing synthetic paths.")?;
    let state = app
        .workspace
        .as_ref()
        .unwrap()
        .review_workflow(review.id)
        .map_err(|e| e.to_string())?;
    app.workspace
        .as_ref()
        .unwrap()
        .append_review_action(
            review.id,
            &complyph_core::review_workflow::ReviewActionRequest {
                review_fingerprint: state.review_fingerprint,
                rule_id: Some("PH-LAB-083-01".into()),
                expected_revision: 0,
                reviewer: "Synthetic desktop tester".into(),
                note: "Preserve this fictional history during reading failures.".into(),
                status: Some(complyph_core::review_workflow::ReviewStatus::Resolved),
            },
        )
        .map_err(|e| e.to_string())?;
    let before = app
        .workspace
        .as_ref()
        .unwrap()
        .review_workflow(review.id)
        .map_err(|e| e.to_string())?;
    let old_reading = app
        .workspace
        .as_ref()
        .unwrap()
        .reading_for_review(review.id)
        .map_err(|e| e.to_string())?
        .ok_or("Missing original retained reading.")?;
    let moved = doc.path.with_file_name("Synthetic moved original.docx");
    fs::rename(&doc.path, &moved).map_err(|e| e.to_string())?;
    app.check_view.clear();
    app.reader_view
        .start(paths.clone(), doc.clone(), true, &h.ctx);
    finish_reading(&mut h, app)?;
    require(
        app.reader_view.current().is_none(),
        "A missing original appeared readable.",
    )?;
    let output = h.settle(app);
    let tree = nodes(&output)?;
    require(
        tree.values()
            .any(|n| label(n, &tree) == "Check document" && n.is_disabled()),
        "A missing original allowed a numeric review.",
    )?;
    fs::rename(&moved, &doc.path).map_err(|e| e.to_string())?;
    h.click(app, "Read again")?;
    finish_reading(&mut h, app)?;
    require(
        app.reader_view
            .current()
            .is_some_and(|s| s.report.status == complyph_extract::Status::Ready),
        "Retry did not recover the restored original.",
    )?;
    results.push(passed("missing_original_and_retry"));
    fs::write(&doc.path, b"PK SYNTHETIC TRUNCATED ZIP - NO REAL DATA")
        .map_err(|e| e.to_string())?;
    h.click(app, "Read again")?;
    finish_reading(&mut h, app)?;
    require(
        app.reader_view
            .current()
            .is_some_and(|s| s.report.status == complyph_extract::Status::Unreadable),
        "Malformed Word content did not fail closed.",
    )?;
    let output = h.settle(app);
    let tree = nodes(&output)?;
    require(
        tree.values()
            .any(|n| label(n, &tree) == "Check document" && n.is_disabled()),
        "Malformed content allowed a numeric review.",
    )?;
    results.push(passed("malformed_original_blocks_review"));
    fs::write(&doc.path, bytes).map_err(|e| e.to_string())?;
    h.click(app, "Read again")?;
    finish_reading(&mut h, app)?;
    require(
        app.reader_view
            .current()
            .is_some_and(|s| s.report.status == complyph_extract::Status::Ready),
        "Repair did not restore reading.",
    )?;
    let workspace = app.workspace.as_ref().unwrap();
    let after = workspace
        .review_workflow(review.id)
        .map_err(|e| e.to_string())?;
    require(
        serde_json::to_value(&before.saved).unwrap() == serde_json::to_value(&after.saved).unwrap()
            && before.review_fingerprint == after.review_fingerprint
            && serde_json::to_value(&before.events).unwrap()
                == serde_json::to_value(&after.events).unwrap(),
        "A reading failure rewrote review history.",
    )?;
    require(
        workspace.reviews().map_err(|e| e.to_string())?.len() == 1,
        "A retry created an automatic review.",
    )?;
    let retained = workspace
        .reading_for_review(review.id)
        .map_err(|e| e.to_string())?
        .ok_or("The historical reading disappeared.")?;
    require(
        retained.reading_sha256 == old_reading.reading_sha256
            && retained.saved.report == old_reading.saved.report,
        "A retry replaced historical text.",
    )?;
    results.push(passed("repaired_reading_preserves_history"));
    Ok(results)
}
fn nodes(output: &egui::FullOutput) -> Result<HashMap<NodeId, Node>, String> {
    output
        .platform_output
        .accesskit_update
        .as_ref()
        .map(|u| u.nodes.clone().into_iter().collect())
        .ok_or("No accessibility output was generated.".into())
}
fn label(node: &Node, tree: &HashMap<NodeId, Node>) -> String {
    node.label().map(str::to_owned).unwrap_or_else(|| {
        node.labelled_by()
            .iter()
            .filter_map(|id| tree.get(id))
            .filter_map(|n| n.value().or(n.label()))
            .collect::<Vec<_>>()
            .join(" ")
    })
}
fn bounds(node: &Node) -> Option<Rect> {
    node.bounds().map(|r| {
        Rect::from_min_max(
            pos2(r.x0 as f32, r.y0 as f32),
            pos2(r.x1 as f32, r.y1 as f32),
        )
    })
}
fn widget_response(ctx: &egui::Context, id: NodeId) -> Option<egui::Response> {
    // SAFETY: these are the original high-entropy egui IDs emitted by this frame.
    ctx.read_response(unsafe { egui::Id::from_high_entropy_bits(id.0) })
}
fn window_present(output: &egui::FullOutput, title: &str) -> Result<(), String> {
    let tree = nodes(output)?;
    if !tree
        .values()
        .any(|n| n.role() == Role::Window && label(n, &tree) == title)
    {
        return Err(format!("The {title:?} window did not open."));
    }
    Ok(())
}
fn inspect(
    output: &egui::FullOutput,
    screen: Rect,
    scale: f32,
    scenario: &str,
    check_navigation: bool,
    ctx: &egui::Context,
) -> Result<(), String> {
    if (output.pixels_per_point - scale).abs() > 0.01 {
        return Err(format!("{scenario}: native scale was not applied."));
    }
    let tree = nodes(output)?;
    for node in tree.values().filter(|n| n.role() == Role::Window) {
        if bounds(node).is_some_and(|b| !screen.expand(2.0).contains_rect(b)) {
            return Err(format!(
                "{scenario}: window {} is outside the viewport: {:?}.",
                label(node, &tree),
                bounds(node)
            ));
        }
    }
    for page in Page::ALL.into_iter().filter(|_| check_navigation) {
        let matches = tree
            .values()
            .filter(|n| n.role() == Role::Button && label(n, &tree) == page.label())
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(format!(
                "{scenario}: navigation {} is absent or ambiguous.",
                page.label()
            ));
        }
        if !bounds(matches[0]).is_some_and(|b| screen.expand(1.0).contains_rect(b)) {
            return Err(format!(
                "{scenario}: navigation {} is off-screen.",
                page.label()
            ));
        }
    }
    for (id, node) in tree.iter().filter(|(_, n)| {
        matches!(
            n.role(),
            Role::Button
                | Role::TextInput
                | Role::MultilineTextInput
                | Role::CheckBox
                | Role::ComboBox
                | Role::RadioButton
                | Role::Tab
        )
    }) {
        if matches!(node.role(), Role::TextInput | Role::MultilineTextInput)
            && label(node, &tree).is_empty()
        {
            return Err(format!(
                "{scenario}: unnamed {:?} with placeholder {:?}.",
                node.role(),
                node.placeholder()
            ));
        }
        let Some(b) = bounds(node) else { continue };
        if !b.is_finite() {
            return Err(format!("{scenario}: non-finite control bounds."));
        }
        // Off-screen items in a clipped horizontal path bar are legitimate.
        // Window bounds and the reachable file-picker actions are checked separately.
        if widget_response(ctx, *id).is_some_and(|r| !r.interact_rect.is_positive()) {
            continue;
        }
        if b.bottom() > screen.top()
            && b.top() < screen.bottom()
            && (b.left() < screen.left() - 2.0 || b.right() > screen.right() + 2.0)
        {
            return Err(format!(
                "{scenario}: {} {:?} overflows horizontally: {b:?} / {screen:?}",
                label(node, &tree),
                node.role()
            ));
        }
    }
    Ok(())
}

fn run_suite(root: &Path) -> Result<serde_json::Value, String> {
    fs::create_dir(root).map_err(|e| e.to_string())?;
    let original =
        root.join("Synthetic document with a long filename for display and keyboard checks.docx");
    let bytes = include_bytes!("../../../examples/Synthetic labour issues.docx");
    fs::write(&original, bytes).map_err(|e| e.to_string())?;
    let paths = WorkspacePaths::under(root.join("data"));
    let mut app = ComplyApp::new(Ok(paths));
    if let Some(error) = &app.startup_error {
        return Err(error.clone());
    }
    let workspace = app
        .workspace
        .as_ref()
        .ok_or("Fixture workspace is unavailable.")?;
    let doc = workspace
        .register_document(&original)
        .map_err(|e| e.to_string())?;
    let report = complyph_extract::extract(complyph_extract::Format::Docx, bytes);
    let saved = workspace
        .save_extraction(
            &doc,
            &complyph_core::document_snapshot(&original).map_err(|e| e.to_string())?,
            report,
        )
        .map_err(|e| e.to_string())?;
    let context = complyph_checks::Context {
        normal_hours: complyph_checks::Applicability::Covered,
        ordinary_overtime: complyph_checks::Applicability::Covered,
        night_differential: complyph_checks::Applicability::Covered,
        meal_period: complyph_checks::Applicability::Covered,
        weekly_rest: complyph_checks::Applicability::Covered,
        operative_terms_confirmed: true,
        ..Default::default()
    };
    let review = workspace
        .save_review_for_reading(
            doc.id,
            &saved,
            complyph_checks::review(&saved.report, &saved.sha256, context),
        )
        .map_err(|e| e.to_string())?;
    app.documents = workspace.documents().map_err(|e| e.to_string())?;
    let mut results = vec![];
    for (width, height) in [(1280, 960), (1920, 1080), (2560, 1440)] {
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let mut h = Harness::new(width, height, scale);
            let mut scenarios = 0;
            app.selected = None;
            for page in Page::ALL {
                app.page = page;
                let output = h.settle(&mut app);
                h.inspect(&output, page.label(), true)?;
                scenarios += 1;
            }
            app.page = Page::Review;
            app.selected = Some(doc.id);
            app.reader_view.open_retained(saved.clone());
            app.check_view.set_review(review.clone());
            for pane in [ReviewPane::Document, ReviewPane::Findings] {
                app.review_pane = pane;
                let output = h.settle(&mut app);
                h.inspect(&output, "Connected review", true)?;
                scenarios += 1;
            }
            let output = h.click(&mut app, "Review or change scope")?;
            h.inspect(&output, "Review applicability form", true)?;
            scenarios += 1;
            app.check_view.set_review(review.clone());
            app.document_versions
                .open(app.workspace.as_ref().unwrap(), doc.id);
            let output = h.settle(&mut app);
            window_present(&output, "Document versions")?;
            h.inspect(&output, "Document versions", true)?;
            scenarios += 1;
            h.click(&mut app, "Close window")
                .map_err(|e| format!("Document versions close at scale {scale}: {e}"))?;
            app.provision_view
                .open(app.catalog.as_ref().unwrap(), &review, 0);
            let output = h.settle(&mut app);
            window_present(&output, "Offline legal library")?;
            h.inspect(&output, "Supporting provision", true)?;
            scenarios += 1;
            h.click(&mut app, "Close window")
                .map_err(|e| format!("Provision close at scale {scale}: {e}"))?;
            app.page = Page::Library;
            app.selected = None;
            h.settle(&mut app);
            let output = h.click(&mut app, "Maintain library")?;
            window_present(&output, "Library maintenance")?;
            h.inspect(&output, "Library maintenance", true)?;
            scenarios += 1;
            h.click(&mut app, "Close window")
                .map_err(|e| format!("Maintenance close at scale {scale}: {e}"))?;
            let output = h.click(&mut app, "Add legal source")?;
            h.inspect(&output, "Add legal source", false)?;
            scenarios += 1;
            h.click(&mut app, "Cancel")?;
            app.pending_remove = Some(doc.id);
            let output = h.settle(&mut app);
            h.inspect(&output, "Remove confirmation", false)?;
            scenarios += 1;
            h.key(&mut app, egui::Key::Escape, egui::Modifiers::NONE);
            app.file_dialog =
                std::mem::take(&mut app.file_dialog).initial_directory(root.to_path_buf());
            app.choose_documents(&h.ctx);
            let output = h.settle(&mut app);
            window_present(&output, "Choose documents")?;
            h.inspect(&output, "File picker", false)?;
            scenarios += 1;
            h.key(&mut app, egui::Key::Escape, egui::Modifiers::NONE);
            if app.file_dialog.state() == &egui_file_dialog::DialogState::Open {
                return Err("Escape did not close the file picker.".into());
            }
            results.push(LayoutResult {
                physical_width: width,
                physical_height: height,
                scale,
                logical_width: h.screen.width(),
                logical_height: h.screen.height(),
                scenarios,
                passed: true,
            });
        }
    }
    if fs::read(&original).map_err(|e| e.to_string())? != bytes {
        return Err("The original fixture changed.".into());
    }
    let keyboard = keyboard_checks(&mut app, &doc, &saved, &review)?;
    let recovery = recovery_checks(&mut app, &doc, &review, bytes)?;
    require(
        fs::read(&original).map_err(|e| e.to_string())? == bytes,
        "The original synthetic bytes were not restored.",
    )?;
    Ok(
        serde_json::json!({"profile":PROFILE,"version":env!("CARGO_PKG_VERSION"),"layout_cases":results,"keyboard_cases":keyboard,"recovery_cases":recovery,"interactive_windows_display_tested":false,"qualified_legal_review":false}),
    )
}

pub fn run_cli(args: Vec<OsString>) -> Result<(), String> {
    if args.len() != 4 || args[0] != "--output" || args[2] != "--data-dir" {
        return Err(
            "Use --desktop-quality-check --output NEW-JSON --data-dir UNUSED-FIXTURE-DIRECTORY"
                .into(),
        );
    }
    let output = std::path::PathBuf::from(&args[1]);
    let root = std::path::PathBuf::from(&args[3]);
    if output.exists() || root.exists() {
        return Err("Choose a new result file and unused fixture directory. Existing workspaces are never used.".into());
    }
    let result = run_suite(&root)?;
    let file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .map_err(|e| e.to_string())?;
    serde_json::to_writer_pretty(file, &result).map_err(|e| e.to_string())
}
