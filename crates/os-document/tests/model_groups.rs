use os_core::{Id, Point2};
use os_document::{Command, Document};
use os_model::*;
use std::collections::BTreeSet;

fn fixture() -> (Document, Id, Id) {
    let mut model = Model::new("Groups");
    let level = *model.levels.keys().next().unwrap();
    let view = View::new("core.view", ViewParams::floor_plan("Plan", level));
    let view_id = view.id();
    model.views.insert(view_id, view);
    let wall = Wall::new(
        "org.openstructure.walls.wall",
        WallParams {
            name: "Host".into(),
            path: WallPath::Straight {
                start: Point2::new(0., 0.),
                end: Point2::new(8., 0.),
            },
            thickness: 0.2,
            height: 3.,
            level,
            material: None,
        },
    );
    let wall_id = wall.id();
    let ty = WallType::new(
        "core.wall_type",
        WallTypeParams {
            name: "Structure".into(),
            layers: vec![WallLayer {
                id: Id::new(),
                name: "Core".into(),
                thickness: 0.2,
                function: LayerFunction::Structure,
                material: None,
            }],
        },
    );
    model.wall_type_assignments.insert(
        wall_id,
        WallTypeAssignment {
            type_id: ty.id(),
            flipped: false,
        },
    );
    model.wall_types.insert(ty.id(), ty);
    let mut commands = vec![Command::AddWall(wall)];
    for (kind, offset, sill) in [(OpeningKind::Door, 1., 0.), (OpeningKind::Window, 4., 0.8)] {
        let ty = OpeningType::new(
            "core.opening_type",
            OpeningTypeParams {
                name: format!("{kind:?}"),
                kind,
                width: 1.,
                height: 2.,
                sill,
                pane_position: Default::default(),
                window_operation: Default::default(),
                family: Default::default(),
            },
        );
        let opening = Opening::new(
            "core.opening",
            OpeningParams {
                name: format!("{kind:?}"),
                host: wall_id,
                offset,
                definition: OpeningDefinition::Typed { type_id: ty.id() },
                open_state: Default::default(),
                width_override: None,
                height_override: None,
                sill_override: None,
                pane_position_override: None,
                lite_side_override: None,
                hinge: Default::default(),
                swing: Default::default(),
            },
        );
        let id = opening.id();
        commands.extend([
            Command::AddOpeningType(ty),
            Command::AddOpening(opening),
            Command::SetOpeningClearance {
                id,
                clearance: Some(OpeningClearance {
                    end: ClearanceEnd::Start,
                    distance: offset,
                }),
            },
            Command::AddOpeningTag(OpeningTag::new(
                "core.opening_tag",
                OpeningTagParams {
                    view: view_id,
                    opening: id,
                    position: Point2::new(offset, 1.),
                    label_preset: Default::default(),
                },
            )),
        ]);
    }
    // Install the host before constructing a validated document with its assignment.
    if let Command::AddWall(wall) = commands.remove(0) {
        model.walls.insert(wall_id, wall);
    }
    let mut doc = Document::from_model(model).unwrap();
    doc.execute("Openings", commands).unwrap();
    doc.drain_events();
    (doc, wall_id, view_id)
}

fn create(doc: &mut Document, wall: Id, view: Id) -> Id {
    doc.execute(
        "Group",
        vec![Command::CreateWallGroup {
            walls: BTreeSet::from([wall]),
            view,
            name: "Assembly".into(),
        }],
    )
    .unwrap();
    *doc.model().group_instances.keys().next().unwrap()
}

fn duplicate(doc: &mut Document, id: Id) -> Id {
    let before: BTreeSet<_> = doc.model().group_instances.keys().copied().collect();
    doc.execute(
        "Duplicate",
        vec![Command::DuplicateGroupInstance {
            id,
            translation: Point2::new(0., 10.),
        }],
    )
    .unwrap();
    *doc.model()
        .group_instances
        .keys()
        .find(|id| !before.contains(id))
        .unwrap()
}

