use super::*;
use os_document::Command;
use os_model::{OpeningDefinition, OpeningKind, OpeningType, OpeningTypeParams};
use serde_json::{Value, json};

fn schema_24_opening_family() -> Value {
    let mut value: Value = serde_json::from_str(include_str!(
        "../../tests/fixtures/schema-23-opening-family.json"
    ))
    .unwrap();
    migrate(&mut value, 23).unwrap();
    value["schema_version"] = json!(24);
    value.as_object_mut().unwrap().remove("columns");
    value
        .as_object_mut()
        .unwrap()
        .remove("plan_graphics_templates");
    value.as_object_mut().unwrap().remove("plan_graphics");
    for collection in [
        "project",
        "sites",
        "buildings",
        "levels",
        "walls",
        "wall_joins",
        "wall_types",
        "openings",
        "opening_types",
        "rooms",
        "room_tags",
        "detail_lines",
        "room_separation_lines",
        "dimensions",
        "grids",
        "materials",
        "views",
        "floors",
        "sheets",
        "schedules",
    ] {
        let data = &mut value[collection];
        let entities: Vec<&mut Value> = if collection == "project" {
            vec![data]
        } else {
            data.as_object_mut().unwrap().values_mut().collect()
        };
        for entity in entities {
            entity["header"]["schema_version"] = json!(24);
            if collection == "opening_types" {
                let family = entity["parameters"]["family"].as_object_mut().unwrap();
                family.insert("version".into(), json!(1));
                family.remove("frame_width");
                family.remove("frame_depth");
                family.remove("cut_profile");
            }
        }
    }
    value
}

fn schema_25_opening_family() -> Value {
    let mut value = schema_24_opening_family();
    value["schema_version"] = json!(25);
    for collection in [
        "project",
        "sites",
        "buildings",
        "levels",
        "walls",
        "wall_joins",
        "wall_types",
        "openings",
        "opening_types",
        "rooms",
        "room_tags",
        "detail_lines",
        "room_separation_lines",
        "dimensions",
        "grids",
        "materials",
        "views",
        "floors",
        "sheets",
        "schedules",
    ] {
        let data = &mut value[collection];
        let entities: Vec<&mut Value> = if collection == "project" {
            vec![data]
        } else {
            data.as_object_mut().unwrap().values_mut().collect()
        };
        for entity in entities {
            entity["header"]["schema_version"] = json!(25);
            if collection == "opening_types" {
                let family = entity["parameters"]["family"].as_object_mut().unwrap();
                family.insert("version".into(), json!(2));
                family.insert("frame_width".into(), json!(0.0));
                family.insert("frame_depth".into(), json!(0.05));
                family.remove("cut_profile");
            }
        }
    }
    value
}

#[test]
fn frozen_schema_23_opening_family_migration_is_lossless_atomic_and_persistent() {
    let original: Value = serde_json::from_str(include_str!(
        "../../tests/fixtures/schema-23-opening-family.json"
    ))
    .unwrap();
    let mut migrated = original.clone();
    migrate(&mut migrated, 23).unwrap();
    let model: Model = serde_json::from_value(migrated.clone()).unwrap();
    let ty = model.opening_types.values().next().unwrap();
    assert_eq!(ty.parameters.family, os_model::OpeningFamily::default());
    assert_eq!(
        ty.parameters.pane_position,
        os_model::WindowPanePosition::RightFace
    );
    assert_eq!(migrated["extensions"], original["extensions"]);
    let mut reversed = migrated.clone();
    reversed["schema_version"] = json!(23);
    for (key, data) in reversed.as_object_mut().unwrap() {
        if key == "project" {
            data["header"]["schema_version"] = json!(23);
        } else if key != "extensions"
            && let Some(map) = data.as_object_mut()
        {
            for entity in map.values_mut() {
                if entity.get("header").is_some() {
                    entity["header"]["schema_version"] = json!(23);
                }
                if key == "opening_types" {
                    entity["parameters"]
                        .as_object_mut()
                        .unwrap()
                        .remove("family");
                }
            }
        }
    }
    reversed.as_object_mut().unwrap().remove("columns");
    reversed
        .as_object_mut()
        .unwrap()
        .remove("plan_graphics_templates");
    reversed.as_object_mut().unwrap().remove("plan_graphics");
    assert_eq!(
        reversed, original,
        "all IDs, parameters, metadata and opaque fields preserved"
    );
    for case in 0..4 {
        let mut bad = original.clone();
        let key = ty.id().to_string();
        match case {
            0 => bad["opening_types"][&key]["parameters"]["family"] = json!({"version":1}),
            1 => bad["opening_types"][&key]["header"]["schema_version"] = json!(22),
            2 => bad["opening_types"][&key]["parameters"]["width"] = json!(-1),
            _ => bad["opening_types"][&key]["parameters"]["pane_position"] = json!("Unknown"),
        }
        let before = bad.clone();
        assert!(migrate(&mut bad, 23).is_err());
        assert_eq!(bad, before);
    }
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("family.osb");
    let type_id = ty.id();
    let doc = os_document::Document::from_model(model).unwrap();
    ZipJsonStorage.save(&doc, &path).unwrap();
    assert_eq!(ZipJsonStorage.open(&path).unwrap().model(), doc.model());
    let mut missing = schema_24_opening_family();
    missing["opening_types"][type_id.to_string()]["parameters"]
        .as_object_mut()
        .unwrap()
        .remove("family");
    assert!(migrate(&mut missing, 24).is_err());
}

