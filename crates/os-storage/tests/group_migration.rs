use os_core::Point2;
use os_document::{Command, Document};
use os_model::*;
use os_storage::{StorageBackend, ZipJsonStorage, migrate};
use serde_json::{Value, json};
use std::collections::BTreeSet;

// Build schema 80 from the frozen schema-79 wire fixture using only the known
// header bump and hidden-openings default, independently of production migration.
fn schema80_fixture() -> Value {
    let mut value: Value =
        serde_json::from_str(include_str!("fixtures/schema-79-hidden-openings.json")).unwrap();
    set_native_versions(&mut value, 80);
    value["plan_hidden_openings"] = json!({});
    value
}

fn set_native_versions(value: &mut Value, version: u32) {
    value["schema_version"] = version.into();
    for (name, data) in value.as_object_mut().unwrap() {
        if name == "project" {
            data["header"]["schema_version"] = version.into();
        } else if name != "extensions"
            && let Some(map) = data.as_object_mut()
        {
            for entity in map.values_mut().filter(|v| v.get("header").is_some()) {
                entity["header"]["schema_version"] = version.into();
            }
        }
    }
}

#[test]
fn schema80_groups_initialize_maps_and_preserve_payloads() {
    let mut wire = schema80_fixture();
    let mut expected = wire.clone();
    set_native_versions(&mut expected, SCHEMA_VERSION);
    expected["group_definitions"] = json!({});
    expected["group_instances"] = json!({});
    expected["plan_element_graphics_overrides"] = json!({});
    expected["plan_graphics_rules"] = json!({});
    migrate(&mut wire, 80).unwrap();
    assert_eq!(wire, expected);
    let model: os_model::Model = serde_json::from_value(wire.clone()).unwrap();
    assert!(model.group_definitions.is_empty());
    assert!(model.group_instances.is_empty());
    model.validate().unwrap();
}

#[test]
fn schema80_groups_reject_premature_fields_atomically() {
    for field in ["group_definitions", "group_instances"] {
        for payload in [Value::Null, json!({}), json!([]), json!({"keep": "data"})] {
            let mut wire = schema80_fixture();
            wire[field] = payload;
            let before = wire.clone();
            assert!(migrate(&mut wire, 80).is_err());
            assert_eq!(wire, before);
        }
    }
}

#[test]
fn schema80_groups_reject_invalid_native_header_atomically() {
    let mut wire = schema80_fixture();
    wire["project"]["header"]["schema_version"] = 79.into();
    let before = wire.clone();
    assert!(migrate(&mut wire, 80).is_err());
    assert_eq!(wire, before);
}

#[test]
fn grouped_doors_and_windows_save_reopen_with_shared_definition_identity() {
    let mut model = Model::new("Group archive");
    let level = *model.levels.keys().next().unwrap();
    let view = View::new("core.view", ViewParams::floor_plan("Plan", level));
    let view_id = view.id();
    model.views.insert(view_id, view);
    let wall = Wall::new(
        "org.openstructure.walls.wall",
        WallParams {
            name: "Host".into(),
            path: WallPath::Straight {
                start: Point2::default(),
                end: Point2::new(8., 0.),
            },
            thickness: 0.2,
            height: 3.,
            level,
            material: None,
        },
    );
    let wall_id = wall.id();
    model.walls.insert(wall_id, wall);
    for (kind, offset, width, height, sill) in [
        (OpeningKind::Door, 1., 0.9, 2.1, 0.),
        (OpeningKind::Window, 4., 1.2, 1.4, 0.8),
    ] {
        let opening = Opening::new(
            "core.opening",
            OpeningParams {
                open_state: OpeningState::default(),
                name: format!("{kind:?}"),
                host: wall_id,
                offset,
                definition: OpeningDefinition::Legacy {
                    kind,
                    width,
                    height,
                    sill,
                },
                width_override: None,
                height_override: None,
                sill_override: None,
                pane_position_override: None,
                lite_side_override: None,
                hinge: DoorHinge::Start,
                swing: DoorSwing::Left,
            },
        );
        model.openings.insert(opening.id(), opening);
    }
    let mut document = Document::from_model(model).unwrap();
    document
        .execute(
            "Group wall with door and window",
            vec![Command::CreateWallGroup {
                walls: BTreeSet::from([wall_id]),
                view: view_id,
                name: "Door-window assembly".into(),
            }],
        )
        .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("group.osb");
    ZipJsonStorage.save(&document, &path).unwrap();
    let reopened = ZipJsonStorage.open(&path).unwrap();
    assert_eq!(reopened.model(), document.model());
    assert_eq!(reopened.model().group_definitions.len(), 1);
    assert_eq!(reopened.model().group_instances.len(), 1);
    assert_eq!(
        reopened
            .model()
            .group_definitions
            .values()
            .next()
            .unwrap()
            .parameters
            .template
            .openings
            .len(),
        2
    );
}
