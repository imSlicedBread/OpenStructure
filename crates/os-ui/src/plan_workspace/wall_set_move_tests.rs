//! Multi-wall move acceptance through real plan frames and disposable candidates.
use super::*;
use crate::plan_workspace::wall_set_move::Draft;
use os_geometry::rooms::derive_faces;
use os_model::{
    Dimension, DimensionEndpoint, DimensionLayout, DimensionParams, DimensionReference, Model,
    Opening, OpeningDefinition, OpeningKind, OpeningParams, OpeningTag, OpeningTagParams, Room,
    RoomParams, RoomTag, RoomTagParams, Wall, WallAnchor, WallEndpoint, WallJoin, WallJoinParams,
    WallPath,
};

fn add_wall(model: &mut Model, template: Id, start: Point2, end: Point2) -> Id {
    let mut parameters = model.walls[&template].parameters.clone();
    parameters.path = WallPath::Straight { start, end };
    let wall = Wall::new(os_walls::WALL_TYPE, parameters);
    let id = wall.id();
    model.walls.insert(id, wall);
    id
}

fn load(h: &mut Harness, model: Model, selection: &[Id]) {
    let geometry: Vec<_> = model
        .walls
        .keys()
        .chain(model.openings.keys())
        .chain(model.floors.keys())
        .chain(model.columns.keys())
        .chain(model.stairs.keys())
        .chain(model.roofs.keys())
        .chain(model.ceilings.keys())
        .copied()
        .collect();
    h.app.editor.document = Document::from_model(model).unwrap();
    h.app.editor.pending_geometry.extend(geometry);
    h.app.editor.regenerate().unwrap();
    h.app.plans.poll(&h.app.editor);
    h.app.focus_plan(Some(h.view));
    h.app.select(Some(selection[0]));
    h.app.selected_ids = selection.iter().copied().collect();
    h.settle();
}

fn assert_point_near(actual: Point2, expected: Point2) {
    assert!(
        (actual.x - expected.x).abs() < 1e-9,
        "x: {actual:?} != {expected:?}"
    );
    assert!(
        (actual.y - expected.y).abs() < 1e-9,
        "y: {actual:?} != {expected:?}"
    );
}

