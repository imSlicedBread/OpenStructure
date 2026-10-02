//! Real-egui acceptance for translating a joined native wall assembly by copy.
use super::*;
use crate::plan_workspace::wall_set_move::Draft;
use os_geometry::rooms::derive_faces;
use os_model::{
    Dimension, DimensionEndpoint, DimensionLayout, DimensionParams, DimensionReference,
    ElementLifecycle, LayerFunction, Opening, OpeningClearance, OpeningDefinition, OpeningKind,
    OpeningParams, OpeningTag, OpeningTagLabelPreset, OpeningTagParams, OpeningType,
    OpeningTypeParams, Room, RoomParams, RoomTag, RoomTagParams, Wall, WallAnchor, WallEndpoint,
    WallJoin, WallJoinParams, WallLayer, WallPath, WallType, WallTypeAssignment, WallTypeParams,
    WindowOperation, WindowPanePosition,
};
use std::collections::BTreeSet;

struct Bay {
    walls: BTreeSet<Id>,
    openings: [Id; 2],
    joins: BTreeSet<Id>,
    tags: [Id; 2],
    dimension: Id,
    room: Id,
    room_tag: Id,
    snap_wall: Id,
}

fn add_wall(model: &mut Model, template: Id, start: Point2, end: Point2) -> Id {
    let mut parameters = model.walls[&template].parameters.clone();
    parameters.path = WallPath::Straight { start, end };
    let wall = Wall::new(os_walls::WALL_TYPE, parameters);
    let id = wall.id();
    model.walls.insert(id, wall);
    id
}

fn add_corner(model: &mut Model, a: WallAnchor, b: WallAnchor, owner: Id) -> Id {
    let join = WallJoin::new("core.wall_join", WallJoinParams::Corner { a, b, owner });
    let id = join.id();
    model.wall_joins.insert(id, join);
    id
}

