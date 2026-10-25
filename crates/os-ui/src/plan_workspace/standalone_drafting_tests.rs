//! Standalone drafting pointer editing at both supported DPI profiles.
use super::*;

const PROFILES: [(egui::Vec2, f32); 2] = [
    (egui::vec2(1280.0, 800.0), 1.0),
    (egui::vec2(1000.0, 650.0), 1.5),
];

struct Harness {
    app: DesktopApp,
    ctx: egui::Context,
    size: egui::Vec2,
    scale: f32,
    time: f64,
    view: Id,
}
impl Harness {
    fn new(size: egui::Vec2, scale: f32) -> Self {
        let mut app = DesktopApp::new().unwrap();
        let view = app
            .editor
            .create_drafting_view("Standalone drafting")
            .unwrap();
        app.editor.document = Document::from_model(app.editor.document.model().clone()).unwrap();
        app.plans.poll(&app.editor);
        app.focus_plan(Some(view));
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let mut h = Self {
            app,
            ctx,
            size,
            scale,
            time: 0.0,
            view,
        };
        h.settle();
        h
    }
    fn frame(&mut self, events: Vec<egui::Event>) -> egui::FullOutput {
        self.time += 1.0 / 60.0;
        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, self.size)),
            time: Some(self.time),
            events,
            ..Default::default()
        };
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .native_pixels_per_point = Some(self.scale);
        self.ctx.run(input, |ctx| self.app.show(ctx))
    }
    fn settle(&mut self) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            self.frame(vec![]);
            if self.app.plans.ready() && !self.app.editor.plugin_work_pending() {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "detail drawing did not settle"
            );
            std::thread::yield_now();
        }
        self.frame(vec![]);
    }
    fn point(&self, world: Point2) -> egui::Pos2 {
        let rect = self.app.plans.canvas_rect.unwrap();
        let context = self.app.editor.native_view_context(self.view).unwrap();
        let p = self.app.plans.cameras[&self.view]
            .project(
                context.basis.world_to_plane(world).unwrap(),
                [f64::from(rect.width()), f64::from(rect.height())],
            )
            .unwrap();
        rect.min + egui::vec2(p.x as f32, p.y as f32)
    }
    fn click(&mut self, point: egui::Pos2) {
        self.frame(vec![egui::Event::PointerMoved(point)]);
        self.frame(vec![button(point, true)]);
        self.frame(vec![button(point, false)]);
    }
    fn drag(&mut self, from: egui::Pos2, to: egui::Pos2) {
        self.frame(vec![egui::Event::PointerMoved(from)]);
        self.frame(vec![button(from, true)]);
        self.frame(vec![egui::Event::PointerMoved(to)]);
        self.frame(vec![button(to, false)]);
    }
}
fn button(pos: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}
fn escape() -> egui::Event {
    egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}

#[test]
fn two_click_preview_does_not_mutate_then_creates_one_line_at_both_dpis() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        assert_eq!(h.ctx.pixels_per_point(), scale);
        let before = h.app.editor.document.model().clone();
        let revision = h.app.editor.document.revision();
        h.app.begin_detail_line();
        h.frame(vec![]);
        h.click(h.point(Point2::new(-1.0, 0.0)));
        h.frame(vec![egui::Event::PointerMoved(
            h.point(Point2::new(1.0, 0.0)),
        )]);
        assert_eq!(h.app.editor.document.model(), &before);
        assert_eq!(h.app.editor.document.revision(), revision);
        assert!(h.app.plans.detail_line_draft.is_some());
        h.click(h.point(Point2::new(1.0, 0.0)));
        h.settle();
        assert_eq!(h.app.editor.document.model().detail_lines.len(), 1);
        assert_eq!(h.app.editor.document.revision(), revision + 1);
        let committed = h.app.editor.document.model().clone();
        assert!(h.app.editor.document.undo());
        assert_eq!(h.app.editor.document.model(), &before);
        assert!(h.app.editor.document.redo());
        assert_eq!(h.app.editor.document.model(), &committed);
    }
}