#[test]
fn schema_24_frame_family_migration_is_lossless_atomic_and_persistent() {
    let original = schema_24_opening_family();
    let mut migrated = original.clone();
    migrate(&mut migrated, 24).unwrap();
    let model: Model = serde_json::from_value(migrated.clone()).unwrap();
    model.validate().unwrap();
    assert_eq!(model.schema_version, SCHEMA_VERSION);
    let ty = model.opening_types.values().next().unwrap();
    assert_eq!(ty.parameters.family.version, 3);
    assert_eq!(ty.parameters.family.frame_width, 0.0);
    assert_eq!(ty.parameters.family.frame_depth, 0.05);
    assert_eq!(migrated["extensions"], original["extensions"]);

    let mut reversed = migrated.clone();
    reversed["schema_version"] = json!(24);
    for collection in [
        "project",
        "sites",
        "buildings",
        "levels",
        "walls",
        "wall_joins",
        "wall_types",
        "openings",
        "opening_types",
        "rooms",
        "room_tags",
        "detail_lines",
        "room_separation_lines",
        "dimensions",
        "grids",
        "materials",
        "views",
        "floors",
        "sheets",
        "schedules",
    ] {
        let data = &mut reversed[collection];
        let entities: Vec<&mut Value> = if collection == "project" {
            vec![data]
        } else {
            data.as_object_mut().unwrap().values_mut().collect()
        };
        for entity in entities {
            entity["header"]["schema_version"] = json!(24);
            if collection == "opening_types" {
                let family = entity["parameters"]["family"].as_object_mut().unwrap();
                family.insert("version".into(), json!(1));
                family.remove("frame_width");
                family.remove("frame_depth");
                family.remove("cut_profile");
            }
        }
    }
    reversed.as_object_mut().unwrap().remove("columns");
    reversed
        .as_object_mut()
        .unwrap()
        .remove("plan_graphics_templates");
    reversed.as_object_mut().unwrap().remove("plan_graphics");
    assert_eq!(
        reversed, original,
        "all old fields and identities are retained"
    );

    let type_id = ty.id().to_string();
    for case in 0..4 {
        let mut bad = original.clone();
        match case {
            0 => bad["opening_types"][&type_id]["parameters"]["family"]["version"] = json!(3),
            1 => {
                bad["opening_types"][&type_id]["parameters"]["family"]["frame_width"] = json!(0.05)
            }
            2 => bad["opening_types"][&type_id]["header"]["schema_version"] = json!(23),
            _ => bad["opening_types"][&type_id]["parameters"]["width"] = json!(-1),
        }
        let before = bad.clone();
        assert!(migrate(&mut bad, 24).is_err());
        assert_eq!(bad, before);
    }

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("framed-openings.osb");
    let document = os_document::Document::from_model(model).unwrap();
    ZipJsonStorage.save(&document, &path).unwrap();
    assert_eq!(
        ZipJsonStorage.open(&path).unwrap().model(),
        document.model()
    );
}