#[test]
fn prepared_group_duplicate_preview_and_execute_keep_identical_uuids() {
    let (mut doc, wall, view) = fixture();
    let source = create(&mut doc, wall, view);
    let before = doc.model().clone();
    let history = doc.history_stats();
    let revision = doc.revision();
    let instance = Id::new();
    let members = before.group_instances[&source]
        .parameters
        .members
        .keys()
        .map(|slot| (*slot, Id::new()))
        .collect();
    let command = Command::DuplicateGroupInstancePrepared {
        id: source,
        translation: Point2::new(0., 10.),
        instance,
        members,
    };
    let first = doc.preview_commands(vec![command.clone()]).unwrap();
    assert_eq!(first, doc.preview_commands(vec![command.clone()]).unwrap());
    assert_eq!(doc.model(), &before);
    assert_eq!(doc.history_stats(), history);
    assert_eq!(doc.revision(), revision);
    doc.execute("Prepared duplicate", vec![command.clone()])
        .unwrap();
    assert_eq!(doc.model(), &first);
    assert_eq!(doc.history_stats().undo_entries, history.undo_entries + 1);
    assert_eq!(
        first.group_instances[&instance].parameters.members.len(),
        before.group_instances[&source].parameters.members.len()
    );
    // Replaying already-used identities must fail atomically.
    let history = doc.history_stats();
    assert!(doc.execute("Collision", vec![command]).is_err());
    assert_eq!(doc.model(), &first);
    assert_eq!(doc.history_stats(), history);
    round_trip(&mut doc, &before);
}

fn round_trip(doc: &mut Document, before: &Model) {
    let after = doc.model().clone();
    assert!(doc.undo());
    assert_eq!(doc.model(), before);
    assert!(doc.redo());
    assert_eq!(doc.model(), &after);
}

#[test]
fn capture_duplicate_transform_and_history_preserve_canonical_members() {
    let (mut doc, wall, view) = fixture();
    let before = doc.model().clone();
    let original = create(&mut doc, wall, view);
    round_trip(&mut doc, &before);
    assert_eq!(doc.model().walls, before.walls);
    assert_eq!(doc.model().openings, before.openings);
    let instance = doc.model().group_instances[&original].clone();
    assert_eq!(instance.parameters.members.len(), 5);
    let event = doc.drain_events().pop().unwrap();
    assert!(event.changed.contains(&original));
    assert!(event.changed.contains(&instance.parameters.definition));
    assert!(
        instance
            .parameters
            .members
            .values()
            .all(|id| event.changed.contains(id))
    );
    let before = doc.model().clone();
    let copy = duplicate(&mut doc, original);
    round_trip(&mut doc, &before);
    let copied = doc.model().group_instances[&copy].clone();
    assert_eq!(copied.parameters.definition, instance.parameters.definition);
    assert!(
        copied.parameters.members.values().all(|id| !instance
            .parameters
            .members
            .values()
            .any(|old| old == id))
    );
    let definition = &doc.model().group_definitions[&instance.parameters.definition];
    let wall_slot = *definition.parameters.template.walls.keys().next().unwrap();
    let copy_wall = copied.parameters.members[&wall_slot];
    assert_eq!(
        doc.model().walls[&copy_wall].parameters.path,
        WallPath::Straight {
            start: Point2::new(0., 10.),
            end: Point2::new(8., 10.)
        }
    );
    for slot in definition.parameters.template.openings.keys() {
        assert_eq!(
            doc.model().openings[&copied.parameters.members[slot]]
                .parameters
                .host,
            copy_wall
        );
    }
    let lifecycles = doc.model().element_lifecycles.clone();
    let before = doc.model().clone();
    let transform = GroupTransform {
        pivot: Point2::new(2., 1.),
        rotation_radians: 0.7,
        translation: Point2::new(20., 10.),
    };
    doc.execute(
        "Transform",
        vec![Command::SetGroupInstanceTransform {
            id: copy,
            transform,
        }],
    )
    .unwrap();
    round_trip(&mut doc, &before);
    assert_eq!(
        doc.model().group_instances[&copy].parameters.members,
        copied.parameters.members
    );
    assert_eq!(doc.model().element_lifecycles, lifecycles);
    assert_eq!(
        doc.model().walls[&copy_wall].parameters.path,
        WallPath::Straight {
            start: transform.apply(Point2::new(0., 0.)).unwrap(),
            end: transform.apply(Point2::new(8., 0.)).unwrap()
        }
    );
    // Setting an absolute transform twice is an exact no-op, with no accumulated drift.
    let revision = doc.revision();
    doc.execute(
        "Same transform",
        vec![Command::SetGroupInstanceTransform {
            id: copy,
            transform,
        }],
    )
    .unwrap();
    assert_eq!(doc.revision(), revision);
}

