//! Real egui frames exercise menus, station mapping, placement, navigation and paper.
use super::*;
use os_model::{Wall, WallPath};
use std::time::{Duration, Instant};

struct Harness {
    app: DesktopApp,
    ctx: egui::Context,
    size: egui::Vec2,
    scale: f32,
    time: f64,
}

#[test]
fn elevation_dimensions_real_egui_both_dpis_preview_commit_repair_cancel_and_paper() {
    use os_model::{DimensionLayout as L, ElevationDimensionFeature as F};
    for (size, scale, side) in [
        (egui::vec2(1280., 800.), 1., ElevationSide::Left),
        (egui::vec2(1000., 650.), 1.5, ElevationSide::Right),
    ] {
        let mut app = DesktopApp::new().unwrap();
        let mut wp = default_wall(app.active_level);
        wp.path = WallPath::Straight {
            start: Point2::new(8., 11.),
            end: Point2::new(2., 3.),
        };
        wp.height = 4.;
        let wall = Wall::new(os_walls::WALL_TYPE, wp);
        let host = wall.id();
        app.editor.command("Host", Command::AddWall(wall)).unwrap();
        let view = app
            .editor
            .create_wall_elevation("Dimensions", host, side)
            .unwrap();
        app.plans.poll(&app.editor);
        app.focus_plan(Some(view));
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let mut h = Harness {
            app,
            ctx,
            size,
            scale,
            time: 0.,
        };
        h.settle();
        for (label, a, b, offset, value) in [
            (
                "Horizontal dimension",
                Point2::new(0., 2.),
                Point2::new(10., 2.),
                Point2::new(5., 3.),
                10.,
            ),
            (
                "Vertical dimension",
                Point2::new(5., 0.),
                Point2::new(5., 4.),
                Point2::new(7., 2.),
                4.,
            ),
        ] {
            let before = h.app.editor.document.model().clone();
            let history = h.app.editor.document.history_stats();
            let scene = h.app.editor.scene.clone();
            h.click(label);
            h.click_at(h.point(a));
            h.click_at(h.point(b));
            h.frame(vec![egui::Event::PointerMoved(h.point(offset))]);
            assert_eq!(h.app.editor.document.model(), &before);
            assert_eq!(h.app.editor.document.history_stats(), history);
            assert_eq!(h.app.editor.scene, scene);
            h.click_at(h.point(offset));
            h.settle();
            assert_eq!(
                h.app.editor.document.model().dimensions.len(),
                before.dimensions.len() + 1,
                "{}",
                h.app.status
            );
            let id = h.app.selected.unwrap();
            assert_eq!(
                h.app.editor.document.model().dimensions[&id]
                    .parameters
                    .resolve(h.app.editor.document.model())
                    .unwrap()
                    .length_metres,
                value
            );
            let committed = h.app.editor.document.model().clone();
            h.app.history(false);
            h.settle();
            assert_eq!(h.app.editor.document.model(), &before);
            h.app.history(true);
            h.settle();
            assert_eq!(h.app.editor.document.model(), &committed);
            h.app.select(Some(id));
            h.app.dimension_offset_draft = 0.2;
            h.app.apply_dimension_properties();
            h.settle();
            assert_eq!(
                h.app.editor.document.model().dimensions[&id]
                    .parameters
                    .offset_m,
                0.2
            );
        }
        let vertical = h.app.selected.unwrap();
        // Repair semantic references through actual canvas acquisition.
        h.app.begin_dimension_repair(0);
        h.click_at(h.point(Point2::new(5., 0.)));
        h.settle();
        assert!(h.app.plans.elevation.dimension.is_none());
        assert_eq!(
            h.app.editor.document.model().dimensions[&vertical]
                .parameters
                .first,
            DimensionReference::ElevationWall {
                wall: host,
                feature: F::Bottom
            }
        );
        let original = h.app.editor.document.model().clone();
        let history = h.app.editor.document.history_stats();
        h.click("Horizontal dimension");
        h.click_at(h.point(Point2::new(0., 2.)));
        h.escape();
        assert!(h.app.plans.elevation.dimension.is_none());
        assert_eq!(h.app.editor.document.model(), &original);
        assert_eq!(h.app.editor.document.history_stats(), history);
        // Wrong-axis/no-edge pick is a no-op; revisions cancel in-progress work.
        h.click("Horizontal dimension");
        h.click_at(h.point(Point2::new(5., 2.)));
        assert_eq!(h.app.editor.document.model(), &original);
        h.app.dimension_offset_draft = 0.3;
        h.app.apply_dimension_properties();
        h.settle();
        assert!(h.app.plans.elevation.dimension.is_none());
        h.app.begin_elevation_dimension(L::ElevationHorizontal);
        h.app.focus_plan(None);
        h.frame(vec![]);
        assert!(h.app.plans.elevation.dimension.is_none());
        h.app.focus_plan(Some(view));
        h.settle();
        let context = h.app.editor.native_elevation_context(view).unwrap();
        let drawing = h.app.editor.native_drawing(view).unwrap();
        let dimensions = drawing.dimensions(context).unwrap();
        assert_eq!(dimensions.len(), 2);
        h.click("Sheets");
        h.click("New sheet from view");
        h.settle();
        let page = h.app.sheet_page().unwrap();
        for value in ["10.000 m", "4.000 m"] {
            assert!(page.marks().iter().any(|m| matches!(&m.kind,os_render::sheet::PaperMarkKind::Text { text,.. } if text==value)),"missing {value}");
            let encoded: String = value.bytes().map(|b| format!("{b:02X}")).collect();
            assert!(
                String::from_utf8_lossy(&page.to_pdf().unwrap())
                    .contains(&format!("<{encoded}> Tj"))
            );
        }
        h.app.focus_plan(Some(view));
        h.settle();
        h.app.select(Some(vertical));
        let before = h.app.editor.document.model().clone();
        h.app.delete_selected_dimension();
        h.settle();
        assert!(
            !h.app
                .editor
                .document
                .model()
                .dimensions
                .contains_key(&vertical)
        );
        h.app.history(false);
        h.settle();
        assert_eq!(h.app.editor.document.model(), &before);
    }
}
impl Harness {
    fn frame(&mut self, events: Vec<egui::Event>) -> egui::FullOutput {
        self.time += 1. / 60.;
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
        self.ctx.run(input, |ctx| {
            self.app.plan_workspace(ctx);
            self.app.opening_dialog(ctx);
        })
    }
    fn settle(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            self.frame(vec![]);
            if self.app.plans.ready() {
                break;
            }
            assert!(Instant::now() < deadline, "{:?}", self.app.plans.error);
            std::thread::yield_now();
        }
        self.frame(vec![]);
    }
    fn click_at(&mut self, pos: egui::Pos2) {
        self.frame(vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        self.frame(vec![egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }]);
        self.frame(vec![]);
    }
    fn click(&mut self, label: &str) {
        self.frame(vec![]);
        let output = self.frame(vec![]);
        let p = output
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.job.text == label => {
                    Some(egui::Rect::from_min_size(t.pos, t.galley.size()).center())
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing visible action {label}; {}", self.app.status));
        assert!(
            egui::Rect::from_min_size(egui::Pos2::ZERO, self.size).contains(p),
            "{label} offscreen"
        );
        self.click_at(p);
    }
    fn point(&self, p: Point2) -> egui::Pos2 {
        let id = self.app.plans.active.unwrap();
        let rect = self.app.plans.canvas_rect.unwrap();
        let p = self.app.plans.cameras[&id]
            .project(p, [f64::from(rect.width()), f64::from(rect.height())])
            .unwrap();
        let pos = rect.min + egui::vec2(p.x as f32, p.y as f32);
        assert!(rect.contains(pos));
        pos
    }
    fn escape(&mut self) {
        self.frame(vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }]);
    }
}