fn fixture(h: &mut Harness) -> (Model, Bay) {
    let mut model = h.app.editor.document.model().clone();
    let bottom = h.wall;
    *model
        .walls
        .get_mut(&bottom)
        .unwrap()
        .parameters
        .path
        .straight_start_mut()
        .unwrap() = Point2::new(-1.0, 0.0);
    *model
        .walls
        .get_mut(&bottom)
        .unwrap()
        .parameters
        .path
        .straight_end_mut()
        .unwrap() = Point2::new(1.0, 0.0);
    model
        .walls
        .get_mut(&bottom)
        .unwrap()
        .header
        .properties
        .insert(
            "copy-fixture".into(),
            serde_json::json!("source-wall-metadata"),
        );
    let top = add_wall(
        &mut model,
        bottom,
        Point2::new(-1.0, 2.0),
        Point2::new(1.0, 2.0),
    );
    let left = add_wall(
        &mut model,
        bottom,
        Point2::new(-1.0, 0.0),
        Point2::new(-1.0, 2.0),
    );
    let right = add_wall(
        &mut model,
        bottom,
        Point2::new(1.0, 0.0),
        Point2::new(1.0, 2.0),
    );
    let walls = [bottom, top, left, right]
        .into_iter()
        .collect::<BTreeSet<_>>();

    let material = model.walls[&bottom].parameters.material;
    let wall_type = WallType::new(
        "core.wall_type",
        WallTypeParams {
            name: "Assembly copy wall".into(),
            layers: vec![WallLayer {
                id: Id::new(),
                name: "Structure".into(),
                thickness: model.walls[&bottom].parameters.thickness,
                function: LayerFunction::Structure,
                material,
            }],
        },
    );
    let wall_type_id = wall_type.id();
    model.wall_types.insert(wall_type_id, wall_type);
    for (index, wall) in walls.iter().copied().enumerate() {
        model.wall_type_assignments.insert(
            wall,
            WallTypeAssignment {
                type_id: wall_type_id,
                flipped: index % 2 == 1,
            },
        );
    }

    let joins = [
        add_corner(
            &mut model,
            WallAnchor {
                wall: bottom,
                endpoint: WallEndpoint::Start,
            },
            WallAnchor {
                wall: left,
                endpoint: WallEndpoint::Start,
            },
            bottom,
        ),
        add_corner(
            &mut model,
            WallAnchor {
                wall: bottom,
                endpoint: WallEndpoint::End,
            },
            WallAnchor {
                wall: right,
                endpoint: WallEndpoint::Start,
            },
            bottom,
        ),
        add_corner(
            &mut model,
            WallAnchor {
                wall: top,
                endpoint: WallEndpoint::Start,
            },
            WallAnchor {
                wall: left,
                endpoint: WallEndpoint::End,
            },
            top,
        ),
        add_corner(
            &mut model,
            WallAnchor {
                wall: top,
                endpoint: WallEndpoint::End,
            },
            WallAnchor {
                wall: right,
                endpoint: WallEndpoint::End,
            },
            top,
        ),
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();

    let door_type = OpeningType::new(
        "core.opening_type",
        OpeningTypeParams {
            window_operation: WindowOperation::Fixed,
            family: Default::default(),
            name: "Copy test door".into(),
            kind: OpeningKind::Door,
            width: 0.9,
            height: 2.1,
            sill: 0.0,
            pane_position: WindowPanePosition::Center,
        },
    );
    let door_type_id = door_type.id();
    model.opening_types.insert(door_type_id, door_type);
    let window_type = OpeningType::new(
        "core.opening_type",
        OpeningTypeParams {
            window_operation: WindowOperation::Sliding,
            family: Default::default(),
            name: "Copy test window".into(),
            kind: OpeningKind::Window,
            width: 0.7,
            height: 1.2,
            sill: 0.6,
            pane_position: WindowPanePosition::LeftFace,
        },
    );
    let window_type_id = window_type.id();
    model.opening_types.insert(window_type_id, window_type);
    let door = Opening::new(
        "core.opening",
        OpeningParams {
            open_state: os_model::OpeningState::DoorAngle(45.0),
            name: "Copy test door instance".into(),
            host: bottom,
            offset: 0.3,
            definition: OpeningDefinition::Typed {
                type_id: door_type_id,
            },
            width_override: Some(0.82),
            height_override: Some(2.15),
            sill_override: None,
            pane_position_override: None,
            lite_side_override: None,
            hinge: os_model::DoorHinge::End,
            swing: os_model::DoorSwing::Right,
        },
    );
    let door_id = door.id();
    model.openings.insert(door_id, door);
    let window = Opening::new(
        "core.opening",
        OpeningParams {
            open_state: os_model::OpeningState::SlidingFraction(0.4),
            name: "Copy test window instance".into(),
            host: top,
            offset: 0.95,
            definition: OpeningDefinition::Typed {
                type_id: window_type_id,
            },
            width_override: Some(0.65),
            height_override: Some(1.1),
            sill_override: Some(0.75),
            pane_position_override: Some(WindowPanePosition::RightFace),
            lite_side_override: None,
            hinge: Default::default(),
            swing: Default::default(),
        },
    );
    let window_id = window.id();
    model.openings.insert(window_id, window);
    let openings = [door_id, window_id];
    model.opening_clearances.insert(
        door_id,
        OpeningClearance {
            end: os_model::ClearanceEnd::Start,
            distance: 0.3,
        },
    );
    model.opening_clearances.insert(
        window_id,
        OpeningClearance {
            end: os_model::ClearanceEnd::End,
            distance: 0.4,
        },
    );

    let existing = model.existing_phase().unwrap();
    let demolition = model.latest_phase().unwrap();
    let lifecycle = ElementLifecycle {
        created_in: existing,
        demolished_in: Some(demolition),
    };
    // The door has an explicit lifecycle; the other walls/window intentionally
    // inherit the legacy Existing default to verify effective phase preservation.
    for id in [bottom, door_id] {
        model.element_lifecycles.insert(id, lifecycle);
    }

    let tags = [
        OpeningTag::new(
            "core.opening_tag",
            OpeningTagParams {
                view: h.view,
                opening: door_id,
                position: Point2::new(0.0, -0.5),
                label_preset: OpeningTagLabelPreset::InstanceName,
            },
        ),
        OpeningTag::new(
            "core.opening_tag",
            OpeningTagParams {
                view: h.view,
                opening: window_id,
                position: Point2::new(0.0, 2.5),
                label_preset: OpeningTagLabelPreset::TypeAndDimensions,
            },
        ),
    ];
    let tag_ids = [tags[0].id(), tags[1].id()];
    for tag in tags {
        model.opening_tags.insert(tag.id(), tag);
    }

    let dimension = Dimension::new(
        "core.dimension",
        DimensionParams {
            layout: DimensionLayout::Aligned,
            view: h.view,
            first: DimensionReference::WallEndpoint {
                wall: bottom,
                endpoint: DimensionEndpoint::Start,
            },
            second: DimensionReference::WallEndpoint {
                wall: bottom,
                endpoint: DimensionEndpoint::End,
            },
            additional: Vec::new(),
            baseline_spacing_m: 0.25,
            offset_m: 0.4,
            orphan_hint: Point2::new(0.0, -0.4),
        },
    );
    let dimension_id = dimension.id();
    model.dimensions.insert(dimension_id, dimension);

    let segments = model
        .room_boundary_segments(model.walls[&bottom].parameters.level)
        .unwrap();
    let faces = derive_faces(&segments).unwrap();
    let face = faces.assign_seed(Point2::new(0.0, 1.0)).unwrap();
    let room = Room::new(
        "core.room",
        RoomParams {
            number: "101".into(),
            name: "Source office".into(),
            floor_finish: None,
            wall_finish: None,
            ceiling_finish: None,
            floor_material: None,
            wall_material: None,
            ceiling_material: None,
            level: model.walls[&bottom].parameters.level,
            seed: Point2::new(0.0, 1.0),
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
            position: Point2::new(0.0, 1.0),
        },
    );
    let room_tag_id = room_tag.id();
    model.room_tags.insert(room_tag_id, room_tag);

    let snap_wall = add_wall(
        &mut model,
        bottom,
        Point2::new(10.0, 3.0),
        Point2::new(11.0, 3.0),
    );
    model.wall_type_assignments.insert(
        snap_wall,
        WallTypeAssignment {
            type_id: wall_type_id,
            flipped: false,
        },
    );
    (
        model,
        Bay {
            walls,
            openings,
            joins,
            tags: tag_ids,
            dimension: dimension_id,
            room: room_id,
            room_tag: room_tag_id,
            snap_wall,
        },
    )
}

fn load(h: &mut Harness, model: Model, selection: &BTreeSet<Id>) {
    let geometry: Vec<_> = model
        .walls
        .keys()
        .chain(model.openings.keys())
        .copied()
        .collect();
    h.app.editor.document = Document::from_model(model).unwrap();
    h.app.editor.pending_geometry.extend(geometry);
    h.app.editor.regenerate().unwrap();
    h.app.plans.poll(&h.app.editor);
    h.app.focus_plan(Some(h.view));
    h.app.select(selection.first().copied());
    h.app.selected_ids = selection.clone();
    h.settle();
}

fn copied_walls(before: &Model, after: &Model) -> BTreeSet<Id> {
    after
        .walls
        .keys()
        .filter(|id| !before.walls.contains_key(id))
        .copied()
        .filter(|id| after.walls[id].header.type_id == os_walls::WALL_TYPE)
        .collect()
}

fn near(a: Point2, b: Point2) {
    assert!(a.distance(b) < 1e-8, "{a:?} != {b:?}");
}

fn near_opening_params(actual: &OpeningParams, expected: &OpeningParams) {
    let mut normalized = actual.clone();
    assert!((normalized.offset - expected.offset).abs() < 1e-9);
    normalized.offset = expected.offset;
    assert_eq!(&normalized, expected);
}

#[test]
fn copy_joined_wall_assembly_preview_commit_history_and_save_reopen_both_dpis() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        let (mut model, bay) = fixture(&mut h);
        let view = h.view;
        let mut settings = model.views[&view].parameters.plan.unwrap();
        settings.basis.rotation = 0.37;
        settings.basis.origin = Point2::new(0.0, 0.0);
        model.views.get_mut(&view).unwrap().parameters.plan = Some(settings);
        load(&mut h, model.clone(), &bay.walls);
        h.app.plans.cameras.get_mut(&view).unwrap().center = Point2::new(4.5, 1.5);
        h.app.plans.cameras.get_mut(&view).unwrap().pixels_per_metre = 34.0;
        h.app.plans.split = true;
        h.frame(vec![]);

        let before = h.app.editor.document.model().clone();
        let scene = h.app.editor.scene.clone();
        let history = h.app.editor.document.history_stats();
        let source_tags = [bay.tags[0], bay.tags[1]]
            .into_iter()
            .map(|id| (id, before.opening_tags[&id].parameters.clone()))
            .collect::<std::collections::BTreeMap<_, _>>();

        h.app.begin_wall_set_copy();
        assert!(h.app.plans.wall_set_move.as_ref().unwrap().is_copy());
        h.frame(vec![]);
        let base_world = Point2::new(2.0, 3.0);
        let base_screen = h.point(base_world);
        h.click(base_screen);
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

        let context = h.app.editor.native_plan_context(view).unwrap();
        let base_plane = context.basis.world_to_plane(base_world).unwrap();
        let destination_plane = Point2::new(base_plane.x + 6.0, base_plane.y);
        let destination_world = context.basis.plane_to_world(destination_plane).unwrap();
        let destination_screen = h.point(destination_world);
        {
            let draft = h.app.plans.wall_set_move.as_mut().unwrap();
            draft.gesture.length = "6".into();
            draft.gesture.angle_degrees = "0".into();
        }
        h.frame(vec![egui::Event::PointerMoved(destination_screen)]);
        let draft = h.app.plans.wall_set_move.as_ref().unwrap();
        let preview_a = draft.candidate(destination_plane).unwrap();
        let preview_b = draft.candidate(destination_plane).unwrap();
        assert_eq!(
            preview_a.model.walls.len(),
            before.walls.len() + bay.walls.len(),
            "candidate includes the complete selected set"
        );
        assert_eq!(
            preview_a.model, preview_b.model,
            "copy identities are stable across preview"
        );
        assert_eq!(h.app.editor.document.model(), &before);
        assert_eq!(h.app.editor.scene, scene);
        assert_eq!(h.app.editor.document.history_stats(), history);

        let (sin, cos) = context.basis.rotation.sin_cos();
        let delta = Point2::new(6.0 * cos, 6.0 * sin);
        h.click(destination_screen);
        h.settle();
        let after = h.app.editor.document.model().clone();
        let copies = copied_walls(&before, &after);
        assert_eq!(
            copies.len(),
            bay.walls.len(),
            "before={} after={} selection={:?} status={}",
            before.walls.len(),
            after.walls.len(),
            h.app.selected_ids,
            h.app.status
        );
        assert!(before.dimensions.contains_key(&bay.dimension));
        assert_eq!(
            after.wall_joins.len(),
            before.wall_joins.len() + bay.joins.len()
        );
        assert_eq!(
            after.openings.len(),
            before.openings.len() + bay.openings.len()
        );
        assert_eq!(
            after.opening_tags.len(),
            before.opening_tags.len() + bay.tags.len()
        );
        assert_eq!(
            after.rooms.len(),
            before.rooms.len(),
            "rooms are not duplicated"
        );
        assert_eq!(
            after.room_tags.len(),
            before.room_tags.len(),
            "room tags stay with source room"
        );
        assert_eq!(
            after.dimensions, before.dimensions,
            "source dimensions remain bound to source IDs"
        );

        let mut wall_map = std::collections::BTreeMap::new();
        for source in &bay.walls {
            let old = &before.walls[source];
            let target = copies
                .iter()
                .copied()
                .find(|id| {
                    let new = &after.walls[id].parameters;
                    new.start().distance(Point2::new(
                        old.parameters.start().x + delta.x,
                        old.parameters.start().y + delta.y,
                    )) < 1e-8
                        && new.end().distance(Point2::new(
                            old.parameters.end().x + delta.x,
                            old.parameters.end().y + delta.y,
                        )) < 1e-8
                })
                .expect("translated copy of every source wall");
            wall_map.insert(*source, target);
            let copy = &after.walls[&target];
            assert_ne!(copy.id(), old.id());
            assert_eq!(copy.header.type_id, old.header.type_id);
            assert_eq!(copy.header.properties, old.header.properties);
            assert_eq!(
                after.wall_type_assignments[&target],
                before.wall_type_assignments[source]
            );
            assert_eq!(
                after.element_lifecycle(target),
                before.element_lifecycle(*source)
            );
            assert!(h.app.editor.scene.contains_key(&target));
        }

        let copied_openings = after
            .openings
            .keys()
            .filter(|id| !before.openings.contains_key(id))
            .copied()
            .collect::<Vec<_>>();
        for source in bay.openings {
            let old = &before.openings[&source];
            let copy_id = copied_openings
                .iter()
                .copied()
                .find(|id| after.openings[id].parameters.name == old.parameters.name)
                .unwrap();
            let copy = &after.openings[&copy_id];
            let mut expected = old.parameters.clone();
            expected.host = wall_map[&old.parameters.host];
            near_opening_params(&copy.parameters, &expected);
            assert_eq!(
                after.opening_clearances[&copy_id],
                before.opening_clearances[&source]
            );
            assert_eq!(
                after.element_lifecycle(copy_id),
                before.element_lifecycle(source)
            );
            assert!(h.app.editor.scene.contains_key(&copy_id));
        }

        let copied_joins = after
            .wall_joins
            .iter()
            .filter(|(id, _)| !before.wall_joins.contains_key(id))
            .collect::<Vec<_>>();
        assert_eq!(copied_joins.len(), bay.joins.len());
        for (_, join) in copied_joins {
            assert!(
                join.parameters
                    .members()
                    .iter()
                    .all(|id| copies.contains(id))
            );
            if let WallJoinParams::Corner { owner, .. } = join.parameters {
                assert!(copies.contains(&owner));
            }
        }

        let copied_tags = after
            .opening_tags
            .iter()
            .filter(|(id, _)| !before.opening_tags.contains_key(id))
            .map(|(_, tag)| tag)
            .collect::<Vec<_>>();
        assert_eq!(copied_tags.len(), bay.tags.len());
        for tag in copied_tags {
            assert_eq!(tag.parameters.view, view);
            assert!(copied_openings.contains(&tag.parameters.opening));
            let source_tag = source_tags
                .values()
                .find(|parameters| parameters.label_preset == tag.parameters.label_preset)
                .unwrap();
            near(
                tag.parameters.position,
                Point2::new(
                    source_tag.position.x + delta.x,
                    source_tag.position.y + delta.y,
                ),
            );
        }
        assert_eq!(
            after.rooms[&bay.room].parameters,
            before.rooms[&bay.room].parameters
        );
        assert_eq!(
            after.room_tags[&bay.room_tag].parameters,
            before.room_tags[&bay.room_tag].parameters
        );
        assert_eq!(
            h.app.editor.document.history_stats().undo_entries,
            history.undo_entries + 1
        );
        assert_eq!(
            h.app.selected_ids, copies,
            "the new assembly becomes the active selection"
        );

        let committed = after.clone();
        let committed_scene = h.app.editor.scene.clone();
        h.app.history(false);
        h.settle();
        assert_eq!(h.app.editor.document.model(), &before);
        assert_eq!(h.app.editor.scene, scene);
        h.app.history(true);
        h.settle();
        assert_eq!(h.app.editor.document.model(), &committed);
        assert_eq!(h.app.editor.scene, committed_scene);

        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("copied-wall-bay.osb");
        h.app.editor.save(&path).unwrap();
        h.app.editor.open(&path).unwrap();
        assert_eq!(h.app.editor.document.model(), &committed);
        assert!(copies.iter().all(|id| h.app.editor.scene.contains_key(id)));
    }
}

