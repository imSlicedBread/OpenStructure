//! Real-egui acceptance for bounded native wall-assembly arrays.
use super::escape;
use super::wall_set_copy_tests as copy_tests;
use super::{Harness, PROFILES};
use crate::plan_workspace::{SnapOptions, wall_assembly_array::Array, wall_set_move::Draft};
use eframe::egui;
use os_core::{Id, Point2};
use os_geometry::rooms::derive_faces;
use os_model::*;
use std::collections::{BTreeMap, BTreeSet};

#[path = "wall_assembly_radial_array_tests.rs"]
mod radial;

fn near(a: Point2, b: Point2) {
    assert!(a.distance(b) < 1e-8, "{a:?} != {b:?}");
}

fn shifted(point: Point2, delta: Point2, repeat: usize) -> Point2 {
    Point2::new(
        point.x + delta.x * repeat as f64,
        point.y + delta.y * repeat as f64,
    )
}

fn new_walls(before: &Model, after: &Model) -> BTreeSet<Id> {
    after
        .walls
        .keys()
        .filter(|id| !before.walls.contains_key(id))
        .copied()
        .collect()
}

#[test]
fn wall_assembly_array_preview_commit_history_and_reopen_both_dpis() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        let (mut model, bay) = copy_tests::fixture(&mut h);
        let view = h.view;
        let mut settings = model.views[&view].parameters.plan.unwrap();
        settings.basis.rotation = 0.37;
        settings.basis.origin = Point2::default();
        model.views.get_mut(&view).unwrap().parameters.plan = Some(settings);
        copy_tests::load(&mut h, model, &bay.walls);
        h.app.plans.cameras.get_mut(&view).unwrap().center = Point2::new(4.5, 1.5);
        h.app.plans.cameras.get_mut(&view).unwrap().pixels_per_metre = 34.0;
        h.app.plans.split = true;
        h.frame(vec![]);

        let before = h.app.editor.document.model().clone();
        let scene = h.app.editor.scene.clone();
        let history = h.app.editor.document.history_stats();
        h.app.begin_wall_assembly_array();
        h.frame(vec![]);
        let draft = h.app.plans.wall_set_move.as_mut().unwrap();
        assert_eq!(draft.array_count(), Some(2));
        assert_eq!(
            draft.max_array_count(),
            17,
            "64 added-wall budget includes the source selection size"
        );
        draft.set_array_count(3).unwrap();
        assert_eq!(
            draft.dependency_summary().as_deref(),
            Some("8 walls, 4 openings, 8 joins, 4 active-plan tags, 4 clearance locks")
        );

        let base_world = Point2::new(2.0, 3.0);
        let base_plane = h
            .app
            .editor
            .native_plan_context(view)
            .unwrap()
            .basis
            .world_to_plane(base_world)
            .unwrap();
        let destination_plane = Point2::new(base_plane.x + 6.0, base_plane.y);
        let destination_world = h
            .app
            .editor
            .native_plan_context(view)
            .unwrap()
            .basis
            .plane_to_world(destination_plane)
            .unwrap();
        h.click(h.point(base_world));
        {
            let draft = h.app.plans.wall_set_move.as_mut().unwrap();
            draft.gesture.length = "6".into();
            draft.gesture.angle_degrees = "0".into();
        }
        let destination_screen = h.point(destination_world);
        h.frame(vec![egui::Event::PointerMoved(destination_screen)]);
        let draft = h.app.plans.wall_set_move.as_ref().unwrap();
        let preview = draft.candidate(destination_plane).unwrap();
        let repeated = draft.candidate(destination_plane).unwrap();
        assert_eq!(
            preview.model, repeated.model,
            "array identities remain stable during preview"
        );
        assert_eq!(
            preview.model.walls.len(),
            before.walls.len() + 2 * bay.walls.len()
        );
        assert_eq!(preview.model.openings.len(), before.openings.len() + 4);
        assert_eq!(
            preview.model.wall_joins.len(),
            before.wall_joins.len() + 2 * bay.joins.len()
        );
        assert_eq!(
            preview.model.opening_tags.len(),
            before.opening_tags.len() + 4
        );
        assert_eq!(h.app.editor.document.model(), &before);
        assert_eq!(h.app.editor.scene, scene);
        assert_eq!(h.app.editor.document.history_stats(), history);

        let context = h.app.editor.native_plan_context(view).unwrap();
        let base_world_on_plane = context.basis.plane_to_world(base_plane).unwrap();
        let delta = Point2::new(
            destination_world.x - base_world_on_plane.x,
            destination_world.y - base_world_on_plane.y,
        );
        h.click(destination_screen);
        h.settle();
        let after = h.app.editor.document.model().clone();
        assert_eq!(
            after, preview.model,
            "published model equals the validated preview"
        );
        let copied = new_walls(&before, &after);
        assert_eq!(copied.len(), 2 * bay.walls.len());
        assert_eq!(
            h.app.selected_ids, copied,
            "all repeated walls become the selection"
        );
        assert_eq!(
            after.dimensions, before.dimensions,
            "dimensions remain attached to source IDs"
        );
        assert_eq!(
            after.rooms, before.rooms,
            "rooms are not copied or rewritten"
        );
        assert_eq!(after.room_tags, before.room_tags);
        assert_eq!(
            h.app.editor.document.history_stats().undo_entries,
            history.undo_entries + 1,
            "the full array is one transaction"
        );

        let mut wall_map = BTreeMap::new();
        for source in &bay.walls {
            let old = &before.walls[source];
            for repeat in 1..=2 {
                let expected_start = shifted(old.parameters.start(), delta, repeat);
                let expected_end = shifted(old.parameters.end(), delta, repeat);
                let target = copied
                    .iter()
                    .copied()
                    .find(|id| {
                        let wall = &after.walls[id].parameters;
                        wall.start().distance(expected_start) < 1e-8
                            && wall.end().distance(expected_end) < 1e-8
                    })
                    .expect("every source wall has one clone at each array station");
                wall_map.insert((*source, repeat), target);
                let clone = &after.walls[&target];
                assert_eq!(clone.header.type_id, old.header.type_id);
                assert_eq!(clone.header.properties, old.header.properties);
                assert_ne!(clone.id(), old.id());
                assert_eq!(
                    after.wall_type_assignments[&target],
                    before.wall_type_assignments[source]
                );
                assert_eq!(
                    after.element_lifecycle(target),
                    before.element_lifecycle(*source)
                );
                let mut parameters = clone.parameters.clone();
                *parameters.path.straight_start_mut().unwrap() = old.parameters.start();
                *parameters.path.straight_end_mut().unwrap() = old.parameters.end();
                assert_eq!(
                    parameters, old.parameters,
                    "wall instance data is preserved"
                );
                assert!(h.app.editor.scene.contains_key(&target));
            }
        }

        let mut opening_map = BTreeMap::new();
        for source in bay.openings {
            for repeat in 1..=2 {
                let old = &before.openings[&source];
                let host = wall_map[&(old.parameters.host, repeat)];
                let (id, clone) = after
                    .openings
                    .iter()
                    .find(|(id, opening)| {
                        !before.openings.contains_key(id)
                            && opening.parameters.host == host
                            && opening.parameters.name == old.parameters.name
                    })
                    .expect("each repeated host keeps its typed hosted opening");
                opening_map.insert((source, repeat), *id);
                let mut expected = old.parameters.clone();
                expected.host = host;
                assert!((clone.parameters.offset - expected.offset).abs() < 1e-9);
                let mut actual = clone.parameters.clone();
                actual.offset = expected.offset;
                assert_eq!(actual, expected);
                assert_eq!(
                    after.opening_clearances[id],
                    before.opening_clearances[&source]
                );
                assert_eq!(
                    after.element_lifecycle(*id),
                    before.element_lifecycle(source)
                );
                assert!(h.app.editor.scene.contains_key(id));
            }
        }
        for source in bay.tags {
            let old = &before.opening_tags[&source];
            for repeat in 1..=2 {
                let opening = opening_map[&(old.parameters.opening, repeat)];
                let clone = after
                    .opening_tags
                    .values()
                    .find(|tag| tag.parameters.opening == opening)
                    .expect("active-plan opening tags repeat with their hosted opening");
                near(
                    clone.parameters.position,
                    shifted(old.parameters.position, delta, repeat),
                );
                assert_eq!(clone.parameters.label_preset, old.parameters.label_preset);
            }
        }
        assert!(copied.iter().all(|id| h.app.editor.scene.contains_key(id)));

        let committed_scene = h.app.editor.scene.clone();
        h.app.history(false);
        h.settle();
        assert_eq!(h.app.editor.document.model(), &before);
        assert_eq!(h.app.editor.scene, scene);
        h.app.history(true);
        h.settle();
        assert_eq!(h.app.editor.document.model(), &after);
        assert_eq!(h.app.editor.scene, committed_scene);

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("wall-assembly-array.osb");
        h.app.editor.save(&path).unwrap();
        h.app.editor.open(&path).unwrap();
        assert_eq!(h.app.editor.document.model(), &after);
        assert!(copied.iter().all(|id| h.app.editor.scene.contains_key(id)));
    }
}

