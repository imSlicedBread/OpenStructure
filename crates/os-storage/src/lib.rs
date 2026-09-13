//! Versioned ZIP/JSON project storage with bounded reads and atomic replacement.
use os_core::{Error, Id, Result, ensure};
use os_document::Document;
use os_model::{Model, SCHEMA_VERSION};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
};
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};
mod json_guard;

const MAX_MODEL_BYTES: u64 = 64 * 1024 * 1024;
const MAX_MANIFEST_BYTES: u64 = 64 * 1024;
pub const CONTAINER_VERSION: u32 = 2;
pub const MAX_AUXILIARY_FILE_BYTES: u64 = 16 * 1024 * 1024;
pub const MAX_AUXILIARY_TOTAL_BYTES: u64 = 64 * 1024 * 1024;
#[derive(Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub format: String,
    pub container_version: u32,
    pub schema_version: u32,
    pub project_id: Id,
    pub model_entry: String,
    pub units: String,
}

pub trait StorageBackend {
    fn save(&self, document: &Document, path: &Path) -> Result<()>;
    fn open(&self, path: &Path) -> Result<Document>;
}
#[derive(Default)]
pub struct ZipJsonStorage;
fn storage(error: impl std::fmt::Display) -> Error {
    Error::Storage(error.to_string())
}

