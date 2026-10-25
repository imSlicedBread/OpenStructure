use super::super::sheet_tests::{app_with_plan, raw_input};
use super::*;
use os_storage::{StorageBackend, ZipJsonStorage};
use std::time::{Duration, Instant};

fn draft(app: &mut DesktopApp, set: Id, previous: Option<Id>) -> Id {
    let id = Id::new();
    let sequence = app
        .editor
        .document
        .model()
        .drawing_issues
        .values()
        .map(|i| i.sequence)
        .max()
        .unwrap_or(0)
        + 1;
    let issue = DrawingIssue {
        id,
        set,
        sequence,
        revision: format!("R{sequence}"),
        description: "Drawing issue acceptance".into(),
        date: "2026-10-11".into(),
        recipients: vec!["Design team".into()],
        sheets: app.editor.document.model().sheet_sets[&set].sheets.clone(),
        supersedes: previous,
        state: IssueState::Draft,
        manifest: None,
        manifest_sha256: None,
        archive_path: None,
    };
    app.editor
        .command("Draft issue", Command::PutDrawingIssue(issue))
        .unwrap();
    id
}
fn fixture() -> (DesktopApp, Id, Id) {
    let (mut app, view) = app_with_plan();
    let wall = os_model::Wall::new(
        os_walls::WALL_TYPE,
        os_model::WallParams {
            name: "Issue snapshot wall".into(),
            path: os_model::WallPath::Straight {
                start: Point2::new(-2.0, 0.0),
                end: Point2::new(2.0, 0.0),
            },
            thickness: 0.2,
            height: 3.0,
            level: app.active_level,
            material: None,
        },
    );
    app.editor
        .command("Add issued drawing geometry", Command::AddWall(wall))
        .unwrap();
    app.create_sheet_from_active_view();
    let first = app.plans.active_sheet.unwrap();
    let mut p = app.editor.document.model().sheets[&first]
        .parameters
        .clone();
    p.number = "A202".into();
    p.name = "Second drawing".into();
    p.viewports[0].id = Id::new();
    p.viewports[0].scale_denominator = 50.0;
    let second_view = app
        .editor
        .create_floor_plan("Second source", app.active_level)
        .unwrap();
    p.viewports[0].view = second_view;
    let second = Sheet::new("core.sheet", p);
    let second_id = second.id();
    let set = Id::new();
    app.editor
        .commands(
            "Ordered sheets",
            vec![
                Command::AddSheet(second),
                Command::PutSheetSet(SheetSet {
                    id: set,
                    name: "Acceptance set".into(),
                    sheets: vec![second_id, first],
                }),
            ],
        )
        .unwrap();
    assert_eq!(app.plans.active, Some(view));
    let issue = draft(&mut app, set, None);
    (app, set, issue)
}
fn settle(app: &mut DesktopApp, issue: Id) -> Pending {
    let end = Instant::now() + Duration::from_secs(15);
    loop {
        app.plans.poll(&app.editor);
        #[cfg(feature = "external-plugins")]
        {
            app.plans.providers.poll(&mut app.editor, app.plans.active);
            app.plans.sheet_sources.poll_providers(&mut app.editor);
        }
        match app.preflight_drawing_issue(issue) {
            Ok(p) => return p,
            Err(e) => assert!(Instant::now() < end, "{e}"),
        }
        std::thread::yield_now();
    }
}
fn acknowledge(p: &mut Pending) {
    for w in &mut p.manifest.warnings {
        w.acknowledgment = "Reviewed and accepted for this issue".into();
    }
}

