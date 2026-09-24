use os_constraints::affected_entities;
use os_core::Point2;
use os_model::{Model, SectionViewSettings, View, ViewKind, ViewParams, Wall, WallParams};
use std::collections::BTreeSet;

#[test]
fn model_geometry_changes_invalidate_configured_section_views() {
    let mut model = Model::new("Section dependencies");
    let level = *model.levels.keys().next().unwrap();
    let wall = Wall::new(
        "org.openstructure.walls.wall",
        WallParams {
            name: "Sectioned wall".into(),
            start: Point2::new(0.0, 0.0),
            end: Point2::new(6.0, 0.0),
            thickness: 0.2,
            height: 3.0,
            level,
            material: None,
        },
    );
    let wall_id = wall.id();
    model.walls.insert(wall_id, wall);
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
                Point2::new(6.0, 0.0),
                -1.0,
                4.0,
            )),
        },
    );
    let section_id = section.id();
    model.views.insert(section_id, section);

    let invalidated = affected_entities(&model, &BTreeSet::from([wall_id]));
    assert!(invalidated.contains(&section_id));
}