fn legacy_door(host: Id) -> Opening {
    Opening::new(
        "core.opening",
        OpeningParams {
            open_state: Default::default(),
            name: "Entry door".into(),
            host,
            offset: 0.5,
            definition: OpeningDefinition::Legacy {
                kind: OpeningKind::Door,
                width: 0.9,
                height: 2.1,
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
    )
}

#[test]
fn wall_set_move_preview_commit_undo_preserves_openings_tags_and_dimensions() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        let mut model = h.app.editor.document.model().clone();
        let top = add_wall(
            &mut model,
            h.wall,
            Point2::new(-1.0, 2.0),
            Point2::new(1.0, 2.0),
        );
        let opening = legacy_door(top);
        let opening_id = opening.id();
        let opening_before = opening.parameters.clone();
        model.openings.insert(opening_id, opening);
        let tag = OpeningTag::new(
            "core.opening_tag",
            OpeningTagParams {
                view: h.view,
                opening: opening_id,
                position: Point2::new(0.25, 2.35),
                label_preset: Default::default(),
            },
        );
        let tag_id = tag.id();
        model.opening_tags.insert(tag_id, tag);
        let dimension = Dimension::new(
            "core.dimension",
            DimensionParams {
                layout: DimensionLayout::Aligned,
                view: h.view,
                first: DimensionReference::WallEndpoint {
                    wall: top,
                    endpoint: DimensionEndpoint::Start,
                },
                second: DimensionReference::WallEndpoint {
                    wall: top,
                    endpoint: DimensionEndpoint::End,
                },
                additional: Vec::new(),
                baseline_spacing_m: 0.25,
                offset_m: 0.3,
                orphan_hint: Point2::new(0.0, 2.3),
            },
        );
        let dimension_id = dimension.id();
        model.dimensions.insert(dimension_id, dimension);
        let wall_id = h.wall;
        load(&mut h, model.clone(), &[wall_id, top]);

        let before = h.app.editor.document.model().clone();
        let scene = h.app.editor.scene.clone();
        let history = h.app.editor.document.history_stats();
        let camera = h.app.plans.cameras[&h.view];
        let drawing_id = h.app.plans.drawing.as_ref().unwrap().identity();
        h.app.plans.split = true;
        h.app.plans.snaps.enabled = false;
        h.app.begin_wall_set_move();
        assert!(h.app.plans.wall_set_move.is_some(), "{}", h.app.status);
        h.frame(vec![]); // refresh canvas geometry after the toolbar switches modes

        let base = h.point(Point2::new(0.0, 0.0));
        h.click(base);
        let destination = h.point(Point2::new(0.75, 1.0));
        h.frame(vec![egui::Event::PointerMoved(destination)]);
        assert_eq!(h.app.editor.document.model(), &before);
        assert_eq!(h.app.editor.scene, scene);
        assert_eq!(h.app.editor.document.history_stats(), history);
        assert_eq!(h.app.plans.cameras[&h.view], camera);
        assert!(
            h.app
                .plans
                .wall_set_move
                .as_ref()
                .unwrap()
                .gesture
                .start
                .is_some()
        );

        h.click(destination);
        h.settle();
        let after = h.app.editor.document.model();
        let delta = Point2::new(0.75, 1.0);
        for id in [h.wall, top] {
            let old = &before.walls[&id];
            let new = &after.walls[&id];
            assert_eq!(new.header, old.header);
            assert_eq!(new.parameters.level, old.parameters.level);
            assert_eq!(new.parameters.thickness, old.parameters.thickness);
            assert_eq!(new.parameters.height, old.parameters.height);
            assert_eq!(new.parameters.material, old.parameters.material);
            assert_point_near(
                new.parameters.start(),
                Point2::new(
                    old.parameters.start().x + delta.x,
                    old.parameters.start().y + delta.y,
                ),
            );
            assert_point_near(
                new.parameters.end(),
                Point2::new(
                    old.parameters.end().x + delta.x,
                    old.parameters.end().y + delta.y,
                ),
            );
        }
        assert_eq!(after.openings[&opening_id].parameters, opening_before);
        assert_eq!(
            after.opening_tags[&tag_id].parameters.position,
            Point2::new(1.0, 3.35)
        );
        assert_eq!(
            DimensionReference::WallEndpoint {
                wall: top,
                endpoint: DimensionEndpoint::Start,
            }
            .resolve(after, before.walls[&top].parameters.level)
            .unwrap(),
            Point2::new(-0.25, 3.0)
        );
        assert_eq!(h.app.selected_ids, [h.wall, top].into_iter().collect());
        assert_eq!(
            h.app.editor.document.history_stats().undo_entries,
            history.undo_entries + 1
        );
        assert_ne!(h.app.plans.drawing.as_ref().unwrap().identity(), drawing_id);
        assert_ne!(h.app.editor.scene, scene);
        let committed = after.clone();
        h.app.history(false);
        h.settle();
        assert_eq!(h.app.editor.document.model(), &before);
        assert_eq!(h.app.editor.scene, scene);
        h.app.history(true);
        h.settle();
        assert_eq!(h.app.editor.document.model(), &committed);
        assert!(
            h.app
                .editor
                .document
                .model()
                .dimensions
                .contains_key(&dimension_id)
        );
    }
}

#[test]
fn wall_set_move_exact_metric_vector_and_join_atomicity() {
    let mut h = Harness::new(PROFILES[0].0, PROFILES[0].1);
    let mut model = h.app.editor.document.model().clone();
    let joined = add_wall(
        &mut model,
        h.wall,
        Point2::new(1.0, 0.0),
        Point2::new(3.0, 0.0),
    );
    let other = add_wall(
        &mut model,
        h.wall,
        Point2::new(-1.0, 3.0),
        Point2::new(1.0, 3.0),
    );
    let join = WallJoin::new(
        "core.wall_join",
        WallJoinParams::Butt {
            a: WallAnchor {
                wall: h.wall,
                endpoint: WallEndpoint::End,
            },
            b: WallAnchor {
                wall: joined,
                endpoint: WallEndpoint::Start,
            },
        },
    );
    let join_id = join.id();
    model.wall_joins.insert(join_id, join);
    let wall_id = h.wall;
    load(&mut h, model, &[wall_id, joined, other]);

    h.app.selected_ids = [h.wall, other].into_iter().collect();
    assert!(
        Draft::begin(&h.app).is_err(),
        "partial join must be rejected"
    );
    h.app.selected_ids = [h.wall, joined, other].into_iter().collect();
    let mut draft = Draft::begin(&h.app).unwrap();
    draft.gesture.start = Some(Point2::new(0.0, 0.0));
    draft.gesture.length = "1.25".into();
    draft.gesture.angle_degrees = "90".into();
    let candidate = draft.candidate(Point2::new(0.4, 0.2)).unwrap();
    for id in [h.wall, joined, other] {
        assert_point_near(
            candidate.model.walls[&id].parameters.start(),
            Point2::new(
                h.app.editor.document.model().walls[&id]
                    .parameters
                    .start()
                    .x,
                h.app.editor.document.model().walls[&id]
                    .parameters
                    .start()
                    .y
                    + 1.25,
            ),
        );
    }
    assert!(candidate.model.wall_joins.contains_key(&join_id));
}