#[test]
fn standalone_drafting_ordered_issue_pdf_reopen_and_manifest_view_deletion_guard() {
    let mut app = DesktopApp::new().unwrap();
    let first_view = app.editor.create_drafting_view("Draft A").unwrap();
    app.plans.poll(&app.editor);
    app.focus_plan(Some(first_view));
    app.create_sheet_from_active_view();
    let first = app.plans.active_sheet.unwrap();
    let second_view = app.editor.create_drafting_view("Draft B").unwrap();
    let mut parameters = app.editor.document.model().sheets[&first]
        .parameters
        .clone();
    parameters.number = "D202".into();
    parameters.viewports[0].id = Id::new();
    parameters.viewports[0].view = second_view;
    let second = Sheet::new("core.sheet", parameters);
    let second_id = second.id();
    let set = Id::new();
    let line = os_model::DetailLine::new(
        "core.detail_line",
        os_model::DetailLineParams {
            view: first_view,
            start: Point2::new(-1., 0.),
            end: Point2::new(1., 0.),
        },
    );
    let line_id = line.id();
    app.editor
        .commands(
            "Drafting issue",
            vec![
                Command::AddSheet(second),
                Command::AddDetailLine(line),
                Command::PutSheetSet(SheetSet {
                    id: set,
                    name: "Drafting set".into(),
                    sheets: vec![second_id, first],
                }),
            ],
        )
        .unwrap();
    let issue = draft(&mut app, set, None);
    let mut pending = settle(&mut app, issue);
    assert_eq!(
        pending
            .manifest
            .sheets
            .iter()
            .map(|s| s.views[0].0)
            .collect::<Vec<_>>(),
        [second_view, first_view]
    );
    let pdf = String::from_utf8(pending.pdf.clone()).unwrap();
    assert!(pdf.contains("/Kids [4 0 R 6 0 R] /Count 2"));
    assert!(pdf.find("<44323032> Tj").unwrap() < pdf.find("<41313031> Tj").unwrap());
    acknowledge(&mut pending);
    app.review_issue_pending(&mut pending).unwrap();
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("drafting.osissue");
    app.publish_issue_pending(&pending, path.to_str().unwrap())
        .unwrap();
    let bundle = IssueBundle::read(&path).unwrap();
    bundle
        .verify_record(&app.editor.document.model().drawing_issues[&issue])
        .unwrap();
    assert_eq!(bundle.pdf(), pending.pdf);
    let native = temp.path().join("drafting.osb");
    ZipJsonStorage.save(&app.editor.document, &native).unwrap();
    assert_eq!(
        ZipJsonStorage.open(&native).unwrap().model(),
        app.editor.document.model()
    );
    let mut p = app.editor.document.model().sheets[&first]
        .parameters
        .clone();
    p.viewports[0].view = second_view;
    app.editor
        .commands(
            "Remove live references",
            vec![
                Command::RemoveDetailLine(line_id),
                Command::UpdateSheet {
                    id: first,
                    parameters: p,
                },
            ],
        )
        .unwrap();
    let before = app.editor.document.model().clone();
    let history = app.editor.document.history_stats();
    assert!(
        app.editor
            .command("Delete issued source", Command::RemoveView(first_view))
            .is_err()
    );
    assert_eq!(app.editor.document.model(), &before);
    assert_eq!(app.editor.document.history_stats(), history);
}

