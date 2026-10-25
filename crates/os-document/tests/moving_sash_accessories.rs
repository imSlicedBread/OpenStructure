use os_core::{Id, Point2};
use os_document::{Command, Document};
use os_model::*;

fn parameters(operation: WindowOperation) -> OpeningTypeParams {
    OpeningTypeParams {
        name: "Operable window".into(),
        kind: OpeningKind::Window,
        width: 1.2,
        height: 1.4,
        sill: 0.7,
        pane_position: WindowPanePosition::Center,
        window_operation: operation,
        family: OpeningFamily::default(),
    }
}

fn moving_solid() -> OpeningExtrusion {
    let mut solid = OpeningExtrusion::new("Latch");
    solid.attachment = OpeningAttachment::MovingSash;
    solid
}

#[test]
fn moving_sash_attachment_operation_profile_and_identity_contract() {
    for operation in [WindowOperation::Casement, WindowOperation::Sliding] {
        let mut p = parameters(operation);
        p.family.frame_width = 0.04;
        p.family.side_lite = Some(SideLite {
            side: LiteSide::Start,
            width_fraction: 0.2,
            mullion_width: 0.03,
            material: None,
        });
        p.family.extrusions.push(moving_solid());
        p.validate().unwrap();
        let value = serde_json::to_value(&p).unwrap();
        assert_eq!(value["family"]["version"], 10);
        assert_eq!(value["family"]["extrusions"][0]["attachment"], "MovingSash");
        assert_eq!(
            serde_json::from_value::<OpeningTypeParams>(value).unwrap(),
            p
        );
        for attachment in [
            OpeningAttachment::MovingSash,
            OpeningAttachment::OpeningFixed,
        ] {
            let mut valid = p.clone();
            valid.family.extrusions[0].attachment = attachment;
            valid.validate().unwrap();
        }
        let mut invalid = p.clone();
        invalid.family.extrusions[0].attachment = OpeningAttachment::PrimaryPanel;
        assert!(invalid.validate().is_err());
        invalid = p.clone();
        invalid.window_operation = WindowOperation::Fixed;
        assert!(invalid.validate().is_err());
        invalid = p.clone();
        invalid.kind = OpeningKind::Door;
        invalid.sill = 0.;
        invalid.window_operation = WindowOperation::Fixed;
        assert!(invalid.validate().is_err());
        invalid = p.clone();
        invalid.family.side_lite = None;
        invalid.family.frame_width = 0.;
        invalid.family.profile[2].y = 0.8;
        assert!(invalid.validate().is_err());
        invalid = p.clone();
        invalid.family.side_lite = None;
        invalid.family.frame_width = 0.;
        invalid.family.host_cut = OpeningHostCut::Profile;
        invalid.family.cut_profile[2].y = 0.8;
        assert!(invalid.validate().is_err());
        invalid = p.clone();
        invalid
            .family
            .extrusions
            .push(invalid.family.extrusions[0].clone());
        assert!(invalid.validate().is_err());
        invalid = p.clone();
        invalid.family.extrusions[0].position.x = 0.5;
        assert!(invalid.validate().is_err());
    }
    for kind in [OpeningKind::Door, OpeningKind::Window] {
        let mut p = parameters(WindowOperation::Fixed);
        p.kind = kind;
        if kind == OpeningKind::Door {
            p.sill = 0.;
        }
        p.family
            .extrusions
            .push(OpeningExtrusion::new("Historical panel latch"));
        p.validate().unwrap();
        p.family.extrusions[0].attachment = OpeningAttachment::OpeningFixed;
        p.validate().unwrap();
    }
}

