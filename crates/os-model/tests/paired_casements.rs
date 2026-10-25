use os_model::*;
fn ty() -> OpeningTypeParams {
    OpeningTypeParams {
        name: "Pair".into(),
        kind: OpeningKind::Window,
        width: 1.8,
        height: 1.3,
        sill: 0.7,
        pane_position: WindowPanePosition::Center,
        window_operation: WindowOperation::PairedCasement {
            start_fraction: 0.5,
            mullion_width: 0.04,
        },
        family: OpeningFamily {
            frame_width: 0.04,
            ..Default::default()
        },
    }
}
#[test]
fn paired_casement_type_validation_and_strict_wire_contract() {
    let p = ty();
    p.validate().unwrap();
    for (fraction, mullion) in [
        (0., 0.04),
        (1., 0.04),
        (f64::NAN, 0.04),
        (0.5, 0.),
        (0.5, 0.31),
        (0.00001, 0.04),
    ] {
        let mut bad = p.clone();
        bad.window_operation = WindowOperation::PairedCasement {
            start_fraction: fraction,
            mullion_width: mullion,
        };
        assert!(bad.validate().is_err());
    }
    let mut bad = p.clone();
    bad.width = 0.1;
    assert!(bad.validate().is_err());
    bad = p.clone();
    bad.kind = OpeningKind::Door;
    bad.sill = 0.;
    assert!(bad.validate().is_err());
    bad = p.clone();
    bad.family.profile = vec![
        os_core::Point2::new(0., 0.),
        os_core::Point2::new(1., 0.),
        os_core::Point2::new(0.5, 1.),
    ];
    assert!(bad.validate().is_err());
    let mut wire = serde_json::to_value(&p).unwrap();
    wire["window_operation"]["PairedCasement"]["unexpected"] = true.into();
    assert!(serde_json::from_value::<OpeningTypeParams>(wire).is_err());
}
#[test]
fn sash_attachments_require_deliberate_reassignment_and_fit() {
    for attachment in [OpeningAttachment::StartSash, OpeningAttachment::EndSash] {
        let mut p = ty();
        let mut solid = OpeningExtrusion::new("Latch");
        solid.attachment = attachment;
        p.family.extrusions.push(solid);
        p.validate().unwrap();
        for operation in [
            WindowOperation::Fixed,
            WindowOperation::Sliding,
            WindowOperation::Casement,
        ] {
            let mut bad = p.clone();
            bad.window_operation = operation;
            assert!(bad.validate().is_err());
        }
        p.family.extrusions[0].position.x = 1.;
        assert!(p.validate().is_err());
    }
    for attachment in [
        OpeningAttachment::MovingSash,
        OpeningAttachment::PrimaryPanel,
        OpeningAttachment::ActiveLeaf,
        OpeningAttachment::InactiveLeaf,
    ] {
        let mut p = ty();
        let mut solid = OpeningExtrusion::new("Invalid attachment");
        solid.attachment = attachment;
        p.family.extrusions.push(solid);
        assert!(p.validate().is_err());
    }
}