#[test]
fn wall_assembly_array_bounds_collision_snap_cancel_and_pan() {
    let mut h = Harness::new(PROFILES[0].0, PROFILES[0].1);
    let (model, bay) = copy_tests::fixture(&mut h);
    copy_tests::load(&mut h, model.clone(), &bay.walls);
    let before = h.app.editor.document.model().clone();
    let history = h.app.editor.document.history_stats();

    h.app.selected_ids = [*bay.walls.first().unwrap()].into_iter().collect();
    assert!(
        Draft::begin_array(&h.app).is_err(),
        "partial explicit joins reject"
    );
    h.app.selected_ids = bay.walls.clone();
    let mut draft = Draft::begin_array(&h.app).unwrap();
    assert_eq!(draft.max_array_count(), 17);
    assert!(
        draft.set_array_count(18).is_err(),
        "added-wall budget is enforced"
    );
    assert_eq!(
        draft.array_count(),
        Some(2),
        "a rejected count leaves the prior draft intact"
    );
    draft.set_array_count(3).unwrap();
    draft.gesture.start = Some(Point2::default());
    assert!(
        draft.candidate(Point2::default()).is_err(),
        "zero spacing is invalid"
    );

    let context = h.app.editor.native_plan_context(h.view).unwrap();
    let camera = h.app.plans.cameras[&h.view];
    let rect = h.app.plans.canvas_rect.unwrap();
    let size = [f64::from(rect.width()), f64::from(rect.height())];
    let target_plane = context
        .basis
        .world_to_plane(Point2::new(10.0, 3.0))
        .unwrap();
    let pointer = camera.project(target_plane, size).unwrap();
    let snapped = draft
        .target(
            h.app.plans.drawing.as_ref().unwrap(),
            camera,
            size,
            pointer,
            SnapOptions {
                enabled: true,
                endpoints: true,
                intersections: false,
                perpendicular: false,
                midpoints: false,
                nearest: false,
                axis_extensions: false,
            },
        )
        .unwrap();
    near(snapped, target_plane);
    assert_eq!(h.app.editor.document.model(), &before);
    assert_eq!(h.app.editor.document.history_stats(), history);

    h.app.begin_wall_assembly_array();
    h.frame(vec![]);
    h.click(h.point(Point2::new(0.0, 0.0)));
    h.frame(vec![escape()]);
    assert!(h.app.plans.wall_set_move.is_none());
    assert_eq!(h.app.editor.document.model(), &before);
    assert_eq!(h.app.editor.document.history_stats(), history);

    h.app.begin_wall_assembly_array();
    h.app.selected_ids = [*bay.walls.first().unwrap()].into_iter().collect();
    h.frame(vec![]);
    assert!(
        h.app.plans.wall_set_move.is_none(),
        "selection changes cancel the array"
    );
    h.app.selected_ids = bay.walls.clone();

    let camera_before = h.app.plans.cameras[&h.view];
    let canvas = h.app.plans.canvas_rect.unwrap();
    let from = canvas.center() + egui::vec2(0.0, -80.0);
    let to = from + egui::vec2(25.0, 18.0);
    h.press(from);
    h.frame(vec![egui::Event::PointerMoved(to)]);
    h.release(to);
    assert_ne!(
        h.app.plans.cameras[&h.view], camera_before,
        "ordinary canvas drag still pans"
    );

    let mut collision_model = model;
    *collision_model
        .walls
        .get_mut(&bay.snap_wall)
        .unwrap()
        .parameters
        .path
        .straight_start_mut()
        .unwrap() = Point2::new(9.0, 1.0);
    *collision_model
        .walls
        .get_mut(&bay.snap_wall)
        .unwrap()
        .parameters
        .path
        .straight_end_mut()
        .unwrap() = Point2::new(10.0, 1.0);
    copy_tests::load(&mut h, collision_model, &bay.walls);
    let mut collision_draft = Draft::begin_array(&h.app).unwrap();
    collision_draft.gesture.start = Some(Point2::default());
    collision_draft.set_array_count(3).unwrap();
    assert!(
        collision_draft.candidate(Point2::new(3.0, 0.0)).is_ok(),
        "first two copies fit"
    );
    collision_draft.set_array_count(4).unwrap();
    assert!(
        collision_draft.candidate(Point2::new(3.0, 0.0)).is_err(),
        "a late-copy collision rejects the whole array"
    );
    assert_eq!(h.app.editor.document.history_stats(), history);
}