#[test]
fn wall_set_move_translates_complete_room_and_rejects_partial_room_boundary() {
    let mut h = Harness::new(PROFILES[0].0, PROFILES[0].1);
    let mut model = h.app.editor.document.model().clone();
    let top = add_wall(
        &mut model,
        h.wall,
        Point2::new(-1.0, 2.0),
        Point2::new(1.0, 2.0),
    );
    let left = add_wall(
        &mut model,
        h.wall,
        Point2::new(-1.0, 0.0),
        Point2::new(-1.0, 2.0),
    );
    let right = add_wall(
        &mut model,
        h.wall,
        Point2::new(1.0, 2.0),
        Point2::new(1.0, 0.0),
    );
    let seed = Point2::new(0.0, 1.0);
    let segments = model
        .room_boundary_segments(model.walls[&h.wall].parameters.level)
        .unwrap();
    let faces = derive_faces(&segments).unwrap();
    let face = faces.assign_seed(seed).unwrap();
    let room = Room::new(
        "core.room",
        RoomParams {
            number: "101".into(),
            name: "Office".into(),
            floor_finish: None,
            wall_finish: None,
            ceiling_finish: None,
            floor_material: None,
            wall_material: None,
            ceiling_material: None,
            level: model.walls[&h.wall].parameters.level,
            seed,
            boundary_signature: face.key.as_signature().to_vec(),
        },
    );
    let room_id = room.id();
    model.rooms.insert(room_id, room);
    let room_tag = RoomTag::new(
        "core.room_tag",
        RoomTagParams {
            view: h.view,
            room: room_id,
            position: Point2::new(0.2, 1.2),
        },
    );
    let room_tag_id = room_tag.id();
    model.room_tags.insert(room_tag_id, room_tag);
    let wall_id = h.wall;
    load(&mut h, model.clone(), &[wall_id, top, left, right]);

    let mut draft = Draft::begin(&h.app).unwrap();
    let delta = Point2::new(0.6, 0.8);
    draft.gesture.start = Some(Point2::new(0.0, 0.0));
    let candidate = draft.candidate(delta).unwrap();
    assert_eq!(
        candidate.model.rooms[&room_id].parameters.seed,
        Point2::new(0.6, 1.8)
    );
    assert_eq!(
        candidate.model.room_tags[&room_tag_id].parameters.position,
        Point2::new(0.8, 2.0)
    );
    let moved_segments = candidate
        .model
        .room_boundary_segments(model.walls[&h.wall].parameters.level)
        .unwrap();
    let moved_faces = derive_faces(&moved_segments).unwrap();
    let moved_face = moved_faces.assign_seed(Point2::new(0.6, 1.8)).unwrap();
    assert_eq!(
        moved_face.key.as_signature(),
        candidate.model.rooms[&room_id]
            .parameters
            .boundary_signature
    );

    h.app.selected_ids = [h.wall, top].into_iter().collect();
    draft = Draft::begin(&h.app).unwrap();
    assert!(
        draft.candidate(delta).is_err(),
        "partial room boundary must not silently open the room"
    );
    assert_eq!(h.app.editor.document.model(), &model);

    let history = h.app.editor.document.history_stats();
    h.app.plans.snaps.enabled = false;
    h.app.begin_wall_set_move();
    h.frame(vec![]);
    h.click(h.point(Point2::new(0.0, 0.0)));
    h.click(h.point(delta));
    assert!(
        h.app.plans.wall_set_move.is_none(),
        "invalid release cancels the draft"
    );
    assert_eq!(h.app.editor.document.model(), &model);
    assert_eq!(h.app.editor.document.history_stats(), history);
}

