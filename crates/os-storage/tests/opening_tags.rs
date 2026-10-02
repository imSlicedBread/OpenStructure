mod common;

use os_core::Point2;
use os_document::{Command, Document};
use os_model::*;
use os_storage::{StorageBackend, ZipJsonStorage, migrate};
fn setup() -> (Model, OpeningTag) {
    let mut model = Model::new("Opening tags");
    let level = *model.levels.keys().next().unwrap();
    let wall = Wall::new(
        "org.openstructure.walls.wall",
        WallParams {
            name: "Host".into(),
            path: os_model::WallPath::Straight {
                start: Point2::new(0., 0.),
                end: Point2::new(8., 0.),
            },
            thickness: 0.2,
            height: 3.,
            level,
            material: None,
        },
    );
    let ty = OpeningType::new(
        "core.opening_type",
        OpeningTypeParams {
            window_operation: Default::default(),
            name: "D900".into(),
            family: OpeningFamily::default(),
            kind: OpeningKind::Door,
            width: 0.9,
            height: 2.1,
            sill: 0.,
            pane_position: WindowPanePosition::Center,
        },
    );
    let opening = Opening::new(
        "core.opening",
        OpeningParams {
            open_state: Default::default(),
            name: "D-01".into(),
            host: wall.id(),
            offset: 1.,
            definition: OpeningDefinition::Typed { type_id: ty.id() },
            width_override: None,
            height_override: None,
            sill_override: None,
            pane_position_override: None,
            lite_side_override: None,
            hinge: DoorHinge::Start,
            swing: DoorSwing::Left,
        },
    );
    let view = View::new("core.view", ViewParams::floor_plan("Plan", level));
    let tag = OpeningTag::new(
        "core.opening_tag",
        OpeningTagParams {
            view: view.id(),
            opening: opening.id(),
            position: Point2::new(2., 1.5),
            label_preset: Default::default(),
        },
    );
    model.walls.insert(wall.id(), wall);
    model.opening_types.insert(ty.id(), ty);
    model.openings.insert(opening.id(), opening);
    model.views.insert(view.id(), view);
    (model, tag)
}

#[test]
fn opening_tags_save_reopen_keeps_live_and_orphan_identity() {
    let (model, tag) = setup();
    let mut tag = tag;
    tag.parameters.label_preset = OpeningTagLabelPreset::TypeAndDimensions;
    let mut doc = Document::from_model(model).unwrap();
    doc.execute("Tag", vec![Command::AddOpeningTag(tag.clone())])
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tags.osb");
    for orphan in [false, true] {
        if orphan {
            doc.execute(
                "Remove",
                vec![Command::RemoveOpening(tag.parameters.opening)],
            )
            .unwrap();
        }
        ZipJsonStorage.save(&doc, &path).unwrap();
        let reopened = ZipJsonStorage.open(&path).unwrap();
        assert_eq!(reopened.model(), doc.model());
        assert_eq!(reopened.model().opening_tags[&tag.id()], tag);
        assert_eq!(tag.parameters.resolve(reopened.model()).is_err(), orphan);
    }
}