#[test]
fn wall_assembly_array_aggregate_dependent_budget_is_enforced() {
    let h = Harness::new(PROFILES[0].0, PROFILES[0].1);
    let mut model = h.app.editor.document.model().clone();
    for index in 0..133 {
        let opening = Opening::new(
            "core.opening",
            OpeningParams {
                open_state: OpeningState::Default,
                name: format!("budget opening {index}"),
                host: h.wall,
                offset: 0.0,
                definition: OpeningDefinition::Legacy {
                    kind: OpeningKind::Window,
                    width: 0.1,
                    height: 0.1,
                    sill: 0.0,
                },
                width_override: None,
                height_override: None,
                sill_override: None,
                pane_position_override: None,
                lite_side_override: None,
                hinge: Default::default(),
                swing: Default::default(),
            },
        );
        model.openings.insert(opening.id(), opening);
    }
    let walls = BTreeSet::from([h.wall]);
    assert!(
        Array::new(&model, &walls, h.view, 32)
            .err()
            .unwrap()
            .to_string()
            .contains("4096"),
        "aggregate openings/joins/tags/locks must fit the 4096 dependent budget"
    );
}

#[test]
fn wall_assembly_array_rejects_room_boundary_changes() {
    let mut h = Harness::new(PROFILES[0].0, PROFILES[0].1);
    let mut model = h.app.editor.document.model().clone();
    let host = h.wall;
    let level = model.walls[&host].parameters.level;
    *model
        .walls
        .get_mut(&host)
        .unwrap()
        .parameters
        .path
        .straight_start_mut()
        .unwrap() = Point2::new(4.0, 0.0);
    *model
        .walls
        .get_mut(&host)
        .unwrap()
        .parameters
        .path
        .straight_end_mut()
        .unwrap() = Point2::new(8.0, 0.0);
    for (start, end) in [
        (Point2::new(-2.0, -2.0), Point2::new(2.0, -2.0)),
        (Point2::new(2.0, -2.0), Point2::new(2.0, 2.0)),
        (Point2::new(2.0, 2.0), Point2::new(-2.0, 2.0)),
        (Point2::new(-2.0, 2.0), Point2::new(-2.0, -2.0)),
    ] {
        let line = RoomSeparationLine::new(
            "core.room_separation_line",
            RoomSeparationLineParams { level, start, end },
        );
        model.room_separation_lines.insert(line.id(), line);
    }
    let faces = derive_faces(&model.room_boundary_segments(level).unwrap()).unwrap();
    let seed = Point2::new(0.0, 1.0);
    let face = faces.assign_seed(seed).unwrap();
    let room = Room::new(
        "core.room",
        RoomParams {
            number: "A101".into(),
            name: "Array-boundary test room".into(),
            floor_finish: None,
            wall_finish: None,
            ceiling_finish: None,
            floor_material: None,
            wall_material: None,
            ceiling_material: None,
            level,
            seed,
            boundary_signature: face.key.as_signature().to_vec(),
        },
    );
    model.rooms.insert(room.id(), room);
    let walls = BTreeSet::from([host]);
    copy_tests::load(&mut h, model.clone(), &walls);
    let mut draft = Draft::begin_array(&h.app).unwrap();
    draft.gesture.start = Some(Point2::default());
    assert!(
        draft
            .candidate(Point2::new(-6.0, 0.0))
            .err()
            .unwrap()
            .to_string()
            .contains("room"),
        "a repeated wall that splits an accepted room boundary is rejected"
    );
    assert_eq!(h.app.editor.document.model(), &model);
}

