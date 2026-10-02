use super::*;
use os_storage::{StorageBackend, ZipJsonStorage};

impl Harness {
    fn text_position(&self, label: &str) -> egui::Pos2 {
        self.output
            .shapes
            .iter()
            .rev()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.job.text == label => {
                    Some(egui::Rect::from_min_size(text.pos, text.galley.size()).center())
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("Missing text: {label}"))
    }

    fn click_text(&mut self, label: &str) {
        self.frame(vec![]);
        self.click(self.text_position(label));
        self.frame(vec![]);
    }

    fn replace_floor_text(&mut self, old: &str, value: &str) {
        self.click_text(old);
        self.frame(vec![
            egui::Event::Key {
                key: egui::Key::A,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::COMMAND,
            },
            egui::Event::Text(value.into()),
        ]);
        self.frame(vec![]);
    }
}

fn fixture(size: egui::Vec2, scale: f32) -> (Harness, Id, Id, Id) {
    let mut h = Harness::new(size, scale);
    let id = add_floor(
        &mut h,
        vec![
            Point2::new(-4., -4.),
            Point2::new(4., -4.),
            Point2::new(4., 4.),
            Point2::new(-4., 4.),
        ],
    );
    let mut model = h.app.editor.document.model().clone();
    model.floors.get_mut(&id).unwrap().parameters.holes = vec![
        vec![
            Point2::new(-2., -2.),
            Point2::new(-1., -2.),
            Point2::new(-1., -1.),
            Point2::new(-2., -1.),
        ],
        vec![
            Point2::new(1., 1.),
            Point2::new(2., 1.),
            Point2::new(2., 2.),
            Point2::new(1., 2.),
        ],
    ];
    let level = Level::new(
        "core.level",
        LevelParams {
            name: "Upper slab level".into(),
            elevation: 3.0,
            building: model.levels[&h.app.active_level].parameters.building,
        },
    );
    let level_id = level.id();
    model.levels.insert(level_id, level);
    let material = os_model::Material::new(
        "core.material",
        os_model::MaterialParams {
            name: "Slab concrete".into(),
            density_kg_m3: 2400.,
            color: [180, 180, 180],
        },
    );
    let material_id = material.id();
    model.materials.insert(material_id, material);
    h.app.editor.document = Document::from_model(model).unwrap();
    h.app.editor.regenerate().unwrap();
    h.app.plans.poll(&h.app.editor);
    h.app.focus_plan(Some(h.view));
    h.app.plans.split = true;
    h.settle();
    (h, id, level_id, material_id)
}

#[test]
fn floor_properties_real_controls_preview_apply_and_history_at_both_dpis() {
    for (size, scale) in PROFILES {
        let (mut h, id, level, material) = fixture(size, scale);
        let before = h.app.editor.document.model().clone();
        let scene = h.app.editor.scene.clone();
        let revision = h.app.editor.document.revision();
        let drawing = h.app.plans.drawing.as_ref().unwrap().identity();
        h.click_text("Edit floor properties…");
        assert!(h.app.plans.floor_properties.is_some());
        h.replace_floor_text("Editable slab", "Upper concrete slab");
        h.replace_floor_text("0.27", "0.4");
        h.replace_floor_text("0.12", "-0.1");
        let level_name = before.levels[&before.floors[&id].parameters.level]
            .parameters
            .name
            .clone();
        h.click_text(&level_name);
        h.click_text(&format!("Upper slab level · {level}"));
        h.click_text("None");
        h.click_text(&format!("Slab concrete · {material}"));
        assert_eq!(h.app.editor.document.model(), &before);
        assert_eq!(h.app.editor.scene, scene);
        assert_eq!(h.app.editor.document.revision(), revision);
        assert_eq!(h.app.editor.document.history_stats().undo_entries, 0);
        assert_eq!(h.app.plans.drawing.as_ref().unwrap().identity(), drawing);
        h.click_text("Apply");
        assert!(h.app.plans.floor_properties.is_none(), "{}", h.app.status);
        assert!(!h.app.status_error, "{}", h.app.status);
        h.settle();
        let after = h.app.editor.document.model().clone();
        let floor = &after.floors[&id];
        assert_eq!(floor.header, before.floors[&id].header);
        assert_eq!(
            floor.parameters.boundary,
            before.floors[&id].parameters.boundary
        );
        assert_eq!(floor.parameters.holes, before.floors[&id].parameters.holes);
        assert_eq!(floor.parameters.name, "Upper concrete slab");
        assert_eq!(floor.parameters.level, level);
        assert_eq!(floor.parameters.material, Some(material));
        assert_eq!(floor.parameters.thickness, 0.4);
        assert_eq!(floor.parameters.top_offset, -0.1);
        assert_eq!(h.app.editor.document.revision(), revision + 1);
        assert_eq!(h.app.editor.document.history_stats().undo_entries, 1);
        assert!((h.app.editor.scene[&id].signed_volume() - 62. * 0.4).abs() < 1e-8);
        assert!(
            h.app.editor.scene[&id]
                .vertices
                .iter()
                .all(|v| v.z >= 2.5 - 1e-10 && v.z <= 2.9 + 1e-10)
        );
        let context = h.app.editor.native_plan_context(h.view).unwrap();
        assert!(
            !h.app
                .plans
                .drawing
                .as_ref()
                .unwrap()
                .floors(context)
                .unwrap()
                .iter()
                .any(|floor| floor.entity == id)
        );
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("slab-properties.osb");
        ZipJsonStorage.save(&h.app.editor.document, &path).unwrap();
        assert_eq!(ZipJsonStorage.open(&path).unwrap().model(), &after);
        h.app.history(false);
        h.settle();
        assert_eq!(h.app.editor.document.model(), &before);
        h.app.history(true);
        h.settle();
        assert_eq!(h.app.editor.document.model(), &after);
    }
}