#[test]
fn wall_set_move_bounds_visibility_stale_cancel_and_pan_precedence() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        let mut model = h.app.editor.document.model().clone();
        let top = add_wall(
            &mut model,
            h.wall,
            Point2::new(-1.0, 2.0),
            Point2::new(1.0, 2.0),
        );
        let wall_id = h.wall;
        load(&mut h, model.clone(), &[wall_id, top]);
        assert!(Draft::begin(&h.app).is_ok());
        h.app.selected_ids = [wall_id].into_iter().collect();
        assert!(
            Draft::begin(&h.app).is_err(),
            "one wall is below the minimum selection size"
        );
        h.app.selected_ids = (0..65).map(|_| Id::new()).collect();
        assert!(
            Draft::begin(&h.app).is_err(),
            "more than 64 walls is rejected before entity lookup"
        );
        h.app.selected_ids = [wall_id, top].into_iter().collect();
        h.app.begin_wall_set_move();
        h.frame(vec![]);
        let base = h.point(Point2::new(0.0, 0.0));
        h.click(base);
        assert!(
            h.app
                .plans
                .wall_set_move
                .as_ref()
                .unwrap()
                .gesture
                .start
                .is_some()
        );
        let before = h.app.editor.document.model().clone();
        let camera = h.app.plans.cameras[&h.view];
        h.frame(vec![escape()]);
        assert!(h.app.plans.wall_set_move.is_none());
        assert_eq!(h.app.editor.document.model(), &before);
        assert_eq!(h.app.plans.cameras[&h.view], camera);
        assert!(!h.app.plans.endpoint_pointer_claimed);

        h.app.selected_ids.insert(Id::new());
        assert!(
            Draft::begin(&h.app).is_err(),
            "stale/non-wall selection rejected"
        );
        h.app.selected_ids = [h.wall, top].into_iter().collect();
        let mut hidden = model.clone();
        hidden
            .views
            .get_mut(&h.view)
            .unwrap()
            .parameters
            .plan
            .as_mut()
            .unwrap()
            .visibility
            .walls = false;
        load(&mut h, hidden, &[wall_id, top]);
        assert!(Draft::begin(&h.app).is_err(), "hidden walls rejected");

        let mut cropped = model.clone();
        cropped
            .views
            .get_mut(&h.view)
            .unwrap()
            .parameters
            .plan
            .as_mut()
            .unwrap()
            .crop = Some(os_model::PlanViewCrop {
            min: Point2::new(10.0, 10.0),
            max: Point2::new(11.0, 11.0),
        });
        load(&mut h, cropped, &[wall_id, top]);
        assert!(
            Draft::begin(&h.app).is_err(),
            "walls fully outside crop are rejected"
        );

        // Without a move draft, a blank-canvas drag remains ordinary navigation.
        load(&mut h, model, &[wall_id, top]);
        let camera = h.app.plans.cameras[&h.view];
        let from = h.app.plans.canvas_rect.unwrap().center() + egui::vec2(0.0, -80.0);
        let to = from + egui::vec2(25.0, 18.0);
        h.press(from);
        h.frame(vec![egui::Event::PointerMoved(to)]);
        h.release(to);
        assert_ne!(h.app.plans.cameras[&h.view], camera);
        assert!(h.app.plans.wall_set_move.is_none());

        // A document edit during the two-click draft makes its captured context stale.
        h.app.begin_wall_set_move();
        h.frame(vec![]);
        let base = h.point(Point2::new(0.0, -1.0));
        h.click(base);
        let mut changed = h.app.editor.document.model().walls[&top].parameters.clone();
        *changed.path.straight_end_mut().unwrap() = Point2::new(1.0, 2.25);
        h.app
            .editor
            .command(
                "Edit during move draft",
                Command::UpdateWall {
                    id: top,
                    parameters: changed,
                },
            )
            .unwrap();
        let edited_model = h.app.editor.document.model().clone();
        h.frame(vec![]);
        assert!(h.app.plans.wall_set_move.is_none());
        assert_eq!(h.app.editor.document.model(), &edited_model);
    }
}

#[test]
fn wall_set_move_snaps_to_other_entities_but_excludes_the_selected_set() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        let mut model = h.app.editor.document.model().clone();
        let top = add_wall(
            &mut model,
            h.wall,
            Point2::new(-1.0, 2.0),
            Point2::new(1.0, 2.0),
        );
        add_wall(
            &mut model,
            h.wall,
            Point2::new(4.0, 4.0),
            Point2::new(5.0, 4.0),
        );
        let wall_id = h.wall;
        load(&mut h, model, &[wall_id, top]);
        let draft = Draft::begin(&h.app).unwrap();
        let drawing = h.app.plans.drawing.as_ref().unwrap();
        let context = h.app.editor.native_plan_context(h.view).unwrap();
        let camera = h.app.plans.cameras[&h.view];
        let rect = h.app.plans.canvas_rect.unwrap();
        let size = [rect.width() as f64, rect.height() as f64];
        let snaps = SnapOptions {
            enabled: true,
            endpoints: true,
            intersections: false,
            perpendicular: false,
            midpoints: false,
            nearest: false,
            axis_extensions: false,
        };
        let screen_point = |world: Point2| {
            let plane = context.basis.world_to_plane(world).unwrap();
            camera.project(plane, size).unwrap()
        };

        let near_unselected = screen_point(Point2::new(4.05, 4.0));
        let snapped = draft
            .target(drawing, camera, size, near_unselected, snaps)
            .unwrap();
        assert_point_near(snapped, Point2::new(4.0, 4.0));

        let near_selected = screen_point(Point2::new(1.08, 0.0));
        let raw = camera.unproject(near_selected, size).unwrap();
        let excluded = draft
            .target(drawing, camera, size, near_selected, snaps)
            .unwrap();
        assert_point_near(excluded, raw);
        assert!(
            (excluded.x - 1.0).abs() > 0.01,
            "the selected wall's endpoint must not attract its own move"
        );
    }
}
