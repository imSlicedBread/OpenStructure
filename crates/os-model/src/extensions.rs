//! Inert, bounded plugin-owned semantics. No plugin runtime is needed to read them.
use os_core::{Id, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub const EXTENSION_ENVELOPE_VERSION: u32 = 1;
pub const MAX_EXTENSION_BYTES: usize = 1024 * 1024;
pub const MAX_EXTENSIONS_BYTES: usize = 16 * MAX_EXTENSION_BYTES;
pub const MAX_EXTENSIONS: usize = 10_000;
pub const MAX_PLUGIN_REQUIREMENTS: usize = 1024;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginRequirement {
    pub version: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtensionEntity {
    pub envelope_version: u32,
    pub id: Id,
    pub owner: String,
    pub type_id: String,
    pub name: String,
    pub payload_schema_version: u32,
    pub relationships: BTreeMap<String, Vec<Id>>,
    /// Directed regeneration prerequisites; extension-to-extension cycles are unsupported.
    pub depends_on: BTreeSet<Id>,
    pub payload: Value,
}

impl ExtensionEntity {
    pub fn references(&self) -> impl Iterator<Item = &Id> {
        self.depends_on
            .iter()
            .chain(self.relationships.values().flatten())
    }
}

pub fn valid_plugin_id(id: &str) -> bool {
    id.len() <= 128
        && id.split('.').count() >= 2
        && id.split('.').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
}

pub fn valid_plugin_version(version: &str) -> bool {
    version.len() <= 32
        && version.split('.').count() == 3
        && version.split('.').all(|part| {
            !part.is_empty()
                && part.bytes().all(|b| b.is_ascii_digit())
                && (part.len() == 1 || !part.starts_with('0'))
                && part.parse::<u32>().is_ok()
        })
}

fn bounded_depth(value: &Value, depth: usize) -> Result<()> {
    ensure(depth <= 32, "extension payload nesting exceeds 32")?;
    match value {
        Value::Array(values) => {
            for value in values {
                bounded_depth(value, depth + 1)?;
            }
        }
        Value::Object(values) => {
            ensure(
                !values.contains_key("$serde_json::private::Number"),
                "reserved JSON number representation key",
            )?;
            for value in values.values() {
                bounded_depth(value, depth + 1)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// Count encoded bytes without allocating a second copy of an oversized payload.
struct Counter(usize);
impl std::io::Write for Counter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > MAX_EXTENSION_BYTES - self.0 {
            return Err(std::io::Error::other("extension envelope exceeds 1 MiB"));
        }
        self.0 += bytes.len();
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub(crate) fn validate(model: &super::Model, ids: &mut BTreeSet<Id>) -> Result<()> {
    ensure(
        model.extensions.len() <= MAX_EXTENSIONS,
        "too many extension entities",
    )?;
    ensure(
        model.plugin_requirements.len() <= MAX_PLUGIN_REQUIREMENTS,
        "too many plugin requirements",
    )?;
    for (owner, requirement) in &model.plugin_requirements {
        ensure(valid_plugin_id(owner), "invalid plugin requirement ID")?;
        ensure(
            valid_plugin_version(&requirement.version),
            "invalid exact plugin version",
        )?;
    }
    let mut total = 0;
    for (id, entity) in &model.extensions {
        ensure(
            *id == entity.id && !id.0.is_nil() && ids.insert(*id),
            "invalid or duplicate extension identity",
        )?;
        ensure(
            entity.envelope_version == EXTENSION_ENVELOPE_VERSION,
            "unsupported extension envelope version",
        )?;
        ensure(
            model.plugin_requirements.contains_key(&entity.owner),
            "extension owner requirement missing",
        )?;
        ensure(
            entity.type_id.len() <= 256
                && entity.type_id.starts_with(&format!("{}.", entity.owner))
                && valid_type_id(&entity.type_id),
            "invalid extension type namespace",
        )?;
        ensure(
            !entity.name.trim().is_empty() && entity.name.len() <= 1024,
            "invalid extension name",
        )?;
        ensure(
            entity.payload_schema_version > 0,
            "invalid plugin payload schema version",
        )?;
        for name in entity.relationships.keys() {
            ensure(
                !name.is_empty()
                    && name.len() <= 128
                    && name
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b)),
                "invalid extension relationship name",
            )?;
        }
        bounded_depth(&entity.payload, 0)?;
        let mut counter = Counter(0);
        serde_json::to_writer(&mut counter, entity)
            .map_err(|e| os_core::Error::Invalid(e.to_string()))?;
        total += counter.0;
        ensure(
            total <= MAX_EXTENSIONS_BYTES,
            "combined extension envelopes exceed 16 MiB",
        )?;
    }
    for entity in model.extensions.values() {
        for target in entity.references() {
            ensure(ids.contains(target), "dangling extension reference")?;
        }
    }
    // Kahn's algorithm: no recursive graph traversal or repeated whole-graph scans.
    let mut remaining = BTreeMap::new();
    let mut dependents: BTreeMap<Id, Vec<Id>> = BTreeMap::new();
    for (id, entity) in &model.extensions {
        let mut count = 0;
        for target in &entity.depends_on {
            if model.extensions.contains_key(target) {
                count += 1;
                dependents.entry(*target).or_default().push(*id);
            }
        }
        remaining.insert(*id, count);
    }
    let mut ready: Vec<_> = remaining
        .iter()
        .filter_map(|(id, n)| (*n == 0).then_some(*id))
        .collect();
    let mut visited = 0;
    while let Some(id) = ready.pop() {
        visited += 1;
        if let Some(children) = dependents.get(&id) {
            for child in children {
                let count = remaining.get_mut(child).expect("validated extension key");
                *count -= 1;
                if *count == 0 {
                    ready.push(*child);
                }
            }
        }
    }
    ensure(
        visited == model.extensions.len(),
        "cyclic extension regeneration dependencies",
    )
}

fn valid_type_id(id: &str) -> bool {
    id.split('.').all(|part| {
        !part.is_empty()
            && part
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
    })
}