#[test]
fn schema_25_host_cut_profile_migration_is_lossless_atomic_and_persistent() {
    let original = schema_25_opening_family();
    let mut migrated = original.clone();
    migrate(&mut migrated, 25).unwrap();
    let model: Model = serde_json::from_value(migrated.clone()).unwrap();
    model.validate().unwrap();
    assert_eq!(model.schema_version, SCHEMA_VERSION);
    let ty = model.opening_types.values().next().unwrap();
    assert_eq!(ty.parameters.family.version, 3);
    assert_eq!(
        ty.parameters.family.host_cut,
        os_model::OpeningHostCut::Rectangular
    );
    assert!(ty.parameters.family.rectangular_cut());
    assert_eq!(
        ty.parameters.family.cut_profile,
        os_model::OpeningFamily::rectangle()
    );
    assert_eq!(migrated["extensions"], original["extensions"]);

    let mut reversed = migrated.clone();
    reversed["schema_version"] = json!(25);
    for collection in [
        "project",
        "sites",
        "buildings",
        "levels",
        "walls",
        "wall_joins",
        "wall_types",
        "openings",
        "opening_types",
        "rooms",
        "room_tags",
        "detail_lines",
        "room_separation_lines",
        "dimensions",
        "grids",
        "materials",
        "views",
        "floors",
        "sheets",
        "schedules",
    ] {
        let data = &mut reversed[collection];
        let entities: Vec<&mut Value> = if collection == "project" {
            vec![data]
        } else {
            data.as_object_mut().unwrap().values_mut().collect()
        };
        for entity in entities {
            entity["header"]["schema_version"] = json!(25);
            if collection == "opening_types" {
                let family = entity["parameters"]["family"].as_object_mut().unwrap();
                family.insert("version".into(), json!(2));
                family.remove("cut_profile");
            }
        }
    }
    reversed.as_object_mut().unwrap().remove("columns");
    reversed
        .as_object_mut()
        .unwrap()
        .remove("plan_graphics_templates");
    reversed.as_object_mut().unwrap().remove("plan_graphics");
    assert_eq!(
        reversed, original,
        "only the documented migration is reversed"
    );

    let type_id = ty.id().to_string();
    for case in 0..4 {
        let mut invalid = original.clone();
        match case {
            0 => invalid["opening_types"][&type_id]["parameters"]["family"]["version"] = json!(3),
            1 => {
                invalid["opening_types"][&type_id]["parameters"]["family"]["cut_profile"] =
                    json!([])
            }
            2 => {
                invalid["opening_types"][&type_id]["parameters"]["family"]["host_cut"] =
                    json!("Profile")
            }
            _ => invalid["opening_types"][&type_id]["header"]["schema_version"] = json!(24),
        }
        let before = invalid.clone();
        assert!(migrate(&mut invalid, 25).is_err(), "case {case}");
        assert_eq!(invalid, before, "case {case} must not partially migrate");
    }

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("host-cut-migration.osb");
    let document = os_document::Document::from_model(model).unwrap();
    ZipJsonStorage.save(&document, &path).unwrap();
    assert_eq!(
        ZipJsonStorage.open(&path).unwrap().model(),
        document.model()
    );
}

const DOOR: &str = "10000000-0000-0000-0000-000000000001";
const WINDOW: &str = "10000000-0000-0000-0000-000000000002";
const HOST: &str = "df965a8f-c122-41f2-8c46-6d77aa3e7749";
const NATIVE_MAPS: &[&str] = &[
    "sites",
    "buildings",
    "levels",
    "walls",
    "materials",
    "views",
    "grids",
    "openings",
    "rooms",
    "dimensions",
];

