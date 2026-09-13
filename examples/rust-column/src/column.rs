#![forbid(unsafe_code)]
use os_plugin_api::generic::*;
use serde_json::Value;
use std::collections::BTreeMap;

const OWNER: &str = "org.example.columns";
const TYPE: &str = "org.example.columns.rectangular";
const BLOCK: &str = "org.example.columns.test-block";
const SCHEMA: u32 = if cfg!(feature = "migration-v2") { 2 } else { 1 };
const HEIGHT: &str = if cfg!(feature = "migration-v2") {
    "height_m"
} else {
    "height"
};

fn failure(message: &str) -> PluginError {
    PluginError {
        code: ErrorCode::InvalidInput,
        message: message.into(),
        field: None,
    }
}

fn catalog() -> Catalog {
    let fields = [
        ("width", "Width", 100.0, 0.4),
        ("depth", "Depth", 100.0, 0.6),
        (HEIGHT, "Height", 1000.0, 3.0),
    ]
    .into_iter()
    .map(|(key, label, max, default)| Field {
        key: key.into(),
        label: label.into(),
        kind: FieldKind::Number {
            unit: Unit::Metres,
            min: 0.001,
            max,
            default,
        },
    })
    .collect::<Vec<_>>();
    let mut catalog = Catalog {
        types: vec![ElementType {
            id: TYPE.into(),
            payload_schema_version: SCHEMA,
            fields: fields.clone(),
        }],
        commands: [
            ("create", Mode::Create),
            ("edit", Mode::Edit),
            ("delete", Mode::Delete),
            ("migrate", Mode::Migrate),
        ]
        .into_iter()
        .filter(|(_, mode)| *mode != Mode::Migrate || SCHEMA == 2)
        .map(|(name, mode)| CommandDescriptor {
            id: format!("{OWNER}.{name}"),
            element_type: TYPE.into(),
            mode,
            fields: if matches!(mode, Mode::Delete | Mode::Migrate) {
                vec![]
            } else {
                fields.clone()
            },
            enabled: true,
            disabled_reason: None,
        })
        .collect(),
    };
    if cfg!(feature = "mixed-migration") {
        let mut block = catalog.types[0].clone();
        block.id = BLOCK.into();
        block.fields[0].key = "width_m".into();
        catalog.types.push(block);
        catalog.commands.push(CommandDescriptor {
            id: format!("{OWNER}.migrate-block"),
            element_type: BLOCK.into(),
            mode: Mode::Migrate,
            fields: vec![],
            enabled: true,
            disabled_reason: None,
        });
    }
    catalog
}

