//! Headless egui checks for hosted stair railing authoring and editing.
use super::*;
use os_model::{Stair, StairParams, StairRailingSide};

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
}

impl Harness {
    fn new(size: egui::Vec2, scale: f32) -> Self {
        let mut app = DesktopApp::new().unwrap();
        let view = app
            .editor
            .create_floor_plan("Railing plan", app.active_level)
            .unwrap();
        app.add_level();
        app.editor.document = Document::from_model(app.editor.document.model().clone()).unwrap();
        app.focus_plan(Some(view));
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        Self {
            app,
            ctx,
            size,
            scale,
            time: 0.0,
        }
    }

    fn properties(&mut self, railing: bool) -> egui::FullOutput {
        self.time += 1.0 / 60.0;
        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, self.size)),
            time: Some(self.time),
            ..Default::default()
        };
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .native_pixels_per_point = Some(self.scale);
        self.ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                if railing {
                    assert!(self.app.railing_properties(ui));
                } else {
                    assert!(self.app.stair_properties(ui));
                }
            });
        })
    }

    fn click_property_button(&mut self, railing: bool, label: &str) {
        self.properties(railing);
        let output = self.properties(railing);
        let position = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.job.text == label => {
                    Some(text.pos + text.galley.size() * 0.5)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("Properties button missing: {label}"));
        self.property_event(railing, egui::Event::PointerMoved(position));
        self.property_event(railing, button(position, true));
        self.property_event(railing, button(position, false));
    }

    fn property_event(&mut self, railing: bool, event: egui::Event) {
        self.time += 1.0 / 60.0;
        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, self.size)),
            time: Some(self.time),
            events: vec![event],
            ..Default::default()
        };
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .native_pixels_per_point = Some(self.scale);
        let _ = self.ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                if railing {
                    self.app.railing_properties(ui);
                } else {
                    self.app.stair_properties(ui);
                }
            });
        });
    }

    fn add_stair(&mut self) -> Id {
        let model = self.app.editor.document.model();
        let lower = self.app.active_level;
        let upper = model
            .levels
            .values()
            .find(|level| level.id() != lower)
            .unwrap()
            .id();
        let stair = Stair::new(
            "core.stair",
            StairParams {
                name: "Test stair".into(),
                lower_level: lower,
                upper_level: upper,
                start: Point2::new(-2.0, 0.0),
                end: Point2::new(2.5, 0.0),
                width: 1.2,
                riser_count: 15,
                structural_thickness: 0.2,
                material: None,
            },
        );
        let id = stair.id();
        self.app
            .editor
            .command("Add test stair", Command::AddStair(stair))
            .unwrap();
        self.app.select(Some(id));
        id
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

#[test]
fn stair_properties_place_shared_left_right_railings_and_edit_type_at_both_profiles() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        let stair_id = h.add_stair();
        let original_stair = h.app.editor.document.model().stairs[&stair_id].clone();
        let original_revision = h.app.editor.document.revision();

        h.click_property_button(false, "Add left railing");
        let left_id = h.app.selected.unwrap();
        let left = h.app.editor.document.model().railings[&left_id].clone();
        let type_id = left.parameters.railing_type;
        assert_eq!(left.parameters.stair, stair_id);
        assert_eq!(left.parameters.side, StairRailingSide::Left);
        assert!(h.app.editor.scene.contains_key(&left_id));
        assert_eq!(h.app.editor.document.revision(), original_revision + 1);

        h.app.select(Some(stair_id));
        h.click_property_button(false, "Add right railing");
        let right_id = h.app.selected.unwrap();
        let right = h.app.editor.document.model().railings[&right_id].clone();
        assert_ne!(left_id, right_id);
        assert_eq!(right.parameters.side, StairRailingSide::Right);
        assert_eq!(right.parameters.railing_type, type_id);

        h.app.select(Some(left_id));
        h.properties(true);
        let before_edit = h.app.editor.document.model().clone();
        let before_left_mesh = h.app.editor.scene[&left_id].clone();
        let before_right_mesh = h.app.editor.scene[&right_id].clone();
        h.app
            .plans
            .railing_edit
            .as_mut()
            .unwrap()
            .type_parameters
            .top_rail_height = 1.1;
        h.properties(true);
        assert_eq!(h.app.editor.document.model(), &before_edit);
        h.app.apply_railing_properties();
        assert!(!h.app.status_error);
        assert_eq!(
            h.app.editor.document.model().railings[&left_id].header,
            left.header
        );
        assert_eq!(
            h.app.editor.document.model().railings[&right_id].header,
            right.header
        );
        assert_eq!(
            h.app.editor.document.model().railing_types[&type_id]
                .parameters
                .top_rail_height,
            1.1
        );
        assert!(h.app.editor.scene.contains_key(&left_id));
        assert!(h.app.editor.scene.contains_key(&right_id));
        assert_ne!(h.app.editor.scene[&left_id], before_left_mesh);
        assert_ne!(h.app.editor.scene[&right_id], before_right_mesh);
        for id in [left_id, right_id] {
            let railing = &h.app.editor.document.model().railings[&id];
            assert_eq!(
                h.app.editor.scene[&id],
                os_geometry::railings::railing_geometry(
                    &railing.parameters,
                    h.app.editor.document.model(),
                )
                .unwrap()
                .mesh
            );
        }

        let committed = h.app.editor.document.model().clone();
        let entries = h.app.editor.document.history_stats().undo_entries;
        h.app.editor.undo().unwrap();
        assert_eq!(
            h.app.editor.document.model().railing_types[&type_id]
                .parameters
                .top_rail_height,
            0.95
        );
        h.app.editor.redo().unwrap();
        assert_eq!(h.app.editor.document.model(), &committed);
        assert_eq!(h.app.editor.document.history_stats().undo_entries, entries);

        h.app.select(Some(left_id));
        h.properties(true);
        h.app
            .plans
            .railing_edit
            .as_mut()
            .unwrap()
            .type_parameters
            .max_post_spacing = 0.1;
        h.app.apply_railing_properties();
        assert!(h.app.status_error);
        assert_eq!(h.app.editor.document.model(), &committed);

        // The stair and both dependent rails are removed in one transaction.
        h.app.select(Some(stair_id));
        let before_delete = h.app.editor.document.history_stats().undo_entries;
        h.app.delete_stair();
        assert!(!h.app.editor.document.model().stairs.contains_key(&stair_id));
        assert!(
            !h.app
                .editor
                .document
                .model()
                .railings
                .contains_key(&left_id)
        );
        assert!(
            !h.app
                .editor
                .document
                .model()
                .railings
                .contains_key(&right_id)
        );
        assert_eq!(
            h.app.editor.document.history_stats().undo_entries,
            before_delete + 1
        );
        h.app.editor.undo().unwrap();
        assert_eq!(
            h.app.editor.document.model().stairs[&stair_id],
            original_stair
        );
        assert!(
            h.app
                .editor
                .document
                .model()
                .railings
                .contains_key(&left_id)
        );
        assert!(
            h.app
                .editor
                .document
                .model()
                .railings
                .contains_key(&right_id)
        );
        h.app.editor.redo().unwrap();
        assert!(!h.app.editor.document.model().stairs.contains_key(&stair_id));
    }
}

#[test]
fn stale_railing_properties_cannot_overwrite_later_document_edits() {
    let (size, scale) = PROFILES[0];
    let mut h = Harness::new(size, scale);
    let stair_id = h.add_stair();
    h.app.add_stair_railing(stair_id, StairRailingSide::Left);
    let railing_id = h.app.selected.unwrap();
    h.properties(true);
    h.app
        .plans
        .railing_edit
        .as_mut()
        .unwrap()
        .type_parameters
        .top_rail_height = 1.2;
    let railing = h.app.editor.document.model().railings[&railing_id].clone();
    let mut stair = h.app.editor.document.model().stairs[&stair_id]
        .parameters
        .clone();
    stair.width = 1.3;
    h.app
        .editor
        .command(
            "Concurrent stair edit",
            Command::UpdateStair {
                id: stair_id,
                parameters: stair,
            },
        )
        .unwrap();
    let after_external = h.app.editor.document.model().clone();
    h.app.apply_railing_properties();
    assert!(h.app.status_error);
    assert_eq!(h.app.editor.document.model(), &after_external);
    assert_eq!(h.app.editor.document.model().railings[&railing_id], railing);
}