// Frozen schema-7 data, independent of today's constructors/serializer and of
// the migration implementation. Reuse only the fixed schema-3 building graph.
fn schema_seven() -> Value {
    let mut value: Value = serde_json::from_str(include_str!(
        "../../../../fixtures/schema-3-pointer-walls.json"
    ))
    .unwrap();
    value["schema_version"] = json!(7);
    value["project"]["header"]["schema_version"] = json!(7);
    value["grids"] = json!({});
    value["rooms"] = json!({});
    value["dimensions"] = json!({});
    value["openings"] = json!({
        DOOR: {
            "header": {"id": DOOR, "type_id": "core.opening", "schema_version": 7,
                "properties": {"note": "unchanged"}, "relationships": {"host": [HOST]}},
            "parameters": {"name": "D1", "host": HOST, "offset": 0.3,
                "kind": "Door", "width": 0.9, "height": 2.1, "sill": 0.0}
        },
        WINDOW: {
            "header": {"id": WINDOW, "type_id": "core.opening", "schema_version": 7,
                "properties": {}, "relationships": {}},
            "parameters": {"name": "W1", "host": HOST, "offset": 2.0,
                "kind": "Window", "width": 1.2, "height": 1.1, "sill": 0.9}
        }
    });
    for key in NATIVE_MAPS {
        for entity in value[*key].as_object_mut().unwrap().values_mut() {
            entity["header"]["schema_version"] = json!(7);
        }
    }
    value["plugin_requirements"] = json!({"org.example.fixture": {"version": "1.0.0"}});
    value["extensions"] = json!({"10000000-0000-0000-0000-000000000003": {
        "envelope_version": 1, "id": "10000000-0000-0000-0000-000000000003",
        "owner": "org.example.fixture", "type_id": "org.example.fixture.item", "name": "Opaque",
        "payload_schema_version": 42, "relationships": {"wall": [HOST]}, "depends_on": [HOST],
        "payload": {"schema_version": 7, "kind": "vendor", "width": 12.75}
    }});
    value
}

fn id(text: &str) -> Id {
    Id(text.parse().unwrap())
}

// Frozen schema-9 shape assembled only from the fixed schema-7 fixture above,
// never from a current constructor or by running the migration under test.
fn schema_nine(typed: bool) -> Value {
    let mut value = schema_seven();
    value["schema_version"] = json!(9);
    value["project"]["header"]["schema_version"] = json!(9);
    for key in NATIVE_MAPS {
        for entity in value[*key].as_object_mut().unwrap().values_mut() {
            entity["header"]["schema_version"] = json!(9);
        }
    }
    value["floors"] = json!({});
    value["opening_types"] = json!({});
    for key in [DOOR, WINDOW] {
        let params = value["openings"][key]["parameters"]
            .as_object_mut()
            .unwrap();
        let mut dimensions = serde_json::Map::new();
        for field in ["kind", "width", "height", "sill"] {
            dimensions.insert(field.into(), params.remove(field).unwrap());
        }
        params.insert("definition".into(), json!({"Legacy": dimensions}));
    }
    if typed {
        let ty = "10000000-0000-0000-0000-000000000004";
        value["opening_types"][ty] = json!({
            "header": {"id": ty, "type_id": "core.opening_type", "schema_version": 9,
                "properties": {}, "relationships": {}},
            "parameters": {"name": "Shared door", "kind": "Door", "width": 0.9,
                "height": 2.1, "sill": 0.0}
        });
        value["openings"][DOOR]["parameters"]["definition"] = json!({"Typed": {"type_id": ty}});
    }
    value
}

