//! Version 2: independent, bounded command/descriptor DTOs. No host types.
use crate::{Manifest, ProtocolResult, RegistrationKind, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub const VERSION: u32 = 2;
pub const MAX_BYTES: usize = 1024 * 1024;
pub const MAX_EDITS: usize = 16;
pub const MAX_SCOPE: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stamp {
    pub project: String,
    pub session: String,
    pub revision: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub api_version: u32,
    pub request_id: String,
    pub context: Option<Stamp>,
    pub operation: Operation,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", deny_unknown_fields)]
pub enum Operation {
    Describe,
    GenerateGeometry {
        element_id: String,
        snapshot: Snapshot,
    },
    Invoke {
        command_id: String,
        inputs: BTreeMap<String, Value>,
        snapshot: Snapshot,
        /// Host-selected writable objects; other snapshot objects are read-only.
        selection: Vec<String>,
        new_ids: Vec<String>,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Response {
    pub api_version: u32,
    pub request_id: String,
    pub context: Option<Stamp>,
    pub result: Reply,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", deny_unknown_fields)]
pub enum Reply {
    Catalog(Catalog),
    Geometry {
        element_id: String,
        recipe: crate::geometry::Recipe,
    },
    Edits(Vec<Edit>),
    Error(PluginError),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginError {
    pub code: ErrorCode,
    pub message: String,
    pub field: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ErrorCode {
    InvalidInput,
    Unsupported,
    Cancelled,
    Internal,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub elements: Vec<Element>,
    pub levels: Vec<Level>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Level {
    pub id: String,
    pub name: String,
    pub elevation_metres: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Element {
    pub id: String,
    pub owner: String,
    pub type_id: String,
    pub name: String,
    pub payload_schema_version: u32,
    pub payload: Value,
    pub relationships: BTreeMap<String, Vec<String>>,
    pub depends_on: BTreeSet<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", deny_unknown_fields)]
pub enum Edit {
    Create(Element),
    Replace {
        expected_schema_version: u32,
        element: Element,
    },
    Delete {
        id: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub types: Vec<ElementType>,
    pub commands: Vec<CommandDescriptor>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ElementType {
    pub id: String,
    pub payload_schema_version: u32,
    pub fields: Vec<Field>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandDescriptor {
    pub id: String,
    pub element_type: String,
    pub mode: Mode,
    pub fields: Vec<Field>,
    pub enabled: bool,
    pub disabled_reason: Option<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    Create,
    Edit,
    Delete,
    /// Explicit whole-owner extension migration; never an ordinary edit fallback.
    Migrate,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Field {
    pub key: String,
    pub label: String,
    pub kind: FieldKind,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", deny_unknown_fields)]
pub enum FieldKind {
    Number {
        unit: Unit,
        min: f64,
        max: f64,
        default: f64,
    },
    Choice {
        values: Vec<String>,
        default: String,
    },
    Text {
        max_bytes: usize,
        default: String,
    },
    Boolean {
        default: bool,
    },
}
/// Wire numeric values are in the declared units; display conversions belong to the host.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Unit {
    Metres,
    Radians,
    Scalar,
}

fn short_text(s: &str) -> bool {
    !s.trim().is_empty() && s.len() <= 256 && !s.chars().any(char::is_control)
}
fn fields_valid(fields: &[Field]) -> ProtocolResult<()> {
    ensure(fields.len() <= 64, "descriptor has too many fields")?;
    let mut keys = BTreeSet::new();
    for field in fields {
        ensure(
            !field.key.is_empty()
                && field.key.len() <= 64
                && field
                    .key
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_')
                && keys.insert(&field.key),
            "invalid or duplicate field key",
        )?;
        ensure(short_text(&field.label), "invalid field label")?;
        match &field.kind {
            FieldKind::Number {
                min, max, default, ..
            } => ensure(
                min.is_finite()
                    && max.is_finite()
                    && default.is_finite()
                    && min <= default
                    && default <= max,
                "invalid numeric descriptor range/default",
            )?,
            FieldKind::Choice { values, default } => {
                ensure(
                    !values.is_empty() && values.len() <= 64 && values.contains(default),
                    "invalid choice descriptor/default",
                )?;
                let mut seen = BTreeSet::new();
                for value in values {
                    ensure(
                        short_text(value) && seen.insert(value),
                        "invalid or duplicate choice",
                    )?;
                }
            }
            FieldKind::Text { max_bytes, default } => ensure(
                *max_bytes > 0 && *max_bytes <= 4096 && default.len() <= *max_bytes,
                "invalid text descriptor/default",
            )?,
            FieldKind::Boolean { .. } => {}
        }
    }
    Ok(())
}

/// Validate tool inputs strictly; payloads may retain unknown vendor fields.
pub fn validate_values(
    fields: &[Field],
    values: &BTreeMap<String, Value>,
    allow_unknown: bool,
) -> ProtocolResult<()> {
    fields_valid(fields)?;
    ensure(
        allow_unknown || values.len() == fields.len(),
        "unexpected or missing input fields",
    )?;
    for field in fields {
        let value = values
            .get(&field.key)
            .ok_or_else(|| crate::ProtocolError(format!("missing field {}", field.key)))?;
        let valid = match &field.kind {
            FieldKind::Number { min, max, .. } => value
                .as_f64()
                .is_some_and(|n| n.is_finite() && n >= *min && n <= *max),
            FieldKind::Choice { values, .. } => value
                .as_str()
                .is_some_and(|s| values.iter().any(|v| v == s)),
            FieldKind::Text { max_bytes, .. } => {
                value.as_str().is_some_and(|s| s.len() <= *max_bytes)
            }
            FieldKind::Boolean { .. } => value.is_boolean(),
        };
        ensure(valid, format!("invalid field {}", field.key))?;
    }
    Ok(())
}
impl Catalog {
    pub fn from_json(input: &str, manifest: &Manifest) -> ProtocolResult<Self> {
        ensure(input.len() <= MAX_BYTES, "catalog exceeds 1 MiB")?;
        let result: Self =
            serde_json::from_str(input).map_err(|e| crate::ProtocolError(e.to_string()))?;
        result.validate(manifest)?;
        Ok(result)
    }
    pub fn validate(&self, manifest: &Manifest) -> ProtocolResult<()> {
        manifest.validate()?;
        ensure(
            manifest.api_version == VERSION,
            "generic catalog requires API 2",
        )?;
        ensure(
            !self.types.is_empty()
                && self.types.len() <= 64
                && !self.commands.is_empty()
                && self.commands.len() <= 128,
            "invalid catalog counts",
        )?;
        let mut types = BTreeSet::new();
        let mut commands = BTreeSet::new();
        for t in &self.types {
            ensure(
                types.insert(&t.id)
                    && t.payload_schema_version > 0
                    && manifest
                        .registrations
                        .iter()
                        .any(|r| r.id == t.id && r.kind == RegistrationKind::ElementType),
                "unregistered or duplicate catalog type",
            )?;
            fields_valid(&t.fields)?;
        }
        for command in &self.commands {
            ensure(
                commands.insert(&command.id)
                    && types.contains(&command.element_type)
                    && manifest
                        .registrations
                        .iter()
                        .any(|r| r.id == command.id && r.kind == RegistrationKind::Tool),
                "unregistered or duplicate command",
            )?;
            ensure(
                command.enabled == command.disabled_reason.is_none()
                    && command
                        .disabled_reason
                        .as_ref()
                        .is_none_or(|s| short_text(s)),
                "disabled command requires an explanation",
            )?;
            fields_valid(&command.fields)?;
        }
        for registration in &manifest.registrations {
            let present = match registration.kind {
                RegistrationKind::ElementType => types.contains(&registration.id),
                RegistrationKind::Tool => commands.contains(&registration.id),
                // Other provider categories are not callable in this initial v2 route.
                _ => true,
            };
            ensure(present, "registration missing callable descriptor")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn manifest() -> Manifest {
        Manifest::from_toml(include_str!("../../../fixtures/generic-column/plugin.toml")).unwrap()
    }
    #[test]
    fn standalone_catalog_resolves_registered_fields_and_commands() {
        let c = Catalog::from_json(
            include_str!("../../../fixtures/generic-column/catalog.json"),
            &manifest(),
        )
        .unwrap();
        assert_eq!(c.commands.len(), 3);
        let values = BTreeMap::from([("width".into(), serde_json::json!(0.4))]);
        validate_values(&c.commands[0].fields, &values, false).unwrap();
        let mut bad = c.clone();
        bad.commands[0].element_type = "org.other.type".into();
        assert!(bad.validate(&manifest()).is_err());
        let mut bad = c.clone();
        bad.commands[0].enabled = false;
        assert!(bad.validate(&manifest()).is_err());
        let mut bad = c;
        let duplicate = bad.types[0].fields[0].clone();
        bad.types[0].fields.push(duplicate);
        assert!(bad.validate(&manifest()).is_err());
    }
    #[test]
    fn typed_values_and_defaults_reject_nonfinite_ranges_choices_and_unknown_inputs() {
        let fields = vec![
            Field {
                key: "length".into(),
                label: "Length".into(),
                kind: FieldKind::Number {
                    unit: Unit::Metres,
                    min: 0.01,
                    max: 10.0,
                    default: 1.0,
                },
            },
            Field {
                key: "kind".into(),
                label: "Kind".into(),
                kind: FieldKind::Choice {
                    values: vec!["A".into(), "B".into()],
                    default: "A".into(),
                },
            },
            Field {
                key: "name".into(),
                label: "Name".into(),
                kind: FieldKind::Text {
                    max_bytes: 8,
                    default: "Name".into(),
                },
            },
            Field {
                key: "enabled".into(),
                label: "Enabled".into(),
                kind: FieldKind::Boolean { default: true },
            },
        ];
        let values = BTreeMap::from([
            ("length".into(), serde_json::json!(1)),
            ("kind".into(), serde_json::json!("A")),
            ("name".into(), serde_json::json!("Name")),
            ("enabled".into(), serde_json::json!(true)),
        ]);
        validate_values(&fields, &values, false).unwrap();
        for (key, value) in [
            ("length", serde_json::json!(-1)),
            ("kind", serde_json::json!("C")),
            ("name", serde_json::json!("too long a name")),
            ("enabled", serde_json::json!("true")),
            ("unexpected", serde_json::json!(1)),
        ] {
            let mut bad = values.clone();
            bad.insert(key.into(), value);
            assert!(validate_values(&fields, &bad, false).is_err());
        }
        let mut bad = fields;
        bad[0].kind = FieldKind::Number {
            unit: Unit::Metres,
            min: f64::NAN,
            max: 10.0,
            default: 1.0,
        };
        assert!(validate_values(&bad, &values, false).is_err());
    }
}
