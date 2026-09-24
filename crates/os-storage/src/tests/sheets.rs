use super::*;
use os_core::Point2;
use os_document::Command;
use os_model::{Sheet, SheetParams, SheetViewport, ViewKind};
use serde_json::{Value, json};

const NATIVE_MAPS: &[&str] = &[
    "sites",
    "buildings",
    "levels",
    "walls",
    "floors",
    "openings",
    "opening_types",
    "rooms",
    "room_tags",
    "dimensions",
    "grids",
    "materials",
    "views",
];

fn frozen_thirteen() -> Value {
    serde_json::from_str(include_str!("schema-13-sheets.json")).unwrap()
}

fn write_archive(path: &Path, value: &Value) {
    let manifest = Manifest {
        format: "OpenStructure".into(),
        container_version: CONTAINER_VERSION,
        schema_version: value["schema_version"].as_u64().unwrap() as u32,
        project_id: serde_json::from_value(value["project"]["header"]["id"].clone()).unwrap(),
        model_entry: "model.json".into(),
        units: "metres".into(),
    };
    let mut zip = ZipWriter::new(File::create(path).unwrap());
    zip.start_file("manifest.toml", SimpleFileOptions::default())
        .unwrap();
    zip.write_all(toml::to_string(&manifest).unwrap().as_bytes())
        .unwrap();
    zip.start_file("model.json", SimpleFileOptions::default())
        .unwrap();
    zip.write_all(&serde_json::to_vec(value).unwrap()).unwrap();
    zip.start_file("assets/keep.bin", SimpleFileOptions::default())
        .unwrap();
    zip.write_all(b"opaque bytes").unwrap();
    zip.finish().unwrap();
}

#[test]
fn sheets_schema_thirteen_migration_preserves_native_data_through_schema_nineteen() {
    let old = frozen_thirteen();
    let mut value = old.clone();
    // Test the frozen step directly so later schema bumps cannot weaken it.
    migrate_inner(&mut value, 13).unwrap();
    let mut expected = old.clone();
    expected["schema_version"] = json!(SCHEMA_VERSION);
    expected["schedules"] = json!({});
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
    for map in NATIVE_MAPS {
        for entity in expected[*map].as_object_mut().unwrap().values_mut() {
            assert_eq!(entity["header"]["schema_version"], 13);
            entity["header"]["schema_version"] = json!(SCHEMA_VERSION);
        }
    }
    assert_eq!(
        value, expected,
        "preserve every prior ID, parameter, metadata and opaque payload"
    );
    let model: Model = serde_json::from_value(value).unwrap();
    model.validate().unwrap();
    let mut public = old;
    migrate(&mut public, 13).unwrap();
    assert_eq!(public, expected);
}

#[test]
fn sheets_migration_rejects_ambiguous_or_corrupt_inputs_atomically() {
    let old = frozen_thirteen();
    let mut cases = Vec::new();
    let mut ambiguous = old.clone();
    ambiguous["sheets"] = json!({});
    cases.push(ambiguous);
    let mut wrong = old.clone();
    wrong["project"]["header"]["schema_version"] = json!(12);
    cases.push(wrong);
    for map in NATIVE_MAPS {
        let mut missing = old.clone();
        missing.as_object_mut().unwrap().remove(*map);
        cases.push(missing);
        let mut bad_map = old.clone();
        bad_map[*map] = json!([]);
        cases.push(bad_map);
        for id in old[*map].as_object().unwrap().keys() {
            let mut wrong = old.clone();
            wrong[*map][id]["header"]["schema_version"] = json!(12);
            cases.push(wrong);
        }
    }
    for mut bad in cases {
        let before = bad.clone();
        assert!(migrate(&mut bad, 13).is_err());
        assert_eq!(bad, before);
    }
    let mut missing_new_map = old;
    migrate(&mut missing_new_map, 13).unwrap();
    missing_new_map.as_object_mut().unwrap().remove("sheets");
    assert!(serde_json::from_value::<Model>(missing_new_map).is_err());
}

#[test]
fn sheets_real_archive_migration_and_save_reopen_preserve_centers_ids_and_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("schema13.osb");
    write_archive(&source, &frozen_thirteen());
    let bytes = std::fs::read(&source).unwrap();
    let mut doc = ZipJsonStorage.open(&source).unwrap();
    assert_eq!(std::fs::read(&source).unwrap(), bytes);
    let view = doc
        .model()
        .views
        .values()
        .find(|v| v.parameters.kind == ViewKind::Plan)
        .unwrap()
        .id();
    let mut view_parameters = doc.model().views[&view].parameters.clone();
    view_parameters.plan.as_mut().unwrap().basis.rotation = 0.7;
    view_parameters.plan.as_mut().unwrap().basis.origin = Point2::new(12.0, 19.0);
    let mut params = SheetParams::new("A101", "Ground plan sheet");
    for (scale, center, title) in [
        (50.0, Point2::new(30.0, -17.0), Some("Detail".into())),
        (100.0, Point2::new(-7.0, 12.0), None),
    ] {
        params.viewports.push(SheetViewport {
            id: Id::new(),
            view,
            model_center_m: center,
            paper_center_mm: Point2::new(210.0, 148.5),
            width_mm: 300.0,
            height_mm: 200.0,
            scale_denominator: scale,
            title_override: title,
        });
    }
    let mut sheet = Sheet::new("core.sheet", params);
    sheet
        .header
        .properties
        .insert("issue_note".into(), "Preserved".into());
    sheet
        .header
        .relationships
        .insert("source".into(), vec![view]);
    doc.execute(
        "Sheet placement",
        vec![
            Command::UpdateView {
                id: view,
                parameters: view_parameters,
            },
            Command::AddSheet(sheet.clone()),
        ],
    )
    .unwrap();
    let placed = doc.model().clone();
    assert!(doc.undo());
    assert!(doc.model().sheets.is_empty());
    assert!(doc.redo());
    assert_eq!(doc.model(), &placed);
    let target = dir.path().join("sheet.osb");
    ZipJsonStorage.save(&doc, &target).unwrap();
    let reopened = ZipJsonStorage.open(&target).unwrap();
    assert_eq!(reopened.model(), doc.model());
    assert_eq!(reopened.model().sheets[&sheet.id()], sheet);
    assert_eq!(
        reopened.auxiliary_files()["assets/keep.bin"],
        b"opaque bytes"
    );
    assert!(!reopened.can_undo());
    let mut zip = ZipArchive::new(File::open(&target).unwrap()).unwrap();
    let manifest: Manifest =
        toml::from_str(&read_entry(&mut zip, "manifest.toml", MAX_MANIFEST_BYTES).unwrap())
            .unwrap();
    assert_eq!(manifest.schema_version, SCHEMA_VERSION);
    assert_eq!(manifest.units, "metres");
    assert_eq!(std::fs::read(&source).unwrap(), bytes);

    let valid = serde_json::to_value(doc.model()).unwrap();
    for case in 0..5 {
        let mut bad = valid.clone();
        let viewport = &mut bad["sheets"][sheet.id().to_string()]["parameters"]["viewports"][0];
        match case {
            0 => viewport["view"] = json!(Id::new()),
            1 => viewport["id"] = json!(view),
            2 => viewport["model_center_m"]["x"] = json!(1e7),
            3 => viewport["width_mm"] = json!(-1),
            _ => viewport["scale_denominator"] = json!(0),
        }
        let path = dir.path().join(format!("invalid-{case}.osb"));
        write_archive(&path, &bad);
        assert!(ZipJsonStorage.open(&path).is_err(), "case {case}");
    }
}
