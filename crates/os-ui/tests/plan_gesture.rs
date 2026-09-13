use os_core::Point2;
use os_document::Command;
use os_model::WallParams;
use os_ui::{Editor, plan_gesture::WallGesture};

fn setup() -> (Editor, os_core::Id, WallParams) {
    let mut editor = Editor::new().unwrap();
    let level = *editor.document.model().levels.keys().next().unwrap();
    let view = editor.create_floor_plan("Plan", level).unwrap();
    let p = WallParams {
        name: "Drawn wall".into(),
        start: Point2::default(),
        end: Point2::new(5.0, 0.0),
        thickness: 0.2,
        height: 3.0,
        level,
        material: None,
    };
    (editor, view, p)
}
#[test]
fn exact_destination_requires_anchor_and_complete_dimensions() {
    let (editor, view, p) = setup();
    let mut gesture = WallGesture::begin(&editor, view, p).unwrap();
    gesture.length = "4".into();
    gesture.angle_degrees = "90".into();
    assert!(!gesture.has_exact_destination());
    gesture.start = Some(Point2::default());
    assert!(gesture.has_exact_destination());
    gesture.angle_degrees.clear();
    assert!(!gesture.has_exact_destination());
    gesture.angle_degrees = "90".into();
    gesture.length = " ".into();
    assert!(!gesture.has_exact_destination());
    gesture.length = "NaN".into();
    assert!(gesture.has_exact_destination());
    assert!(gesture.parameters(Point2::default()).is_err());
}

#[test]
fn offset_copy_is_signed_parallel_atomic_and_preserves_source() {
    use os_ui::plan_gesture::WallEdit;
    let (mut editor, view, mut p) = setup();
    p.start = Point2::new(1_000_000.0, 2_000_000.0);
    p.end = Point2::new(1_000_003.0, 2_000_004.0);
    editor
        .wall_command("Create", os_plugin_api::Request::CreateWall(p.clone()))
        .unwrap();
    let id = *editor.document.model().walls.keys().next().unwrap();
    let mut settings = editor.document.model().views[&view]
        .parameters
        .plan
        .unwrap();
    settings.basis.origin = p.start;
    settings.basis.rotation = 0.37;
    editor
        .update_floor_plan(view, "Rotated", p.level, settings)
        .unwrap();
    let original = editor.document.model().clone();
    let mut gesture = WallGesture::begin_edit(&editor, view, id, WallEdit::OffsetCopy).unwrap();
    assert_eq!(gesture.snap_exclusion(), Some(id));
    for distance in [-2.0, 2.0] {
        gesture.length = distance.to_string();
        let preview = gesture.parameters(Point2::new(30.0, 50.0)).unwrap();
        assert!(
            preview.start.distance(Point2::new(
                p.start.x - 0.8 * distance,
                p.start.y + 0.6 * distance
            )) < 1e-8
        );
        assert!((preview.length() - 5.0).abs() < 1e-8);
        assert_eq!(preview.level, p.level);
        assert_eq!(editor.document.model(), &original);
    }
    for bad in ["0", "NaN", "inf", "-", "0.0000001"] {
        gesture.length = bad.into();
        assert!(
            gesture
                .commit(&mut editor, Some(view), Point2::new(0.0, 2.0))
                .is_err()
        );
        assert_eq!(editor.document.model(), &original);
    }
    gesture.length = "2".into();
    let expected = gesture.parameters(Point2::default()).unwrap();
    gesture
        .commit(&mut editor, Some(view), Point2::default())
        .unwrap();
    let changed = editor.document.model().clone();
    assert_eq!(changed.walls[&id], original.walls[&id]);
    assert_eq!(changed.walls.len(), 2);
    let copied = changed.walls.values().find(|wall| wall.id() != id).unwrap();
    assert_eq!(copied.parameters, expected);
    assert_eq!(editor.scene.len(), 2);
    assert!(
        gesture
            .commit(&mut editor, Some(view), Point2::default())
            .is_err()
    );
    editor.undo().unwrap();
    assert_eq!(editor.document.model(), &original);
    editor.redo().unwrap();
    assert_eq!(editor.document.model(), &changed);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("offset.osb");
    editor.save(&path).unwrap();
    let mut opened = Editor::new().unwrap();
    opened.open(&path).unwrap();
    assert_eq!(opened.document.model(), &changed);
}

