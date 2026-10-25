use super::*;
use crate::plan_workspace::sheet_tests::raw_input;
use os_model::*;
#[path = "../../../../os-document/tests/support/hidden_openings.rs"]
mod support;

fn app() -> (DesktopApp, Id, Vec<Id>) {
    let (m, view, ids) = support::fixture();
    let mut app = DesktopApp::new().unwrap();
    app.editor.document = os_document::Document::from_model(m).unwrap();
    app.plans.poll(&app.editor);
    app.focus_plan(Some(view));
    (app, view, ids)
}

#[test]
fn hidden_openings_straight_signed_arcs_apertures_annotations_picks_overlap_snaps_and_pdf() {
    let (mut app, view, ids) = app();
    let other = app
        .editor
        .create_floor_plan("Other", app.active_level)
        .unwrap();
    let mut annotation_ids = Vec::new();
    for id in &ids {
        let tag = OpeningTag::new(
            "core.opening_tag",
            OpeningTagParams {
                view,
                opening: *id,
                position: Point2::new(2., -1.),
                label_preset: Default::default(),
            },
        );
        let dimension = Dimension::new(
            "core.dimension",
            DimensionParams {
                view,
                first: DimensionReference::OpeningJamb {
                    opening: *id,
                    jamb: DimensionJamb::Start,
                },
                second: DimensionReference::OpeningJamb {
                    opening: *id,
                    jamb: DimensionJamb::End,
                },
                additional: vec![],
                layout: DimensionLayout::Aligned,
                baseline_spacing_m: 0.25,
                offset_m: 0.4,
                orphan_hint: Point2::new(2., -0.4),
            },
        );
        annotation_ids.extend([tag.id(), dimension.id()]);
        app.editor
            .commands(
                "Annotations",
                vec![
                    Command::AddOpeningTag(tag),
                    Command::AddDimension(dimension),
                ],
            )
            .unwrap();
    }
    let before = app.editor.document.model().clone();
    let c0 = app.editor.native_plan_context(view).unwrap();
    let d0 = app.editor.native_drawing(view).unwrap();
    assert!(ids.iter().all(|id| d0.is_native_line(*id)));
    app.editor
        .command(
            "Hide",
            Command::SetPlanHiddenOpenings {
                view,
                hidden: ids.iter().copied().collect(),
            },
        )
        .unwrap();
    let c = app.editor.native_plan_context(view).unwrap();
    let d = app.editor.native_drawing(view).unwrap();
    assert_eq!(
        d.items(c).unwrap(),
        d0.items(c0).unwrap(),
        "wall apertures must be byte-for-byte identical"
    );
    assert_eq!(app.editor.document.model().walls, before.walls);
    assert_eq!(app.editor.document.model().openings, before.openings);
    assert!(d.opening_tags(c).unwrap().is_empty());
    assert!(d.dimensions(c).unwrap().is_empty());
    let other_drawing = app.editor.native_drawing(other).unwrap();
    assert!(ids.iter().all(|id| other_drawing.is_native_line(*id)));
    let camera = PlanCamera {
        center: Point2::new(16., 0.),
        ..PlanCamera::default()
    };
    let size = [1280., 800.];
    for line in d0
        .provider_lines(c0)
        .unwrap()
        .iter()
        .filter(|l| ids.contains(&l.entity))
    {
        assert!(!d.is_native_line(line.entity));
        let pointer = camera.project(line.start, size).unwrap();
        assert!(
            d.pick_screen(c, camera, size, pointer, 1.)
                .unwrap()
                .is_none_or(|id| !ids.contains(&id) && !annotation_ids.contains(&id))
        );
        assert!(
            d.hits_screen(c, camera, size, pointer, 1.)
                .unwrap()
                .iter()
                .all(|id| !ids.contains(id) && !annotation_ids.contains(id))
        );
        let q = os_render::snapping::SnapQuery {
            camera,
            viewport: size,
            pointer,
            radius_pixels: 1.,
            endpoints: true,
            midpoints: true,
            intersections: true,
            perpendicular_from: None,
            nearest: true,
            axis_extensions: false,
            exclude_entity: None,
        };
        assert!(
            d.snap(c, q)
                .unwrap()
                .candidate(c, q)
                .unwrap()
                .is_none_or(|s| !ids.contains(&s.entity))
        );
    }
    let page = |drawing: &PlanDrawing, context| {
        os_render::sheet::compose_view_sheet(
            os_render::sheet::PaperSheetInfo {
                width_mm: 420.,
                height_mm: 297.,
                number: "A1",
                name: "Hide",
            },
            "Plan",
            os_render::sheet::PaperViewport {
                center_mm: Point2::new(210., 148.5),
                width_mm: 380.,
                height_mm: 250.,
                model_center_m: Point2::new(16., 0.),
                scale_denominator: 100.,
            },
            context,
            drawing,
        )
        .unwrap()
        .to_pdf()
        .unwrap()
    };
    let original_pdf = page(&d0, c0);
    let hidden_pdf = page(&d, c);
    assert!(hidden_pdf.starts_with(b"%PDF"));
    assert_ne!(original_pdf, hidden_pdf);
    app.editor
        .command(
            "Restore",
            Command::SetPlanHiddenOpenings {
                view,
                hidden: Default::default(),
            },
        )
        .unwrap();
    let restored = app.editor.native_drawing(view).unwrap();
    let rc = app.editor.native_plan_context(view).unwrap();
    assert_eq!(page(&restored, rc), original_pdf);
    // Restore must not override category visibility.
    let mut p = before.views[&view].parameters.clone();
    p.plan.as_mut().unwrap().visibility.doors = false;
    app.editor
        .command(
            "Category",
            Command::UpdateView {
                id: view,
                parameters: p,
            },
        )
        .unwrap();
    let category = app.editor.native_drawing(view).unwrap();
    assert!(
        ids.iter()
            .step_by(2)
            .all(|id| !category.is_native_line(*id))
    );
}