#[test]
fn drawing_issue_order_freeze_reopen_supersession_integrity_and_history_boundary() {
    let (mut app, set, issue) = fixture();
    let mut pending = settle(&mut app, issue);
    assert_eq!(
        pending
            .manifest
            .sheets
            .iter()
            .map(|s| s.number.as_str())
            .collect::<Vec<_>>(),
        ["A202", "A101"]
    );
    let pdf = String::from_utf8(pending.pdf.clone()).unwrap();
    assert!(pdf.contains("/Kids [4 0 R 6 0 R] /Count 2"));
    assert!(pdf.find("<41323032> Tj").unwrap() < pdf.find("<41313031> Tj").unwrap());
    assert!(
        app.review_issue_pending(&mut pending).is_err(),
        "every warning needs a decision"
    );
    acknowledge(&mut pending);
    app.review_issue_pending(&mut pending).unwrap();
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("issue.osissue");
    app.publish_issue_pending(&pending, path.to_str().unwrap())
        .unwrap();
    let issued = app.editor.document.model().drawing_issues[&issue].clone();
    assert_eq!(issued.state, IssueState::Issued);
    assert!(!app.editor.document.can_undo() && !app.editor.document.can_redo());
    assert!(!app.editor.document.undo());
    let bytes = std::fs::read(&path).unwrap();
    let bundle = IssueBundle::read(&path).unwrap();
    bundle.verify_record(&issued).unwrap();
    assert_eq!(bundle.pdf(), pending.pdf);
    assert!(
        app.editor
            .document
            .execute("Illegal removal", vec![Command::RemoveDrawingIssue(issue)])
            .is_err()
    );
    let mut rewrite = issued.clone();
    rewrite.state = IssueState::Draft;
    rewrite.manifest = None;
    rewrite.manifest_sha256 = None;
    rewrite.archive_path = None;
    assert!(
        app.editor
            .document
            .execute("Illegal rewrite", vec![Command::PutDrawingIssue(rewrite)])
            .is_err()
    );
    let wall = app
        .editor
        .document
        .model()
        .walls
        .values()
        .find(|w| w.parameters.name == "Issue snapshot wall")
        .unwrap();
    let wall_id = wall.id();
    let mut parameters = wall.parameters.clone();
    parameters.path = os_model::WallPath::Straight {
        start: Point2::new(-2.0, 0.0),
        end: Point2::new(4.0, 0.0),
    };
    app.editor
        .command(
            "Source geometry mutation",
            Command::UpdateWall {
                id: wall_id,
                parameters,
            },
        )
        .unwrap();
    assert_eq!(app.editor.document.model().drawing_issues[&issue], issued);
    assert!(app.editor.document.undo());
    assert_eq!(app.editor.document.model().drawing_issues[&issue], issued);
    assert!(!app.editor.document.undo());
    assert!(app.editor.document.redo());
    let project = temp.path().join("project.osm");
    ZipJsonStorage.save(&app.editor.document, &project).unwrap();
    let reopened = ZipJsonStorage.open(&project).unwrap();
    assert_eq!(reopened.model().drawing_issues[&issue], issued);
    assert_eq!(
        reopened.model().sheet_sets[&set].sheets,
        pending
            .manifest
            .sheets
            .iter()
            .map(|s| s.id)
            .collect::<Vec<_>>()
    );
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    let next = draft(&mut app, set, Some(issue));
    let mut next_pending = settle(&mut app, next);
    acknowledge(&mut next_pending);
    app.review_issue_pending(&mut next_pending).unwrap();
    assert_ne!(
        next_pending.pdf, pending.pdf,
        "superseding PDF reflects the changed wall"
    );
    app.publish_issue_pending(
        &next_pending,
        temp.path().join("superseding.osissue").to_str().unwrap(),
    )
    .unwrap();
    assert_eq!(app.editor.document.model().drawing_issues[&issue], issued);
    assert_eq!(
        app.editor.document.model().drawing_issues[&next].supersedes,
        Some(issue)
    );
    // Independent review evidence, deliberately opt-in and outside source control.
    if let Some(dir) = std::env::var_os("OS_ISSUE_EVIDENCE_DIR") {
        let dir = PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("ordered.osissue"), &bytes).unwrap();
        std::fs::write(dir.join("ordered.pdf"), bundle.pdf()).unwrap();
        std::fs::write(
            dir.join("manifest.json"),
            bundle.manifest().bytes().unwrap(),
        )
        .unwrap();
        std::fs::write(dir.join("superseding.pdf"), &next_pending.pdf).unwrap();
    }
}

