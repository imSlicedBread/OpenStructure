use os_core::Point2;
use os_model::*;

fn window(operation: WindowOperation, attachment: OpeningAttachment) -> OpeningTypeParams {
    let mut solid = OpeningExtrusion::new("Sash handle");
    solid.attachment = attachment;
    OpeningTypeParams {
        name: "Accessory compatibility".into(),
        kind: OpeningKind::Window,
        width: 1.2,
        height: 1.4,
        sill: 0.8,
        pane_position: WindowPanePosition::Center,
        window_operation: operation,
        family: OpeningFamily {
            frame_width: 0.04,
            extrusions: vec![solid],
            ..Default::default()
        },
    }
}

#[test]
fn attachments_have_explicit_operation_compatibility_and_preserve_fixed_semantics() {
    for operation in [
        WindowOperation::Fixed,
        WindowOperation::Casement,
        WindowOperation::Sliding,
    ] {
        window(operation, OpeningAttachment::OpeningFixed)
            .validate()
            .unwrap();
        assert_eq!(
            window(operation, OpeningAttachment::PrimaryPanel)
                .validate()
                .is_ok(),
            operation == WindowOperation::Fixed
        );
        assert_eq!(
            window(operation, OpeningAttachment::MovingSash)
                .validate()
                .is_ok(),
            operation != WindowOperation::Fixed
        );
    }
    let mut door = window(WindowOperation::Fixed, OpeningAttachment::MovingSash);
    door.kind = OpeningKind::Door;
    door.sill = 0.;
    assert!(door.validate().is_err());
    door.family.extrusions[0].attachment = OpeningAttachment::PrimaryPanel;
    door.validate().unwrap();
    door.family.door_leaves = DoorLeaves::Paired {
        active_fraction: 0.5,
    };
    assert!(door.validate().is_err());
    let mut custom = window(WindowOperation::Casement, OpeningAttachment::MovingSash);
    custom.family.profile[2].y = 0.8;
    assert!(custom.validate().is_err());
}

#[test]
fn moving_sash_frame_uses_start_track_and_rechecks_effective_instance_sizes() {
    let mut p = window(WindowOperation::Sliding, OpeningAttachment::MovingSash);
    p.family.side_lite = Some(SideLite {
        side: LiteSide::Start,
        width_fraction: 0.2,
        mullion_width: 0.04,
        material: None,
    });
    p.family.extrusions[0].position = Point2::new(0.035, 0.);
    p.validate().unwrap();
    // Fits the full clear bay, but exceeds the moving Start track of a narrow pin.
    assert!(
        p.family
            .validate_attachments(0.7, p.height, p.kind, p.window_operation)
            .is_err()
    );
    p.window_operation = WindowOperation::Casement;
    p.family
        .validate_attachments(0.7, p.height, p.kind, p.window_operation)
        .unwrap();
    for _ in 1..OpeningFamily::MAX_EXTRUSIONS {
        let mut s = p.family.extrusions[0].clone();
        s.id = os_core::Id::new();
        p.family.extrusions.push(s);
    }
    p.validate().unwrap();
    let mut extra = p.family.extrusions[0].clone();
    extra.id = os_core::Id::new();
    p.family.extrusions.push(extra);
    assert!(
        p.validate().is_err(),
        "16 authored solids remains the limit"
    );
}
