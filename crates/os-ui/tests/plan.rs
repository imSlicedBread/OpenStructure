use os_core::{Id, Point2};
use os_document::{Command, Document};
use os_geometry::plan::PlanRole;
use os_model::{ViewKind, WallParams};
use os_plugin_api::Request;
use os_ui::Editor;

#[test]
fn native_snaps_follow_semantic_wall_axes_rotated_views_visibility_and_revisions() {
    use os_render::{
        plan::PlanCamera,
        snapping::{SnapKind, SnapQuery},
    };
    let (mut editor, view) = editor();
    let level = editor.document.model().views[&view]
        .parameters
        .level
        .unwrap();
    let (wall, start) = editor
        .document
        .model()
        .walls
        .iter()
        .next()
        .map(|(id, w)| (*id, w.parameters.start))
        .unwrap();
    let mut settings = editor.document.model().views[&view]
        .parameters
        .plan
        .unwrap();
    settings.basis.origin = Point2::new(2.0, 3.0);
    settings.basis.rotation = 0.3;
    editor
        .update_floor_plan(view, "Rotated", level, settings)
        .unwrap();
    let context = editor.native_plan_context(view).unwrap();
    let point = context.basis.world_to_plane(start).unwrap();
    let camera = PlanCamera {
        center: point,
        pixels_per_metre: 100.0,
    };
    let query = SnapQuery {
        camera,
        viewport: [800.0, 600.0],
        pointer: Point2::new(400.0, 300.0),
        radius_pixels: 12.0,
        endpoints: true,
        midpoints: true,
        intersections: false,
        perpendicular_from: None,
        nearest: true,
        axis_extensions: false,
        exclude_entity: None,
    };
    let drawing = editor.native_wall_plan(view).unwrap();
    let model = editor.document.model().clone();
    let hit = drawing
        .snap(context, query)
        .unwrap()
        .candidate(context, query)
        .unwrap()
        .unwrap();
    assert_eq!(hit.entity, wall);
    assert_eq!(hit.feature, 0);
    assert_eq!(hit.kind, SnapKind::Endpoint);
    assert!(
        context
            .basis
            .plane_to_world(hit.point)
            .unwrap()
            .distance(start)
            < 1e-10
    );
    assert_eq!(editor.document.model(), &model);
    settings.visibility.walls = false;
    editor
        .update_floor_plan(view, "Hidden", level, settings)
        .unwrap();
    let hidden = editor.native_plan_context(view).unwrap();
    assert!(drawing.snap(hidden, query).is_err());
    assert!(
        editor
            .native_wall_plan(view)
            .unwrap()
            .snap(hidden, query)
            .unwrap()
            .candidate(hidden, query)
            .unwrap()
            .is_none()
    );
    assert!(editor.scene.contains_key(&wall));
    editor.undo().unwrap();
    let restored = editor.native_plan_context(view).unwrap();
    assert!(drawing.snap(restored, query).is_err());
    assert_eq!(
        editor
            .native_wall_plan(view)
            .unwrap()
            .snap(restored, query)
            .unwrap()
            .candidate(restored, query)
            .unwrap()
            .unwrap()
            .entity,
        wall
    );
}

#[test]
fn persisted_settings_and_named_views_reopen_with_exact_ids_and_new_session() {
    let (mut editor, view) = editor();
    let level = editor.document.model().views[&view]
        .parameters
        .level
        .unwrap();
    let mut settings = editor.document.model().views[&view]
        .parameters
        .plan
        .unwrap();
    settings.basis.origin = Point2::new(1.0, 0.0);
    settings.basis.rotation = 0.3;
    settings.range.cut = 1.5;
    settings.crop = Some(os_model::PlanViewCrop {
        min: Point2::new(-2.0, -3.0),
        max: Point2::new(10.0, 3.0),
    });
    settings.scale_denominator = 50.0;
    settings.visibility.extensions = false;
    editor
        .update_floor_plan(view, "Upper detail plan", level, settings)
        .unwrap();
    let context = editor.native_plan_context(view).unwrap();
    assert_eq!(context.scale_denominator, 50.0);
    assert!(!context.show_extensions);
    let drawing = editor.native_wall_plan(view).unwrap();
    let items = drawing.items(context).unwrap().to_vec();
    let model = editor.document.model().clone();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("plans.osb");
    editor.save(&path).unwrap();
    assert!(!editor.is_dirty());
    editor.open(&path).unwrap();
    assert_eq!(editor.document.model(), &model);
    assert_eq!(
        editor.document.model().views[&view].parameters.plan,
        Some(settings)
    );
    assert_eq!(
        editor.document.model().views[&view].parameters.name,
        "Upper detail plan"
    );
    let reopened = editor.native_plan_context(view).unwrap();
    assert_ne!(reopened.session_id, context.session_id);
    assert!(drawing.items(reopened).is_err());
    assert_eq!(
        editor
            .native_wall_plan(view)
            .unwrap()
            .items(reopened)
            .unwrap(),
        items
    );
    assert!(!editor.document.can_undo());
    assert!(!editor.is_dirty());
}

