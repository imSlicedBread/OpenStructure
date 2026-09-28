use os_core::{Id, Point2};
use os_document::{Command, Document};
use os_model::*;

fn setup_model() -> (Model, OpeningTag) {
    let mut model = Model::new("Opening tags");
    let level = *model.levels.keys().next().unwrap();
    let wall = Wall::new(
        "org.openstructure.walls.wall",
        WallParams {
            name: "Host".into(),
            path: os_model::WallPath::Straight {
                start: Point2::new(0., 0.),
                end: Point2::new(8., 0.),
            },
            thickness: 0.2,
            height: 3.,
            level,
            material: None,
        },
    );
    let ty = OpeningType::new(
        "core.opening_type",
        OpeningTypeParams {
            window_operation: Default::default(),
            name: "D900".into(),
            family: OpeningFamily::default(),
            kind: OpeningKind::Door,
            width: 0.9,
            height: 2.1,
            sill: 0.,
            pane_position: WindowPanePosition::Center,
        },
    );
    let opening = Opening::new(
        "core.opening",
        OpeningParams {
            name: "D-01".into(),
            host: wall.id(),
            offset: 1.,
            definition: OpeningDefinition::Typed { type_id: ty.id() },
            width_override: None,
            height_override: None,
            sill_override: None,
            pane_position_override: None,
            lite_side_override: None,
            hinge: DoorHinge::Start,
            swing: DoorSwing::Left,
        },
    );
    let view = View::new("core.view", ViewParams::floor_plan("Plan", level));
    let tag = OpeningTag::new(
        "core.opening_tag",
        OpeningTagParams {
            label_preset: Default::default(),
            view: view.id(),
            opening: opening.id(),
            position: Point2::new(2., 1.5),
        },
    );
    model.walls.insert(wall.id(), wall);
    model.opening_types.insert(ty.id(), ty);
    model.openings.insert(opening.id(), opening);
    model.views.insert(view.id(), view);
    (model, tag)
}
fn setup() -> (Document, OpeningTag) {
    let (model, tag) = setup_model();
    (Document::from_model(model).unwrap(), tag)
}

fn assert_atomic_rejection(doc: &mut Document, commands: Vec<Command>) {
    doc.drain_events();
    let model = doc.model().clone();
    let revision = doc.revision();
    let history = doc.history_stats();
    assert!(doc.execute("Rejected batch", commands).is_err());
    assert_eq!(doc.model(), &model);
    assert_eq!(doc.revision(), revision);
    assert_eq!(doc.history_stats(), history);
    assert!(doc.drain_events().is_empty());
}

#[test]
fn opening_tag_commands_are_atomic_invalidate_view_and_preserve_identity_in_history() {
    let (mut doc, tag) = setup();
    let mut invalid = tag.clone();
    invalid.parameters.opening = Id::new();
    assert_atomic_rejection(
        &mut doc,
        vec![
            Command::RenameProject("Rejected".into()),
            Command::AddOpeningTag(invalid),
        ],
    );
    let before = doc.model().clone();
    doc.execute("Place", vec![Command::AddOpeningTag(tag.clone())])
        .unwrap();
    assert_eq!(doc.history_stats().undo_entries, 1);
    let placed = doc.model().clone();
    let event = doc.drain_events().pop().unwrap();
    assert!(event.invalidated.contains(&tag.id()));
    assert!(event.invalidated.contains(&tag.parameters.view));
    assert!(doc.undo());
    assert_eq!(doc.model(), &before);
    assert!(doc.redo());
    assert_eq!(doc.model(), &placed);
    for duplicate in [
        tag.clone(),
        OpeningTag::new("core.opening_tag", tag.parameters.clone()),
    ] {
        assert_atomic_rejection(
            &mut doc,
            vec![
                Command::RenameProject("Rejected".into()),
                Command::AddOpeningTag(duplicate),
            ],
        );
    }
    let mut parameters = tag.parameters.clone();
    parameters.position.x = f64::INFINITY;
    assert_atomic_rejection(
        &mut doc,
        vec![
            Command::RenameProject("Rejected".into()),
            Command::UpdateOpeningTag {
                id: tag.id(),
                parameters,
            },
        ],
    );
    assert_atomic_rejection(
        &mut doc,
        vec![
            Command::RemoveOpeningTag(tag.id()),
            Command::RemoveOpeningTag(tag.id()),
        ],
    );
    let mut parameters = tag.parameters.clone();
    parameters.position = Point2::new(3., 4.);
    doc.execute(
        "Move",
        vec![Command::UpdateOpeningTag {
            id: tag.id(),
            parameters,
        }],
    )
    .unwrap();
    let moved = doc.model().clone();
    assert_eq!(moved.opening_tags[&tag.id()].header, tag.header);
    assert_eq!(doc.history_stats().undo_entries, 2);
    assert!(
        doc.drain_events()[0]
            .invalidated
            .contains(&tag.parameters.view)
    );
    assert!(doc.undo());
    assert_eq!(doc.model(), &placed);
    assert!(doc.redo());
    assert_eq!(doc.model(), &moved);
    doc.drain_events();
    doc.execute("Delete", vec![Command::RemoveOpeningTag(tag.id())])
        .unwrap();
    assert!(doc.model().opening_tags.is_empty());
    assert!(
        doc.drain_events()[0]
            .invalidated
            .contains(&tag.parameters.view)
    );
    assert!(doc.undo());
    assert_eq!(doc.model(), &moved);
    assert!(doc.redo());
    assert!(doc.model().opening_tags.is_empty());
}

#[test]
fn opening_tag_room_edits_invalidate_and_room_deletion_retains_orphan_view_deletion_is_batched() {
    let (mut doc, tag) = setup();
    doc.execute("Place", vec![Command::AddOpeningTag(tag.clone())])
        .unwrap();
    let mut parameters = doc.model().openings[&tag.parameters.opening]
        .parameters
        .clone();
    parameters.name = "Study".into();
    doc.drain_events();
    doc.execute(
        "Rename room",
        vec![Command::UpdateOpening {
            id: tag.parameters.opening,
            parameters,
        }],
    )
    .unwrap();
    let event = doc.drain_events().pop().unwrap();
    assert!(event.invalidated.contains(&tag.id()));
    assert!(event.invalidated.contains(&tag.parameters.view));
    assert_eq!(doc.model().opening_tags[&tag.id()], tag);
    let before = doc.model().clone();
    doc.execute(
        "Delete room",
        vec![Command::RemoveOpening(tag.parameters.opening)],
    )
    .unwrap();
    assert_eq!(doc.model().opening_tags[&tag.id()], tag);
    assert_eq!(
        tag.parameters.resolve(doc.model()).unwrap_err(),
        "Missing opening"
    );
    assert!(doc.drain_events()[0].invalidated.contains(&tag.id()));
    assert!(doc.undo());
    assert_eq!(doc.model(), &before);
    assert!(doc.redo());
    assert!(tag.parameters.resolve(doc.model()).is_err());
    assert_atomic_rejection(&mut doc, vec![Command::RemoveView(tag.parameters.view)]);
    let orphan = doc.model().clone();
    doc.execute(
        "Delete view with tags",
        vec![
            Command::RemoveOpeningTag(tag.id()),
            Command::RemoveView(tag.parameters.view),
        ],
    )
    .unwrap();
    assert!(!doc.model().views.contains_key(&tag.parameters.view));
    assert!(doc.model().opening_tags.is_empty());
    assert!(doc.undo());
    assert_eq!(doc.model(), &orphan);
    assert!(doc.redo());
    assert!(!doc.model().views.contains_key(&tag.parameters.view));
}
