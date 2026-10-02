use super::*;
use os_core::Point2;
use os_document::Command;
use os_model::{Level, LevelParams, Material, MaterialParams, Ramp, RampParams};
use serde_json::{Value, json};

fn schema_60_without_ramps() -> Value {
    let mut value = serde_json::to_value(Model::new("Schema 60")).unwrap();
    let object = value.as_object_mut().unwrap();
    object.remove("ramps");
    object.remove("casework_types");
    object.remove("casework");
    object.insert("schema_version".into(), json!(60));
    fn set_headers(value: &mut Value) {
        match value {
            Value::Object(object) => {
                if let Some(header) = object.get_mut("header") {
                    header["schema_version"] = json!(60);
                } else {
                    for value in object.values_mut() {
                        set_headers(value);
                    }
                }
            }
            Value::Array(values) => values.iter_mut().for_each(set_headers),
            _ => {}
        }
    }
    set_headers(&mut value);
    value
}

#[test]
fn schema_60_migration_adds_ramps_and_advances_native_headers_atomically() {
    let old = schema_60_without_ramps();
    let mut migrated = old.clone();
    migrate(&mut migrated, 60).unwrap();
    let model: Model = serde_json::from_value(migrated.clone()).unwrap();
    model.validate().unwrap();
    assert_eq!(model.schema_version, SCHEMA_VERSION);
    assert!(model.ramps.is_empty());
    assert_eq!(migrated["ramps"], json!({}));
    assert_eq!(
        migrated["project"]["header"]["schema_version"],
        json!(SCHEMA_VERSION)
    );

    let mut ambiguous = old;
    ambiguous["ramps"] = json!({});
    let before = ambiguous.clone();
    assert!(migrate(&mut ambiguous, 60).is_err());
    assert_eq!(ambiguous, before);
}

#[test]
fn native_ramp_save_reopen_preserves_parameters_levels_material_and_identity() {
    let mut model = Model::new("Ramp persistence");
    let lower = *model.levels.keys().next().unwrap();
    let building = model.levels[&lower].parameters.building;
    let upper = Level::new(
        "core.level",
        LevelParams {
            name: "Upper".into(),
            elevation: 1.25,
            building,
        },
    );
    let upper_id = upper.id();
    model.levels.insert(upper_id, upper);
    let material = Material::new(
        "core.material",
        MaterialParams {
            name: "Concrete".into(),
            density_kg_m3: 2400.0,
            color: [148, 152, 160],
        },
    );
    let material_id = material.id();
    model.materials.insert(material_id, material);
    let ramp = Ramp::new(
        "core.ramp",
        RampParams {
            name: "Entry ramp".into(),
            lower_level: lower,
            upper_level: upper_id,
            start: Point2::new(1.0, 2.0),
            end: Point2::new(7.0, 4.0),
            width: 1.5,
            structural_thickness: 0.18,
            material: Some(material_id),
        },
    );
    let ramp_id = ramp.id();
    let mut document = Document::from_model(model).unwrap();
    document
        .execute("Place ramp", vec![Command::AddRamp(ramp.clone())])
        .unwrap();

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("ramp.osb");
    ZipJsonStorage.save(&document, &path).unwrap();
    let reopened = ZipJsonStorage.open(&path).unwrap();
    assert_eq!(reopened.model().ramps[&ramp_id], ramp);
    assert_eq!(reopened.model(), document.model());
    assert!(!reopened.can_undo());
}