#[test]
fn schema_40_opening_tag_presets_migrate_explicitly_and_atomically() {
    let mut legacy: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/schema-40-opening-tags.json")).unwrap();
    migrate(&mut legacy, 40).unwrap();
    assert_eq!(legacy["schema_version"], SCHEMA_VERSION);
    for tag in legacy["opening_tags"].as_object().unwrap().values() {
        assert_eq!(tag["header"]["schema_version"], SCHEMA_VERSION);
        assert_eq!(tag["parameters"]["label_preset"], "Full");
    }
    assert_eq!(
        legacy["opening_tags"]["00000000-0000-4000-8000-000000000061"]["header"]["properties"]["retained"],
        "tag fixture metadata"
    );
    let model: Model = serde_json::from_value(legacy.clone()).unwrap();
    model.validate().unwrap();

    let mut ambiguous: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/schema-40-opening-tags.json")).unwrap();
    ambiguous["opening_tags"]["00000000-0000-4000-8000-000000000061"]["parameters"]["label_preset"] =
        "DimensionsOnly".into();
    let before = ambiguous.clone();
    assert!(migrate(&mut ambiguous, 40).is_err());
    assert_eq!(ambiguous, before);

    let mut malformed: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/schema-40-opening-tags.json")).unwrap();
    malformed["walls"]["00000000-0000-4000-8000-000000000006"]["header"]["schema_version"] =
        39.into();
    let before = malformed.clone();
    assert!(migrate(&mut malformed, 40).is_err());
    assert_eq!(malformed, before);

    let mut current = legacy;
    current["opening_tags"]["00000000-0000-4000-8000-000000000061"]["parameters"]
        .as_object_mut()
        .unwrap()
        .remove("label_preset");
    let before = current.clone();
    assert!(migrate(&mut current, SCHEMA_VERSION).is_err());
    assert_eq!(current, before);
}
#[test]
fn opening_tags_schema_30_migration_is_explicit_atomic_and_preserves_prior_fields() {
    let (model, _) = setup();
    let mut old = serde_json::to_value(model).unwrap();
    common::remove_phase_fields(&mut old);
    old.as_object_mut().unwrap().remove("stairs");
    old.as_object_mut().unwrap().remove("roofs");
    for ty in old["opening_types"].as_object_mut().unwrap().values_mut() {
        let family = ty["parameters"]["family"].as_object_mut().unwrap();
        family.insert("version".into(), 3.into());
        family.remove("panel_material");
        family.remove("frame_material");
    }
    old.as_object_mut().unwrap().remove("opening_tags");
    old["schema_version"] = 30.into();
    for (key, value) in old.as_object_mut().unwrap() {
        if key == "project" {
            value["header"]["schema_version"] = 30.into();
        } else if key != "extensions"
            && let Some(map) = value.as_object_mut()
        {
            for entity in map.values_mut() {
                if entity.get("header").is_some() {
                    entity["header"]["schema_version"] = 30.into();
                }
            }
        }
    }
    let original = old.clone();
    migrate(&mut old, 30).unwrap();
    let current: Model = serde_json::from_value(old.clone()).unwrap();
    current.validate().unwrap();
    assert!(current.opening_tags.is_empty());
    for ty in old["opening_types"].as_object_mut().unwrap().values_mut() {
        let family = ty["parameters"]["family"].as_object_mut().unwrap();
        family.insert("version".into(), 3.into());
        family.remove("panel_material");
        family.remove("frame_material");
    }
    old.as_object_mut().unwrap().remove("opening_tags");
    old["schema_version"] = 30.into();
    for (key, value) in old.as_object_mut().unwrap() {
        if key == "project" {
            value["header"]["schema_version"] = 30.into();
        } else if key != "extensions"
            && let Some(map) = value.as_object_mut()
        {
            for entity in map.values_mut() {
                if entity.get("header").is_some() {
                    entity["header"]["schema_version"] = 30.into();
                }
            }
        }
    }
    old.as_object_mut().unwrap().remove("stairs");
    old.as_object_mut().unwrap().remove("roofs");
    common::remove_phase_fields(&mut old);
    old.as_object_mut().unwrap().remove("stairs");
    old.as_object_mut().unwrap().remove("roofs");
    assert_eq!(old, original);
    for case in 0..3 {
        let mut bad = original.clone();
        match case {
            0 => bad["opening_tags"] = serde_json::json!({}),
            1 => bad["project"]["header"]["schema_version"] = 29.into(),
            _ => {
                bad.as_object_mut().unwrap().remove("openings");
            }
        }
        let before = bad.clone();
        assert!(migrate(&mut bad, 30).is_err());
        assert_eq!(bad, before);
    }
    let mut missing = serde_json::to_value(current).unwrap();
    missing.as_object_mut().unwrap().remove("opening_tags");
    assert!(serde_json::from_value::<Model>(missing).is_err());
}