#[test]
fn escape_cancels_unfinished_two_click_draft_at_both_dpis() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        let before = h.app.editor.document.model().clone();
        h.app.begin_detail_line();
        h.frame(vec![]);
        h.click(h.point(Point2::new(-1.0, 0.0)));
        assert!(h.app.plans.detail_line_draft.is_some());
        h.frame(vec![escape()]);
        assert!(
            h.app.plans.detail_line_draft.is_none(),
            "Escape left detail-line draft active at {scale}x"
        );
        assert_eq!(h.app.editor.document.model(), &before);
    }
}

#[test]
fn selected_handle_body_and_blank_canvas_have_distinct_pointer_owners() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        let line = os_model::DetailLine::new(
            "core.detail_line",
            os_model::DetailLineParams {
                view: h.view,
                start: Point2::new(-1.0, 0.0),
                end: Point2::new(1.0, 0.0),
            },
        );
        let id = line.id();
        h.app
            .editor
            .command("Fixture", Command::AddDetailLine(line))
            .unwrap();
        h.settle();
        h.app.select(Some(id));
        h.frame(vec![]);
        let center = h.app.plans.cameras[&h.view].center;
        let before = h.app.editor.document.model().clone();
        h.drag(
            h.point(Point2::new(-1.0, 0.0)),
            h.point(Point2::new(-1.5, 0.5)),
        );
        h.settle();
        assert_eq!(
            h.app.plans.cameras[&h.view].center, center,
            "handle drag panned canvas at {scale}x"
        );
        assert_ne!(h.app.editor.document.model(), &before);
        assert_eq!(h.app.editor.document.model().detail_lines[&id].id(), id);
        let after_handle = h.app.editor.document.model().clone();
        let edited = &after_handle.detail_lines[&id].parameters;
        let middle = Point2::new(
            (edited.start.x + edited.end.x) * 0.5,
            (edited.start.y + edited.end.y) * 0.5,
        );
        h.drag(
            h.point(middle),
            h.point(Point2::new(middle.x, middle.y + 0.5)),
        );
        h.settle();
        assert_eq!(
            h.app.plans.cameras[&h.view].center, center,
            "body drag panned canvas at {scale}x"
        );
        assert_ne!(h.app.editor.document.model(), &after_handle);
        let after_body = h.app.editor.document.model().clone();
        let end = after_body.detail_lines[&id].parameters.end;
        h.drag(h.point(end), h.point(Point2::new(end.x + 0.5, end.y)));
        h.settle();
        assert_eq!(h.app.plans.cameras[&h.view].center, center);
        assert_ne!(h.app.editor.document.model(), &after_body);
        let after_end = h.app.editor.document.model().clone();
        h.drag(
            h.point(Point2::new(0.0, -0.5)),
            h.point(Point2::new(0.2, -0.7)),
        );
        assert_ne!(
            h.app.plans.cameras[&h.view].center, center,
            "blank drag did not pan at {scale}x"
        );
        assert_eq!(h.app.editor.document.model(), &after_end);
        h.app.select(Some(id));
        h.frame(vec![]);
        let output = h.frame(vec![]);
        let button = output
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.job.text == "Delete detail line" => {
                    Some(egui::Rect::from_min_size(t.pos, t.galley.size()).center())
                }
                _ => None,
            })
            .expect("detail line inspector delete button");
        h.click(button);
        assert!(!h.app.editor.document.model().detail_lines.contains_key(&id));
        h.app.history(false);
        assert_eq!(h.app.editor.document.model(), &after_end);
    }
}

