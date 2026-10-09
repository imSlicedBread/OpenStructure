use os_core::{Id, Point2};
use os_document::{Command, Document};
use os_model::*;
use os_storage::{StorageBackend, ZipJsonStorage, migrate};
use serde_json::{Value, json};

fn frozen() -> Value {
    serde_json::from_str(include_str!("fixtures/schema-63-elevations.json")).unwrap()
}

fn advance_headers(value: &mut Value) {
    for ty in value["opening_types"].as_object_mut().unwrap().values_mut() {
        ty["parameters"]["family"]["version"] = 7.into();
        ty["parameters"]["family"]["extrusions"] = json!([]);
    }
    value["schema_version"] = SCHEMA_VERSION.into();
    for (name, data) in value.as_object_mut().unwrap() {
        if name == "project" {
            data["header"]["schema_version"] = SCHEMA_VERSION.into();
        } else if name != "extensions"
            && let Some(map) = data.as_object_mut()
        {
            for entity in map.values_mut().filter(|v| v.get("header").is_some()) {
                entity["header"]["schema_version"] = SCHEMA_VERSION.into();
            }
        }
    }
}

#[test]
fn schema_64_elevation_dimensions_migrate_headers_only_and_reject_smuggled_references() {
    let frozen: Value =
        serde_json::from_str(include_str!("fixtures/schema-64-elevation-dimensions.json")).unwrap();
    let mut expected = frozen.clone();
    advance_headers(&mut expected);
    let mut migrated = frozen.clone();
    migrate(&mut migrated, 64).unwrap();
    assert_eq!(migrated, expected);
    for case in 0..4 {
        let mut bad = frozen.clone();
        bad["dimensions"]["00000000-0000-4000-8000-000000000099"] = json!({"parameters":{
            "layout":if case == 0 {"ElevationHorizontal"} else if case == 1 {"ElevationVertical"} else {"Aligned"},
            "first":if case == 2 {json!({"ElevationWall":{}})} else {Value::Null},
            "additional":if case == 3 {json!([{"ElevationOpening":{}}])} else {json!([])}
        }});
        let before = bad.clone();
        assert!(migrate(&mut bad, 64).is_err());
        assert_eq!(bad, before);
    }
}

#[test]
fn elevation_dimensions_save_reopen_live_and_orphan_without_cached_values() {
    let (mut doc, host, view) = document();
    let d = Dimension::new(
        "core.dimension",
        DimensionParams {
            layout: DimensionLayout::ElevationVertical,
            additional: vec![],
            baseline_spacing_m: 0.25,
            view,
            first: DimensionReference::ElevationWall {
                wall: host,
                feature: ElevationDimensionFeature::Bottom,
            },
            second: DimensionReference::ElevationWall {
                wall: host,
                feature: ElevationDimensionFeature::Top,
            },
            offset_m: 0.5,
            orphan_hint: Point2::new(4.5, 1.5),
        },
    );
    let id = d.id();
    doc.execute("Dimension", vec![Command::AddDimension(d.clone())])
        .unwrap();
    let dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
    let path = dir.path().join("dimension.osb");
    for orphan in [false, true] {
        if orphan {
            doc.execute("Delete host", vec![Command::RemoveWall(host)])
                .unwrap();
        }
        ZipJsonStorage.save(&doc, &path).unwrap();
        let reopened = ZipJsonStorage.open(&path).unwrap();
        assert_eq!(doc.model(), reopened.model());
        assert_eq!(reopened.model().dimensions[&id], d);
        assert_eq!(d.parameters.resolve(reopened.model()).is_ok(), !orphan);
        let v = serde_json::to_value(&d.parameters).unwrap();
        assert!(v.get("length_metres").is_none());
    }
}

#[test]
fn schema_63_elevation_migration_only_advances_headers_and_preserves_opaque_metadata() {
    let mut value = frozen();
    value["project"]["header"]["properties"]["opaque_elevation"] = json!({
        "header": {"schema_version": 17}, "parameters": {"kind":"Elevation", "elevation":null},
        "number": 123456789012345678901234567890_u128.to_string(),
    });
    let mut expected = value.clone();
    advance_headers(&mut expected);
    migrate(&mut value, 63).unwrap();
    assert_eq!(value, expected);
    let model: Model = serde_json::from_value(value).unwrap();
    model.validate().unwrap();
    assert!(
        model
            .views
            .values()
            .all(|v| v.parameters.elevation.is_none())
    );
}

#[test]
fn migration_rejects_legacy_elevation_fields_kinds_and_inconsistent_headers_atomically() {
    for case in 0..6 {
        let mut value = frozen();
        let view = value["views"]
            .as_object_mut()
            .unwrap()
            .values_mut()
            .next()
            .unwrap();
        match case {
            0 => view["parameters"]["elevation"] = Value::Null,
            1 => view["parameters"]["elevation"] = json!({}),
            2 => view["parameters"]["kind"] = "Elevation".into(),
            3 => view["header"]["schema_version"] = 62.into(),
            4 => value["project"]["header"]["schema_version"] = 64.into(),
            _ => {
                // Earlier migrations must not conceal an ambiguous new field.
                value =
                    serde_json::from_str(include_str!("fixtures/schema-62-arc-dimensions.json"))
                        .unwrap();
                value["views"]
                    .as_object_mut()
                    .unwrap()
                    .values_mut()
                    .next()
                    .unwrap()["parameters"]["elevation"] = Value::Null;
            }
        }
        let before = value.clone();
        let from = value["schema_version"].as_u64().unwrap() as u32;
        assert!(migrate(&mut value, from).is_err(), "{case}");
        assert_eq!(value, before);
    }
}

