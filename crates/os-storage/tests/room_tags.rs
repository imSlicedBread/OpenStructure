use os_core::Point2;
use os_document::{Command, Document};
use os_model::{Model, RoomTag, RoomTagParams, SCHEMA_VERSION, ViewKind};
use os_storage::{StorageBackend, ZipJsonStorage, migrate};
use serde_json::{Value, json};

#[test]
fn room_tag_frozen_schema_twelve_migration_preserves_all_data_except_versions_and_new_map() {
    let original: Value =
        serde_json::from_str(include_str!("../../../fixtures/schema-12-room.json")).unwrap();
    assert_eq!(original["schema_version"], 12);
    assert!(original.get("room_tags").is_none());
    let mut migrated = original.clone();
    migrate(&mut migrated, 12).unwrap();
    let mut expected = original.clone();
    expected["schema_version"] = json!(SCHEMA_VERSION);
    expected["schedules"] = json!({});
    expected["room_tags"] = json!({});
    expected["sheets"] = json!({});
    expected["detail_lines"] = json!({});
    expected["room_separation_lines"] = json!({});
    expected["wall_joins"] = json!({});
    expected["wall_types"] = json!({});
    expected["wall_type_assignments"] = json!({});
    expected["columns"] = json!({});
    expected["plan_graphics_templates"] = json!({});
    expected["plan_graphics"] = json!({});
    expected["project"]["header"]["schema_version"] = json!(SCHEMA_VERSION);
    for collection in [
        "sites",
        "buildings",
        "levels",
        "materials",
        "walls",
        "openings",
        "opening_types",
        "floors",
        "rooms",
        "dimensions",
        "grids",
        "views",
    ] {
        for entity in expected[collection].as_object_mut().unwrap().values_mut() {
            assert_eq!(entity["header"]["schema_version"], 12);
            entity["header"]["schema_version"] = json!(SCHEMA_VERSION);
        }
    }
    assert_eq!(
        migrated, expected,
        "every UUID, header field and parameter must survive unchanged"
    );
    let model: Model = serde_json::from_value(migrated).unwrap();
    model.validate().unwrap();
    for case in 0..3 {
        let mut invalid = original.clone();
        match case {
            0 => invalid["room_tags"] = json!({}),
            1 => invalid["project"]["header"]["schema_version"] = json!(11),
            _ => {
                invalid.as_object_mut().unwrap().remove("rooms");
            }
        }
        let before = invalid.clone();
        assert!(migrate(&mut invalid, 12).is_err());
        assert_eq!(invalid, before);
    }
}

#[test]
fn room_tag_current_schema_save_reopen_preserves_live_and_orphan_tags() {
    let mut value: Value =
        serde_json::from_str(include_str!("../../../fixtures/schema-12-room.json")).unwrap();
    migrate(&mut value, 12).unwrap();
    let model: Model = serde_json::from_value(value).unwrap();
    let room = *model.rooms.keys().next().unwrap();
    let view = model
        .views
        .values()
        .find(|v| v.parameters.kind == ViewKind::Plan)
        .unwrap()
        .id();
    let tag = RoomTag::new(
        "core.room_tag",
        RoomTagParams {
            room,
            view,
            position: Point2::new(2., 1.),
        },
    );
    let mut doc = Document::from_model(model).unwrap();
    doc.execute("Tag", vec![Command::AddRoomTag(tag.clone())])
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("room-tag.osb");
    for orphan in [false, true] {
        if orphan {
            doc.execute("Delete room", vec![Command::RemoveRoom(room)])
                .unwrap();
        }
        ZipJsonStorage.save(&doc, &path).unwrap();
        let reopened = ZipJsonStorage.open(&path).unwrap();
        assert_eq!(reopened.model(), doc.model());
        assert_eq!(reopened.model().room_tags[&tag.id()], tag);
        assert_eq!(tag.parameters.resolve(reopened.model()).is_err(), orphan);
        assert_eq!(reopened.model().schema_version, SCHEMA_VERSION);
    }
}
