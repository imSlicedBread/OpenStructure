//! Real egui frames exercise menus, station mapping, placement, navigation and paper.
use super::*;
use os_model::{ElevationSide, Wall, WallPath};
use std::time::{Duration, Instant};

struct Harness {
    app: DesktopApp,
    ctx: egui::Context,
    size: egui::Vec2,
    scale: f32,
    time: f64,
}

#[test]
fn section_dimensions_real_egui_both_dpis_preview_commit_repair_cancel_and_paper() {
    use os_model::{DimensionLayout as L, ElevationDimensionFeature as F};
    for (size, scale, side) in [
        (egui::vec2(1280., 800.), 1., ElevationSide::Left),
        (egui::vec2(1000., 650.), 1.5, ElevationSide::Right),
    ] {
        let mut app = DesktopApp::new().unwrap();
        let mut wp = default_wall(app.active_level);
        wp.path = WallPath::Straight {
            start: Point2::new(0., 0.),
            end: Point2::new(10., 0.),
        };
        wp.height = 4.;
        let wall = Wall::new(os_walls::WALL_TYPE, wp);
        let host = wall.id();
        app.editor.command("Host", Command::AddWall(wall)).unwrap();
        let view = app
            .editor
            .create_section_view("Dimensions", app.active_level, settings(side))
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
        h.click("Fit section");
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
        assert!(h.app.plans.section_dimension.is_none());
        assert_eq!(
            h.app.editor.document.model().dimensions[&vertical]
                .parameters
                .first,
            DimensionReference::SectionWall {
                wall: host,
                feature: F::Bottom
            }
        );
        let original = h.app.editor.document.model().clone();
        let history = h.app.editor.document.history_stats();
        h.click("Horizontal dimension");
        h.click_at(h.point(Point2::new(0., 2.)));
        h.escape();
        assert!(h.app.plans.section_dimension.is_none());
        assert_eq!(h.app.editor.document.model(), &original);
        assert_eq!(h.app.editor.document.history_stats(), history);
        // Wrong-axis/no-edge pick is a no-op; revisions cancel in-progress work.
        h.click("Horizontal dimension");
        h.click_at(h.point(Point2::new(5., 2.)));
        assert_eq!(h.app.editor.document.model(), &original);
        h.app.dimension_offset_draft = 0.3;
        h.app.apply_dimension_properties();
        h.settle();
        assert!(h.app.plans.section_dimension.is_none());
        h.app.begin_section_dimension(L::SectionHorizontal);
        h.app.focus_plan(None);
        h.frame(vec![]);
        assert!(h.app.plans.section_dimension.is_none());
        h.app.focus_plan(Some(view));
        h.settle();
        let context = h.app.editor.native_section_context(view).unwrap();
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
fn section_opening_dimensions_egui_orphan_repair_and_pan_precedence() {
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
            .create_section_view(
                "Window dimensions",
                app.active_level,
                settings(ElevationSide::Left),
            )
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
        h.click("Fit section");
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
        assert!(h.app.plans.section_dimension.is_none());
        h.click("Horizontal dimension");
        h.click_at(h.point(Point2::new(3., 1.75)));
        h.click_at(h.point(Point2::new(5., 1.75)));
        h.click_at(h.point(Point2::new(4., 3.)));
        h.settle();
        let id = h.app.selected.unwrap();
        let p = &h.app.editor.document.model().dimensions[&id].parameters;
        assert!(
            matches!(p.first,DimensionReference::SectionOpening { opening,.. } if opening==opening_id)
        );
        assert!(
            matches!(p.second,DimensionReference::SectionOpening { opening,.. } if opening==opening_id)
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
        let c = h.app.editor.native_section_context(view).unwrap();
        assert!(
            h.app.plans.drawing.as_ref().unwrap().dimensions(c).unwrap()[0]
                .diagnostic
                .is_some()
        );
        h.app.select(Some(id));
        h.app.begin_dimension_repair(0);
        h.click_at(h.point(Point2::new(0., 2.)));
        h.settle();
        assert!(h.app.plans.section_dimension.is_none());
        assert!(
            h.app.editor.document.model().dimensions[&id]
                .parameters
                .resolve(h.app.editor.document.model())
                .is_err()
        );
        h.app.begin_dimension_repair(1);
        h.click_at(h.point(Point2::new(10., 2.)));
        h.settle();
        assert!(h.app.plans.section_dimension.is_none());
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

fn settings(side: ElevationSide) -> os_model::SectionViewSettings {
    let mut s =
        os_model::SectionViewSettings::new(Point2::new(0., 0.), Point2::new(10., 0.), -0.25, 4.25);
    s.min_station = -0.25;
    s.max_station = 10.25;
    s.side = side;
    s
}