fn document() -> (Document, Id, Id) {
    let mut doc = Document::new("Elevations").unwrap();
    let level = *doc.model().levels.keys().next().unwrap();
    let wall = Wall::new(
        "org.openstructure.walls.wall",
        WallParams {
            name: "Host".into(),
            path: WallPath::Straight {
                start: Point2::new(0.0, 0.0),
                end: Point2::new(8.0, 0.0),
            },
            thickness: 0.2,
            height: 3.0,
            level,
            material: None,
        },
    );
    let host = wall.id();
    let view = View::new(
        "core.view",
        ViewParams {
            name: "Right elevation".into(),
            kind: ViewKind::Elevation,
            level: None,
            settings_revision: 0,
            plan: None,
            section: None,
            facade: None,
            elevation: Some(ElevationViewSettings::new(
                host,
                ElevationSide::Right,
                -0.5,
                9.0,
                -1.0,
                5.0,
            )),
        },
    );
    let view_id = view.id();
    doc.execute(
        "Create",
        vec![Command::AddWall(wall), Command::AddView(view)],
    )
    .unwrap();
    let mut sheet = Sheet::new("core.sheet", SheetParams::new("A1", "Elevation sheet"));
    sheet.parameters.viewports.push(SheetViewport {
        id: Id::new(),
        view: view_id,
        model_center_m: Point2::new(4.0, 2.0),
        paper_center_mm: Point2::new(210.0, 148.5),
        width_mm: 200.0,
        height_mm: 100.0,
        scale_denominator: 100.0,
        title_override: None,
    });
    doc.execute("Sheet", vec![Command::AddSheet(sheet)])
        .unwrap();
    (doc, host, view_id)
}

#[test]
fn elevation_and_sheet_save_reopen_live_curved_and_deleted_hosts_without_derived_payloads() {
    let (mut doc, host, view) = document();
    let authored = doc.model().views[&view].clone();
    let dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
    let path = dir.path().join("elevation.osb");
    for case in 0..3 {
        match case {
            1 => {
                let mut parameters = doc.model().walls[&host].parameters.clone();
                parameters.path = WallPath::CircularArc {
                    center: Point2::new(0.0, 0.0),
                    radius: 4.0,
                    start_angle_rad: 0.0,
                    signed_sweep_rad: 1.0,
                };
                doc.execute(
                    "Curve host",
                    vec![Command::UpdateWall {
                        id: host,
                        parameters,
                    }],
                )
                .unwrap();
            }
            2 => doc
                .execute("Delete host", vec![Command::RemoveWall(host)])
                .unwrap(),
            _ => {}
        }
        ZipJsonStorage.save(&doc, &path).unwrap();
        let reopened = ZipJsonStorage.open(&path).unwrap();
        assert_eq!(reopened.model(), doc.model());
        assert_eq!(reopened.model().views[&view], authored);
        assert_eq!(
            authored
                .parameters
                .elevation
                .unwrap()
                .resolve_host(reopened.model())
                .is_ok(),
            case == 0
        );
        let mut zip = zip::ZipArchive::new(std::fs::File::open(&path).unwrap()).unwrap();
        let mut manifest = String::new();
        std::io::Read::read_to_string(&mut zip.by_name("manifest.toml").unwrap(), &mut manifest)
            .unwrap();
        assert!(manifest.contains("container_version = 2"));
        assert!(manifest.contains(&format!("schema_version = {SCHEMA_VERSION}")));
        let payload = serde_json::to_value(reopened.model()).unwrap();
        assert_eq!(
            payload["views"][view.to_string()]["parameters"]["elevation"]
                .as_object()
                .unwrap()
                .len(),
            7
        );
    }
}

#[test]
fn malformed_current_elevation_settings_reject_before_adoption() {
    let (doc, _, view) = document();
    let original = serde_json::to_value(doc.model()).unwrap();
    for case in 0..8 {
        let mut value = original.clone();
        let parameters = &mut value["views"][view.to_string()]["parameters"];
        match case {
            0 => parameters["elevation"]["schema_version"] = 2.into(),
            1 => parameters["elevation"]["side"] = "Back".into(),
            2 => parameters["elevation"]["max_station"] = (-1.0).into(),
            3 => parameters["elevation"]["top_elevation"] = 1e20.into(),
            4 => parameters["elevation"]["extra"] = true.into(),
            5 => {
                parameters["elevation"]
                    .as_object_mut()
                    .unwrap()
                    .remove("host");
            }
            6 => parameters["elevation"] = Value::Null,
            _ => parameters["kind"] = "Perspective".into(),
        }
        let before = value.clone();
        assert!(migrate(&mut value, SCHEMA_VERSION).is_err(), "{case}");
        assert_eq!(value, before);
    }
}