#[test]
fn shared_update_synchronizes_optional_metadata_and_preserves_lifecycles() {
    let (mut doc, wall, view) = fixture();
    let id = create(&mut doc, wall, view);
    let copy = duplicate(&mut doc, id);
    let definition = doc.model().group_instances[&id].parameters.definition;
    let mut parameters = doc.model().group_definitions[&definition]
        .parameters
        .clone();
    for wall in parameters.template.walls.values_mut() {
        wall.parameters.height = 4.;
    }
    parameters.template.wall_type_assignments.clear();
    parameters.template.opening_clearances.clear();
    let before = doc.model().clone();
    doc.drain_events();
    doc.execute(
        "Shared edit",
        vec![Command::UpdateGroupDefinition {
            id: definition,
            parameters,
        }],
    )
    .unwrap();
    assert!(
        doc.model()
            .walls
            .values()
            .all(|wall| wall.parameters.height == 4.)
    );
    assert!(doc.model().wall_type_assignments.is_empty());
    assert!(doc.model().opening_clearances.is_empty());
    assert_eq!(doc.model().element_lifecycles, before.element_lifecycles);
    let event = doc.drain_events().pop().unwrap();
    for instance in doc.model().group_instances.values() {
        assert!(event.changed.contains(&instance.id()));
        assert!(
            instance
                .parameters
                .members
                .values()
                .all(|member| event.changed.contains(member))
        );
    }
    assert!(event.changed.contains(&definition) && event.changed.contains(&copy));
    round_trip(&mut doc, &before);
    // Restore metadata through the same shared command, including expected absence -> presence.
    doc.execute(
        "Restore metadata",
        vec![Command::UpdateGroupDefinition {
            id: definition,
            parameters: before.group_definitions[&definition].parameters.clone(),
        }],
    )
    .unwrap();
    assert_eq!(doc.model(), &before);
}

#[test]
fn unique_isolates_updates_and_ungroup_keeps_members() {
    let (mut doc, wall, view) = fixture();
    let id = create(&mut doc, wall, view);
    let copy = duplicate(&mut doc, id);
    let before = doc.model().clone();
    doc.execute(
        "Unique",
        vec![Command::MakeGroupInstanceUnique {
            id: copy,
            name: "Unique".into(),
        }],
    )
    .unwrap();
    round_trip(&mut doc, &before);
    let original_definition = doc.model().group_instances[&id].parameters.definition;
    let unique_definition = doc.model().group_instances[&copy].parameters.definition;
    assert_ne!(original_definition, unique_definition);
    assert_eq!(doc.model().walls, before.walls);
    assert_eq!(doc.model().openings, before.openings);
    let mut parameters = doc.model().group_definitions[&original_definition]
        .parameters
        .clone();
    for wall in parameters.template.walls.values_mut() {
        wall.parameters.height = 4.;
    }
    doc.execute(
        "Original edit",
        vec![Command::UpdateGroupDefinition {
            id: original_definition,
            parameters,
        }],
    )
    .unwrap();
    for member in doc.model().group_instances[&copy]
        .parameters
        .members
        .values()
    {
        if let Some(wall) = doc.model().walls.get(member) {
            assert_eq!(wall.parameters.height, 3.);
        }
    }
    for instance in [copy, id] {
        let before = doc.model().clone();
        let definition = before.group_instances[&instance].parameters.definition;
        doc.execute("Ungroup", vec![Command::UngroupInstance(instance)])
            .unwrap();
        assert!(!doc.model().group_definitions.contains_key(&definition));
        assert_eq!(doc.model().walls, before.walls);
        assert_eq!(doc.model().openings, before.openings);
        assert_eq!(doc.model().opening_tags, before.opening_tags);
        assert_eq!(doc.model().element_lifecycles, before.element_lifecycles);
        round_trip(&mut doc, &before);
    }
    assert!(doc.model().group_instances.is_empty());
}

