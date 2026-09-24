use os_core::Point2;
use os_document::{Command, Document};
use os_model::{SectionViewSettings, View, ViewKind, ViewParams, Wall, WallParams};

#[test]
fn editing_native_wall_geometry_invalidates_linked_section_view() {
    let mut document = Document::new("Linked sections").unwrap();
    let level = *document.model().levels.keys().next().unwrap();
    let section = View::new(
        "core.view",
        ViewParams {
            name: "Section A".into(),
            kind: ViewKind::Section,
            level: Some(level),
            settings_revision: 0,
            plan: None,
            section: Some(SectionViewSettings::new(
                Point2::new(0.0, 0.0),
                Point2::new(8.0, 0.0),
                -1.0,
                9.0,
            )),
        },
    );
    let section_id = section.id();
    document
        .execute("Create section view", vec![Command::AddView(section)])
        .unwrap();
    document.drain_events();

    let wall = Wall::new(
        "org.openstructure.walls.wall",
        WallParams {
            name: "Wall".into(),
            start: Point2::new(0.0, 0.0),
            end: Point2::new(8.0, 0.0),
            thickness: 0.2,
            height: 3.0,
            level,
            material: None,
        },
    );
    document
        .execute("Add sectioned wall", vec![Command::AddWall(wall)])
        .unwrap();
    let event = document.drain_events().pop().unwrap();
    assert!(event.invalidated.contains(&section_id));
}