#[test]
fn schema_nine_openings_migrate_only_orientation_and_native_headers_and_roundtrip() {
    for typed in [false, true] {
        let original = schema_nine(typed);
        let mut migrated = original.clone();
        migrate(&mut migrated, 9).unwrap();
        let model: Model = serde_json::from_value(migrated.clone()).unwrap();
        model.validate().unwrap();
        for opening in model.openings.values() {
            assert_eq!(opening.parameters.hinge, os_model::DoorHinge::Start);
            assert_eq!(opening.parameters.swing, os_model::DoorSwing::Left);
        }
        let mut reversed = migrated.clone();
        for ty in reversed["opening_types"]
            .as_object_mut()
            .unwrap()
            .values_mut()
        {
            ty["parameters"].as_object_mut().unwrap().remove("family");
            assert_eq!(
                ty["parameters"]
                    .as_object_mut()
                    .unwrap()
                    .remove("pane_position")
                    .unwrap(),
                "Center"
            );
        }
        for key in [DOOR, WINDOW] {
            let params = reversed["openings"][key]["parameters"]
                .as_object_mut()
                .unwrap();
            assert_eq!(params.remove("hinge").unwrap(), "Start");
            assert_eq!(params.remove("swing").unwrap(), "Left");
        }
        for key in NATIVE_MAPS
            .iter()
            .copied()
            .chain(["opening_types", "floors"])
        {
            for entity in reversed[key].as_object_mut().unwrap().values_mut() {
                assert_eq!(entity["header"]["schema_version"], SCHEMA_VERSION);
                entity["header"]["schema_version"] = json!(9);
            }
        }
        assert_eq!(
            reversed["project"]["header"]["schema_version"],
            SCHEMA_VERSION
        );
        reversed["project"]["header"]["schema_version"] = json!(9);
        reversed["schema_version"] = json!(9);
        reversed.as_object_mut().unwrap().remove("room_tags");
        reversed.as_object_mut().unwrap().remove("sheets");
        reversed.as_object_mut().unwrap().remove("schedules");
        reversed.as_object_mut().unwrap().remove("detail_lines");
        reversed
            .as_object_mut()
            .unwrap()
            .remove("room_separation_lines");
        reversed.as_object_mut().unwrap().remove("wall_joins");
        reversed.as_object_mut().unwrap().remove("wall_types");
        reversed
            .as_object_mut()
            .unwrap()
            .remove("wall_type_assignments");
        reversed.as_object_mut().unwrap().remove("columns");
        reversed
            .as_object_mut()
            .unwrap()
            .remove("plan_graphics_templates");
        reversed.as_object_mut().unwrap().remove("plan_graphics");
        assert_eq!(reversed, original);

        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("schema-nine.osb");
        let saved = directory.path().join("schema-ten.osb");
        let manifest = Manifest {
            format: "OpenStructure".into(),
            container_version: 2,
            schema_version: 9,
            project_id: model.project.id(),
            model_entry: "model.json".into(),
            units: "metres".into(),
        };
        let mut zip = ZipWriter::new(File::create(&source).unwrap());
        zip.start_file("manifest.toml", SimpleFileOptions::default())
            .unwrap();
        zip.write_all(toml::to_string(&manifest).unwrap().as_bytes())
            .unwrap();
        zip.start_file("model.json", SimpleFileOptions::default())
            .unwrap();
        zip.write_all(&serde_json::to_vec(&original).unwrap())
            .unwrap();
        zip.finish().unwrap();
        let bytes = std::fs::read(&source).unwrap();
        let doc = ZipJsonStorage.open(&source).unwrap();
        assert_eq!(doc.model(), &model);
        ZipJsonStorage.save(&doc, &saved).unwrap();
        assert_eq!(ZipJsonStorage.open(&saved).unwrap().model(), &model);
        assert_eq!(std::fs::read(&source).unwrap(), bytes);
    }
}

#[test]
fn opening_orientation_migration_rejects_ambiguous_and_malformed_input_atomically() {
    for case in 0..5 {
        let mut value = schema_nine(true);
        match case {
            0 => value["openings"][DOOR]["parameters"]["hinge"] = json!("Start"),
            1 => value["openings"][WINDOW]["parameters"]["swing"] = json!("Left"),
            2 => {
                value["opening_types"]
                    .as_object_mut()
                    .unwrap()
                    .values_mut()
                    .next()
                    .unwrap()["header"]["schema_version"] = json!(8)
            }
            3 => {
                value.as_object_mut().unwrap().remove("floors");
            }
            _ => value["openings"][DOOR]["parameters"]["offset"] = json!(-1),
        }
        let before = value.clone();
        assert!(migrate(&mut value, 9).is_err());
        assert_eq!(value, before);
    }
    let mut value = schema_nine(false);
    migrate(&mut value, 9).unwrap();
    value["openings"][DOOR]["parameters"]
        .as_object_mut()
        .unwrap()
        .remove("hinge");
    let before = value.clone();
    assert!(migrate(&mut value, 10).is_err());
    assert_eq!(value, before);
}