#[test]
fn collapsing_release_and_stale_revision_do_not_commit_a_drag() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        let line = os_model::DetailLine::new(
            "core.detail_line",
            os_model::DetailLineParams {
                view: h.view,
                start: Point2::new(-1.0, 0.0),
                end: Point2::new(1.0, 0.0),
            },
        );
        let id = line.id();
        h.app
            .editor
            .command("Fixture", Command::AddDetailLine(line))
            .unwrap();
        h.settle();
        h.app.select(Some(id));
        h.frame(vec![]);
        let before = h.app.editor.document.model().clone();
        let revision = h.app.editor.document.revision();
        h.drag(
            h.point(Point2::new(-1.0, 0.0)),
            h.point(Point2::new(1.0, 0.0)),
        );
        assert_eq!(
            h.app.editor.document.model(),
            &before,
            "collapsed release changed model at {scale}x"
        );
        assert_eq!(h.app.editor.document.revision(), revision);
        h.frame(vec![]);
        let start = h.point(Point2::new(-1.0, 0.0));
        let target = h.point(Point2::new(-1.5, 0.5));
        h.frame(vec![egui::Event::PointerMoved(start)]);
        h.frame(vec![button(start, true)]);
        h.frame(vec![egui::Event::PointerMoved(target)]);
        assert!(h.app.plans.detail_line_draft.is_some());
        let external = os_model::DetailLine::new(
            "core.detail_line",
            os_model::DetailLineParams {
                view: h.view,
                start: Point2::new(3.0, 0.0),
                end: Point2::new(4.0, 0.0),
            },
        );
        h.app
            .editor
            .command("External edit", Command::AddDetailLine(external))
            .unwrap();
        let after_external = h.app.editor.document.model().clone();
        let external_revision = h.app.editor.document.revision();
        h.frame(vec![button(target, false)]);
        assert!(h.app.plans.detail_line_draft.is_none());
        assert_eq!(h.app.editor.document.model(), &after_external);
        assert_eq!(h.app.editor.document.revision(), external_revision);
    }
}

#[test]
fn drafting_detail_line_camera_and_canvas_changes_cancel_claimed_drags() {
    for (size, scale) in PROFILES {
        for resize in [false, true] {
            let mut h = Harness::new(size, scale);
            let line = os_model::DetailLine::new(
                "core.detail_line",
                os_model::DetailLineParams {
                    view: h.view,
                    start: Point2::new(-1.0, 0.0),
                    end: Point2::new(1.0, 0.0),
                },
            );
            let id = line.id();
            h.app
                .editor
                .command("Fixture", Command::AddDetailLine(line))
                .unwrap();
            h.settle();
            h.app.select(Some(id));
            h.frame(vec![]);
            let before = h.app.editor.document.model().clone();
            let history = h.app.editor.document.history_stats();
            let start = h.point(Point2::new(-1.0, 0.0));
            let end = h.point(Point2::new(-0.8, 0.3));
            h.frame(vec![egui::Event::PointerMoved(start), button(start, true)]);
            h.frame(vec![egui::Event::PointerMoved(end)]);
            assert!(h.app.plans.detail_line_draft.is_some());
            if resize {
                h.size += egui::vec2(120.0, 80.0);
            } else {
                h.app.plans.cameras.get_mut(&h.view).unwrap().center.x += 0.25;
            }
            h.frame(vec![]);
            assert!(h.app.plans.detail_line_draft.is_none());
            h.frame(vec![button(end, false)]);
            assert_eq!(h.app.editor.document.model(), &before);
            assert_eq!(h.app.editor.document.history_stats(), history);
        }
    }
}

