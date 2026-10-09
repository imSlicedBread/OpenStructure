use os_core::Point2;
use os_document::Document;
use os_model::*;
use os_storage::{StorageBackend, ZipJsonStorage, migrate};
use serde_json::Value;

fn frozen() -> Value {
    serde_json::from_str(include_str!("fixtures/schema-62-arc-dimensions.json")).unwrap()
}

#[test]
fn arc_length_schema_62_migration_only_advances_headers_and_preserves_opaque_data() {
    let mut value = frozen();
    let mut expected = value.clone();
    expected["schema_version"] = SCHEMA_VERSION.into();
    for (key, data) in expected.as_object_mut().unwrap() {
        if key == "project" {
            data["header"]["schema_version"] = SCHEMA_VERSION.into();
        } else if key != "extensions"
            && let Some(map) = data.as_object_mut()
        {
            for entity in map.values_mut().filter(|v| v.get("header").is_some()) {
                entity["header"]["schema_version"] = SCHEMA_VERSION.into();
            }
        }
    }
    // Schema 66→67 adds an empty authored-solid list and advances the native
    // opening-family version; all older payloads remain unchanged.
    for ty in expected["opening_types"]
        .as_object_mut()
        .unwrap()
        .values_mut()
    {
        ty["parameters"]["family"]["version"] = 7.into();
        ty["parameters"]["family"]["extrusions"] = serde_json::json!([]);
    }
    migrate(&mut value, 62).unwrap();
    assert_eq!(value, expected);
    let mut model: Model = serde_json::from_value(value).unwrap();
    model.validate().unwrap();
    let dim = model.dimensions.values().next().unwrap().clone();
    let wall = dim.parameters.first.entity();
    // Use a separate unhosted arc so existing fixture opening constraints remain intact.
    let mut host = model.walls[&wall].clone();
    host.header.id = os_core::Id::new();
    host.parameters.path = WallPath::CircularArc {
        center: Point2::new(0.0, 0.0),
        radius: 3.0,
        start_angle_rad: 0.0,
        signed_sweep_rad: -4.5,
    };
    let host_id = host.id();
    model.walls.insert(host_id, host);
    let p = &mut model.dimensions.get_mut(&dim.id()).unwrap().parameters;
    p.layout = DimensionLayout::ArcLength;
    p.additional.clear();
    p.baseline_spacing_m = 0.25;
    p.offset_m = 0.5;
    p.first = DimensionReference::WallEndpoint {
        wall: host_id,
        endpoint: DimensionEndpoint::Start,
    };
    p.second = DimensionReference::WallEndpoint {
        wall: host_id,
        endpoint: DimensionEndpoint::End,
    };
    let dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
    let path = dir.path().join("arc.osb");
    for orphan in [false, true] {
        if orphan {
            model.walls.remove(&host_id);
        }
        let doc = Document::from_model(model.clone()).unwrap();
        ZipJsonStorage.save(&doc, &path).unwrap();
        let reopened = ZipJsonStorage.open(&path).unwrap();
        assert_eq!(reopened.model(), doc.model());
        assert_eq!(
            reopened.model().dimensions[&dim.id()]
                .parameters
                .resolve_arc_length(reopened.model())
                .is_err(),
            orphan
        );
        let mut zip = zip::ZipArchive::new(std::fs::File::open(&path).unwrap()).unwrap();
        let mut manifest = String::new();
        std::io::Read::read_to_string(&mut zip.by_name("manifest.toml").unwrap(), &mut manifest)
            .unwrap();
        assert!(manifest.contains("container_version = 2"));
    }
}

#[test]
fn arc_length_migration_rejects_legacy_layout_and_bad_headers_atomically() {
    for case in 0..3 {
        let mut value = frozen();
        match case {
            0 => value["project"]["header"]["schema_version"] = 61.into(),
            1 => {
                value["dimensions"]
                    .as_object_mut()
                    .unwrap()
                    .values_mut()
                    .next()
                    .unwrap()["parameters"]["layout"] = "ArcLength".into()
            }
            _ => {
                value["walls"]
                    .as_object_mut()
                    .unwrap()
                    .values_mut()
                    .next()
                    .unwrap()["header"]["schema_version"] = 63.into()
            }
        }
        let before = value.clone();
        assert!(migrate(&mut value, 62).is_err());
        assert_eq!(value, before);
    }
}