#[test]
fn frozen_schema_22_adds_center_pane_position_without_losing_identity() {
    let old: Value = serde_json::from_str(include_str!(
        "../../tests/fixtures/schema-22-window-pane-position.json"
    ))
    .unwrap();
    let mut migrated = old.clone();
    migrate(&mut migrated, 22).unwrap();
    let model: Model = serde_json::from_value(migrated.clone()).unwrap();
    model.validate().unwrap();
    assert_eq!(model.schema_version, os_model::SCHEMA_VERSION);
    let type_id = id("00000000-0000-4000-8000-000000000011");
    let window_id = id("00000000-0000-4000-8000-000000000008");
    assert_eq!(
        model.opening_types[&type_id].parameters.pane_position,
        os_model::WindowPanePosition::Center
    );
    assert_eq!(
        model.opening_types[&type_id].header.properties["retained"],
        json!("opening type metadata")
    );
    assert_eq!(
        model
            .resolve_opening(&model.openings[&window_id].parameters)
            .unwrap()
            .pane_position,
        os_model::WindowPanePosition::Center,
        "legacy openings remain independent and centered"
    );

    let mut restored = migrated.clone();
    restored["opening_types"][type_id.to_string()]["parameters"]
        .as_object_mut()
        .unwrap()
        .remove("family");
    restored["opening_types"][type_id.to_string()]["parameters"]
        .as_object_mut()
        .unwrap()
        .remove("pane_position");
    for collection in [
        "project",
        "sites",
        "buildings",
        "levels",
        "walls",
        "wall_joins",
        "wall_types",
        "openings",
        "opening_types",
        "rooms",
        "room_tags",
        "detail_lines",
        "room_separation_lines",
        "dimensions",
        "grids",
        "materials",
        "views",
        "floors",
        "sheets",
        "schedules",
        "columns",
    ] {
        if collection == "project" {
            restored[collection]["header"]["schema_version"] = json!(22);
        } else {
            for entity in restored[collection].as_object_mut().unwrap().values_mut() {
                entity["header"]["schema_version"] = json!(22);
            }
        }
    }
    restored.as_object_mut().unwrap().remove("columns");
    restored
        .as_object_mut()
        .unwrap()
        .remove("plan_graphics_templates");
    restored.as_object_mut().unwrap().remove("plan_graphics");
    restored["schema_version"] = json!(22);
    assert_eq!(restored, old);

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("pane-position.osb");
    ZipJsonStorage
        .save(
            &os_document::Document::from_model(model.clone()).unwrap(),
            &path,
        )
        .unwrap();
    assert_eq!(ZipJsonStorage.open(&path).unwrap().model(), &model);

    for case in 0..3 {
        let mut invalid = old.clone();
        match case {
            0 => {
                invalid["opening_types"][type_id.to_string()]["parameters"]["pane_position"] =
                    json!("LeftFace");
            }
            1 => {
                invalid["walls"]["00000000-0000-4000-8000-000000000006"]["header"]["schema_version"] =
                    json!(21)
            }
            _ => {
                invalid.as_object_mut().unwrap().remove("opening_types");
            }
        }
        let before = invalid.clone();
        assert!(migrate(&mut invalid, 22).is_err(), "case {case}");
        assert_eq!(invalid, before);
    }
}

