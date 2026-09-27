//! Portable opening types and their exact material dependencies. No project mutation.
use crate::{json_guard, storage};
use os_core::{Id, Result, ensure};
use os_model::{MaterialParams, Model, OpeningTypeParams};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs::File,
    io::{Read, Write},
    path::Path,
};

pub const OPENING_TYPE_PACKAGE_FORMAT: &str = "OpenStructure.OpeningTypePackage";
pub const OPENING_TYPE_PACKAGE_VERSION: u32 = 1;
pub const MAX_OPENING_TYPE_PACKAGE_BYTES: usize = 1024 * 1024;

/// Source identities are references for import remapping, not destination identities.
#[derive(Clone, Debug, PartialEq)]
pub struct OpeningTypePackage {
    pub source_type_id: Id,
    pub parameters: OpeningTypeParams,
    pub materials: Vec<OpeningTypePackageMaterial>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpeningTypePackageMaterial {
    pub source_id: Id,
    #[serde(with = "StrictMaterialParams")]
    pub parameters: MaterialParams,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WirePackage {
    format: String,
    version: u32,
    source_type_id: Id,
    #[serde(deserialize_with = "deserialize_type_parameters")]
    parameters: OpeningTypeParams,
    materials: Vec<OpeningTypePackageMaterial>,
}

// MaterialParams is intentionally permissive in the model. Packages have a
// strict independent wire contract without changing project deserialization.
#[derive(Serialize, Deserialize)]
#[serde(remote = "MaterialParams", deny_unknown_fields)]
struct StrictMaterialParams {
    name: String,
    density_kg_m3: f64,
    color: [u8; 3],
}

fn deserialize_type_parameters<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<OpeningTypeParams, D::Error> {
    let value = serde_json::Value::deserialize(deserializer)?;
    // Type and family already deny unknown fields; shared Point2 does not.
    for field in ["profile", "cut_profile"] {
        if let Some(points) = value["family"][field].as_array() {
            for point in points {
                if let Some(fields) = point.as_object()
                    && fields.keys().any(|key| key != "x" && key != "y")
                {
                    return Err(serde::de::Error::custom(
                        "unknown opening profile point field",
                    ));
                }
            }
        }
    }
    serde_json::from_value(value).map_err(serde::de::Error::custom)
}

impl OpeningTypePackage {
    /// Validate a package before importing or encoding it. Does not resolve hosts.
    pub fn validate(&self) -> Result<()> {
        ensure(!self.source_type_id.0.is_nil(), "nil opening type identity")?;
        self.parameters.validate()?;
        let referenced = material_references(&self.parameters);
        ensure(
            referenced.iter().all(|id| !id.0.is_nil()),
            "nil opening material reference",
        )?;
        ensure(
            self.materials.len() <= 2,
            "too many opening material snapshots",
        )?;
        let mut supplied = BTreeSet::new();
        for material in &self.materials {
            ensure(!material.source_id.0.is_nil(), "nil material identity")?;
            ensure(
                supplied.insert(material.source_id),
                "duplicate material identity",
            )?;
            let params = &material.parameters;
            ensure(
                !params.name.trim().is_empty()
                    && params.name.len() <= 256
                    && !params.name.chars().any(char::is_control),
                "material name must be 1-256 bytes without control characters",
            )?;
            ensure(
                params.density_kg_m3.is_finite() && params.density_kg_m3 > 0.0,
                "invalid material density",
            )?;
        }
        ensure(
            supplied == referenced,
            "material snapshots must exactly match opening type references",
        )
    }
}

fn material_references(params: &OpeningTypeParams) -> BTreeSet<Id> {
    [params.family.panel_material, params.family.frame_material]
        .into_iter()
        .flatten()
        .collect()
}

/// Snapshot one native type and only its referenced materials, deduplicated and
/// sorted by source UUID. Unrelated project entities are neither read nor validated.
pub fn export_opening_type_package(model: &Model, type_id: Id) -> Result<Vec<u8>> {
    let ty = model
        .opening_types
        .get(&type_id)
        .ok_or_else(|| storage("opening type not found"))?;
    ensure(
        ty.id() == type_id && ty.header.type_id == "core.opening_type",
        "invalid source opening type identity",
    )?;
    let mut materials = Vec::new();
    for id in material_references(&ty.parameters) {
        let material = model
            .materials
            .get(&id)
            .ok_or_else(|| storage("referenced opening material not found"))?;
        ensure(
            material.id() == id && material.header.type_id == "core.material",
            "invalid source material identity",
        )?;
        materials.push(OpeningTypePackageMaterial {
            source_id: id,
            parameters: material.parameters.clone(),
        });
    }
    let package = OpeningTypePackage {
        source_type_id: type_id,
        parameters: ty.parameters.clone(),
        materials,
    };
    encode_opening_type_package(&package)
}

/// Deterministic UTF-8 JSON: fixed field order and snapshots sorted by UUID.
fn encode_opening_type_package(package: &OpeningTypePackage) -> Result<Vec<u8>> {
    package.validate()?;
    let mut canonical = WirePackage {
        format: OPENING_TYPE_PACKAGE_FORMAT.into(),
        version: OPENING_TYPE_PACKAGE_VERSION,
        source_type_id: package.source_type_id,
        parameters: package.parameters.clone(),
        materials: package.materials.clone(),
    };
    canonical
        .materials
        .sort_by_key(|material| material.source_id);
    let bytes = serde_json::to_vec_pretty(&canonical).map_err(storage)?;
    ensure(
        bytes.len() <= MAX_OPENING_TYPE_PACKAGE_BYTES,
        "opening type package exceeds 1 MiB limit",
    )?;
    Ok(bytes)
}

/// Parse bounded bytes, rejecting duplicate keys at every depth before serde
/// can discard them. Call this helper, not raw serde, at the import boundary.
pub fn parse_opening_type_package(bytes: &[u8]) -> Result<OpeningTypePackage> {
    ensure(
        bytes.len() <= MAX_OPENING_TYPE_PACKAGE_BYTES,
        "opening type package exceeds 1 MiB limit",
    )?;
    let raw = std::str::from_utf8(bytes).map_err(storage)?;
    json_guard::validate(raw).map_err(storage)?;
    let wire: WirePackage = serde_json::from_str(raw).map_err(storage)?;
    ensure(
        wire.format == OPENING_TYPE_PACKAGE_FORMAT,
        "unsupported opening type package format",
    )?;
    ensure(
        wire.version == OPENING_TYPE_PACKAGE_VERSION,
        "unsupported opening type package version",
    )?;
    let mut package = OpeningTypePackage {
        source_type_id: wire.source_type_id,
        parameters: wire.parameters,
        materials: wire.materials,
    };
    package.validate()?;
    package.materials.sort_by_key(|material| material.source_id);
    Ok(package)
}

/// Validate first, then flush a temporary sibling and atomically replace path.
/// Any validation or write failure leaves an existing destination untouched.
pub fn write_opening_type_package(model: &Model, type_id: Id, path: &Path) -> Result<()> {
    let bytes = export_opening_type_package(model, type_id)?;
    let directory = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(directory).map_err(storage)?;
    temporary.write_all(&bytes).map_err(storage)?;
    temporary.as_file_mut().sync_all().map_err(storage)?;
    temporary.persist(path).map_err(storage)?;
    Ok(())
}

/// Read at most 1 MiB + one sentinel byte, even if the file grows during reading.
pub fn read_opening_type_package(path: &Path) -> Result<OpeningTypePackage> {
    let file = File::open(path).map_err(storage)?;
    ensure(
        file.metadata().map_err(storage)?.len() <= MAX_OPENING_TYPE_PACKAGE_BYTES as u64,
        "opening type package exceeds 1 MiB limit",
    )?;
    let mut bytes = Vec::new();
    file.take(MAX_OPENING_TYPE_PACKAGE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(storage)?;
    parse_opening_type_package(&bytes)
}