#[test]
fn floor_properties_respect_crop_and_popup_escape_keeps_the_draft() {
    for (size, scale) in PROFILES {
        let (mut h, id, _, _) = fixture(size, scale);
        let view = h.app.editor.document.model().views[&h.view]
            .parameters
            .clone();
        let mut plan = view.plan.unwrap();
        for (min, max, expected_visible) in [
            (Point2::new(10.0, 10.0), Point2::new(12.0, 12.0), false),
            (Point2::new(1.1, 1.1), Point2::new(1.9, 1.9), false),
            (Point2::new(-4.0, -4.0), Point2::new(-3.0, 3.0), true),
        ] {
            plan.crop = Some(os_model::PlanViewCrop { min, max });
            h.app
                .editor
                .update_floor_plan(h.view, &view.name, view.level.unwrap(), plan)
                .unwrap();
            h.settle();
            assert_eq!(h.app.can_edit_floor_properties(id), expected_visible);
        }

        plan.crop = None;
        h.app
            .editor
            .update_floor_plan(h.view, &view.name, view.level.unwrap(), plan)
            .unwrap();
        h.settle();
        let current_level = h.app.editor.document.model().levels
            [&h.app.editor.document.model().floors[&id].parameters.level]
            .parameters
            .name
            .clone();
        h.click_text("Edit floor properties…");
        h.replace_floor_text("Editable slab", "Staged slab name");

        h.click_text(&current_level);
        h.frame(vec![escape()]);
        assert!(h.app.plans.floor_properties.is_some());
        assert_eq!(
            h.app.plans.floor_properties.as_ref().unwrap().name,
            "Staged slab name"
        );
        h.click_text("None");
        h.frame(vec![escape()]);
        assert!(h.app.plans.floor_properties.is_some());
        h.click_text("Apply");
        assert_eq!(
            h.app.editor.document.model().floors[&id].parameters.name,
            "Staged slab name"
        );
    }
}

#[test]
fn floor_properties_invalid_and_geometry_failure_leave_live_state_untouched() {
    for (size, scale) in PROFILES {
        let (mut h, _, _, _) = fixture(size, scale);
        let before = h.app.editor.document.model().clone();
        let scene = h.app.editor.scene.clone();
        for invalid in ["0", "-1", "NaN", "inf", "not a number", "1000001"] {
            h.click_text("Edit floor properties…");
            h.replace_floor_text("0.27", invalid);
            h.click_text("Apply");
            assert!(h.app.plans.floor_properties.is_some());
            assert!(h.app.apply_floor_properties().is_err());
            assert_eq!(h.app.editor.document.model(), &before);
            assert_eq!(h.app.editor.scene, scene);
            assert!(!h.app.editor.document.can_undo());
            h.click_text("Cancel");
        }
        // Valid model scalar, but its top and bottom exceed the geometry bounds.
        h.click_text("Edit floor properties…");
        h.replace_floor_text("0.12", "-1000000");
        assert!(h.app.apply_floor_properties().is_err());
        h.click_text("Apply");
        assert_eq!(h.app.editor.document.model(), &before);
        assert_eq!(h.app.editor.scene, scene);
        assert!(!h.app.editor.document.can_undo());
        h.frame(vec![escape()]);
        assert!(h.app.plans.floor_properties.is_none());
    }
}

#[test]
fn floor_properties_cancel_escape_and_stale_context_at_both_dpis() {
    for (size, scale) in PROFILES {
        for cause in [
            "cancel",
            "escape",
            "session",
            "revision",
            "selection",
            "selection set",
            "view",
            "drawing",
        ] {
            let (mut h, id, _, _) = fixture(size, scale);
            h.click_text("Edit floor properties…");
            h.replace_floor_text("0.27", "0.5");
            match cause {
                "cancel" => h.click_text("Cancel"),
                "escape" => h.frame(vec![escape()]),
                "session" => {
                    h.app.editor.document =
                        Document::from_model(h.app.editor.document.model().clone()).unwrap()
                }
                "revision" => h
                    .app
                    .editor
                    .command("Concurrent edit", Command::RenameProject("Changed".into()))
                    .unwrap(),
                "selection" => h.app.select(None),
                "selection set" => {
                    h.app.selected_ids.insert(Id::new());
                }
                "view" => h.app.focus_plan(None),
                "drawing" => {
                    h.app.plans.drawing = Some(h.app.editor.native_drawing(h.view).unwrap())
                }
                _ => unreachable!(),
            }
            let before = h.app.editor.document.model().clone();
            let revision = h.app.editor.document.revision();
            h.frame(vec![]);
            assert!(h.app.plans.floor_properties.is_none(), "{cause}");
            assert_eq!(h.app.editor.document.model(), &before);
            assert_eq!(
                h.app.editor.document.model().floors[&id]
                    .parameters
                    .thickness,
                0.27
            );
            assert_eq!(h.app.editor.document.revision(), revision);
        }
    }
}