#[test]
fn visibility_is_persisted_and_invalid_setting_drafts_preserve_saved_state_and_redo() {
    let (mut editor, view) = editor();
    let level = editor.document.model().views[&view]
        .parameters
        .level
        .unwrap();
    let mut settings = editor.document.model().views[&view]
        .parameters
        .plan
        .unwrap();
    let original_context = editor.native_plan_context(view).unwrap();
    let old = editor.native_wall_plan(view).unwrap();
    settings.visibility.walls = false;
    editor
        .update_floor_plan(view, "Hidden walls", level, settings)
        .unwrap();
    let hidden_context = editor.native_plan_context(view).unwrap();
    assert!(old.items(hidden_context).is_err());
    assert!(
        editor
            .native_wall_plan(view)
            .unwrap()
            .items(hidden_context)
            .unwrap()
            .is_empty()
    );
    assert_eq!(editor.document.model().walls.len(), 1);
    assert_eq!(editor.scene.len(), 1);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("hidden.osb");
    editor.save(&path).unwrap();
    editor
        .update_floor_plan(view, "Changed again", level, settings)
        .unwrap();
    editor.undo().unwrap();
    assert!(!editor.is_dirty());
    let original = editor.document.model().clone();
    let stats = editor.document.history_stats();
    let revision = editor.document.revision();
    for case in ["range", "basis", "version", "level", "name", "scale"] {
        let mut bad = settings;
        let mut name = "Invalid";
        let mut target = level;
        match case {
            "range" => bad.range.cut = 100.0,
            "basis" => bad.basis.rotation = f64::NAN,
            "version" => bad.schema_version = 99,
            "level" => target = Id::new(),
            "name" => name = "",
            "scale" => bad.scale_denominator = 0.0,
            _ => unreachable!(),
        }
        assert!(
            editor.update_floor_plan(view, name, target, bad).is_err(),
            "{case}"
        );
        assert_eq!(editor.document.model(), &original);
        assert_eq!(editor.document.history_stats(), stats);
        assert_eq!(editor.document.revision(), revision);
        assert!(!editor.is_dirty());
    }
    assert!(editor.document.can_redo());
    editor.open(&path).unwrap();
    let context = editor.native_plan_context(view).unwrap();
    assert!(!context.show_walls);
    assert!(
        editor
            .native_wall_plan(view)
            .unwrap()
            .items(context)
            .unwrap()
            .is_empty()
    );
    assert!(old.items(context).is_err());
    assert!(original_context.show_walls);
}

fn editor() -> (Editor, Id) {
    let mut editor = Editor::new().unwrap();
    let level = *editor.document.model().levels.keys().next().unwrap();
    let id = editor.create_floor_plan("Ground plan", level).unwrap();
    editor
        .wall_command(
            "Create",
            Request::CreateWall(WallParams {
                name: "Wall".into(),
                start: Point2::new(0.0, 0.0),
                end: Point2::new(5.0, 0.0),
                thickness: 0.2,
                height: 3.0,
                level,
                material: None,
            }),
        )
        .unwrap();
    (editor, id)
}