#[test]
fn elevation_opening_dimensions_egui_orphan_repair_and_pan_precedence() {
    use os_model::*;
    for (size, scale) in [
        (egui::vec2(1280., 800.), 1.),
        (egui::vec2(1000., 650.), 1.5),
    ] {
        let mut app = DesktopApp::new().unwrap();
        let mut w = default_wall(app.active_level);
        w.path = WallPath::Straight {
            start: Point2::new(0., 0.),
            end: Point2::new(10., 0.),
        };
        w.height = 4.;
        let wall = Wall::new(os_walls::WALL_TYPE, w);
        let host = wall.id();
        app.editor.command("Wall", Command::AddWall(wall)).unwrap();
        let opening = Opening::new(
            "core.opening",
            OpeningParams {
                name: "Window".into(),
                host,
                offset: 3.,
                definition: OpeningDefinition::Legacy {
                    kind: OpeningKind::Window,
                    width: 2.,
                    height: 1.5,
                    sill: 1.,
                },
                width_override: None,
                height_override: None,
                sill_override: None,
                pane_position_override: None,
                lite_side_override: None,
                hinge: Default::default(),
                swing: Default::default(),
                open_state: Default::default(),
            },
        );
        let opening_id = opening.id();
        app.editor
            .command("Opening", Command::AddOpening(opening))
            .unwrap();
        let view = app
            .editor
            .create_wall_elevation("Window dimensions", host, ElevationSide::Left)
            .unwrap();
        app.plans.poll(&app.editor);
        app.focus_plan(Some(view));
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let mut h = Harness {
            app,
            ctx,
            size,
            scale,
            time: 0.,
        };
        h.settle();
        h.click("Horizontal dimension");
        let camera = h.app.plans.cameras[&view];
        let pos = h.point(Point2::new(7., 2.));
        h.frame(vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        h.frame(vec![egui::Event::PointerMoved(pos + egui::vec2(25., 15.))]);
        h.frame(vec![egui::Event::PointerButton {
            pos: pos + egui::vec2(25., 15.),
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }]);
        assert_eq!(camera, h.app.plans.cameras[&view]);
        h.click_at(h.point(Point2::new(3., 1.75)));
        h.click_at(h.point(Point2::new(5., 1.75)));
        h.click_at(h.point(Point2::new(4., 3.)));
        h.settle();
        let id = h.app.selected.unwrap();
        let p = &h.app.editor.document.model().dimensions[&id].parameters;
        assert!(
            matches!(p.first,DimensionReference::ElevationOpening { opening,.. } if opening==opening_id)
        );
        assert!(
            matches!(p.second,DimensionReference::ElevationOpening { opening,.. } if opening==opening_id)
        );
        assert_eq!(
            p.resolve(h.app.editor.document.model())
                .unwrap()
                .length_metres,
            2.
        );
        let header = h.app.editor.document.model().dimensions[&id].header.clone();
        h.app
            .editor
            .command("Delete opening", Command::RemoveOpening(opening_id))
            .unwrap();
        h.settle();
        let c = h.app.editor.native_elevation_context(view).unwrap();
        assert!(
            h.app.plans.drawing.as_ref().unwrap().dimensions(c).unwrap()[0]
                .diagnostic
                .is_some()
        );
        h.app.select(Some(id));
        h.app.begin_dimension_repair(0);
        h.click_at(h.point(Point2::new(0., 2.)));
        h.settle();
        assert!(h.app.plans.elevation.dimension.is_none());
        assert!(
            h.app.editor.document.model().dimensions[&id]
                .parameters
                .resolve(h.app.editor.document.model())
                .is_err()
        );
        h.app.begin_dimension_repair(1);
        h.click_at(h.point(Point2::new(10., 2.)));
        h.settle();
        assert!(h.app.plans.elevation.dimension.is_none());
        assert_eq!(
            h.app.editor.document.model().dimensions[&id]
                .parameters
                .resolve(h.app.editor.document.model())
                .unwrap()
                .length_metres,
            10.
        );
        assert_eq!(h.app.editor.document.model().dimensions[&id].header, header);
        // Ordinary body drag still pans after the annotation operation completes.
        let camera = h.app.plans.cameras[&view];
        let pos = h.point(Point2::new(7., 2.));
        h.frame(vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        h.frame(vec![egui::Event::PointerMoved(pos + egui::vec2(25., 15.))]);
        h.frame(vec![egui::Event::PointerButton {
            pos: pos + egui::vec2(25., 15.),
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }]);
        assert_ne!(camera, h.app.plans.cameras[&view]);
    }
}

#[test]
fn elevation_real_egui_create_place_apply_cancel_pan_and_sheet_at_both_dpis() {
    for (size, scale) in [
        (egui::vec2(1280., 800.), 1.),
        (egui::vec2(1000., 650.), 1.5),
    ] {
        let mut app = DesktopApp::new().unwrap();
        let mut wp = default_wall(app.active_level);
        wp.path = WallPath::Straight {
            start: Point2::new(2., 3.),
            end: Point2::new(8., 11.),
        };
        wp.height = 4.;
        let wall = Wall::new(os_walls::WALL_TYPE, wp);
        let host = wall.id();
        app.editor
            .command("Fixture", Command::AddWall(wall))
            .unwrap();
        let plan = app
            .editor
            .create_floor_plan("Plan", app.active_level)
            .unwrap();
        app.plans.poll(&app.editor);
        app.focus_plan(Some(plan));
        app.select(Some(host));
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let mut h = Harness {
            app,
            ctx,
            size,
            scale,
            time: 0.,
        };
        h.settle();
        let count = h.app.editor.document.model().views.len();
        h.click("Wall elevation");
        h.click("View left side");
        h.settle();
        let view = h.app.plans.active.unwrap();
        assert_eq!(h.app.editor.document.model().views.len(), count + 1);
        assert_eq!(h.app.active_elevation().unwrap().1.host, host);
        assert_eq!(
            h.app.active_elevation().unwrap().1.side,
            ElevationSide::Left
        );
        h.click("Fit elevation");
        h.frame(vec![]);
        for (kind, label, point) in [
            (OpeningKind::Door, "Place door", Point2::new(2., 0.)),
            (OpeningKind::Window, "Place window", Point2::new(6., 1.1)),
        ] {
            let original = h.app.editor.document.model().clone();
            let scene = h.app.editor.scene.clone();
            let history = h.app.editor.document.history_stats();
            h.click(label);
            let pos = h.point(point);
            h.frame(vec![egui::Event::PointerMoved(pos)]);
            assert_eq!(h.app.editor.document.model(), &original);
            assert_eq!(h.app.editor.scene, scene);
            assert_eq!(h.app.editor.document.history_stats(), history);
            h.click_at(pos);
            assert!(
                h.app.opening_draft.is_some(),
                "status={}, placement={}, open={}, selected={:?}, rect={:?}, pos={:?}",
                h.app.status,
                h.app.plans.elevation.placement.is_some(),
                h.app.opening_draft.is_some(),
                h.app.selected,
                h.app.plans.canvas_rect,
                pos
            );
            assert_eq!(h.app.editor.document.model(), &original);
            h.click("Apply opening");
            h.settle();
            let output = h.frame(vec![]);
            let visible_text: Vec<_> = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) => Some(text.galley.job.text.clone()),
                    _ => None,
                })
                .collect();
            assert!(
                h.app.opening_draft.is_none(),
                "status={}, visible_text={visible_text:?}",
                h.app.status
            );
            assert_eq!(
                h.app.editor.document.history_stats().undo_entries,
                history.undo_entries + 1
            );
            let opening = h
                .app
                .editor
                .document
                .model()
                .openings
                .values()
                .find(|o| !original.openings.contains_key(&o.id()))
                .unwrap();
            let resolved = h
                .app
                .editor
                .document
                .model()
                .resolve_opening(&opening.parameters)
                .unwrap();
            assert_eq!(resolved.kind, kind);
            assert!((resolved.offset + resolved.width * 0.5 - point.x).abs() < 1e-5);
            if kind == OpeningKind::Window {
                assert!((resolved.sill - point.y).abs() < 1e-5);
            }
            let committed = h.app.editor.document.model().clone();
            h.app.history(false);
            h.settle();
            assert_eq!(h.app.editor.document.model(), &original);
            h.app.history(true);
            h.settle();
            assert_eq!(h.app.editor.document.model(), &committed);
        }
        let original = h.app.editor.document.model().clone();
        let history = h.app.editor.document.history_stats();
        h.click("Place window");
        let invalid = h.point(Point2::new(0., 3.9));
        h.click_at(invalid);
        assert!(h.app.opening_draft.is_none());
        assert_eq!(h.app.editor.document.model(), &original);
        h.escape();
        assert!(h.app.plans.elevation.placement.is_none());
        h.click("Place window");
        h.click_at(h.point(Point2::new(8., 1.)));
        assert!(h.app.opening_draft.is_some());
        h.escape();
        assert!(h.app.opening_draft.is_none());
        assert_eq!(h.app.editor.document.model(), &original);
        assert_eq!(h.app.editor.document.history_stats(), history);
        // Let the just-closed placement dialog relinquish its egui layer before
        // checking that an ordinary canvas drag still pans the view.
        h.frame(vec![]);
        h.frame(vec![]);
        // Canvas pan uses actual pointer movement, without changing the model.
        let before = h.app.plans.cameras[&view];
        let pos = h.app.plans.canvas_rect.unwrap().center();
        h.frame(vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        h.frame(vec![egui::Event::PointerMoved(pos + egui::vec2(25., 15.))]);
        h.frame(vec![egui::Event::PointerButton {
            pos: pos + egui::vec2(25., 15.),
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }]);
        assert_ne!(h.app.plans.cameras[&view], before);
        assert_eq!(h.app.editor.document.model(), &original);
        h.click("Sheets");
        h.click("New sheet from view");
        h.settle();
        assert!(h.app.plans.active_sheet.is_some(), "{}", h.app.status);
        let page = h.app.sheet_page().unwrap();
        assert!(page.to_pdf().unwrap().starts_with(b"%PDF"));
    }
}