#[test]
fn preview_exact_input_and_single_commit_share_one_history_entry() {
    let (mut editor, view, p) = setup();
    let before = editor.document.model().clone();
    let mut gesture = WallGesture::begin(&editor, view, p).unwrap();
    gesture.start = Some(Point2::new(1.0, 2.0));
    gesture.length = "4".into();
    gesture.angle_degrees = "90".into();
    let p = gesture.parameters(Point2::new(20.0, 20.0)).unwrap();
    assert!((p.end.x - 1.0).abs() < 1e-10 && (p.end.y - 6.0).abs() < 1e-10);
    assert_eq!(editor.document.model(), &before);
    gesture
        .commit(&mut editor, Some(view), Point2::new(20.0, 20.0))
        .unwrap();
    let changed = editor.document.model().clone();
    assert_eq!(changed.walls.len(), 1);
    assert_eq!(editor.scene.len(), 1);
    assert!(
        gesture
            .commit(&mut editor, Some(view), Point2::new(20.0, 20.0))
            .is_err()
    );
    editor.undo().unwrap();
    assert_eq!(editor.document.model(), &before);
    editor.redo().unwrap();
    assert_eq!(editor.document.model(), &changed);
}
#[test]
fn invalid_and_stale_gestures_never_commit() {
    let (mut editor, view, p) = setup();
    let mut gesture = WallGesture::begin(&editor, view, p).unwrap();
    gesture.start = Some(Point2::default());
    let original = editor.document.model().clone();
    for text in ["-", "NaN", "0", "-5", "inf"] {
        gesture.length = text.into();
        assert!(
            gesture
                .commit(&mut editor, Some(view), Point2::new(3.0, 0.0))
                .is_err()
        );
        assert_eq!(editor.document.model(), &original);
    }
    gesture.length = String::new();
    assert!(
        gesture
            .commit(&mut editor, None, Point2::new(3.0, 0.0))
            .is_err()
    );
    editor
        .command("Other edit", Command::RenameProject("Changed".into()))
        .unwrap();
    assert!(!gesture.current(&editor, Some(view)));
    let changed = editor.document.model().clone();
    assert!(
        gesture
            .commit(&mut editor, Some(view), Point2::new(3.0, 0.0))
            .is_err()
    );
    assert_eq!(editor.document.model(), &changed);
}

#[test]
fn move_and_resize_preserve_identity_properties_and_single_history_steps() {
    use os_ui::plan_gesture::WallEdit;
    let (mut editor, view, p) = setup();
    editor
        .wall_command("Create", os_plugin_api::Request::CreateWall(p.clone()))
        .unwrap();
    let id = *editor.document.model().walls.keys().next().unwrap();
    let original = editor.document.model().clone();
    let mut gesture = WallGesture::begin_edit(&editor, view, id, WallEdit::Move).unwrap();
    assert_eq!(gesture.snap_exclusion(), None);
    gesture.start = Some(Point2::new(2.0, 0.0));
    assert_eq!(gesture.snap_exclusion(), Some(id));
    let preview = gesture.parameters(Point2::new(3.0, 2.0)).unwrap();
    assert_eq!(preview.start, Point2::new(1.0, 2.0));
    assert_eq!(preview.end, Point2::new(6.0, 2.0));
    assert_eq!(editor.document.model(), &original);
    let history = editor.document.history_stats().undo_entries;
    gesture
        .commit(&mut editor, Some(view), Point2::new(3.0, 2.0))
        .unwrap();
    assert_eq!(editor.document.history_stats().undo_entries, history + 1);
    assert_eq!(
        editor.document.model().walls[&id].header,
        original.walls[&id].header
    );
    assert_eq!(editor.document.model().walls.len(), 1);
    assert_eq!(editor.scene.len(), 1);
    assert!(
        gesture
            .commit(&mut editor, Some(view), Point2::new(4.0, 2.0))
            .is_err()
    );
    let moved = editor.document.model().clone();
    editor.undo().unwrap();
    assert_eq!(editor.document.model(), &original);
    editor.redo().unwrap();
    assert_eq!(editor.document.model(), &moved);
    let mut end = WallGesture::begin_edit(&editor, view, id, WallEdit::ResizeEnd).unwrap();
    end.length = "3".into();
    end.angle_degrees = "90".into();
    let preview = end.parameters(Point2::new(100.0, 100.0)).unwrap();
    assert_eq!(preview.start, Point2::new(1.0, 2.0));
    assert!(preview.end.distance(Point2::new(1.0, 5.0)) < 1e-12);
    end.commit(&mut editor, Some(view), Point2::new(100.0, 100.0))
        .unwrap();
    let end_fixed = editor.document.model().walls[&id].parameters.end;
    let start = WallGesture::begin_edit(&editor, view, id, WallEdit::ResizeStart).unwrap();
    let preview = start.parameters(Point2::new(3.0, 2.0)).unwrap();
    assert_eq!(preview.start, Point2::new(3.0, 2.0));
    assert_eq!(preview.end, end_fixed);
    assert_eq!(
        (
            preview.height,
            preview.thickness,
            preview.level,
            preview.material
        ),
        (p.height, p.thickness, p.level, p.material)
    );
    start
        .commit(&mut editor, Some(view), Point2::new(3.0, 2.0))
        .unwrap();
    assert_eq!(
        editor.document.model().walls[&id].header,
        original.walls[&id].header
    );
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("moved-resized.osb");
    let saved = editor.document.model().clone();
    editor.save(&path).unwrap();
    editor.open(&path).unwrap();
    assert_eq!(editor.document.model(), &saved);
}