impl StorageBackend for ZipJsonStorage {
    fn save(&self, document: &Document, path: &Path) -> Result<()> {
        document.model().validate()?;
        validate_auxiliary_files(document.auxiliary_files())?;
        let model = serde_json::to_vec_pretty(document.model()).map_err(storage)?;
        ensure(
            model.len() as u64 <= MAX_MODEL_BYTES,
            "model exceeds 64 MiB limit",
        )?;
        let manifest = Manifest {
            format: "OpenStructure".into(),
            container_version: CONTAINER_VERSION,
            schema_version: SCHEMA_VERSION,
            project_id: document.model().project.id(),
            model_entry: "model.json".into(),
            units: "metres".into(),
        };
        let directory = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let mut temporary = tempfile::NamedTempFile::new_in(directory).map_err(storage)?;
        {
            let mut zip = ZipWriter::new(temporary.as_file_mut());
            let options =
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
            zip.start_file("manifest.toml", options).map_err(storage)?;
            zip.write_all(toml::to_string(&manifest).map_err(storage)?.as_bytes())
                .map_err(storage)?;
            zip.start_file("model.json", options).map_err(storage)?;
            zip.write_all(&model).map_err(storage)?;
            for name in ["assets/", "previews/"] {
                if !document.auxiliary_files().contains_key(name) {
                    zip.add_directory(name, options).map_err(storage)?;
                }
            }
            for (name, bytes) in document.auxiliary_files() {
                if name.ends_with('/') {
                    zip.add_directory(name, options).map_err(storage)?;
                } else {
                    zip.start_file(name, options).map_err(storage)?;
                    zip.write_all(bytes).map_err(storage)?;
                }
            }
            zip.finish().map_err(storage)?;
        }
        temporary.as_file_mut().sync_all().map_err(storage)?;
        temporary.persist(path).map_err(storage)?;
        Ok(())
    }
    fn open(&self, path: &Path) -> Result<Document> {
        let file = File::open(path).map_err(storage)?;
        let mut zip = ZipArchive::new(file.try_clone().map_err(storage)?).map_err(storage)?;
        ensure(zip.len() <= 1024, "too many archive entries")?;
        validate_directory(file, zip.central_directory_start(), zip.len())?;
        let text = read_entry(&mut zip, "manifest.toml", MAX_MANIFEST_BYTES)?;
        let manifest: Manifest = toml::from_str(&text).map_err(storage)?;
        ensure(
            manifest.format == "OpenStructure" && matches!(manifest.container_version, 1 | 2),
            "unsupported container format/version",
        )?;
        ensure(
            manifest.model_entry == "model.json" && manifest.units == "metres",
            "unsupported model backend or units",
        )?;
        ensure(
            manifest.schema_version <= SCHEMA_VERSION,
            "file requires a newer OpenStructure version",
        )?;
        let raw = read_entry(&mut zip, "model.json", MAX_MODEL_BYTES)?;
        json_guard::validate(&raw).map_err(storage)?;
        let mut value: serde_json::Value = serde_json::from_str(&raw).map_err(storage)?;
        ensure(
            value["schema_version"].as_u64() == Some(manifest.schema_version as u64),
            "manifest/model schema mismatch",
        )?;
        migrate(&mut value, manifest.schema_version)?;
        let model: Model = serde_json::from_value(value).map_err(storage)?;
        ensure(
            model.project.id() == manifest.project_id,
            "manifest project identity mismatch",
        )?;
        let auxiliary_files = read_auxiliary_files(&mut zip)?;
        Document::from_model_and_files(model, auxiliary_files)
    }
}
/// ZIP lookup deduplicates names internally. Inspect the actual directory rather
/// than counting an already-deduplicated iterator. No archive path is extracted.
fn validate_directory(mut file: File, start: u64, expected: usize) -> Result<()> {
    let mut names = std::collections::BTreeSet::new();
    file.seek(SeekFrom::Start(start)).map_err(storage)?;
    loop {
        let mut signature = [0_u8; 4];
        file.read_exact(&mut signature).map_err(storage)?;
        if signature != *b"PK\x01\x02" {
            ensure(
                matches!(&signature, b"PK\x05\x06" | b"PK\x06\x06" | b"PK\x05\x05"),
                "invalid central-directory terminator",
            )?;
            return ensure(names.len() == expected, "ambiguous archive directory");
        }
        ensure(names.len() < 1024, "too many archive entries")?;
        let mut header = [0_u8; 42];
        file.read_exact(&mut header).map_err(storage)?;
        let length = |offset| u16::from_le_bytes([header[offset], header[offset + 1]]) as usize;
        let mut name = vec![0_u8; length(24)];
        file.read_exact(&mut name).map_err(storage)?;
        validate_entry_name(std::str::from_utf8(&name).map_err(storage)?)?;
        ensure(names.insert(name), "duplicate archive entry")?;
        file.seek(SeekFrom::Current((length(26) + length(28)) as i64))
            .map_err(storage)?;
    }
}
fn read_entry(zip: &mut ZipArchive<File>, name: &str, limit: u64) -> Result<String> {
    ensure(
        zip.file_names().filter(|entry| *entry == name).count() == 1,
        format!("missing or duplicate {name}"),
    )?;
    let entry = zip.by_name(name).map_err(storage)?;
    ensure(entry.size() <= limit, format!("{name} exceeds size limit"))?;
    let mut bytes = Vec::new();
    entry
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(storage)?;
    ensure(
        bytes.len() as u64 <= limit,
        "decompressed entry exceeds size limit",
    )?;
    String::from_utf8(bytes).map_err(storage)
}

/// Portable virtual archive paths, never filesystem extraction targets.
fn validate_entry_name(name: &str) -> Result<()> {
    ensure(
        !name.is_empty() && name.len() <= 1024,
        "invalid archive path length",
    )?;
    ensure(
        !name.contains(['\\', ':', '<', '>', '"', '|', '?', '*'])
            && !name.chars().any(char::is_control),
        "unsafe archive path",
    )?;
    let path = name.strip_suffix('/').unwrap_or(name);
    for part in path.split('/') {
        ensure(
            !part.is_empty()
                && part != "."
                && part != ".."
                && part.len() <= 255
                && !part.ends_with(['.', ' ']),
            "unsafe archive path component",
        )?;
        let stem = part.split('.').next().unwrap_or("").to_ascii_uppercase();
        ensure(
            !matches!(
                stem.as_str(),
                "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
            ) && !(stem.len() == 4
                && (stem.starts_with("COM") || stem.starts_with("LPT"))
                && matches!(stem.as_bytes()[3], b'1'..=b'9')),
            "reserved device path",
        )?;
    }
    Ok(())
}