#[test]
fn copy_wall_assembly_snaps_and_rejects_partial_join_overlap_escape_and_stale_selection() {
    let mut h = Harness::new(PROFILES[0].0, PROFILES[0].1);
    let (model, bay) = fixture(&mut h);
    load(&mut h, model.clone(), &bay.walls);
    assert!(
        h.app
            .editor
            .document
            .model()
            .walls
            .contains_key(&bay.snap_wall)
    );
    let before = h.app.editor.document.model().clone();
    let scene = h.app.editor.scene.clone();
    let history = h.app.editor.document.history_stats();

    h.app.selected_ids = [*bay.walls.first().unwrap()].into_iter().collect();
    assert!(
        Draft::begin_copy(&h.app).is_err(),
        "partial join group must be rejected"
    );
    h.app.selected_ids = bay.walls.clone();
    let mut draft = Draft::begin_copy(&h.app).unwrap();
    draft.gesture.start = Some(Point2::new(0.0, 0.0));
    assert!(
        draft.candidate(Point2::new(0.2, 0.0)).is_err(),
        "overlapping copy must fail preflight"
    );
    assert_eq!(h.app.editor.document.model(), &before);
    assert_eq!(h.app.editor.scene, scene);
    assert_eq!(h.app.editor.document.history_stats(), history);

    let context = h.app.editor.native_plan_context(h.view).unwrap();
    let camera = h.app.plans.cameras[&h.view];
    let rect = h.app.plans.canvas_rect.unwrap();
    let size = [f64::from(rect.width()), f64::from(rect.height())];
    let target_world = Point2::new(10.0, 3.0);
    let target_plane = context.basis.world_to_plane(target_world).unwrap();
    let pointer = camera.project(target_plane, size).unwrap();
    let snaps = SnapOptions {
        enabled: true,
        endpoints: true,
        intersections: false,
        perpendicular: false,
        midpoints: false,
        nearest: false,
        axis_extensions: false,
    };
    let snapped = draft
        .target(
            h.app.plans.drawing.as_ref().unwrap(),
            camera,
            size,
            pointer,
            snaps,
        )
        .unwrap();
    near(snapped, target_plane);

    h.app.plans.snaps.enabled = false;
    h.app.begin_wall_set_copy();
    h.frame(vec![]);
    h.click(h.point(Point2::new(0.0, 0.0)));
    h.click(h.point(Point2::new(0.2, 0.0)));
    assert!(
        h.app.plans.wall_set_move.is_none(),
        "invalid collision release cancels the draft"
    );
    assert!(
        h.app.status_error,
        "invalid copy release reports its rejection"
    );
    assert_eq!(h.app.editor.document.model(), &before);
    assert_eq!(h.app.editor.scene, scene);
    assert_eq!(h.app.editor.document.history_stats(), history);

    h.app.begin_wall_set_copy();
    h.frame(vec![]);
    let base = h.point(Point2::new(2.0, 3.0));
    h.click(base);
    h.frame(vec![egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: Some(egui::Key::Escape),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }]);
    assert!(h.app.plans.wall_set_move.is_none());
    assert_eq!(h.app.editor.document.model(), &before);
    assert_eq!(h.app.editor.scene, scene);
    assert_eq!(h.app.editor.document.history_stats(), history);

    h.app.begin_wall_set_copy();
    h.app.selected_ids = [*bay.walls.first().unwrap()].into_iter().collect();
    h.frame(vec![]);
    assert!(
        h.app.plans.wall_set_move.is_none(),
        "changed selection revokes copy draft"
    );
    assert_eq!(h.app.editor.document.model(), &before);
    assert_eq!(h.app.editor.scene, scene);
    assert_eq!(h.app.editor.document.history_stats(), history);

    h.app.selected_ids = bay.walls.clone();
    h.app.begin_wall_set_copy();
    h.frame(vec![]);
    let mut changed = h.app.editor.document.model().walls[&bay.snap_wall]
        .parameters
        .clone();
    *changed.path.straight_end_mut().unwrap() = Point2::new(12.0, 3.0);
    h.app
        .editor
        .command(
            "Edit during copy draft",
            Command::UpdateWall {
                id: bay.snap_wall,
                parameters: changed,
            },
        )
        .unwrap();
    let edited_model = h.app.editor.document.model().clone();
    h.frame(vec![]);
    assert!(
        h.app.plans.wall_set_move.is_none(),
        "a document revision revokes the copy draft"
    );
    assert_eq!(h.app.editor.document.model(), &edited_model);
    assert_eq!(
        h.app.editor.document.history_stats().undo_entries,
        history.undo_entries + 1
    );
}