#[test]
fn drawing_issue_stale_failed_interrupted_and_no_clobber_publication_are_atomic() {
    let (mut app, _, issue) = fixture();
    let mut p = settle(&mut app, issue);
    acknowledge(&mut p);
    app.editor
        .command(
            "Invalidate preflight",
            Command::RenameProject("Changed".into()),
        )
        .unwrap();
    assert!(app.review_issue_pending(&mut p).is_err());
    let mut p = settle(&mut app, issue);
    acknowledge(&mut p);
    app.review_issue_pending(&mut p).unwrap();
    app.editor.document.drain_events();
    let before = app.editor.document.model().clone();
    let history = app.editor.document.history_stats();
    let rev = app.editor.document.revision();
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("issue.osissue");
    std::fs::write(&path, b"existing immutable package").unwrap();
    assert!(
        app.publish_issue_pending(&p, path.to_str().unwrap())
            .is_err()
    );
    assert_eq!(std::fs::read(&path).unwrap(), b"existing immutable package");
    assert!(
        app.publish_issue_pending(
            &p,
            temp.path().join("missing/issue.osissue").to_str().unwrap()
        )
        .is_err()
    );
    let interrupted = temp.path().join("interrupted.osissue");
    assert!(
        app.editor
            .document
            .publish_drawing_issue(
                issue,
                p.manifest.source_session,
                rev,
                interrupted.to_string_lossy().into_owned(),
                |_| {
                    let mut t = tempfile::NamedTempFile::new_in(temp.path()).unwrap();
                    t.write_all(b"partial").unwrap();
                    Err(Error::Storage("interrupted before publish".into()))
                }
            )
            .is_err()
    );
    assert!(!interrupted.exists());
    assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 1);
    assert_eq!(app.editor.document.model(), &before);
    assert_eq!(app.editor.document.revision(), rev);
    assert_eq!(app.editor.document.history_stats(), history);
    assert!(app.editor.document.drain_events().is_empty());
    // Reserve fails before the publication callback if history cannot fit.
    app.editor
        .document
        .set_history_limits(os_document::HistoryLimits {
            max_entries: 1,
            max_estimated_bytes: history.estimated_bytes,
        })
        .unwrap();
    let single = app.editor.document.history_stats().estimated_bytes;
    app.editor
        .document
        .set_history_limits(os_document::HistoryLimits {
            max_entries: 1,
            max_estimated_bytes: single,
        })
        .unwrap();
    let mut invoked = false;
    assert!(
        app.editor
            .document
            .publish_drawing_issue(
                issue,
                p.manifest.source_session,
                rev,
                "unused.osissue".into(),
                |_| {
                    invoked = true;
                    Ok(())
                }
            )
            .is_err()
    );
    assert!(!invoked);
    app.editor
        .document
        .set_history_limits(history.limits)
        .unwrap();
    app.editor
        .command(
            "Invalidate review",
            Command::RenameProject("Changed again".into()),
        )
        .unwrap();
    assert!(
        app.publish_issue_pending(&p, interrupted.to_str().unwrap())
            .is_err()
    );
    assert!(!interrupted.exists());
}

#[test]
fn drawing_issue_preflight_blocks_missing_stale_unavailable_and_overflow_graphics() {
    let (mut app, _, issue) = fixture();
    assert!(app.preflight_drawing_issue(issue).is_err()); // required second source has never been requested
    let _ = settle(&mut app, issue);
    let view = app.plans.active.unwrap();
    let context = app.editor.native_view_context(view).unwrap();
    app.plans.drawing =
        Some(PlanDrawing::from_prisms(context, &BTreeMap::new(), vec![Id::new()]).unwrap());
    assert!(
        app.preflight_drawing_issue(issue)
            .unwrap_err()
            .to_string()
            .contains("unavailable")
    );
    let mut stale = context;
    stale.model_revision += 1;
    app.plans.drawing = Some(PlanDrawing::from_prisms(stale, &BTreeMap::new(), vec![]).unwrap());
    assert!(app.preflight_drawing_issue(issue).is_err());
    app.plans.drawing = Some(
        PlanDrawing::from_prisms(context, &BTreeMap::new(), vec![])
            .unwrap()
            .with_grids(vec![os_render::plan::PlanGrid {
                entity: Id::new(),
                name: "Overflow".into(),
                start: Point2::new(-100.0, 0.0),
                end: Point2::new(100.0, 0.0),
            }])
            .unwrap(),
    );
    assert!(
        app.preflight_drawing_issue(issue)
            .unwrap_err()
            .to_string()
            .contains("overflow")
    );
    // The live preview still intentionally clips the same source.
    assert!(app.sheet_page().is_ok());
}