#[test]
fn moving_sash_type_wide_update_rejects_unshown_narrow_sash_and_missing_material_atomically() {
    let mut doc = Document::new("Moving sash accessories").unwrap();
    let level = *doc.model().levels.keys().next().unwrap();
    let wall = Wall::new(
        "org.openstructure.walls.wall",
        WallParams {
            name: "Host".into(),
            path: WallPath::Straight {
                start: Point2::new(0., 0.),
                end: Point2::new(8., 0.),
            },
            height: 4.,
            thickness: 0.3,
            level,
            material: None,
        },
    );
    let host = wall.id();
    let ty = OpeningType::new("core.opening_type", parameters(WindowOperation::Sliding));
    let tid = ty.id();
    doc.execute(
        "Setup",
        vec![Command::AddWall(wall), Command::AddOpeningType(ty)],
    )
    .unwrap();
    let mut ids = Vec::new();
    for (offset, width) in [(1., 1.2), (4., 0.01)] {
        let opening = Opening::new(
            "core.opening",
            OpeningParams {
                name: "Instance".into(),
                host,
                offset,
                definition: OpeningDefinition::Typed { type_id: tid },
                open_state: OpeningState::SlidingFraction(0.5),
                width_override: Some(width),
                height_override: None,
                sill_override: Some(0.8),
                pane_position_override: Some(WindowPanePosition::RightFace),
                lite_side_override: None,
                hinge: DoorHinge::Start,
                swing: DoorSwing::Left,
            },
        );
        ids.push(opening.id());
        doc.execute("Place", vec![Command::AddOpening(opening)])
            .unwrap();
    }
    let before = doc.model().clone();
    let history = doc.history_stats();
    let mut p = before.opening_types[&tid].parameters.clone();
    p.family.extrusions.push(moving_solid());
    p.validate().unwrap(); // Type defaults fit; only the unshown overridden sash fails.
    assert!(
        doc.execute(
            "Add latch",
            vec![Command::UpdateOpeningType {
                id: tid,
                parameters: p.clone(),
            }]
        )
        .is_err()
    );
    assert_eq!(doc.model(), &before);
    assert_eq!(doc.history_stats(), history);
    // Widen the profile so its edges meet the 1 mm instance minimum without capping depth.
    p.family.extrusions[0].profile = OpeningFamily::rectangle();
    p.family.extrusions[0].material = Some(Id::new());
    assert!(
        doc.execute(
            "Missing material",
            vec![Command::UpdateOpeningType {
                id: tid,
                parameters: p.clone(),
            }]
        )
        .is_err()
    );
    assert_eq!(doc.model(), &before);
    assert_eq!(doc.history_stats(), history);
    p.family.extrusions[0].material = None;
    doc.execute(
        "Add latch",
        vec![Command::UpdateOpeningType {
            id: tid,
            parameters: p,
        }],
    )
    .unwrap();
    let after = doc.model().clone();
    assert_eq!(after.openings, before.openings);
    assert_eq!(after.walls, before.walls);
    assert_eq!(doc.history_stats().undo_entries, history.undo_entries + 1);
    for id in ids {
        assert_eq!(after.openings[&id].id(), id);
    }
    assert!(doc.undo());
    assert_eq!(doc.model(), &before);
    assert!(doc.redo());
    assert_eq!(doc.model(), &after);
}

#[test]
fn moving_sash_accessory_type_rejects_curved_host_update_without_history_change() {
    let mut doc = Document::new("Curved host").unwrap();
    let level = *doc.model().levels.keys().next().unwrap();
    let wall = Wall::new(
        "org.openstructure.walls.wall",
        WallParams {
            name: "Arc host".into(),
            path: WallPath::CircularArc {
                center: Point2::new(0., 0.),
                radius: 8.,
                start_angle_rad: 0.,
                signed_sweep_rad: 1.,
            },
            height: 4.,
            thickness: 0.3,
            level,
            material: None,
        },
    );
    let host = wall.id();
    let ty = OpeningType::new("core.opening_type", parameters(WindowOperation::Casement));
    let tid = ty.id();
    let opening = Opening::new(
        "core.opening",
        OpeningParams {
            name: "Arc window".into(),
            host,
            offset: 1.,
            definition: OpeningDefinition::Typed { type_id: tid },
            open_state: OpeningState::Default,
            width_override: None,
            height_override: None,
            sill_override: None,
            pane_position_override: None,
            lite_side_override: None,
            hinge: DoorHinge::Start,
            swing: DoorSwing::Left,
        },
    );
    doc.execute(
        "Setup",
        vec![
            Command::AddWall(wall),
            Command::AddOpeningType(ty),
            Command::AddOpening(opening),
        ],
    )
    .unwrap();
    let before = doc.model().clone();
    let history = doc.history_stats();
    let mut p = before.opening_types[&tid].parameters.clone();
    p.family.extrusions.push(moving_solid());
    assert!(
        doc.execute(
            "Add latch",
            vec![Command::UpdateOpeningType {
                id: tid,
                parameters: p,
            }]
        )
        .is_err()
    );
    assert_eq!(doc.model(), &before);
    assert_eq!(doc.history_stats(), history);
}
