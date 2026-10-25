use os_core::Point2;
use os_document::{Command, Document};
use os_model::*;

#[test]
fn paired_leaf_type_transaction_preflights_unshown_pins_and_preserves_history() {
    let mut doc = Document::new("Extrusions").unwrap();
    let level = *doc.model().levels.keys().next().unwrap();
    let wall = Wall::new(
        "org.openstructure.walls.wall",
        WallParams {
            name: "Host".into(),
            path: WallPath::Straight {
                start: Point2::new(0., 0.),
                end: Point2::new(8., 0.),
            },
            height: 3.,
            thickness: 0.2,
            level,
            material: None,
        },
    );
    let host = wall.id();
    let ty = OpeningType::new(
        "core.opening_type",
        OpeningTypeParams {
            name: "Door".into(),
            kind: OpeningKind::Door,
            width: 1.,
            height: 2.,
            sill: 0.,
            pane_position: WindowPanePosition::Center,
            window_operation: WindowOperation::Fixed,
            family: OpeningFamily {
                door_leaves: DoorLeaves::Paired {
                    active_fraction: 0.63,
                },
                ..Default::default()
            },
        },
    );
    let id = ty.id();
    doc.execute(
        "Setup",
        vec![Command::AddWall(wall), Command::AddOpeningType(ty)],
    )
    .unwrap();
    for offset in [1., 4.] {
        let opening = Opening::new(
            "core.opening",
            OpeningParams {
                name: "Instance".into(),
                host,
                offset,
                definition: OpeningDefinition::Typed { type_id: id },
                hinge: DoorHinge::Start,
                swing: DoorSwing::Left,
                open_state: OpeningState::DoorPairAngles {
                    active_degrees: 30.,
                    inactive_degrees: 90.,
                },
                width_override: Some(if offset == 4. { 0.005 } else { 1. }),
                height_override: None,
                sill_override: None,
                pane_position_override: None,
                lite_side_override: None,
            },
        );
        doc.execute("Place", vec![Command::AddOpening(opening)])
            .unwrap();
    }
    let before = doc.model().clone();
    let history = doc.history_stats();
    let mut parameters = before.opening_types[&id].parameters.clone();
    parameters.family.extrusions.push(OpeningExtrusion {
        bindings: Default::default(),
        attachment: OpeningAttachment::InactiveLeaf,
        ..OpeningExtrusion::new("Handle")
    });
    // This narrow overridden placement need not be visible in any open view.
    assert!(
        doc.execute(
            "Add solid",
            vec![Command::UpdateOpeningType {
                id,
                parameters: parameters.clone()
            }]
        )
        .is_err()
    );
    assert_eq!(doc.model(), &before);
    assert_eq!(doc.history_stats(), history);
    parameters.family.extrusions[0].profile = OpeningFamily::rectangle();
    parameters.family.extrusions[0].material = Some(os_core::Id::new());
    assert!(
        doc.execute(
            "Missing material",
            vec![Command::UpdateOpeningType {
                id,
                parameters: parameters.clone()
            }]
        )
        .is_err()
    );
    parameters.family.extrusions[0].material = None;
    doc.execute(
        "Add solid",
        vec![Command::UpdateOpeningType { id, parameters }],
    )
    .unwrap();
    let after = doc.model().clone();
    assert_eq!(after.openings, before.openings);
    assert_eq!(after.walls, before.walls);
    assert_eq!(after.opening_clearances, before.opening_clearances);
    assert_eq!(doc.history_stats().undo_entries, history.undo_entries + 1);
    assert!(doc.undo());
    assert_eq!(doc.model(), &before);
    assert!(doc.redo());
    assert_eq!(doc.model(), &after);
}