struct Harness {
    app: DesktopApp,
    ctx: egui::Context,
    size: egui::Vec2,
    dpi: f32,
    time: f64,
}
impl Harness {
    fn frame(&mut self, events: Vec<egui::Event>) -> egui::FullOutput {
        self.time += 0.1;
        self.ctx
            .run(raw_input(self.size, self.dpi, self.time, events), |ctx| {
                self.app.show(ctx)
            })
    }
    fn settle(&mut self) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            self.frame(vec![]);
            if self.app.plans.ready() {
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        self.frame(vec![]);
    }
    fn click(&mut self, p: egui::Pos2) {
        self.frame(vec![egui::Event::PointerMoved(p)]);
        for pressed in [true, false] {
            self.frame(vec![button(p, pressed, false)]);
        }
    }
    fn label(&mut self, label: &str) {
        self.frame(vec![]);
        let out = self.frame(vec![]);
        let p = out
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.job.text == label => {
                    Some(egui::Rect::from_min_size(t.pos, t.galley.size()).center())
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing {label}"));
        assert!(egui::Rect::from_min_size(egui::Pos2::ZERO, self.size).contains(p));
        self.click(p);
    }
}
fn button(p: egui::Pos2, pressed: bool, shift: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos: p,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers {
            shift,
            ..Default::default()
        },
    }
}
fn key(key: egui::Key) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}

