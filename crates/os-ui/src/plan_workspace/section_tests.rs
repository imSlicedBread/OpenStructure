//! Real egui-frame section marker authoring and cancellation evidence.
use super::*;
use os_model::{SectionViewSettings, ViewKind, Wall, WallParams};

const SIZE: egui::Vec2 = egui::vec2(1280.0, 800.0);

struct Harness {
    size: egui::Vec2,
    scale: f32,
    output: Option<egui::FullOutput>,
    app: DesktopApp,
    ctx: egui::Context,
    time: f64,
    view: Id,
}

impl Harness {
    fn new() -> Self {
        let mut app = DesktopApp::new().unwrap();
        let wall = Wall::new(
            os_walls::WALL_TYPE,
            WallParams {
                name: "Section fixture".into(),
                path: os_model::WallPath::Straight {
                    start: Point2::new(18.0, 20.0),
                    end: Point2::new(22.0, 20.0),
                },
                thickness: 0.2,
                height: 4.0,
                level: app.active_level,
                material: None,
            },
        );
        app.editor
            .command("Add section fixture", Command::AddWall(wall))
            .unwrap();
        let view = app
            .editor
            .create_floor_plan("Section placement", app.active_level)
            .unwrap();
        app.editor.document = Document::from_model(app.editor.document.model().clone()).unwrap();
        app.plans.poll(&app.editor);
        app.focus_plan(Some(view));
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let mut harness = Self {
            size: SIZE,
            scale: 1.,
            output: None,
            app,
            ctx,
            time: 0.0,
            view,
        };
        harness.settle();
        let camera = harness.app.plans.cameras.get_mut(&view).unwrap();
        camera.center = Point2::new(20.0, 20.0);
        camera.pixels_per_metre = 30.0;
        harness.frame(vec![]);
        harness.app.plans.snaps.enabled = false;
        harness
    }

    fn frame(&mut self, events: Vec<egui::Event>) {
        self.time += 1.0 / 60.0;
        let input = super::sheet_tests::raw_input(self.size, self.scale, self.time, events);
        self.output = Some(self.ctx.run(input, |ctx| self.app.show(ctx)));
    }