#[test]
fn plan_derives_the_same_wall_ids_and_revisions_as_the_3d_scene() {
    let (mut editor, view) = editor();
    let context = editor.native_plan_context(view).unwrap();
    let drawing = editor.native_wall_plan(view).unwrap();
    let item = &drawing.items(context).unwrap()[0];
    let id = item.entity;
    assert!(editor.scene.contains_key(&id));
    assert_eq!(item.footprint.role, PlanRole::Cut);
    assert!((item.footprint.area() - 1.0).abs() < 1e-9);
    assert!((editor.scene[&id].signed_volume() - 3.0).abs() < 1e-9);
    assert_eq!(
        drawing.pick(context, Point2::new(2.0, 0.0)).unwrap(),
        Some(id)
    );
    let mut parameters = editor.document.model().walls[&id]
        .parameters
        .with_length(8.0)
        .unwrap();
    parameters.height = 0.8;
    editor
        .wall_command("Resize", Request::EditWall { id, parameters })
        .unwrap();
    let new_context = editor.native_plan_context(view).unwrap();
    assert!(drawing.items(new_context).is_err());
    let edited = editor.native_wall_plan(view).unwrap();
    assert_eq!(edited.items(new_context).unwrap()[0].entity, id);
    assert_eq!(
        edited.items(new_context).unwrap()[0].footprint.role,
        PlanRole::Projected
    );
    assert!((edited.items(new_context).unwrap()[0].footprint.area() - 1.6).abs() < 1e-9);
    assert!((editor.scene[&id].signed_volume() - 1.28).abs() < 1e-9);
    editor.undo().unwrap();
    let undo_context = editor.native_plan_context(view).unwrap();
    assert!(drawing.items(undo_context).is_err()); // restoring geometry cannot revive an old revision
    assert_eq!(
        editor
            .native_wall_plan(view)
            .unwrap()
            .items(undo_context)
            .unwrap()[0]
            .footprint
            .role,
        PlanRole::Cut
    );
    editor.redo().unwrap();
    assert_eq!(
        editor
            .native_wall_plan(view)
            .unwrap()
            .items(editor.native_plan_context(view).unwrap())
            .unwrap()[0]
            .footprint
            .role,
        PlanRole::Projected
    );
}

#[test]
fn level_and_range_changes_invalidate_plans_without_mutating_model_for_navigation() {
    let (mut editor, view) = editor();
    let drawing = editor.native_wall_plan(view).unwrap();
    let level = editor.document.model().views[&view]
        .parameters
        .level
        .unwrap();
    let mut parameters = editor.document.model().levels[&level].parameters.clone();
    parameters.elevation = 4.2;
    editor
        .command(
            "Raise level",
            Command::UpdateLevel {
                id: level,
                parameters,
            },
        )
        .unwrap();
    let context = editor.native_plan_context(view).unwrap();
    assert!((context.range.cut - 5.4).abs() < 1e-9);
    assert!(drawing.items(context).is_err());
    let raised = editor.native_wall_plan(view).unwrap();
    assert_eq!(
        raised.items(context).unwrap()[0].footprint.role,
        PlanRole::Cut
    );
    let mut settings = editor.document.model().views[&view]
        .parameters
        .plan
        .unwrap();
    settings.range.top = 5.0;
    settings.range.cut = 4.0;
    editor
        .update_floor_plan(view, "Ground plan", level, settings)
        .unwrap();
    let changed_context = editor.native_plan_context(view).unwrap();
    assert!(raised.items(changed_context).is_err());
    assert_eq!(changed_context.settings_revision, 1);
    let before = editor.document.model().clone();
    let stats = editor.document.history_stats();
    assert_eq!(
        editor
            .native_wall_plan(view)
            .unwrap()
            .items(changed_context)
            .unwrap()[0]
            .footprint
            .role,
        PlanRole::Projected
    );
    assert_eq!(editor.document.model(), &before);
    assert_eq!(editor.document.history_stats(), stats);
}

#[test]
fn unsupported_extension_payloads_are_reported_not_guessed_and_3d_view_is_rejected() {
    let (mut editor, view) = editor();
    let mut model = editor.document.model().clone();
    let id = Id::new();
    model.plugin_requirements.insert(
        "org.example.unknown".into(),
        os_model::PluginRequirement {
            version: "1.0.0".into(),
        },
    );
    model.extensions.insert(
        id,
        os_model::ExtensionEntity {
            envelope_version: 1,
            id,
            owner: "org.example.unknown".into(),
            type_id: "org.example.unknown.wall".into(),
            name: "Not a native wall".into(),
            payload_schema_version: 1,
            relationships: Default::default(),
            depends_on: Default::default(),
            payload: serde_json::json!({"height": 3, "width": 5}),
        },
    );
    editor.document = Document::from_model(model).unwrap();
    let context = editor.native_plan_context(view).unwrap();
    let drawing = editor.native_wall_plan(view).unwrap();
    assert_eq!(drawing.unavailable(context).unwrap(), &[id]);
    assert_eq!(drawing.items(context).unwrap().len(), 1);
    assert!(
        drawing
            .items(context)
            .unwrap()
            .iter()
            .all(|item| item.entity != id)
    );
    let perspective = editor
        .document
        .model()
        .views
        .values()
        .find(|v| v.parameters.kind == ViewKind::Perspective)
        .unwrap()
        .id();
    assert!(editor.native_wall_plan(perspective).is_err());
    assert!(editor.native_wall_plan(Id::new()).is_err());
}
