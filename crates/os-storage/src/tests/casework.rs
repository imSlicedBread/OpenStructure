use super::*;
use os_core::Point2;
use os_document::Command;
use os_model::{Casework, CaseworkParams, CaseworkType, CaseworkTypeParams};
use serde_json::{Value, json};

fn schema_61_without_casework() -> Value {
    let mut value = serde_json::to_value(Model::new("Schema 61")).unwrap();
    let object = value.as_object_mut().unwrap();
    object.remove("casework_types");
    object.remove("casework");
    object.insert("schema_version".into(), json!(61));
    fn set_headers(value: &mut Value) {
        match value {
            Value::Object(object) => {
                if let Some(header) = object.get_mut("header") {
                    header["schema_version"] = json!(61);
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
fn schema_61_migration_adds_casework_maps_and_advances_headers_atomically() {
    let old = schema_61_without_casework();
    let mut migrated = old.clone();
    migrate(&mut migrated, 61).unwrap();
    let model: Model = serde_json::from_value(migrated.clone()).unwrap();
    model.validate().unwrap();
    assert_eq!(model.schema_version, SCHEMA_VERSION);
    assert!(model.casework_types.is_empty());
    assert!(model.casework.is_empty());
    assert_eq!(migrated["casework_types"], json!({}));
    assert_eq!(migrated["casework"], json!({}));
    assert_eq!(
        migrated["project"]["header"]["schema_version"],
        json!(SCHEMA_VERSION)
    );

    let mut ambiguous = old;
    ambiguous["casework"] = json!({});
    let before = ambiguous.clone();
    assert!(migrate(&mut ambiguous, 61).is_err());
    assert_eq!(ambiguous, before);
}

#[test]
fn shared_casework_type_and_instances_survive_save_and_reopen() {
    let mut model = Model::new("Casework persistence");
    let level = *model.levels.keys().next().unwrap();
    let casework_type = CaseworkType::new(
        "core.casework_type",
        CaseworkTypeParams {
            name: "Base cabinet 600".into(),
            width: 0.6,
            depth: 0.6,
            height: 0.9,
            material: None,
        },
    );
    let type_id = casework_type.id();
    let mut document = Document::from_model(model.clone()).unwrap();
    document
        .execute(
            "Create casework type and instance",
            vec![
                Command::AddCaseworkType(casework_type.clone()),
                Command::AddCasework(Casework::new(
                    "core.casework",
                    CaseworkParams {
                        name: "Kitchen base cabinet".into(),
                        type_id,
                        level,
                        center: Point2::new(2.0, 3.0),
                        yaw: std::f64::consts::FRAC_PI_4,
                        base_offset: 0.0,
                    },
                )),
            ],
        )
        .unwrap();
    model = document.model().clone();
    model.validate().unwrap();
    let instance_id = *model.casework.keys().next().unwrap();

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("casework.osb");
    ZipJsonStorage.save(&document, &path).unwrap();
    let reopened = ZipJsonStorage.open(&path).unwrap();
    assert_eq!(reopened.model().casework_types[&type_id], casework_type);
    assert_eq!(
        reopened.model().casework[&instance_id],
        model.casework[&instance_id]
    );
    assert_eq!(reopened.model(), &model);
    assert!(!reopened.can_undo());
}