#[test]
fn drafting_snapshot_excludes_model_and_foreign_annotations_clips_snaps_and_exports_vectors() {
    use os_render::{sheet::*, snapping::SnapQuery};
    let mut h = Harness::new(PROFILES[0].0, 1.);
    let foreign = h
        .app
        .editor
        .create_floor_plan("Foreign", h.app.active_level)
        .unwrap();
    let wall = os_model::Wall::new(
        os_walls::WALL_TYPE,
        os_model::WallParams {
            name: "Invisible wall".into(),
            path: os_model::WallPath::Straight {
                start: Point2::new(-2., 2.),
                end: Point2::new(2., 2.),
            },
            thickness: 0.2,
            height: 3.,
            level: h.app.active_level,
            material: None,
        },
    );
    let line = os_model::DetailLine::new(
        "core.detail_line",
        os_model::DetailLineParams {
            view: h.view,
            start: Point2::new(-8., 0.),
            end: Point2::new(8., 0.),
        },
    );
    let id = line.id();
    let other = os_model::DetailLine::new(
        "core.detail_line",
        os_model::DetailLineParams {
            view: foreign,
            start: Point2::new(-2., 2.),
            end: Point2::new(2., 2.),
        },
    );
    let mut p = os_model::TextNoteParams::new(h.view);
    p.text = "AB".into();
    p.position = Point2::new(-1., 1.);
    p.text_height_mm = 2.;
    let note = os_model::TextNote::new("core.text_note", p);
    h.app
        .editor
        .commands(
            "Sources",
            vec![
                Command::AddWall(wall),
                Command::AddDetailLine(line),
                Command::AddDetailLine(other),
                Command::AddTextNote(note),
            ],
        )
        .unwrap();
    h.app.plans.split = true;
    h.app.focus_plan(Some(h.view));
    h.settle();
    assert!(!h.app.plans.split);
    assert!(h.app.editor.native_plan_context(h.view).is_err());
    let c = h.app.editor.native_view_context(h.view).unwrap();
    assert!(!c.show_walls && !c.show_extensions);
    let frozen = h.app.editor.native_view_snapshot(h.view).unwrap();
    h.app
        .editor
        .command("Later edit", Command::RemoveDetailLine(id))
        .unwrap();
    let d = frozen.derive().unwrap();
    assert!(d.items(c).unwrap().is_empty());
    assert!(d.provider_lines(c).unwrap().is_empty());
    assert_eq!(d.pick(c, Point2::new(0., 2.)).unwrap(), None);
    let lines = d.detail_lines(c).unwrap();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].entity, id);
    assert_eq!(lines[0].start, Point2::new(-5., 0.));
    assert_eq!(lines[0].end, Point2::new(5., 0.));
    let camera = os_render::plan::PlanCamera {
        center: Point2::default(),
        pixels_per_metre: 50.,
    };
    for (point, expected) in [
        (Point2::new(0., 0.), Some(id)),
        (Point2::new(0., 2.), None),
        (Point2::new(8., 0.), None),
    ] {
        let q = SnapQuery {
            camera,
            viewport: [800., 600.],
            pointer: camera.project(point, [800., 600.]).unwrap(),
            radius_pixels: 8.,
            endpoints: true,
            midpoints: true,
            intersections: true,
            perpendicular_from: None,
            nearest: true,
            axis_extensions: false,
            exclude_entity: None,
        };
        assert_eq!(
            d.snap(c, q)
                .unwrap()
                .candidate(c, q)
                .unwrap()
                .map(|s| s.entity),
            expected
        );
    }
    for scale in [25., 100.] {
        let vp = PaperViewport {
            center_mm: Point2::new(210., 148.5),
            width_mm: 300.,
            height_mm: 250.,
            model_center_m: Point2::default(),
            scale_denominator: scale,
        };
        let page = compose_view_sheet(
            PaperSheetInfo {
                width_mm: 420.,
                height_mm: 297.,
                number: "D1",
                name: "Drafting",
            },
            "Standalone",
            vp,
            c,
            &d,
        )
        .unwrap();
        let pdf = String::from_utf8(page.to_pdf().unwrap()).unwrap();
        for (anchor, ch) in d.text_notes(c).unwrap()[0].layout(scale).unwrap().glyphs {
            let p = vp.model_to_paper(anchor).unwrap();
            assert!(page.marks().iter().any(|m|matches!(&m.kind,PaperMarkKind::Text {baseline_mm,text,size_mm,..} if *baseline_mm==p && text==&ch.to_string() && *size_mm==2.)));
            assert!(pdf.contains(&format!(
                "1 0 0 1 {:.6} {:.6} Tm <{:02X}>",
                p.x * 72. / 25.4,
                (297. - p.y) * 72. / 25.4,
                ch as u32
            )));
        }
        assert!(!pdf.contains("/Subtype /Image"));
    }
}