pub(super) fn invoke(request: &Request) -> Result<Reply, PluginError> {
    let catalog = catalog();
    if request.api_version != VERSION {
        return Err(failure("Unsupported API version"));
    }
    match &request.operation {
        Operation::GenerateGeometry {
            element_id,
            snapshot,
        } => {
            if request.context.is_none() {
                return Err(failure("Missing geometry context"));
            }
            let element = snapshot
                .elements
                .iter()
                .find(|e| e.id == *element_id)
                .ok_or_else(|| failure("Missing geometry target"))?;
            if element.owner != OWNER
                || (element.type_id != TYPE
                    && !(cfg!(feature = "mixed-migration") && element.type_id == BLOCK))
                || element.payload_schema_version != SCHEMA
            {
                return Err(failure("Unsupported geometry type or schema"));
            }
            let values = element
                .payload
                .as_object()
                .ok_or_else(|| failure("Invalid geometry payload"))?;
            let values = values.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            let kind = catalog
                .types
                .iter()
                .find(|kind| kind.id == element.type_id)
                .unwrap();
            validate_values(&kind.fields, &values, true)
                .map_err(|_| failure("Invalid geometry dimensions"))?;
            if element.depends_on.len() != 1 {
                return Err(failure("Column needs one level"));
            }
            let level = snapshot
                .levels
                .iter()
                .find(|l| element.depends_on.contains(&l.id))
                .ok_or_else(|| failure("Level not in scoped snapshot"))?;
            let recipe = os_plugin_api::geometry::Recipe::RectangularPrism(
                os_plugin_api::geometry::RectangularPrism {
                    width: element.payload[if element.type_id == BLOCK {
                        "width_m"
                    } else {
                        "width"
                    }]
                    .as_f64()
                    .unwrap(),
                    depth: element.payload["depth"].as_f64().unwrap(),
                    height: element.payload[HEIGHT].as_f64().unwrap(),
                    translation: [0.0, 0.0, level.elevation_metres],
                    rotation_z: 0.0,
                },
            );
            recipe
                .validate()
                .map_err(|_| failure("Invalid column placement"))?;
            Ok(Reply::Geometry {
                element_id: element_id.clone(),
                recipe,
            })
        }
        Operation::Describe => {
            if request.context.is_some() {
                return Err(failure("Describe must be document-free"));
            }
            Ok(Reply::Catalog(catalog))
        }
        Operation::Invoke {
            command_id,
            inputs,
            snapshot,
            selection,
            new_ids,
        } => {
            if request.context.is_none() {
                return Err(failure("Missing document context"));
            }
            let descriptor = catalog
                .commands
                .iter()
                .find(|c| c.id == *command_id)
                .ok_or_else(|| failure("Unknown command"))?;
            validate_values(&descriptor.fields, inputs, false)
                .map_err(|_| failure("Invalid dimensions or inputs"))?;
            if snapshot.elements.len() + snapshot.levels.len() > MAX_SCOPE {
                return Err(failure("Snapshot exceeds scope limit"));
            }
            let edit = match descriptor.mode {
                Mode::Migrate => {
                    if SCHEMA != 2
                        || selection.is_empty()
                        || selection.len() > MAX_EDITS
                        || !new_ids.is_empty()
                    {
                        return Err(failure("Invalid migration scope"));
                    }
                    let mut edits = Vec::new();
                    for id in selection {
                        let mut element = snapshot
                            .elements
                            .iter()
                            .find(|e| e.id == *id)
                            .ok_or_else(|| failure("Missing migration target"))?
                            .clone();
                        if element.owner != OWNER
                            || element.type_id != descriptor.element_type
                            || element.payload_schema_version != 1
                        {
                            return Err(failure("Migration supports column schema 1 only"));
                        }
                        let payload = element
                            .payload
                            .as_object_mut()
                            .ok_or_else(|| failure("Invalid migration payload"))?;
                        if payload.contains_key("height_m") {
                            return Err(failure(
                                "Existing height_m vendor key conflicts with migration",
                            ));
                        }
                        let height = payload
                            .remove("height")
                            .ok_or_else(|| failure("Missing schema-1 height"))?;
                        payload.insert("height_m".into(), height);
                        if element.type_id == BLOCK {
                            if payload.contains_key("width_m") {
                                return Err(failure(
                                    "Existing width_m vendor key conflicts with block migration",
                                ));
                            }
                            let width = payload
                                .remove("width")
                                .ok_or_else(|| failure("Missing schema-1 block width"))?;
                            payload.insert("width_m".into(), width);
                        }
                        let values = payload
                            .iter()
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect();
                        let kind = catalog
                            .types
                            .iter()
                            .find(|kind| kind.id == descriptor.element_type)
                            .unwrap();
                        validate_values(&kind.fields, &values, true)
                            .map_err(|_| failure("Migrated dimensions invalid"))?;
                        element.payload_schema_version = 2;
                        edits.push(Edit::Replace {
                            expected_schema_version: 1,
                            element,
                        });
                    }
                    return Ok(Reply::Edits(edits));
                }
                Mode::Create => {
                    if new_ids.len() != 1 || !selection.is_empty() || snapshot.levels.len() != 1 {
                        return Err(failure("Create needs one reserved identity and one level"));
                    }
                    Edit::Create(Element {
                        id: new_ids[0].clone(),
                        owner: OWNER.into(),
                        type_id: TYPE.into(),
                        name: "Rectangular column".into(),
                        payload_schema_version: SCHEMA,
                        payload: Value::Object(
                            inputs.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
                        ),
                        relationships: BTreeMap::new(),
                        depends_on: [snapshot.levels[0].id.clone()].into(),
                    })
                }
                Mode::Edit | Mode::Delete => {
                    if selection.len() != 1 || !new_ids.is_empty() {
                        return Err(failure("Select one existing column"));
                    }
                    let mut element = snapshot
                        .elements
                        .iter()
                        .find(|e| e.id == selection[0])
                        .ok_or_else(|| failure("Selected column is missing"))?
                        .clone();
                    if element.owner != OWNER
                        || element.type_id != TYPE
                        || element.payload_schema_version != SCHEMA
                    {
                        return Err(failure("Selected column requires another owner or schema"));
                    }
                    if descriptor.mode == Mode::Delete {
                        Edit::Delete { id: element.id }
                    } else {
                        let payload = element
                            .payload
                            .as_object_mut()
                            .ok_or_else(|| failure("Invalid column payload"))?;
                        // Preserve opaque vendor data; edit only declared fields.
                        for (key, value) in inputs {
                            payload.insert(key.clone(), value.clone());
                        }
                        Edit::Replace {
                            expected_schema_version: SCHEMA,
                            element,
                        }
                    }
                }
            };
            Ok(Reply::Edits(vec![edit]))
        }
    }
}