    fn settle(&mut self) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            self.frame(vec![]);
            if self.app.plans.ready() && !self.app.editor.plugin_work_pending() {
                break;
            }
            assert!(std::time::Instant::now() < deadline, "plan did not settle");
            std::thread::yield_now();
        }
        self.frame(vec![]);
    }

    fn point(&self, world: Point2) -> egui::Pos2 {
        let rect = self.app.plans.canvas_rect.unwrap();
        let context = self.app.editor.native_plan_context(self.view).unwrap();
        let local = self.app.plans.cameras[&self.view]
            .project(
                context.basis.world_to_plane(world).unwrap(),
                [f64::from(rect.width()), f64::from(rect.height())],
            )
            .unwrap();
        let position = rect.min + egui::vec2(local.x as f32, local.y as f32);
        assert!(rect.contains(position));
        position
    }

    fn click(&mut self, point: egui::Pos2) {
        self.frame(vec![egui::Event::PointerMoved(point)]);
        self.frame(vec![button(point, true)]);
        self.frame(vec![button(point, false)]);
    }

    fn action(&mut self, label: &str) {
        self.frame(vec![]);
        let point = self
            .output
            .as_ref()
            .unwrap()
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.job.text == label => {
                    Some(egui::Rect::from_min_size(t.pos, t.galley.size()).center())
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing {label}"));
        self.click(point);
    }

    fn number(&mut self, label: &'static str, value: &str) {
        self.time += 0.5;
        self.frame(vec![]);
        self.frame(vec![]);
        let point = self
            .app
            .plans
            .section_placement
            .as_ref()
            .unwrap()
            .numeric_rects[label]
            .center();
        self.click(point);
        assert!(
            self.ctx.wants_keyboard_input(),
            "{label} must accept exact keyboard input"
        );
        self.frame(vec![
            egui::Event::Key {
                key: egui::Key::A,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::CTRL | egui::Modifiers::COMMAND,
            },
            egui::Event::Text(value.into()),
        ]);
        self.frame(vec![egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }]);
        self.frame(vec![]);
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
fn two_click_section_is_preview_only_then_one_undoable_linked_view() {
    let mut h = Harness::new();
    let original = h.app.editor.document.model().clone();
    let revision = h.app.editor.document.revision();
    h.app.begin_section_placement(h.view);
    h.frame(vec![]);

    h.click(h.point(Point2::new(18.0, 20.0)));
    assert_eq!(h.app.editor.document.model(), &original);
    assert_eq!(h.app.editor.document.revision(), revision);
    assert!(
        h.app
            .plans
            .section_placement
            .as_ref()
            .unwrap()
            .first
            .is_some()
    );

    h.click(h.point(Point2::new(18.0, 20.0)));
    assert!(h.app.status_error, "degenerate marker should be rejected");
    assert_eq!(h.app.editor.document.model(), &original);
    assert_eq!(h.app.editor.document.revision(), revision);
    assert!(h.app.plans.section_placement.is_some());

    let end = h.point(Point2::new(22.0, 20.0));
    h.frame(vec![egui::Event::PointerMoved(end)]);
    assert_eq!(h.app.editor.document.model(), &original);
    assert_eq!(h.app.editor.document.revision(), revision);
    h.click(end);

    assert_eq!(h.app.editor.document.model(), &original);
    assert_eq!(h.app.editor.document.revision(), revision);
    h.action("Apply section");

    let section = h.app.plans.active.unwrap();
    assert_eq!(h.app.editor.document.revision(), revision + 1);
    assert_eq!(h.app.editor.document.history_stats().undo_entries, 1);
    assert_eq!(
        h.app.editor.document.model().views[&section]
            .parameters
            .kind,
        ViewKind::Section
    );
    let params = &h.app.editor.document.model().views[&section].parameters;
    assert_eq!(params.level, Some(h.app.active_level));
    let definition = params.section.unwrap();
    assert_eq!(definition.start, Point2::new(18.0, 20.0));
    assert_eq!(definition.end, Point2::new(22.0, 20.0));
    assert!((definition.bottom_elevation + 0.25).abs() < 1e-9);
    assert!((definition.top_elevation - 4.25).abs() < 1e-9);

    h.settle();
    let context = h.app.editor.native_section_context(section).unwrap();
    assert!(
        !h.app
            .plans
            .drawing
            .as_ref()
            .unwrap()
            .provider_lines(context)
            .unwrap()
            .is_empty()
    );

    let committed = h.app.editor.document.model().clone();
    h.app.history(false);
    assert!(!h.app.editor.document.model().views.contains_key(&section));
    h.app.history(true);
    assert_eq!(h.app.editor.document.model(), &committed);
}

#[test]
fn escape_and_stale_document_cancel_section_marker_without_partial_view() {
    let mut h = Harness::new();
    let original = h.app.editor.document.model().clone();
    h.app.begin_section_placement(h.view);
    h.frame(vec![]);
    h.click(h.point(Point2::new(18.0, 20.0)));
    h.frame(vec![escape()]);
    assert!(h.app.plans.section_placement.is_none());
    assert_eq!(h.app.editor.document.model(), &original);

    h.app.begin_section_placement(h.view);
    h.frame(vec![]);
    h.click(h.point(Point2::new(18.0, 20.0)));
    let wall = Wall::new(
        os_walls::WALL_TYPE,
        WallParams {
            name: "Concurrent model edit".into(),
            path: os_model::WallPath::Straight {
                start: Point2::new(0.0, 0.0),
                end: Point2::new(1.0, 0.0),
            },
            thickness: 0.2,
            height: 3.0,
            level: h.app.active_level,
            material: None,
        },
    );
    h.app
        .editor
        .command("Concurrent edit", Command::AddWall(wall))
        .unwrap();
    h.frame(vec![]);
    assert!(h.app.plans.section_placement.is_none());
    assert_eq!(
        h.app.editor.document.model().views.len(),
        original.views.len()
    );
}

#[test]
fn section_settings_real_numeric_apply_history_cancel_and_stale_authority_both_dpis() {
    for (size, scale) in [(SIZE, 1.), (egui::vec2(1000., 650.), 1.5)] {
        let mut h = Harness::new();
        h.size = size;
        h.scale = scale;
        h.frame(vec![]);
        h.app.begin_section_placement(h.view);
        h.frame(vec![]);
        let original = h.app.editor.document.model().clone();
        h.click(h.point(Point2::new(18., 20.)));
        h.click(h.point(Point2::new(22., 20.)));
        assert_eq!(h.app.editor.document.model(), &original);
        h.number("Depth", "7.5");
        h.number("Scale 1:", "50");
        let draft = h
            .app
            .plans
            .section_placement
            .as_ref()
            .unwrap()
            .settings
            .unwrap();
        assert_eq!(draft.far_depth, 7.5);
        assert_eq!(draft.scale_denominator, 50.);
        h.action("Apply section");
        let id = h.app.plans.active.unwrap();
        assert_ne!(id, h.view);
        h.settle();
        let saved = h.app.editor.document.model().clone();
        let history = h.app.editor.document.history_stats().undo_entries;
        h.action("Section settings");
        h.number("Crop start", "-1.25");
        h.number("Top", "6.5");
        assert_eq!(h.app.editor.document.model(), &saved);
        h.action("View from right");
        h.action("Apply section");
        h.settle();
        assert_eq!(h.app.plans.active, Some(id));
        let edited = h.app.editor.document.model().clone();
        let s = edited.views[&id].parameters.section.unwrap();
        assert_eq!(s.min_station, -1.25);
        assert_eq!(s.top_elevation, 6.5);
        assert_eq!(s.side, os_model::ElevationSide::Right);
        assert_eq!(
            h.app.editor.document.history_stats().undo_entries,
            history + 1
        );
        h.app.history(false);
        assert_eq!(h.app.editor.document.model(), &saved);
        h.app.history(true);
        assert_eq!(h.app.editor.document.model(), &edited);
        h.settle();
        h.action("Section settings");
        h.number("Depth", "-2");
        assert_eq!(h.app.editor.document.model(), &edited);
        h.frame(vec![escape()]);
        assert!(h.app.plans.section_placement.is_none());
        h.app.begin_section_settings(id).unwrap();
        h.frame(vec![egui::Event::PointerGone]);
        assert!(h.app.plans.section_placement.is_none());
        h.app.begin_section_settings(id).unwrap();
        let mut newer = h.app.editor.document.model().views[&id].parameters.clone();
        newer.section.as_mut().unwrap().far_depth = 99.;
        h.app
            .editor
            .command(
                "Newer section definition",
                Command::UpdateView {
                    id,
                    parameters: newer,
                },
            )
            .unwrap();
        let newest = h.app.editor.document.model().clone();
        assert!(h.app.commit_section_placement(s.start, s.end).is_err());
        assert_eq!(h.app.editor.document.model(), &newest);
        h.frame(vec![]);
        assert!(h.app.plans.section_placement.is_none());
        h.settle();
        for authority in [0, 1, 2] {
            h.app.begin_section_settings(id).unwrap();
            let d = h.app.plans.section_placement.as_mut().unwrap();
            match authority {
                0 => d.session = Id::new(),
                1 => d.revision += 1,
                _ => d.provider_signature.push(("obsolete".into(), Id::new())),
            }
            assert!(h.app.commit_section_placement(s.start, s.end).is_err());
            h.frame(vec![]);
            assert!(h.app.plans.section_placement.is_none());
            assert_eq!(h.app.editor.document.model(), &newest);
        }
    }
}

#[test]
fn section_elevation_bounds_use_building_wall_and_floor_extents() {
    let mut model = os_model::Model::new("section bounds");
    let level_id = *model.levels.keys().next().unwrap();
    model
        .levels
        .get_mut(&level_id)
        .unwrap()
        .parameters
        .elevation = 2.0;
    let wall = Wall::new(
        os_walls::WALL_TYPE,
        WallParams {
            name: "Tall wall".into(),
            path: os_model::WallPath::Straight {
                start: Point2::new(0.0, 0.0),
                end: Point2::new(4.0, 0.0),
            },
            thickness: 0.2,
            height: 4.0,
            level: level_id,
            material: None,
        },
    );
    model.walls.insert(wall.id(), wall);
    let floor = os_model::Floor::new(
        "core.floor",
        os_model::FloorParams {
            name: "Lower slab".into(),
            level: level_id,
            material: None,
            boundary: vec![
                Point2::new(0.0, 0.0),
                Point2::new(4.0, 0.0),
                Point2::new(4.0, 2.0),
            ],
            holes: Vec::new(),
            thickness: 0.3,
            top_offset: -0.5,
        },
    );
    model.floors.insert(floor.id(), floor);

    assert_eq!(
        section_elevation_bounds(&model, level_id).unwrap(),
        (0.95, 6.25)
    );
    assert!(
        SectionViewSettings::new(Point2::new(0.0, 0.0), Point2::new(4.0, 0.0), 0.95, 6.25,)
            .validate()
            .is_ok()
    );
}

#[test]
fn section_fills_desktop_mesh_precedes_contours_at_two_scales() {
    for (size, scale) in [(SIZE, 1.0), (egui::vec2(1000., 650.), 1.5)] {
        let mut h = Harness::new();
        let host = h
            .app
            .editor
            .document
            .model()
            .walls
            .values()
            .find(|w| w.parameters.name == "Section fixture")
            .unwrap()
            .id();
        let material = os_model::Material::new(
            "core.material",
            os_model::MaterialParams {
                cut_pattern: None,
                name: "Section fill color".into(),
                density_kg_m3: 800.,
                color: [37, 83, 129],
            },
        );
        let material_id = material.id();
        let mut wall = h.app.editor.document.model().walls[&host]
            .parameters
            .clone();
        wall.material = Some(material_id);
        h.app
            .editor
            .commands(
                "Section material",
                vec![
                    Command::AddMaterial(material),
                    Command::UpdateWall {
                        id: host,
                        parameters: wall,
                    },
                ],
            )
            .unwrap();
        let section = h
            .app
            .editor
            .create_section_view(
                "Filled section",
                h.app.active_level,
                SectionViewSettings::new(Point2::new(18., 20.), Point2::new(22., 20.), -1., 5.),
            )
            .unwrap();
        h.app.focus_plan(Some(section));
        h.settle();
        h.ctx.set_pixels_per_point(scale);
        for selected in [false, true] {
            h.app.selected = selected.then_some(host);
            h.app.selected_ids.clear();
            if selected {
                h.app.selected_ids.insert(host);
            }
            let mut output = None;
            for _ in 0..2 {
                h.time += 1. / 60.;
                output = Some(h.ctx.run(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                        time: Some(h.time),
                        ..Default::default()
                    },
                    |ctx| h.app.show(ctx),
                ));
            }
            let output = output.unwrap();
            let drawing = h.app.plans.drawing.as_ref().unwrap();
            let context = h.app.editor.native_section_context(section).unwrap();
            let rect = h.app.plans.canvas_rect.unwrap();
            let camera = h.app.plans.cameras[&section];
            let expected = section_fills::mesh(drawing, context, camera, rect).unwrap();
            assert!(!expected.is_empty());
            assert!(
                expected
                    .vertices
                    .iter()
                    .all(|v| v.color == egui::Color32::from_rgb(37, 83, 129))
            );
            let index = output
                .shapes
                .iter()
                .position(|s| {
                    matches!(&s.shape,
                egui::Shape::Mesh(mesh) if **mesh==expected)
                })
                .expect("canvas consumed checked fill mesh");
            let screen = |p: Point2| {
                let p = camera
                    .project(p, [rect.width() as f64, rect.height() as f64])
                    .unwrap();
                rect.min + egui::vec2(p.x as f32, p.y as f32)
            };
            for line in drawing
                .provider_lines(context)
                .unwrap()
                .iter()
                .filter(|l| l.entity == host)
            {
                let points = [screen(line.start), screen(line.end)];
                assert!(
                    output.shapes[index + 1..].iter().any(|s| matches!(&s.shape,
                    egui::Shape::LineSegment {points:actual,..} if *actual==points)),
                    "section contours must be drawn after the opaque fill"
                );
            }
            let mut stale = context;
            stale.model_revision += 1;
            assert!(section_fills::mesh(drawing, stale, camera, rect).is_err());
        }
    }
}