#[test]
fn invalid_commands_roll_back_model_history_events_and_revision() {
    let (mut doc, wall, view) = fixture();
    let id = create(&mut doc, wall, view);
    duplicate(&mut doc, id);
    let definition = doc.model().group_instances[&id].parameters.definition;
    let mut unstable = doc.model().group_definitions[&definition]
        .parameters
        .clone();
    unstable.template.opening_tags.clear();
    let mut invalid = doc.model().group_definitions[&definition]
        .parameters
        .clone();
    invalid
        .template
        .walls
        .values_mut()
        .next()
        .unwrap()
        .parameters
        .height = -1.;
    let mut member_edit = doc.model().walls[&wall].parameters.clone();
    member_edit.height = 9.;
    for command in [
        Command::UpdateGroupDefinition {
            id: definition,
            parameters: unstable,
        },
        Command::UpdateGroupDefinition {
            id: definition,
            parameters: invalid,
        },
        Command::SetGroupInstanceTransform {
            id,
            transform: GroupTransform {
                translation: Point2::new(f64::INFINITY, 0.),
                ..Default::default()
            },
        },
        Command::DuplicateGroupInstance {
            id,
            translation: Point2::new(2e6, 0.),
        },
        Command::CreateWallGroup {
            walls: BTreeSet::from([wall]),
            view,
            name: "Duplicate ownership".into(),
        },
        Command::MakeGroupInstanceUnique {
            id,
            name: " ".into(),
        },
        Command::UngroupInstance(Id::new()),
        Command::UpdateWall {
            id: wall,
            parameters: member_edit,
        },
    ] {
        doc.drain_events();
        let before = doc.model().clone();
        let history = doc.history_stats();
        let revision = doc.revision();
        assert!(
            doc.execute(
                "Invalid batch",
                vec![Command::RenameProject("Must roll back".into()), command]
            )
            .is_err()
        );
        assert_eq!(doc.model(), &before);
        assert_eq!(doc.history_stats(), history);
        assert_eq!(doc.revision(), revision);
        assert!(doc.drain_events().is_empty());
    }
}

#[test]
fn ungroup_shared_instance_retains_definition_and_unique_single_removes_orphan() {
    let (mut doc, wall, view) = fixture();
    let id = create(&mut doc, wall, view);
    let copy = duplicate(&mut doc, id);
    let old = doc.model().group_instances[&id].parameters.definition;
    doc.execute("Ungroup copy", vec![Command::UngroupInstance(copy)])
        .unwrap();
    assert!(doc.model().group_definitions.contains_key(&old));
    let before = doc.model().clone();
    doc.execute(
        "Unique single",
        vec![Command::MakeGroupInstanceUnique {
            id,
            name: "Single".into(),
        }],
    )
    .unwrap();
    assert!(!doc.model().group_definitions.contains_key(&old));
    assert_eq!(doc.model().group_definitions.len(), 1);
    round_trip(&mut doc, &before);
}

#[test]
fn complete_join_membership_is_required_and_duplicate_remaps_join_targets() {
    let (mut doc, wall, view) = fixture();
    let mut parameters = doc.model().walls[&wall].parameters.clone();
    parameters.path = WallPath::Straight {
        start: Point2::new(8., 0.),
        end: Point2::new(12., 0.),
    };
    let second = Wall::new("org.openstructure.walls.wall", parameters);
    let second_id = second.id();
    let join = WallJoin::new(
        "core.wall_join",
        WallJoinParams::Butt {
            a: WallAnchor {
                wall,
                endpoint: WallEndpoint::End,
            },
            b: WallAnchor {
                wall: second_id,
                endpoint: WallEndpoint::Start,
            },
        },
    );
    doc.execute(
        "Join",
        vec![Command::AddWall(second), Command::AddWallJoin(join)],
    )
    .unwrap();
    let before = doc.model().clone();
    assert!(
        doc.execute(
            "Incomplete group",
            vec![Command::CreateWallGroup {
                walls: BTreeSet::from([wall]),
                view,
                name: "Incomplete".into(),
            }]
        )
        .is_err()
    );
    assert_eq!(doc.model(), &before);
    doc.execute(
        "Complete group",
        vec![Command::CreateWallGroup {
            walls: BTreeSet::from([wall, second_id]),
            view,
            name: "Joined".into(),
        }],
    )
    .unwrap();
    let id = *doc.model().group_instances.keys().next().unwrap();
    let copy = duplicate(&mut doc, id);
    let instance = &doc.model().group_instances[&copy];
    let definition = &doc.model().group_definitions[&instance.parameters.definition];
    let slot = *definition.parameters.template.joins.keys().next().unwrap();
    let join = &doc.model().wall_joins[&instance.parameters.members[&slot]];
    assert!(join.parameters.members().iter().all(|id| {
        *id != wall
            && *id != second_id
            && instance
                .parameters
                .members
                .values()
                .any(|member| member == id)
    }));
    let before = doc.model().clone();
    doc.execute(
        "Rotate joined group",
        vec![Command::SetGroupInstanceTransform {
            id: copy,
            transform: GroupTransform {
                rotation_radians: 0.5,
                translation: Point2::new(20., 20.),
                ..Default::default()
            },
        }],
    )
    .unwrap();
    round_trip(&mut doc, &before);
}