#[test]
fn hidden_openings_dialog_both_profiles_restore_cancel_stale_input_and_marquee() {
    for (size, dpi) in [
        (egui::vec2(1280., 800.), 1.),
        (egui::vec2(1000., 650.), 1.5),
    ] {
        let (app, view, ids) = app();
        let mut h = Harness {
            app,
            ctx: egui::Context::default(),
            size,
            dpi,
            time: 0.,
        };
        theme::apply(&h.ctx);
        h.settle();
        h.app.select(Some(ids[0]));
        let before = h.app.editor.document.model().clone();
        h.app.begin_detail_line();
        h.label("Hide openings");
        h.settle();
        assert!(h.app.selected_ids.is_empty());
        assert!(h.app.plans.detail_line_draft.is_none());
        let hidden = h.app.editor.document.model().clone();
        let history = h.app.editor.document.history_stats();
        assert_ne!(hidden, before);
        h.label("Hidden openings…");
        h.frame(vec![key(egui::Key::Delete), key(egui::Key::D)]);
        let canvas = h.app.plans.canvas_rect.unwrap();
        h.click(canvas.left_bottom() + egui::vec2(20., -20.));
        assert!(h.app.selected_ids.is_empty());
        assert!(h.app.plans.detail_line_draft.is_none());
        let name = format!("Door 0 · Door\n{}", ids[0]);
        h.label(&name);
        h.label("Restore selected");
        assert_eq!(h.app.editor.document.model(), &hidden);
        h.label("Cancel restore");
        assert_eq!(h.app.editor.document.model(), &hidden);
        assert_eq!(h.app.editor.document.history_stats(), history);
        h.label("Hidden openings…");
        h.label("Restore all");
        h.frame(vec![key(egui::Key::Escape)]);
        assert!(h.app.plans.hidden_openings.is_none());
        assert_eq!(h.app.editor.document.model(), &hidden);
        h.label("Hidden openings…");
        h.label("Restore all");
        h.label("Apply restore");
        h.settle();
        assert_eq!(h.app.editor.document.model(), &before);
        h.app.history(false);
        assert_eq!(h.app.editor.document.model(), &hidden);
        h.app.history(true);
        assert_eq!(h.app.editor.document.model(), &before);
        h.app.history(false);
        h.settle();
        // Shift marquee across the canvas cannot reacquire the hidden opening.
        let rect = h.app.plans.canvas_rect.unwrap().shrink(4.);
        let a = rect.left_top();
        let b = rect.right_bottom();
        h.frame(vec![egui::Event::PointerMoved(a), button(a, true, true)]);
        h.frame(vec![egui::Event::PointerMoved(b)]);
        h.frame(vec![button(b, false, true)]);
        assert!(!h.app.selected_ids.contains(&ids[0]));
        h.label("Hidden openings…");
        h.app
            .editor
            .command("External", Command::RenameProject("Changed".into()))
            .unwrap();
        let external = h.app.editor.document.model().clone();
        h.frame(vec![]);
        assert!(h.app.plans.hidden_openings.is_none());
        assert_eq!(h.app.editor.document.model(), &external);
        h.label("Hidden openings…");
        h.app.focus_plan(None);
        h.frame(vec![]);
        assert!(h.app.plans.hidden_openings.is_none());
        h.app.focus_plan(Some(view));
        h.app.begin_hidden_openings();
        h.app.editor.document = os_document::Document::from_model(external.clone()).unwrap();
        h.frame(vec![]);
        assert!(h.app.plans.hidden_openings.is_none());
        assert_eq!(h.app.editor.document.model(), &external);
    }
}

#[test]
fn hidden_openings_hide_revalidates_whole_selection_and_current_visibility() {
    let (mut app, view, ids) = app();
    let before = app.editor.document.model().clone();
    for selection in [
        BTreeSet::new(),
        BTreeSet::from([ids[0], ids[1]]),
        BTreeSet::from([ids[0], *before.walls.keys().next().unwrap()]),
        (0..257).map(|_| Id::new()).collect(),
    ] {
        app.selected_ids = selection;
        assert!(app.hide_selected_openings().is_err());
        assert_eq!(app.editor.document.model(), &before);
    }
    app.select(Some(ids[0]));
    app.hide_selected_openings().unwrap();
    let hidden = app.editor.document.model().clone();
    app.select(Some(ids[0]));
    assert!(app.hide_selected_openings().is_err());
    assert_eq!(app.editor.document.model(), &hidden);
    app.editor
        .command(
            "Restore",
            Command::SetPlanHiddenOpenings {
                view,
                hidden: Default::default(),
            },
        )
        .unwrap();
    let mut p = before.views[&view].parameters.clone();
    p.plan.as_mut().unwrap().visibility.doors = false;
    app.editor
        .command(
            "Category",
            Command::UpdateView {
                id: view,
                parameters: p,
            },
        )
        .unwrap();
    let category = app.editor.document.model().clone();
    assert!(app.hide_selected_openings().is_err());
    assert_eq!(app.editor.document.model(), &category);
}