/// Bounded UTF-8 JSON entry point shared by native tests and the Wasm adapter.
/// Unparseable input has no trustworthy correlation and returns no envelope.
pub fn respond(input: &[u8]) -> Option<Vec<u8>> {
    if input.len() > MAX_BYTES {
        return None;
    }
    #[cfg(feature = "plan-graphics")]
    if let Ok(request) = serde_json::from_slice::<os_plugin_api::plan::Request>(input) {
        return crate::plan_graphics::respond(request);
    }
    let request: Request = serde_json::from_slice(input).ok()?;
    let response = Response {
        api_version: VERSION,
        request_id: request.request_id.clone(),
        context: request.context.clone(),
        result: invoke(&request).unwrap_or_else(Reply::Error),
    };
    let output = serde_json::to_vec(&response).ok()?;
    (output.len() <= MAX_BYTES).then_some(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use os_plugin_api::Manifest;
    use serde_json::json;
    #[cfg(feature = "mixed-migration")]
    #[test]
    fn block_migration_is_distinct_and_rejects_vendor_collisions() {
        let mut request = Request {
            api_version: VERSION,
            request_id: "mixed".into(),
            context: Some(Stamp {
                project: "p".into(),
                session: "s".into(),
                revision: 0,
            }),
            operation: Operation::Invoke {
                command_id: format!("{OWNER}.migrate-block"),
                inputs: BTreeMap::new(),
                selection: vec!["block".into()],
                new_ids: vec![],
                snapshot: Snapshot {
                    levels: vec![],
                    elements: vec![Element {
                        id: "block".into(),
                        owner: OWNER.into(),
                        type_id: BLOCK.into(),
                        name: "Block".into(),
                        payload_schema_version: 1,
                        payload: json!({"width":0.4,"depth":0.6,"height":3.0,"vendor":{"note":"retained"}}),
                        relationships: BTreeMap::new(),
                        depends_on: Default::default(),
                    }],
                },
            },
        };
        let Reply::Edits(edits) = invoke(&request).unwrap() else {
            panic!("expected edits")
        };
        let Edit::Replace {
            element,
            expected_schema_version,
        } = &edits[0]
        else {
            panic!("expected replacement")
        };
        assert_eq!(*expected_schema_version, 1);
        assert_eq!(element.payload_schema_version, 2);
        assert_eq!(element.payload["width_m"], json!(0.4));
        assert_eq!(element.payload["height_m"], json!(3.0));
        assert_eq!(element.payload["vendor"], json!({"note":"retained"}));
        assert!(element.payload.get("width").is_none());
        if let Operation::Invoke { command_id, .. } = &mut request.operation {
            *command_id = format!("{OWNER}.migrate");
        }
        assert!(invoke(&request).is_err());
        if let Operation::Invoke {
            command_id,
            snapshot,
            ..
        } = &mut request.operation
        {
            *command_id = format!("{OWNER}.migrate-block");
            snapshot.elements[0].payload["width_m"] = json!("vendor conflict");
        }
        assert!(invoke(&request).is_err());
    }
    #[test]
    fn descriptors_match_manifest_and_inputs_are_typed() {
        catalog()
            .validate(
                &Manifest::from_toml(if cfg!(feature = "mixed-migration") {
                    include_str!("../plugin-mixed.toml")
                } else if cfg!(feature = "migration-v2") {
                    include_str!("../plugin-v2.toml")
                } else {
                    include_str!("../plugin.toml")
                })
                .unwrap(),
            )
            .unwrap();
        let fields = &catalog().commands[0].fields;
        assert!(
            validate_values(
                fields,
                &BTreeMap::from([
                    ("width".into(), json!(0.4)),
                    ("depth".into(), json!(0.6)),
                    (HEIGHT.into(), json!(3.0))
                ]),
                false
            )
            .is_ok()
        );
        assert!(validate_values(fields, &BTreeMap::new(), false).is_err());
    }
    #[test]
    fn semantic_lifecycle_preserves_unknown_payload_and_rejects_bad_selection() {
        let dimensions = BTreeMap::from([
            ("width".into(), json!(0.4)),
            ("depth".into(), json!(0.6)),
            (HEIGHT.into(), json!(3.0)),
        ]);
        let mut request = Request {
            api_version: VERSION,
            request_id: "request".into(),
            context: Some(Stamp {
                project: "project".into(),
                session: "session".into(),
                revision: 0,
            }),
            operation: Operation::Invoke {
                command_id: format!("{OWNER}.create"),
                inputs: dimensions.clone(),
                snapshot: Snapshot {
                    elements: vec![],
                    levels: vec![Level {
                        id: "level".into(),
                        name: "Level".into(),
                        elevation_metres: 0.0,
                    }],
                },
                selection: vec![],
                new_ids: vec!["reserved".into()],
            },
        };
        let Reply::Edits(edits) = invoke(&request).unwrap() else {
            panic!("missing create");
        };
        let Edit::Create(mut element) = edits.into_iter().next().unwrap() else {
            panic!("wrong edit");
        };
        assert_eq!(element.id, "reserved");
        assert!(element.depends_on.contains("level"));
        element.payload["vendor"] = json!({"unknown": [1, "keep"]});
        let mut changed = dimensions;
        changed.insert("width".into(), json!(0.9));
        request.operation = Operation::Invoke {
            command_id: format!("{OWNER}.edit"),
            inputs: changed,
            snapshot: Snapshot {
                elements: vec![element.clone()],
                levels: vec![],
            },
            selection: vec![element.id.clone()],
            new_ids: vec![],
        };
        let Reply::Edits(edits) = invoke(&request).unwrap() else {
            panic!("missing edit");
        };
        let Edit::Replace {
            element: edited, ..
        } = &edits[0]
        else {
            panic!("wrong edit");
        };
        assert_eq!(edited.id, element.id);
        assert_eq!(edited.payload["width"], json!(0.9));
        assert_eq!(edited.payload["vendor"], element.payload["vendor"]);
        if let Operation::Invoke { selection, .. } = &mut request.operation {
            selection[0] = "outside".into();
        }
        assert!(invoke(&request).is_err());
        request.operation = Operation::Invoke {
            command_id: format!("{OWNER}.delete"),
            inputs: BTreeMap::new(),
            snapshot: Snapshot {
                elements: vec![element.clone()],
                levels: vec![],
            },
            selection: vec![element.id.clone()],
            new_ids: vec![],
        };
        let Reply::Edits(edits) = invoke(&request).unwrap() else {
            panic!("missing delete");
        };
        assert!(matches!(&edits[0], Edit::Delete { id } if *id == element.id));
    }

    #[test]
    fn bounded_dispatch_and_correlation() {
        assert!(respond(b"bad JSON").is_none());
        assert!(respond(&vec![b' '; MAX_BYTES + 1]).is_none());
        let request = Request {
            api_version: VERSION,
            request_id: "nonce".into(),
            context: None,
            operation: Operation::Describe,
        };
        let reply: Response =
            serde_json::from_slice(&respond(&serde_json::to_vec(&request).unwrap()).unwrap())
                .unwrap();
        assert_eq!(reply.request_id, "nonce");
        assert!(matches!(reply.result, Reply::Catalog(_)));
    }
}