fn validate_path_set<'a>(names: impl IntoIterator<Item = &'a str>) -> Result<()> {
    let mut paths = BTreeMap::new();
    for name in names {
        validate_entry_name(name)?;
        let path = name.trim_end_matches('/').to_lowercase();
        ensure(
            paths.insert(path, name.ends_with('/')).is_none(),
            "ambiguous case or file/directory collision",
        )?;
    }
    for name in paths.keys() {
        for (i, _) in name.match_indices('/') {
            ensure(
                paths.get(&name[..i]) != Some(&false),
                "archive file is also a parent directory",
            )?;
        }
    }
    Ok(())
}

fn validate_auxiliary_files(files: &BTreeMap<String, Vec<u8>>) -> Result<()> {
    ensure(
        !files.contains_key("manifest.toml") && !files.contains_key("model.json"),
        "reserved auxiliary filename",
    )?;
    let mut names: Vec<_> = files.keys().map(String::as_str).collect();
    names.extend(["manifest.toml", "model.json"]);
    for directory in ["assets/", "previews/"] {
        if !files.contains_key(directory) {
            names.push(directory);
        }
    }
    ensure(names.len() <= 1024, "too many archive entries")?;
    validate_path_set(names)?;
    let mut total = 0_u64;
    for (name, bytes) in files {
        ensure(
            bytes.len() as u64 <= MAX_AUXILIARY_FILE_BYTES,
            "auxiliary file exceeds 16 MiB",
        )?;
        ensure(
            !name.ends_with('/') || bytes.is_empty(),
            "directory contains data",
        )?;
        total += bytes.len() as u64;
        ensure(
            total <= MAX_AUXILIARY_TOTAL_BYTES,
            "auxiliary contents exceed 64 MiB",
        )?;
    }
    Ok(())
}

fn read_auxiliary_files(zip: &mut ZipArchive<File>) -> Result<BTreeMap<String, Vec<u8>>> {
    validate_path_set(zip.file_names())?;
    let mut total = 0_u64;
    // Inspect every entry before allocating contents. Special files and links
    // must not be repackaged into apparently ordinary files.
    for i in 0..zip.len() {
        let entry = zip.by_index(i).map_err(storage)?;
        ensure(
            entry.name_raw() == entry.name().as_bytes(),
            "archive name is not canonical UTF-8",
        )?;
        if let Some(mode) = entry.unix_mode() {
            let kind = mode & 0o170000;
            ensure(
                kind == 0
                    || (kind == 0o040000 && entry.is_dir())
                    || (kind == 0o100000 && !entry.is_dir()),
                "unsupported archive entry type",
            )?;
        }
        if matches!(entry.name(), "manifest.toml" | "model.json") {
            continue;
        }
        ensure(
            entry.size() <= MAX_AUXILIARY_FILE_BYTES,
            "auxiliary file exceeds 16 MiB",
        )?;
        ensure(
            !entry.is_dir() || entry.size() == 0,
            "directory contains data",
        )?;
        total += entry.size();
        ensure(
            total <= MAX_AUXILIARY_TOTAL_BYTES,
            "auxiliary contents exceed 64 MiB",
        )?;
    }
    let mut files = BTreeMap::new();
    let mut actual_total = 0_u64;
    for i in 0..zip.len() {
        let entry = zip.by_index(i).map_err(storage)?;
        if matches!(entry.name(), "manifest.toml" | "model.json") {
            continue;
        }
        let name = entry.name().to_owned();
        let mut bytes = Vec::new();
        let limit = MAX_AUXILIARY_FILE_BYTES.min(MAX_AUXILIARY_TOTAL_BYTES - actual_total);
        entry
            .take(limit + 1)
            .read_to_end(&mut bytes)
            .map_err(storage)?;
        ensure(
            bytes.len() as u64 <= limit,
            "decompressed auxiliary contents exceed limit",
        )?;
        actual_total += bytes.len() as u64;
        files.insert(name, bytes);
    }
    validate_auxiliary_files(&files)?;
    Ok(files)
}