#[test]
fn wall_assembly_array_preserves_exact_non_cardinal_tee_contacts() {
    let mut h = Harness::new(PROFILES[0].0, PROFILES[0].1);
    let mut model = h.app.editor.document.model().clone();
    let host = h.wall;
    *model
        .walls
        .get_mut(&host)
        .unwrap()
        .parameters
        .path
        .straight_start_mut()
        .unwrap() = Point2::new(-4.0, 0.0);
    *model
        .walls
        .get_mut(&host)
        .unwrap()
        .parameters
        .path
        .straight_end_mut()
        .unwrap() = Point2::new(4.0, 0.0);
    let mut branch_parameters = model.walls[&host].parameters.clone();
    branch_parameters.path = WallPath::Straight {
        start: Point2::new(0.0, 0.0),
        end: Point2::new(0.0, -2.0),
    };
    let branch_wall = Wall::new(os_walls::WALL_TYPE, branch_parameters);
    let branch_id = branch_wall.id();
    model.walls.insert(branch_id, branch_wall);
    let join = WallJoin::new(
        "core.wall_join",
        WallJoinParams::Tee {
            host,
            station: 4.0,
            branch: WallAnchor {
                wall: branch_id,
                endpoint: WallEndpoint::Start,
            },
        },
    );
    model.wall_joins.insert(join.id(), join);
    let walls = BTreeSet::from([host, branch_id]);
    copy_tests::load(&mut h, model.clone(), &walls);
    let mut draft = Draft::begin_array(&h.app).unwrap();
    draft.gesture.start = Some(Point2::default());
    draft.set_array_count(3).unwrap();
    let candidate = draft
        .candidate(Point2::new(7.371, 5.619))
        .expect("non-cardinal array spacing preserves model-valid tees");
    let copied_joins = candidate
        .model
        .wall_joins
        .iter()
        .filter(|(id, _)| !model.wall_joins.contains_key(id));
    let mut count = 0;
    for (_, join) in copied_joins {
        let WallJoinParams::Tee {
            host,
            station,
            branch,
        } = join.parameters
        else {
            panic!("array changed a tee into another join type");
        };
        let host_path = &candidate.model.walls[&host].parameters.path;
        let branch_parameters = &candidate.model.walls[&branch.wall].parameters;
        assert_eq!(
            host_path.point(station),
            branch.endpoint.point(branch_parameters)
        );
        count += 1;
    }
    assert_eq!(count, 2);
    assert_eq!(
        h.app.editor.document.model(),
        &model,
        "preview leaves the source unchanged"
    );
}