#[test]
fn frozen_seven_to_ten_preserves_geometry_identity_metadata_and_plugin_payload() {
    let original = schema_seven();
    let mut value = original.clone();
    migrate(&mut value, 7).unwrap();
    let model: Model = serde_json::from_value(value.clone()).unwrap();
    model.validate().unwrap();
    assert_eq!(model.schema_version, SCHEMA_VERSION);
    assert!(model.opening_types.is_empty());
    assert_eq!(value["extensions"], original["extensions"]);
    assert_eq!(
        value["plugin_requirements"],
        original["plugin_requirements"]
    );
    for key in [DOOR, WINDOW] {
        let opening = &model.openings[&id(key)];
        let resolved = model.resolve_opening(&opening.parameters).unwrap();
        let old = &original["openings"][key]["parameters"];
        assert_eq!(resolved.offset, old["offset"].as_f64().unwrap());
        assert_eq!(resolved.width, old["width"].as_f64().unwrap());
        assert_eq!(resolved.height, old["height"].as_f64().unwrap());
        assert_eq!(resolved.sill, old["sill"].as_f64().unwrap());
        assert_eq!(resolved.name, old["name"].as_str().unwrap());
        assert_eq!(resolved.host, id(HOST));
        assert!(matches!(
            opening.parameters.definition,
            OpeningDefinition::Legacy { .. }
        ));
        assert_eq!(resolved.type_id, None);
        assert_eq!(
            resolved.kind,
            if key == DOOR {
                OpeningKind::Door
            } else {
                OpeningKind::Window
            }
        );
        let p = value["openings"][key]["parameters"]
            .as_object_mut()
            .unwrap();
        assert_eq!(p.remove("hinge").unwrap(), "Start");
        assert_eq!(p.remove("swing").unwrap(), "Left");
        let legacy = p.remove("definition").unwrap()["Legacy"]
            .as_object()
            .unwrap()
            .clone();
        p.extend(legacy);
    }
    // Full equality after reversing only the documented schema transformation.
    value.as_object_mut().unwrap().remove("opening_types");
    value.as_object_mut().unwrap().remove("floors");
    value.as_object_mut().unwrap().remove("room_tags");
    value.as_object_mut().unwrap().remove("sheets");
    value.as_object_mut().unwrap().remove("schedules");
    value.as_object_mut().unwrap().remove("detail_lines");
    value
        .as_object_mut()
        .unwrap()
        .remove("room_separation_lines");
    value.as_object_mut().unwrap().remove("wall_joins");
    value.as_object_mut().unwrap().remove("wall_types");
    value
        .as_object_mut()
        .unwrap()
        .remove("wall_type_assignments");
    value.as_object_mut().unwrap().remove("columns");
    value
        .as_object_mut()
        .unwrap()
        .remove("plan_graphics_templates");
    value.as_object_mut().unwrap().remove("plan_graphics");
    value["schema_version"] = json!(7);
    assert_eq!(value["project"]["header"]["schema_version"], SCHEMA_VERSION);
    value["project"]["header"]["schema_version"] = json!(7);
    for key in NATIVE_MAPS {
        for entity in value[*key].as_object_mut().unwrap().values_mut() {
            assert_eq!(entity["header"]["schema_version"], SCHEMA_VERSION);
            entity["header"]["schema_version"] = json!(7);
        }
    }
    assert_eq!(value, original);
}

#[test]
fn ambiguous_old_fields_and_contradictory_headers_fail_without_mutation() {
    for case in 0..9 {
        let mut bad = schema_seven();
        match case {
            0 => bad["opening_types"] = json!({}),
            1 => {
                bad["openings"][DOOR]["parameters"]["definition"] =
                    json!({"Typed": {"type_id": DOOR}})
            }
            2 => bad["openings"][DOOR]["parameters"]["type_id"] = json!(DOOR),
            3 => bad["openings"][DOOR]["header"]["schema_version"] = json!(8),
            4 => bad["project"]["header"]["schema_version"] = json!(6),
            5 => {
                bad["openings"][DOOR]["parameters"]
                    .as_object_mut()
                    .unwrap()
                    .remove("width");
            }
            6 => {
                bad.as_object_mut().unwrap().remove("dimensions");
            }
            7 => bad["openings"][WINDOW]["parameters"]["sill"] = json!(-1),
            _ => bad["openings"][DOOR]["parameters"]["width"] = json!(3),
        }
        let before = bad.clone();
        assert!(migrate(&mut bad, 7).is_err(), "case {case}");
        assert_eq!(bad, before);
    }
    let mut current = serde_json::to_value(Model::new("Required type map")).unwrap();
    current.as_object_mut().unwrap().remove("floors");
    assert!(migrate(&mut current, 9).is_err());
}