#[test]
fn rotated_moves_keep_level_and_fixed_resize_endpoint_without_coordinate_roundtrip() {
    use os_ui::plan_gesture::WallEdit;
    let (mut editor, view, mut p) = setup();
    p.start = Point2::new(1_000_000.0, 2_000_000.0);
    p.end = Point2::new(1_000_005.0, 2_000_000.0);
    editor
        .wall_command("Create", os_plugin_api::Request::CreateWall(p.clone()))
        .unwrap();
    let id = *editor.document.model().walls.keys().next().unwrap();
    let building = editor.document.model().levels[&p.level].parameters.building;
    let level = os_model::Level::new(
        "core.level",
        os_model::LevelParams {
            name: "Other".into(),
            elevation: 1.0,
            building,
        },
    );
    editor
        .command("Level", Command::AddLevel(level.clone()))
        .unwrap();
    let mut settings = editor.document.model().views[&view]
        .parameters
        .plan
        .unwrap();
    settings.basis.origin = p.start;
    settings.basis.rotation = 0.3;
    editor
        .update_floor_plan(view, "Rotated", level.id(), settings)
        .unwrap();
    let mut gesture = WallGesture::begin_edit(&editor, view, id, WallEdit::Move).unwrap();
    gesture.start = Some(Point2::default());
    gesture.length = "2".into();
    gesture.angle_degrees = "90".into();
    let preview = gesture.parameters(Point2::new(20.0, 0.0)).unwrap();
    assert!(
        preview.start.distance(Point2::new(
            p.start.x - 2.0 * 0.3_f64.sin(),
            p.start.y + 2.0 * 0.3_f64.cos()
        )) < 1e-8
    );
    assert_eq!(preview.level, p.level);
    let resize = WallGesture::begin_edit(&editor, view, id, WallEdit::ResizeEnd).unwrap();
    assert_eq!(
        resize.parameters(Point2::new(2.0, 1.0)).unwrap().start,
        p.start
    );
    let resize = WallGesture::begin_edit(&editor, view, id, WallEdit::ResizeStart).unwrap();
    assert_eq!(resize.parameters(Point2::new(2.0, 1.0)).unwrap().end, p.end);
}

#[test]
fn invalid_edit_hidden_target_and_noop_leave_history_intact() {
    use os_ui::plan_gesture::WallEdit;
    let (mut editor, view, p) = setup();
    editor
        .wall_command("Create", os_plugin_api::Request::CreateWall(p.clone()))
        .unwrap();
    let id = *editor.document.model().walls.keys().next().unwrap();
    let original = editor.document.model().clone();
    let mut resize = WallGesture::begin_edit(&editor, view, id, WallEdit::ResizeEnd).unwrap();
    let stats = editor.document.history_stats();
    let rev = editor.document.revision();
    assert!(resize.commit(&mut editor, Some(view), p.start).is_err());
    for text in ["NaN", "-1", "0", "inf"] {
        resize.length = text.into();
        assert!(
            resize
                .commit(&mut editor, Some(view), Point2::new(3.0, 0.0))
                .is_err()
        );
        assert_eq!(editor.document.model(), &original);
        assert_eq!(editor.document.history_stats(), stats);
        assert_eq!(editor.document.revision(), rev);
    }
    let mut movement = WallGesture::begin_edit(&editor, view, id, WallEdit::Move).unwrap();
    movement.start = Some(Point2::default());
    movement
        .commit(&mut editor, Some(view), Point2::default())
        .unwrap();
    assert_eq!(editor.document.history_stats(), stats);
    assert_eq!(editor.document.revision(), rev);
    editor
        .command("Intervening edit", Command::RenameProject("Changed".into()))
        .unwrap();
    assert!(
        movement
            .commit(&mut editor, Some(view), Point2::new(1.0, 2.0))
            .is_err()
    );
    let mut settings = editor.document.model().views[&view]
        .parameters
        .plan
        .unwrap();
    settings.visibility.walls = false;
    editor
        .update_floor_plan(view, "Hidden", p.level, settings)
        .unwrap();
    assert!(WallGesture::begin_edit(&editor, view, id, WallEdit::Move).is_err());
}
