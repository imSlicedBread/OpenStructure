use os_core::Id;
use os_model::*;
use serde_json::json;

fn model() -> Model {
    let mut model = Model::new("Opaque semantics");
    let e: ExtensionEntity =
        serde_json::from_str(include_str!("../../../fixtures/extension-envelope-v1.json")).unwrap();
    model.plugin_requirements.insert(
        e.owner.clone(),
        PluginRequirement {
            version: "1.0.0".into(),
        },
    );
    model.extensions.insert(e.id, e);
    model
}

#[test]
fn opaque_unknown_payload_round_trips_without_runtime() {
    let mut model = model();
    model
        .extensions
        .values_mut()
        .next()
        .unwrap()
        .payload_schema_version = 999;
    model.validate().unwrap();
    assert_eq!(
        model,
        serde_json::from_slice::<Model>(&serde_json::to_vec(&model).unwrap()).unwrap()
    );
    let mut value = serde_json::to_value(&model).unwrap();
    let id = model.extensions.keys().next().unwrap().to_string();
    value["extensions"][&id]["future_envelope_field"] = json!(true);
    assert!(serde_json::from_value::<Model>(value).is_err());
}

#[test]
fn invalid_envelopes_requirements_and_graphs_are_rejected() {
    for case in [
        "id",
        "duplicate",
        "owner",
        "type",
        "envelope",
        "schema",
        "name",
        "reference",
        "cycle",
        "version",
    ] {
        let mut model = model();
        let e = model.extensions.values_mut().next().unwrap();
        match case {
            "id" => e.id = Id::new(),
            "duplicate" => {
                let mut e = e.clone();
                e.id = model.project.id();
                model.extensions.clear();
                model.extensions.insert(e.id, e);
            }
            "owner" => e.owner = "org.other.owner".into(),
            "type" => e.type_id = "org.example.columns-other.rectangular".into(),
            "envelope" => e.envelope_version = 2,
            "schema" => e.payload_schema_version = 0,
            "name" => e.name.clear(),
            "reference" => {
                e.depends_on.insert(Id::new());
            }
            "cycle" => {
                e.depends_on.insert(e.id);
            }
            "version" => {
                model
                    .plugin_requirements
                    .values_mut()
                    .next()
                    .unwrap()
                    .version = "latest".into()
            }
            _ => unreachable!(),
        }
        assert!(model.validate().is_err(), "accepted {case}");
    }
}

#[test]
fn ordinary_reference_cycles_are_preserved_but_prerequisite_cycles_fail() {
    let mut model = model();
    let mut second = model.extensions.values().next().unwrap().clone();
    second.id = Id::new();
    let first = model.extensions.values_mut().next().unwrap();
    let first_id = first.id;
    first.relationships.insert("peer".into(), vec![second.id]);
    second.relationships.insert("peer".into(), vec![first_id]);
    let second_id = second.id;
    model.extensions.insert(second.id, second);
    model.validate().unwrap();
    model
        .extensions
        .get_mut(&first_id)
        .unwrap()
        .depends_on
        .insert(second_id);
    model
        .extensions
        .get_mut(&second_id)
        .unwrap()
        .depends_on
        .insert(first_id);
    assert!(model.validate().is_err());
}

#[test]
fn bounds_cover_serialized_envelopes_aggregate_count_and_nesting() {
    let mut m = model();
    m.extensions.values_mut().next().unwrap().payload = json!("x".repeat(MAX_EXTENSION_BYTES));
    assert!(m.validate().is_err());
    let mut m = model();
    let mut deep = json!(0);
    for _ in 0..33 {
        deep = json!([deep]);
    }
    m.extensions.values_mut().next().unwrap().payload = deep;
    assert!(m.validate().is_err());
    let mut m = model();
    let mut e = m.extensions.values().next().unwrap().clone();
    e.payload = json!("x".repeat(MAX_EXTENSION_BYTES - 2048));
    m.extensions.clear();
    for _ in 0..17 {
        e.id = Id::new();
        m.extensions.insert(e.id, e.clone());
    }
    assert!(m.validate().is_err());
    let mut m = model();
    let mut e = m.extensions.values().next().unwrap().clone();
    for _ in 0..MAX_EXTENSIONS {
        e.id = Id::new();
        m.extensions.insert(e.id, e.clone());
    }
    assert!(m.validate().is_err());
}
