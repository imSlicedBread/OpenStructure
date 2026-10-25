use os_model::*;

#[test]
fn paired_accessories_require_explicit_reassignment_and_effective_leaf_bounds() {
    let mut family = OpeningFamily {
        door_leaves: DoorLeaves::Paired {
            active_fraction: 0.7,
        },
        frame_width: 0.03,
        ..Default::default()
    };
    let mut s = OpeningExtrusion::new("Handle");
    s.attachment = OpeningAttachment::ActiveLeaf;
    s.position.x = 0.03;
    family.extrusions.push(s);
    family
        .validate_attachments(1.8, 2.1, OpeningKind::Door, WindowOperation::Fixed)
        .unwrap();
    family.extrusions[0].attachment = OpeningAttachment::InactiveLeaf;
    family
        .validate_attachments(1.8, 2.1, OpeningKind::Door, WindowOperation::Fixed)
        .unwrap();
    assert!(family.validate_for(0.65, 2.1, OpeningKind::Door).is_err());
    family.extrusions[0].attachment = OpeningAttachment::ActiveLeaf;
    family.validate_for(0.65, 2.1, OpeningKind::Door).unwrap();
    family.door_leaves = DoorLeaves::Single;
    assert!(family.validate().is_err());
    family.extrusions[0].attachment = OpeningAttachment::PrimaryPanel;
    family.validate().unwrap();
    family.door_leaves = DoorLeaves::Paired {
        active_fraction: 0.7,
    };
    assert!(family.validate().is_err());
    family.extrusions[0].attachment = OpeningAttachment::OpeningFixed;
    family.validate_for(1.8, 2.1, OpeningKind::Door).unwrap();
    assert!(family.validate_for(1.8, 2.1, OpeningKind::Window).is_err());
    for _ in 1..16 {
        let mut s = family.extrusions[0].clone();
        s.id = os_core::Id::new();
        family.extrusions.push(s);
    }
    family.validate().unwrap();
    family.extrusions.push(OpeningExtrusion::new("Too many"));
    assert!(family.validate().is_err());
}