#[test]
fn legacy_and_shared_types_survive_archive_migration_and_save_reopen() {
    let original = schema_seven();
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("schema-seven.osb");
    let current = directory.path().join("schema-nine.osb");
    let manifest = Manifest {
        format: "OpenStructure".into(),
        container_version: 2,
        schema_version: 7,
        project_id: serde_json::from_value(original["project"]["header"]["id"].clone()).unwrap(),
        model_entry: "model.json".into(),
        units: "metres".into(),
    };
    {
        let mut zip = ZipWriter::new(File::create(&source).unwrap());
        zip.start_file("manifest.toml", SimpleFileOptions::default())
            .unwrap();
        zip.write_all(toml::to_string(&manifest).unwrap().as_bytes())
            .unwrap();
        zip.start_file("model.json", SimpleFileOptions::default())
            .unwrap();
        zip.write_all(&serde_json::to_vec(&original).unwrap())
            .unwrap();
        zip.finish().unwrap();
    }
    let original_bytes = std::fs::read(&source).unwrap();
    let mut doc = ZipJsonStorage.open(&source).unwrap();
    ZipJsonStorage.save(&doc, &current).unwrap();
    assert_eq!(ZipJsonStorage.open(&current).unwrap().model(), doc.model());
    assert_eq!(std::fs::read(&source).unwrap(), original_bytes);
    let ty = OpeningType::new(
        "core.opening_type",
        OpeningTypeParams {
            family: Default::default(),
            name: "Door type".into(),
            pane_position: Default::default(),
            kind: OpeningKind::Door,
            width: 0.9,
            height: 2.1,
            sill: 0.0,
        },
    );
    let type_id = ty.id();
    let mut p = doc.model().openings[&id(DOOR)].parameters.clone();
    p.definition = OpeningDefinition::Typed { type_id };
    let mut second = os_model::Opening::new("core.opening", p.clone());
    second.parameters.host = *doc
        .model()
        .walls
        .keys()
        .find(|host| **host != id(HOST))
        .unwrap();
    let second_id = second.id();
    doc.execute(
        "Convert and place shared type",
        vec![
            Command::AddOpeningType(ty),
            Command::UpdateOpening {
                id: id(DOOR),
                parameters: p,
            },
            Command::AddOpening(second),
        ],
    )
    .unwrap();
    ZipJsonStorage.save(&doc, &current).unwrap();
    let reopened = ZipJsonStorage.open(&current).unwrap();
    assert_eq!(reopened.model(), doc.model());
    for key in [id(DOOR), second_id] {
        let resolved = reopened
            .model()
            .resolve_opening(&reopened.model().openings[&key].parameters)
            .unwrap();
        assert_eq!(resolved.type_id, Some(type_id));
        assert_eq!(resolved.width, 0.9);
    }
    assert!(matches!(
        reopened.model().openings[&id(WINDOW)].parameters.definition,
        OpeningDefinition::Legacy { .. }
    ));
    let mut zip = ZipArchive::new(File::open(&current).unwrap()).unwrap();
    let saved_manifest: Manifest =
        toml::from_str(&read_entry(&mut zip, "manifest.toml", MAX_MANIFEST_BYTES).unwrap())
            .unwrap();
    assert_eq!(saved_manifest.schema_version, SCHEMA_VERSION);
}

#[test]
fn type_map_is_included_in_serialized_model_limits() {
    let mut model = Model::new("Bounded types");
    let empty_length = serde_json::to_vec_pretty(&model).unwrap().len() as u64;
    let ty = OpeningType::new(
        "core.opening_type",
        OpeningTypeParams {
            family: Default::default(),
            name: "x".repeat(256),
            pane_position: Default::default(),
            kind: OpeningKind::Window,
            width: 1.2,
            height: 1.1,
            sill: 0.9,
        },
    );
    model.opening_types.insert(ty.id(), ty);
    let length = serde_json::to_vec_pretty(&model).unwrap().len() as u64;
    assert!(length > empty_length);
    assert!(validate_serialized_model_size(&model, empty_length).is_err());
    assert!(validate_serialized_model_size(&model, length - 1).is_err());
    validate_serialized_model_size(&model, length).unwrap();
}
