use super::*;
use os_core::Point2;
use os_model::{Ceiling, CeilingParams};

#[test]
fn ceiling_create_edit_undo_redo_and_delete_are_atomic() {
    let mut document = Document::new("Ceilings").unwrap();
    let level = *document.model().levels.keys().next().unwrap();
    let parameters = CeilingParams {
        name: "Main ceiling".into(),
        level,
        material: None,
        boundary_room: None,
        boundary: vec![
            Point2::new(0.0, 0.0),
            Point2::new(4.0, 0.0),
            Point2::new(4.0, 3.0),
            Point2::new(0.0, 3.0),
        ],
        holes: Vec::new(),
        thickness: 0.12,
        elevation_offset: 2.55,
    };
    let ceiling = Ceiling::new("core.ceiling", parameters.clone());
    let id = ceiling.id();
    document
        .execute("Create ceiling", vec![Command::AddCeiling(ceiling.clone())])
        .unwrap();
    let mut updated = parameters.clone();
    updated.elevation_offset = 2.7;
    document
        .execute(
            "Edit ceiling",
            vec![Command::UpdateCeiling {
                id,
                parameters: updated.clone(),
            }],
        )
        .unwrap();
    assert_eq!(document.model().ceilings[&id].parameters, updated);
    assert!(document.undo());
    assert_eq!(document.model().ceilings[&id], ceiling);
    assert!(document.redo());
    assert_eq!(document.model().ceilings[&id].parameters, updated);
    assert!(
        document
            .execute("Delete ceiling", vec![Command::RemoveCeiling(id)])
            .is_ok()
    );
    assert!(!document.model().ceilings.contains_key(&id));
}
