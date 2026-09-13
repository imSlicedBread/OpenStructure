#![forbid(unsafe_code)]
use os_plugin_api::{
    generic::*,
    geometry::{Recipe, RectangularPrism},
    wall::*,
};

fn error(message: &str) -> PluginError {
    PluginError {
        code: ErrorCode::InvalidInput,
        message: message.into(),
        field: None,
    }
}
fn catalog() -> Catalog {
    let fields = [
        ("start_x", "Start X", -1e6, 1e6, 0.0),
        ("start_y", "Start Y", -1e6, 1e6, 0.0),
        ("end_x", "End X", -1e6, 1e6, 5.0),
        ("end_y", "End Y", -1e6, 1e6, 0.0),
        ("thickness", "Thickness", 0.001, 100.0, 0.2),
        ("height", "Height", 0.001, 1000.0, 3.0),
    ]
    .into_iter()
    .map(|(key, label, min, max, default)| Field {
        key: key.into(),
        label: label.into(),
        kind: FieldKind::Number {
            unit: Unit::Metres,
            min,
            max,
            default,
        },
    })
    .collect::<Vec<_>>();
    Catalog {
        types: vec![ElementType {
            id: TYPE.into(),
            payload_schema_version: PAYLOAD_VERSION,
            fields: fields.clone(),
        }],
        commands: [
            ("create", Mode::Create),
            ("edit", Mode::Edit),
            ("delete", Mode::Delete),
        ]
        .into_iter()
        .map(|(name, mode)| CommandDescriptor {
            id: format!("{OWNER}.{name}"),
            element_type: TYPE.into(),
            mode,
            fields: if mode == Mode::Delete {
                vec![]
            } else {
                fields.clone()
            },
            enabled: true,
            disabled_reason: None,
        })
        .collect(),
    }
}
fn target<'a>(snapshot: &'a Snapshot, id: &str) -> Result<&'a Element, PluginError> {
    let e = snapshot
        .elements
        .iter()
        .find(|e| e.id == id)
        .ok_or_else(|| error("Wall missing from scope"))?;
    if e.owner != OWNER || e.type_id != TYPE || e.payload_schema_version != PAYLOAD_VERSION {
        return Err(error("Unsupported wall owner/type/schema"));
    }
    Ok(e)
}
fn dispatch(request: &Request) -> Result<Reply, PluginError> {
    if request.api_version != VERSION {
        return Err(error("Unsupported API"));
    }
    let catalog = catalog();
    match &request.operation {
        Operation::Describe => {
            if request.context.is_some() {
                return Err(error("Describe must be document-free"));
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
                return Err(error("Missing context"));
            }
            let descriptor = catalog
                .commands
                .iter()
                .find(|c| c.id == *command_id)
                .ok_or_else(|| error("Unknown tool"))?;
            validate_values(&descriptor.fields, inputs, false)
                .map_err(|_| error("Invalid wall inputs"))?;
            let existing = if descriptor.mode != Mode::Create {
                if selection.len() != 1 || !new_ids.is_empty() {
                    return Err(error("Select one wall"));
                }
                Some(target(snapshot, &selection[0])?)
            } else {
                if new_ids.len() != 1 || !selection.is_empty() {
                    return Err(error("Reserve one identity"));
                }
                None
            };
            if descriptor.mode == Mode::Delete {
                return Ok(Reply::Edits(vec![Edit::Delete {
                    id: existing.unwrap().id.clone(),
                }]));
            }
            if snapshot.levels.len() != 1 {
                return Err(error("Scope one target level"));
            }
            let mut payload =
                serde_json::to_value(inputs).map_err(|_| error("Cannot encode inputs"))?;
            payload["level"] = serde_json::Value::String(snapshot.levels[0].id.clone());
            let p: Parameters =
                serde_json::from_value(payload.clone()).map_err(|_| error("Invalid payload"))?;
            if (p.end_x - p.start_x).hypot(p.end_y - p.start_y) <= 1e-6 {
                return Err(error("Wall endpoints coincide"));
            }
            let element = Element {
                id: existing.map_or_else(|| new_ids[0].clone(), |e| e.id.clone()),
                owner: OWNER.into(),
                type_id: TYPE.into(),
                name: existing.map_or_else(|| "Wall".into(), |e| e.name.clone()),
                payload_schema_version: PAYLOAD_VERSION,
                payload,
                relationships: Default::default(),
                depends_on: [p.level].into(),
            };
            Ok(Reply::Edits(vec![if existing.is_some() {
                Edit::Replace {
                    expected_schema_version: PAYLOAD_VERSION,
                    element,
                }
            } else {
                Edit::Create(element)
            }]))
        }
        Operation::GenerateGeometry {
            element_id,
            snapshot,
        } => {
            if request.context.is_none() {
                return Err(error("Missing context"));
            }
            let e = target(snapshot, element_id)?;
            let p: Parameters = serde_json::from_value(e.payload.clone())
                .map_err(|_| error("Invalid wall payload"))?;
            let level = snapshot
                .levels
                .iter()
                .find(|l| l.id == p.level)
                .ok_or_else(|| error("Wall level not in scope"))?;
            let dx = p.end_x - p.start_x;
            let dy = p.end_y - p.start_y;
            let angle = dy.atan2(dx);
            let half = p.thickness / 2.0;
            let recipe = Recipe::RectangularPrism(RectangularPrism {
                width: dx.hypot(dy),
                depth: p.thickness,
                height: p.height,
                translation: [
                    p.start_x + angle.sin() * half,
                    p.start_y - angle.cos() * half,
                    level.elevation_metres,
                ],
                rotation_z: angle,
            });
            recipe
                .validate()
                .map_err(|_| error("Invalid wall geometry"))?;
            Ok(Reply::Geometry {
                element_id: element_id.clone(),
                recipe,
            })
        }
    }
}
pub fn respond(input: &[u8]) -> Option<Vec<u8>> {
    if input.len() > MAX_BYTES {
        return None;
    }
    let request: Request = serde_json::from_slice(input).ok()?;
    let result = dispatch(&request).unwrap_or_else(Reply::Error);
    let bytes = serde_json::to_vec(&Response {
        api_version: VERSION,
        request_id: request.request_id,
        context: request.context,
        result,
    })
    .ok()?;
    (bytes.len() <= MAX_BYTES).then_some(bytes)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn descriptors_validate_and_malformed_requests_fail() {
        catalog()
            .validate(&os_plugin_api::Manifest::from_toml(include_str!("../plugin.toml")).unwrap())
            .unwrap();
        assert!(respond(b"bad").is_none());
        assert!(respond(&vec![0; MAX_BYTES + 1]).is_none());
    }
}