/// Migrate a copy and validate before replacing the supplied value.
pub fn migrate(value: &mut serde_json::Value, from: u32) -> Result<()> {
    ensure(
        value.get("schema_version").and_then(|v| v.as_u64()) == Some(u64::from(from)),
        "migration source schema mismatch",
    )?;
    let mut candidate = value.clone();
    migrate_inner(&mut candidate, from)?;
    let model: Model = serde_json::from_value(candidate.clone()).map_err(storage)?;
    model.validate()?;
    // Added defaults can expand an old compact file. Do not adopt a migrated
    // document that cannot be saved under the current writer's model-byte limit.
    validate_serialized_model_size(&model, MAX_MODEL_BYTES)?;
    *value = candidate;
    Ok(())
}

fn validate_serialized_model_size(model: &Model, limit: u64) -> Result<()> {
    struct Counter(u64);
    impl Write for Counter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            let count = bytes.len() as u64;
            if count > self.0 {
                return Err(std::io::Error::other(
                    "native model exceeds serialized byte limit",
                ));
            }
            self.0 -= count;
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    serde_json::to_writer_pretty(Counter(limit), model).map_err(storage)
}

/// Explicit native schema chain; never infer a version from optional field shapes.
fn migrate_inner(value: &mut serde_json::Value, from: u32) -> Result<()> {
    match from {
        4 => Ok(()),
        3 => {
            let object = value
                .as_object_mut()
                .ok_or_else(|| Error::Invalid("model must be an object".into()))?;
            ensure(!object.contains_key("grids"), "ambiguous schema 3 grids")?;
            for collection in [
                "project",
                "sites",
                "buildings",
                "levels",
                "walls",
                "materials",
                "views",
            ] {
                let data = object
                    .get_mut(collection)
                    .ok_or_else(|| Error::Invalid(format!("missing {collection}")))?;
                let entities: Vec<&mut serde_json::Value> = if collection == "project" {
                    vec![data]
                } else {
                    data.as_object_mut()
                        .ok_or_else(|| Error::Invalid("invalid entity map".into()))?
                        .values_mut()
                        .collect()
                };
                for entity in entities {
                    let header = entity
                        .get_mut("header")
                        .and_then(|v| v.as_object_mut())
                        .ok_or_else(|| Error::Invalid("missing header".into()))?;
                    ensure(
                        header.get("schema_version").and_then(|v| v.as_u64()) == Some(3),
                        "unsupported schema 3 entity header",
                    )?;
                    header.insert("schema_version".into(), 4.into());
                }
            }
            object.insert("grids".into(), serde_json::json!({}));
            object.insert("schema_version".into(), 4.into());
            Ok(())
        }
        2 => {
            let object = value
                .as_object_mut()
                .ok_or_else(|| Error::Invalid("model must be an object".into()))?;
            for collection in [
                "project",
                "sites",
                "buildings",
                "levels",
                "walls",
                "materials",
                "views",
            ] {
                let data = object
                    .get_mut(collection)
                    .ok_or_else(|| Error::Invalid(format!("missing {collection}")))?;
                let entities: Vec<&mut serde_json::Value> = if collection == "project" {
                    vec![data]
                } else {
                    data.as_object_mut()
                        .ok_or_else(|| Error::Invalid("invalid entity map".into()))?
                        .values_mut()
                        .collect()
                };
                for entity in entities {
                    let header = entity
                        .get_mut("header")
                        .and_then(|v| v.as_object_mut())
                        .ok_or_else(|| Error::Invalid("missing header".into()))?;
                    ensure(
                        header.get("schema_version").and_then(|v| v.as_u64()) == Some(2),
                        "unsupported schema 2 entity header",
                    )?;
                    header.insert("schema_version".into(), 3.into());
                    if collection == "views" {
                        let params = entity
                            .get_mut("parameters")
                            .and_then(|v| v.as_object_mut())
                            .ok_or_else(|| Error::Invalid("missing view parameters".into()))?;
                        ensure(
                            !params.contains_key("plan")
                                && !params.contains_key("settings_revision"),
                            "ambiguous schema 2 view settings",
                        )?;
                        let settings =
                            if params.get("kind").and_then(|v| v.as_str()) == Some("Plan") {
                                serde_json::to_value(os_model::PlanSettings::default())
                                    .map_err(storage)?
                            } else {
                                serde_json::Value::Null
                            };
                        params.insert("plan".into(), settings);
                        params.insert("settings_revision".into(), 0.into());
                    }
                }
            }
            object.insert("schema_version".into(), 3.into());
            migrate_inner(value, 3)
        }
        1 => {
            let object = value
                .as_object_mut()
                .ok_or_else(|| Error::Invalid("model must be an object".into()))?;
            ensure(
                !object.contains_key("extensions") && !object.contains_key("plugin_requirements"),
                "ambiguous schema 1 extension fields",
            )?;
            object.insert("extensions".into(), serde_json::json!({}));
            object.insert("plugin_requirements".into(), serde_json::json!({}));
            object.insert("schema_version".into(), 2.into());
            for collection in [
                "project",
                "sites",
                "buildings",
                "levels",
                "walls",
                "materials",
                "views",
            ] {
                let data = object
                    .get_mut(collection)
                    .ok_or_else(|| Error::Invalid(format!("missing {collection}")))?;
                let entities: Vec<&mut serde_json::Value> = if collection == "project" {
                    vec![data]
                } else {
                    data.as_object_mut()
                        .ok_or_else(|| Error::Invalid("invalid entity map".into()))?
                        .values_mut()
                        .collect()
                };
                for entity in entities {
                    let header = entity
                        .get_mut("header")
                        .and_then(|v| v.as_object_mut())
                        .ok_or_else(|| Error::Invalid("missing header".into()))?;
                    ensure(
                        header.get("schema_version").and_then(|v| v.as_u64()) == Some(1),
                        "unsupported schema 1 entity header",
                    )?;
                    header.insert("schema_version".into(), 2.into());
                }
            }
            migrate_inner(value, 2)
        }
        0 => {
            let object = value
                .as_object_mut()
                .ok_or_else(|| Error::Invalid("model must be an object".into()))?;
            object.insert("schema_version".into(), 1.into());
            for collection in [
                "project",
                "sites",
                "buildings",
                "levels",
                "walls",
                "materials",
                "views",
            ] {
                let data = object
                    .get_mut(collection)
                    .ok_or_else(|| Error::Invalid(format!("missing {collection}")))?;
                let entities: Vec<&mut serde_json::Value> = if collection == "project" {
                    vec![data]
                } else {
                    data.as_object_mut()
                        .ok_or_else(|| Error::Invalid("invalid entity map".into()))?
                        .values_mut()
                        .collect()
                };
                for entity in entities {
                    let header = entity
                        .get_mut("header")
                        .and_then(|h| h.as_object_mut())
                        .ok_or_else(|| Error::Invalid("missing header".into()))?;
                    header.insert("schema_version".into(), 1.into());
                    header
                        .entry("properties")
                        .or_insert_with(|| serde_json::json!({}));
                    header
                        .entry("relationships")
                        .or_insert_with(|| serde_json::json!({}));
                    if collection == "levels" {
                        let p = entity
                            .get_mut("parameters")
                            .and_then(|p| p.as_object_mut())
                            .ok_or_else(|| Error::Invalid("missing level parameters".into()))?;
                        let z = p
                            .remove("z")
                            .ok_or_else(|| Error::Invalid("schema 0 level is missing z".into()))?;
                        ensure(!p.contains_key("elevation"), "ambiguous schema 0 level")?;
                        p.insert("elevation".into(), z);
                    }
                }
            }
            migrate_inner(value, 1)
        }
        version => Err(Error::Unsupported(format!("model schema {version}"))),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn migrated_model_budget_matches_the_native_writer_without_an_extra_buffer() {
        let model = os_model::Model::new("Budget");
        let length = serde_json::to_vec_pretty(&model).unwrap().len() as u64;
        assert!(super::validate_serialized_model_size(&model, length).is_ok());
        assert!(super::validate_serialized_model_size(&model, length - 1).is_err());
        assert!(super::validate_serialized_model_size(&model, 0).is_err());
    }
    use super::*;
    fn archive(path: &Path, manifest: &Manifest, model: &serde_json::Value, extra: bool) {
        let mut zip = ZipWriter::new(File::create(path).unwrap());
        let options = SimpleFileOptions::default();
        zip.start_file("manifest.toml", options).unwrap();
        zip.write_all(toml::to_string(manifest).unwrap().as_bytes())
            .unwrap();
        let data = serde_json::to_vec(model).unwrap();
        zip.start_file("model.json", options).unwrap();
        zip.write_all(&data).unwrap();
        if extra {
            zip.start_file("model.copy", options).unwrap();
            zip.write_all(&data).unwrap();
        }
        zip.finish().unwrap();
    }
    fn manifest(model: &serde_json::Value) -> Manifest {
        Manifest {
            format: "OpenStructure".into(),
            container_version: 1,
            schema_version: model["schema_version"].as_u64().unwrap() as u32,
            project_id: serde_json::from_value(model["project"]["header"]["id"].clone()).unwrap(),
            model_entry: "model.json".into(),
            units: "metres".into(),
        }
    }
    #[test]
    fn old_container_opens_migrates_and_resaves_as_current_schema() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("old.osb");
        let value: serde_json::Value =
            serde_json::from_str(include_str!("../../../fixtures/schema-0-model.json")).unwrap();
        let old_manifest = manifest(&value);
        archive(&path, &old_manifest, &value, false);
        let document = ZipJsonStorage.open(&path).unwrap();
        assert_eq!(document.model().project.id(), old_manifest.project_id);
        assert_eq!(document.model().schema_version, SCHEMA_VERSION);
        assert_eq!(
            document
                .model()
                .levels
                .values()
                .next()
                .unwrap()
                .parameters
                .elevation,
            2.5
        );
        ZipJsonStorage.save(&document, &path).unwrap();
        assert_eq!(
            ZipJsonStorage.open(&path).unwrap().model(),
            document.model()
        );
        let mut zip = ZipArchive::new(File::open(path).unwrap()).unwrap();
        let manifest: Manifest =
            toml::from_str(&read_entry(&mut zip, "manifest.toml", MAX_MANIFEST_BYTES).unwrap())
                .unwrap();
        assert_eq!(manifest.schema_version, SCHEMA_VERSION);
    }
    #[test]
    fn all_entity_types_and_extension_data_survive_storage() {
        use os_core::Point2;
        use os_model::{Material, MaterialParams, Wall, WallParams};
        let mut model = Model::new("All entities");
        let material = Material::new(
            "core.material",
            MaterialParams {
                name: "Timber".into(),
                density_kg_m3: 500.0,
            },
        );
        let material_id = material.id();
        model.materials.insert(material_id, material);
        let level = *model.levels.keys().next().unwrap();
        let mut wall = Wall::new(
            "org.openstructure.walls.wall",
            WallParams {
                name: "Wall".into(),
                start: Point2::new(1.0, 2.0),
                end: Point2::new(6.0, 2.0),
                thickness: 0.2,
                height: 3.0,
                level,
                material: Some(material_id),
            },
        );
        wall.header.properties.insert(
            "example.fire-rating".into(),
            serde_json::json!({"minutes":60,"notes":["test fixture"]}),
        );
        wall.header
            .relationships
            .insert("example.reference".into(), vec![material_id, level]);
        model.walls.insert(wall.id(), wall);
        let document = Document::from_model(model).unwrap();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("complete.osb");
        ZipJsonStorage.save(&document, &path).unwrap();
        assert_eq!(
            ZipJsonStorage.open(&path).unwrap().model(),
            document.model()
        );
    }
    #[test]
    fn corrupt_versions_identity_and_relationships_fail_at_the_open_boundary() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("bad.osb");
        for case in [
            "container",
            "future",
            "mismatch",
            "project",
            "units",
            "backend",
            "entity_schema",
            "relationship",
        ] {
            let mut model = serde_json::to_value(Model::new("Test")).unwrap();
            let mut metadata = manifest(&model);
            match case {
                "container" => metadata.container_version = 999,
                "future" => {
                    metadata.schema_version = 999;
                    model["schema_version"] = 999.into();
                }
                "mismatch" => model["schema_version"] = 0.into(),
                "project" => metadata.project_id = Id::new(),
                "units" => metadata.units = "feet".into(),
                "backend" => metadata.model_entry = "model.sqlite".into(),
                "entity_schema" => model["project"]["header"]["schema_version"] = 999.into(),
                "relationship" => {
                    model["project"]["header"]["relationships"] =
                        serde_json::json!({"missing":[Id::new()]})
                }
                _ => unreachable!(),
            }
            archive(&path, &metadata, &model, false);
            assert!(
                ZipJsonStorage.open(&path).is_err(),
                "accepted corrupt {case}"
            );
        }
    }
    #[test]
    fn duplicate_model_entries_are_not_silently_deduplicated() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("duplicate.osb");
        let model = serde_json::to_value(Model::new("Test")).unwrap();
        archive(&path, &manifest(&model), &model, true);
        // The ZIP writer forbids duplicates. Rename the second entry in both
        // its local and central headers to create an independently malformed ZIP.
        let mut bytes = std::fs::read(&path).unwrap();
        let matches: Vec<_> = bytes
            .windows(10)
            .enumerate()
            .filter_map(|(i, b)| (b == b"model.copy").then_some(i))
            .collect();
        assert_eq!(matches.len(), 2);
        for i in matches {
            bytes[i..i + 10].copy_from_slice(b"model.json");
        }
        std::fs::write(&path, bytes).unwrap();
        let error = ZipJsonStorage
            .open(&path)
            .err()
            .expect("duplicate model accepted");
        assert!(
            error.to_string().contains("duplicate archive entry"),
            "{error}"
        );
    }
    #[test]
    fn save_reopen_and_replace() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.osb");
        let mut doc = Document::new("Original").unwrap();
        ZipJsonStorage.save(&doc, &path).unwrap();
        doc.execute(
            "rename",
            vec![os_document::Command::RenameProject("Changed".into())],
        )
        .unwrap();
        ZipJsonStorage.save(&doc, &path).unwrap();
        let opened = ZipJsonStorage.open(&path).unwrap();
        assert_eq!(doc.model(), opened.model());
        assert!(!opened.can_undo());
    }
    #[test]
    fn migration_fixture_and_future_rejection() {
        let mut v: serde_json::Value =
            serde_json::from_str(include_str!("../../../fixtures/schema-0-model.json")).unwrap();
        migrate(&mut v, 0).unwrap();
        let model: Model = serde_json::from_value(v).unwrap();
        model.validate().unwrap();
        assert_eq!(
            model.levels.values().next().unwrap().parameters.elevation,
            2.5
        );
        assert!(migrate(&mut serde_json::json!({}), 99).is_err());
    }
    #[test]
    fn malformed_archives_are_errors() {
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(file.path(), b"not a zip").unwrap();
        assert!(ZipJsonStorage.open(file.path()).is_err());
    }
}