struct UiHarness {
    app: DesktopApp,
    ctx: egui::Context,
    size: egui::Vec2,
    scale: f32,
    time: f64,
}
impl UiHarness {
    fn frame(&mut self, events: Vec<egui::Event>) -> egui::FullOutput {
        self.time += 0.1;
        self.ctx
            .run(raw_input(self.size, self.scale, self.time, events), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| self.app.drawing_issues_ui(ui));
            })
    }
    fn click(&mut self, label: &str) {
        self.frame(vec![]);
        for _ in 0..20 {
            let out = self.frame(vec![]);
            let at = out.shapes.iter().rev().find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.job.text == label => {
                    let at = egui::Rect::from_min_size(t.pos, t.galley.size()).center();
                    (s.clip_rect.contains(at)
                        && egui::Rect::from_min_size(egui::Pos2::ZERO, self.size).contains(at))
                    .then_some(at)
                }
                _ => None,
            });
            if let Some(at) = at {
                for pressed in [true, false] {
                    self.frame(vec![
                        egui::Event::PointerMoved(at),
                        egui::Event::PointerButton {
                            pos: at,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ]);
                }
                return;
            }
            self.frame(vec![
                egui::Event::PointerMoved(egui::pos2(400.0, 300.0)),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0.0, -180.0),
                    modifiers: egui::Modifiers::NONE,
                },
            ]);
        }
        panic!("visible control not found: {label}");
    }
}

#[test]
fn drawing_issue_real_egui_issue_and_typed_set_name_at_both_display_profiles() {
    for (size, scale) in [
        (egui::vec2(1280.0, 800.0), 1.0),
        (egui::vec2(1000.0, 650.0), 1.5),
    ] {
        let (app, set, issue) = fixture();
        let mut h = UiHarness {
            app,
            ctx: egui::Context::default(),
            size,
            scale,
            time: 0.0,
        };
        theme::apply(&h.ctx);
        let _ = settle(&mut h.app, issue);
        h.click("Drawing sets and issues");
        assert!(h.app.plans.issues.open);
        h.app.plans.issues.set = Some(set);
        h.app.plans.issues.issue = Some(issue);
        h.frame(vec![]);
        h.frame(vec![]);
        h.click("Acceptance set");
        h.frame(vec![
            egui::Event::Key {
                key: egui::Key::Home,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::Text("Renamed ".into()),
        ]);
        h.frame(vec![]);
        h.frame(vec![]);
        h.click("Apply set name");
        assert_eq!(
            h.app.editor.document.model().sheet_sets[&set].name,
            "Renamed Acceptance set"
        );
        let _ = settle(&mut h.app, issue);
        h.click("Preflight all sheets");
        assert!(!h.app.status_error, "{}", h.app.status);
        assert!(h.app.plans.issues.pending.is_some());
        // Review decisions/path are injected; all workflow actions use pointer events.
        acknowledge(h.app.plans.issues.pending.as_mut().unwrap());
        h.click("Accept review");
        assert_eq!(
            h.app.editor.document.model().drawing_issues[&issue].state,
            IssueState::Review,
            "{}",
            h.app.status
        );
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("egui.osissue");
        h.app.plans.issues.path = path.to_string_lossy().into_owned();
        h.click("Issue frozen package");
        assert!(!h.app.status_error, "{}", h.app.status);
        assert_eq!(
            h.app.editor.document.model().drawing_issues[&issue].state,
            IssueState::Issued
        );
        assert_eq!(
            IssueBundle::read(&path).unwrap().manifest().sheets[0].number,
            "A202"
        );
    }
}
