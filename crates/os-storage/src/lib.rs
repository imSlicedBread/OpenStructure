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
        28 => Ok(()),
        27 => {
            let object = value
                .as_object_mut()
                .ok_or_else(|| Error::Invalid("model must be an object".into()))?;
            ensure(
                !object.contains_key("plan_graphics_templates")
                    && !object.contains_key("plan_graphics"),
                "ambiguous schema 27 plan graphics",
            )?;
            for collection in [
                "project",
                "sites",
                "buildings",
                "levels",
                "walls",
                "wall_joins",
                "wall_types",
                "openings",
                "opening_types",
                "rooms",
                "room_tags",
                "detail_lines",
                "room_separation_lines",
                "dimensions",
                "grids",
                "materials",
                "views",
                "floors",
                "columns",
                "sheets",
                "schedules",
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
                    ensure(
                        entity["header"]["schema_version"] == 27,
                        "invalid schema 27 native header",
                    )?;
                    entity["header"]["schema_version"] = 28.into();
                }
            }
            object.insert("plan_graphics_templates".into(), serde_json::json!({}));
            object.insert("plan_graphics".into(), serde_json::json!({}));
            object.insert("schema_version".into(), 28.into());
            migrate_inner(value, 28)
        }
        26 => {
            let object = value
                .as_object_mut()
                .ok_or_else(|| Error::Invalid("model must be an object".into()))?;
            ensure(
                !object.contains_key("columns"),
                "ambiguous schema 26 columns",
            )?;
            for collection in [
                "project",
                "sites",
                "buildings",
                "levels",
                "walls",
                "wall_joins",
                "wall_types",
                "openings",
                "opening_types",
                "rooms",
                "room_tags",
                "detail_lines",
                "room_separation_lines",
                "dimensions",
                "grids",
                "materials",
                "views",
                "floors",
                "sheets",
                "schedules",
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
                    ensure(
                        entity["header"]["schema_version"] == 26,
                        "invalid schema 26 native header",
                    )?;
                    entity["header"]["schema_version"] = 27.into();
                }
            }
            object.insert("columns".into(), serde_json::json!({}));
            object.insert("schema_version".into(), 27.into());
            migrate_inner(value, 27)
        }
        25 => {
            let object = value
                .as_object_mut()
                .ok_or_else(|| Error::Invalid("model must be an object".into()))?;
            for collection in [
                "project",
                "sites",
                "buildings",
                "levels",
                "walls",
                "wall_joins",
                "wall_types",
                "openings",
                "opening_types",
                "rooms",
                "room_tags",
                "detail_lines",
                "room_separation_lines",
                "dimensions",
                "grids",
                "materials",
                "views",
                "floors",
                "sheets",
                "schedules",
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
                    ensure(
                        entity["header"]["schema_version"] == 25,
                        "invalid schema 25 native header",
                    )?;
                    entity["header"]["schema_version"] = 26.into();
                    if collection == "opening_types" {
                        let family = entity
                            .get_mut("parameters")
                            .and_then(|p| p.get_mut("family"))
                            .and_then(serde_json::Value::as_object_mut)
                            .ok_or_else(|| Error::Invalid("invalid opening family".into()))?;
                        ensure(
                            family.get("version").and_then(serde_json::Value::as_u64) == Some(2),
                            "invalid schema 25 opening family version",
                        )?;
                        ensure(
                            !family.contains_key("cut_profile"),
                            "ambiguous schema 25 cut profile",
                        )?;
                        ensure(
                            family.get("host_cut") == Some(&serde_json::json!("Rectangular")),
                            "invalid schema 25 host cut",
                        )?;
                        family.insert("version".into(), 3.into());
                        family.insert(
                            "cut_profile".into(),
                            serde_json::to_value(os_model::OpeningFamily::rectangle())
                                .map_err(|e| Error::Invalid(e.to_string()))?,
                        );
                    }
                }
            }
            object.insert("schema_version".into(), 26.into());
            migrate_inner(value, 26)
        }
        24 => {
            let object = value
                .as_object_mut()
                .ok_or_else(|| Error::Invalid("model must be an object".into()))?;
            for collection in [
                "project",
                "sites",
                "buildings",
                "levels",
                "walls",
                "wall_joins",
                "wall_types",
                "openings",
                "opening_types",
                "rooms",
                "room_tags",
                "detail_lines",
                "room_separation_lines",
                "dimensions",
                "grids",
                "materials",
                "views",
                "floors",
                "sheets",
                "schedules",
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
                    ensure(
                        entity["header"]["schema_version"] == 24,
                        "invalid schema 24 native header",
                    )?;
                    entity["header"]["schema_version"] = 25.into();
                    if collection == "opening_types" {
                        let family = entity
                            .get_mut("parameters")
                            .and_then(|p| p.get_mut("family"))
                            .and_then(serde_json::Value::as_object_mut)
                            .ok_or_else(|| Error::Invalid("invalid opening family".into()))?;
                        ensure(
                            family.get("version").and_then(serde_json::Value::as_u64) == Some(1),
                            "invalid schema 24 opening family version",
                        )?;
                        ensure(
                            !family.contains_key("frame_width")
                                && !family.contains_key("frame_depth"),
                            "ambiguous schema 24 frame definition",
                        )?;
                        family.insert("version".into(), 2.into());
                        family.insert("frame_width".into(), 0.0.into());
                        family.insert("frame_depth".into(), 0.05.into());
                    }
                }
            }
            object.insert("schema_version".into(), 25.into());
            migrate_inner(value, 25)
        }
        23 => {
            let object = value
                .as_object_mut()
                .ok_or_else(|| Error::Invalid("model must be an object".into()))?;
            for collection in [
                "project",
                "sites",
                "buildings",
                "levels",
                "walls",
                "wall_joins",
                "wall_types",
                "openings",
                "opening_types",
                "rooms",
                "room_tags",
                "detail_lines",
                "room_separation_lines",
                "dimensions",
                "grids",
                "materials",
                "views",
                "floors",
                "sheets",
                "schedules",
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
                    ensure(
                        entity["header"]["schema_version"] == 23,
                        "invalid schema 23 native header",
                    )?;
                    entity["header"]["schema_version"] = 24.into();
                    if collection == "opening_types" {
                        let p = entity
                            .get_mut("parameters")
                            .and_then(|p| p.as_object_mut())
                            .ok_or_else(|| {
                                Error::Invalid("invalid opening type parameters".into())
                            })?;
                        ensure(
                            !p.contains_key("family"),
                            "ambiguous schema 23 family definition",
                        )?;
                        let mut family = serde_json::to_value(os_model::OpeningFamily::default())
                            .map_err(storage)?;
                        let family = family
                            .as_object_mut()
                            .ok_or_else(|| Error::Invalid("invalid opening family".into()))?;
                        family.insert("version".into(), 1.into());
                        family.remove("frame_width");
                        family.remove("frame_depth");
                        family.remove("cut_profile");
                        p.insert("family".into(), serde_json::Value::Object(family.clone()));
                    }
                }
            }
            object.insert("schema_version".into(), 24.into());
            migrate_inner(value, 24)
        }
        22 => {
            let object = value
                .as_object_mut()
                .ok_or_else(|| Error::Invalid("model must be an object".into()))?;
            for collection in [
                "project",
                "sites",
                "buildings",
                "levels",
                "walls",
                "wall_joins",
                "wall_types",
                "openings",
                "opening_types",
                "rooms",
                "room_tags",
                "detail_lines",
                "room_separation_lines",
                "dimensions",
                "grids",
                "materials",
                "views",
                "floors",
                "sheets",
                "schedules",
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
                        .ok_or_else(|| Error::Invalid("missing native header".into()))?;
                    ensure(
                        header.get("schema_version").and_then(|v| v.as_u64()) == Some(22),
                        "invalid schema 22 native header",
                    )?;
                    header.insert("schema_version".into(), 23.into());
                    if collection == "opening_types" {
                        let parameters = entity
                            .get_mut("parameters")
                            .and_then(|p| p.as_object_mut())
                            .ok_or_else(|| {
                                Error::Invalid("invalid opening type parameters".into())
                            })?;
                        ensure(
                            !parameters.contains_key("pane_position"),
                            "ambiguous schema 22 pane position",
                        )?;
                        parameters.insert("pane_position".into(), "Center".into());
                    }
                }
            }
            object.insert("schema_version".into(), 23.into());
            migrate_inner(value, 23)
        }
        21 => {
            let object = value
                .as_object_mut()
                .ok_or_else(|| Error::Invalid("model must be an object".into()))?;
            ensure(
                !object.contains_key("wall_types") && !object.contains_key("wall_type_assignments"),
                "ambiguous schema 21 wall types",
            )?;
            for collection in [
                "project",
                "sites",
                "buildings",
                "levels",
                "walls",
                "wall_joins",
                "openings",
                "opening_types",
                "rooms",
                "room_tags",
                "detail_lines",
                "room_separation_lines",
                "dimensions",
                "grids",
                "materials",
                "views",
                "floors",
                "sheets",
                "schedules",
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
                    ensure(
                        entity["header"]["schema_version"] == 21,
                        "invalid schema 21 native header",
                    )?;
                    entity["header"]["schema_version"] = 22.into();
                }
            }
            object.insert("wall_types".into(), serde_json::json!({}));
            object.insert("wall_type_assignments".into(), serde_json::json!({}));
            object.insert("schema_version".into(), 22.into());
            migrate_inner(value, 22)
        }
        20 => {
            let object = value
                .as_object_mut()
                .ok_or_else(|| Error::Invalid("model must be an object".into()))?;
            for collection in [
                "project",
                "sites",
                "buildings",
                "levels",
                "walls",
                "wall_joins",
                "openings",
                "opening_types",
                "rooms",
                "room_tags",
                "detail_lines",
                "room_separation_lines",
                "dimensions",
                "grids",
                "materials",
                "views",
                "floors",
                "sheets",
                "schedules",
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
                    if collection == "wall_joins" {
                        ensure(
                            entity["header"]["type_id"] == "core.wall_butt_join",
                            "invalid schema 20 join type",
                        )?;
                        let p: os_model::ButtJoinParams =
                            serde_json::from_value(entity["parameters"].clone())
                                .map_err(storage)?;
                        entity["parameters"] =
                            serde_json::to_value(os_model::WallJoinParams::from(p))
                                .map_err(storage)?;
                        entity["header"]["type_id"] = "core.wall_join".into();
                    }
                    let header = entity
                        .get_mut("header")
                        .and_then(|h| h.as_object_mut())
                        .ok_or_else(|| Error::Invalid("missing native header".into()))?;
                    ensure(
                        header.get("schema_version").and_then(|v| v.as_u64()) == Some(20),
                        "invalid schema 20 native header",
                    )?;
                    header.insert("schema_version".into(), 21.into());
                }
            }
            object.insert("schema_version".into(), 21.into());
            migrate_inner(value, 21)
        }
        19 => {
            let object = value
                .as_object_mut()
                .ok_or_else(|| Error::Invalid("model must be an object".into()))?;
            ensure(
                !object.contains_key("wall_joins"),
                "ambiguous schema 19 wall_joins",
            )?;
            for collection in [
                "project",
                "sites",
                "buildings",
                "levels",
                "walls",
                "openings",
                "opening_types",
                "rooms",
                "room_tags",
                "detail_lines",
                "room_separation_lines",
                "dimensions",
                "grids",
                "materials",
                "views",
                "floors",
                "sheets",
                "schedules",
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
                        .ok_or_else(|| Error::Invalid("missing native header".into()))?;
                    ensure(
                        header.get("schema_version").and_then(|v| v.as_u64()) == Some(19),
                        "invalid schema 19 native header",
                    )?;
                    header.insert("schema_version".into(), 20.into());
                }
            }
            object.insert("wall_joins".into(), serde_json::json!({}));
            object.insert("schema_version".into(), 20.into());
            migrate_inner(value, 20)
        }
        18 => {
            let object = value
                .as_object_mut()
                .ok_or_else(|| Error::Invalid("model must be an object".into()))?;
            ensure(
                !object.contains_key("room_separation_lines"),
                "ambiguous schema 18 room_separation_lines",
            )?;
            for collection in [
                "project",
                "sites",
                "buildings",
                "levels",
                "walls",
                "openings",
                "opening_types",
                "rooms",
                "room_tags",
                "detail_lines",
                "dimensions",
                "grids",
                "materials",
                "views",
                "floors",
                "sheets",
                "schedules",
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
                        .ok_or_else(|| Error::Invalid("missing native header".into()))?;
                    ensure(
                        header.get("schema_version").and_then(|v| v.as_u64()) == Some(18),
                        "invalid schema 18 native header",
                    )?;
                    header.insert("schema_version".into(), 19.into());
                }
            }
            object.insert("room_separation_lines".into(), serde_json::json!({}));
            object.insert("schema_version".into(), 19.into());
            migrate_inner(value, 19)
        }

        17 => {
            let object = value
                .as_object_mut()
                .ok_or_else(|| Error::Invalid("model must be an object".into()))?;
            ensure(
                !object.contains_key("detail_lines"),
                "ambiguous schema 17 detail_lines",
            )?;
            for collection in [
                "project",
                "sites",
                "buildings",
                "levels",
                "walls",
                "openings",
                "opening_types",
                "rooms",
                "room_tags",
                "dimensions",
                "grids",
                "materials",
                "views",
                "floors",
                "sheets",
                "schedules",
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
                        .ok_or_else(|| Error::Invalid("missing native header".into()))?;
                    ensure(
                        header.get("schema_version").and_then(|v| v.as_u64()) == Some(17),
                        "invalid schema 17 native header",
                    )?;
                    header.insert("schema_version".into(), 18.into());
                }
            }
            object.insert("detail_lines".into(), serde_json::json!({}));
            object.insert("schema_version".into(), 18.into());
            migrate_inner(value, 18)
        }
        16 => {
            let object = value
                .as_object_mut()
                .ok_or_else(|| Error::Invalid("model must be an object".into()))?;
            let sheets = object
                .get_mut("sheets")
                .and_then(|s| s.as_object_mut())
                .ok_or_else(|| Error::Invalid("missing or invalid sheets".into()))?;
            for sheet in sheets.values_mut() {
                let parameters = sheet
                    .get_mut("parameters")
                    .and_then(|p| p.as_object_mut())
                    .ok_or_else(|| Error::Invalid("missing sheet parameters".into()))?;
                ensure(
                    !parameters.contains_key("schedule_placements"),
                    "ambiguous schema 16 schedule placements",
                )?;
                parameters.insert("schedule_placements".into(), serde_json::json!([]));
            }
            for collection in [
                "project",
                "sites",
                "buildings",
                "levels",
                "walls",
                "openings",
                "opening_types",
                "rooms",
                "room_tags",
                "dimensions",
                "grids",
                "materials",
                "views",
                "floors",
                "sheets",
                "schedules",
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
                        .ok_or_else(|| Error::Invalid("missing native header".into()))?;
                    ensure(
                        header.get("schema_version").and_then(|v| v.as_u64()) == Some(16),
                        "invalid schema 16 native header",
                    )?;
                    header.insert("schema_version".into(), 17.into());
                }
            }
            object.insert("schema_version".into(), 17.into());
            migrate_inner(value, 17)
        }
        15 => {
            let object = value
                .as_object_mut()
                .ok_or_else(|| Error::Invalid("model must be an object".into()))?;
            ensure(
                !object.contains_key("schedules"),
                "ambiguous schema 15 schedules",
            )?;
            for collection in [
                "project",
                "sites",
                "buildings",
                "levels",
                "walls",
                "openings",
                "opening_types",
                "rooms",
                "room_tags",
                "dimensions",
                "grids",
                "materials",
                "views",
                "floors",
                "sheets",
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
                        .ok_or_else(|| Error::Invalid("missing native header".into()))?;
                    ensure(
                        header.get("schema_version").and_then(|v| v.as_u64()) == Some(15),
                        "invalid schema 15 native header",
                    )?;
                    header.insert("schema_version".into(), 16.into());
                }
            }
            object.insert("schedules".into(), serde_json::json!({}));
            object.insert("schema_version".into(), 16.into());
            migrate_inner(value, 16)
        }
        14 => {
            let object = value
                .as_object_mut()
                .ok_or_else(|| Error::Invalid("model must be an object".into()))?;
            for collection in [
                "project",
                "sites",
                "buildings",
                "levels",
                "walls",
                "openings",
                "opening_types",
                "rooms",
                "room_tags",
                "dimensions",
                "grids",
                "materials",
                "views",
                "floors",
                "sheets",
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
                    if collection == "views" {
                        ensure(
                            entity
                                .get("parameters")
                                .and_then(|parameters| parameters.get("section"))
                                .is_none(),
                            "ambiguous schema 14 section settings",
                        )?;
                    }
                    let header = entity
                        .get_mut("header")
                        .and_then(|h| h.as_object_mut())
                        .ok_or_else(|| Error::Invalid("missing native header".into()))?;
                    ensure(
                        header.get("schema_version").and_then(|v| v.as_u64()) == Some(14),
                        "invalid schema 14 native header",
                    )?;
                    header.insert("schema_version".into(), 15.into());
                }
            }
            object.insert("schema_version".into(), 15.into());
            migrate_inner(value, 15)
        }
        13 => {
            let object = value
                .as_object_mut()
                .ok_or_else(|| Error::Invalid("model must be an object".into()))?;
            ensure(!object.contains_key("sheets"), "ambiguous schema 13 sheets")?;
            for collection in [
                "project",
                "sites",
                "buildings",
                "levels",
                "walls",
                "openings",
                "opening_types",
                "rooms",
                "room_tags",
                "dimensions",
                "grids",
                "materials",
                "views",
                "floors",
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
                        .ok_or_else(|| Error::Invalid("missing native header".into()))?;
                    ensure(
                        header.get("schema_version").and_then(|v| v.as_u64()) == Some(13),
                        "invalid schema 13 native header",
                    )?;
                    header.insert("schema_version".into(), 14.into());
                }
            }
            object.insert("sheets".into(), serde_json::json!({}));
            object.insert("schema_version".into(), 14.into());
            migrate_inner(value, 14)
        }
        12 => {
            let object = value
                .as_object_mut()
                .ok_or_else(|| Error::Invalid("model must be an object".into()))?;
            ensure(
                !object.contains_key("room_tags"),
                "ambiguous schema 12 room_tags",
            )?;
            for collection in [
                "project",
                "sites",
                "buildings",
                "levels",
                "walls",
                "openings",
                "opening_types",
                "rooms",
                "dimensions",
                "grids",
                "materials",
                "views",
                "floors",
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
                        .ok_or_else(|| Error::Invalid("missing native header".into()))?;
                    ensure(
                        header.get("schema_version").and_then(|v| v.as_u64()) == Some(12),
                        "invalid schema 12 native header",
                    )?;
                    header.insert("schema_version".into(), 13.into());
                }
            }
            object.insert("room_tags".into(), serde_json::json!({}));
            object.insert("schema_version".into(), 13.into());
            migrate_inner(value, 13)
        }
        11 => {
            let object = value
                .as_object_mut()
                .ok_or_else(|| Error::Invalid("model must be an object".into()))?;
            for collection in [
                "project",
                "sites",
                "buildings",
                "levels",
                "walls",
                "openings",
                "opening_types",
                "rooms",
                "dimensions",
                "grids",
                "materials",
                "views",
                "floors",
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
                    if collection == "dimensions" {
                        ensure(
                            matches!(
                                entity["parameters"]["layout"].as_str(),
                                Some("Aligned" | "Chain" | "Baseline")
                            ),
                            "invalid schema 11 dimension layout",
                        )?;
                    }
                    let header = entity
                        .get_mut("header")
                        .and_then(|h| h.as_object_mut())
                        .ok_or_else(|| Error::Invalid("missing native header".into()))?;
                    ensure(
                        header.get("schema_version").and_then(|v| v.as_u64()) == Some(11),
                        "invalid schema 11 native header",
                    )?;
                    header.insert("schema_version".into(), 12.into());
                }
            }
            object.insert("schema_version".into(), 12.into());
            migrate_inner(value, 12)
        }
        10 => {
            let object = value
                .as_object_mut()
                .ok_or_else(|| Error::Invalid("model must be an object".into()))?;
            for collection in [
                "project",
                "sites",
                "buildings",
                "levels",
                "walls",
                "openings",
                "opening_types",
                "rooms",
                "dimensions",
                "grids",
                "materials",
                "views",
                "floors",
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
                        .ok_or_else(|| Error::Invalid("missing native header".into()))?;
                    ensure(
                        header.get("schema_version").and_then(|v| v.as_u64()) == Some(10),
                        "invalid schema 10 native header",
                    )?;
                    header.insert("schema_version".into(), 11.into());
                    if collection == "dimensions" {
                        let params = entity
                            .get_mut("parameters")
                            .and_then(|p| p.as_object_mut())
                            .ok_or_else(|| Error::Invalid("missing dimension parameters".into()))?;
                        ensure(
                            !params.contains_key("layout")
                                && !params.contains_key("additional")
                                && !params.contains_key("baseline_spacing_m"),
                            "ambiguous schema 10 dimension layout",
                        )?;
                        params.insert("layout".into(), "Aligned".into());
                        params.insert("additional".into(), serde_json::json!([]));
                        params.insert("baseline_spacing_m".into(), 0.25.into());
                    }
                }
            }
            object.insert("schema_version".into(), 11.into());
            migrate_inner(value, 11)
        }
        9 => {
            let object = value
                .as_object_mut()
                .ok_or_else(|| Error::Invalid("model must be an object".into()))?;
            for collection in [
                "project",
                "sites",
                "buildings",
                "levels",
                "walls",
                "openings",
                "opening_types",
                "rooms",
                "dimensions",
                "grids",
                "materials",
                "views",
                "floors",
            ] {
                let data = object
                    .get_mut(collection)
                    .ok_or_else(|| Error::Invalid(format!("missing {collection}")))?;
                let entities: Vec<&mut serde_json::Value> = if collection == "project" {
                    vec![data]
                } else {
                    data.as_object_mut()
                        .ok_or_else(|| Error::Invalid("invalid collection".into()))?
                        .values_mut()
                        .collect()
                };
                for entity in entities {
                    let header = entity
                        .get_mut("header")
                        .and_then(|h| h.as_object_mut())
                        .ok_or_else(|| Error::Invalid("missing native header".into()))?;
                    ensure(
                        header.get("schema_version").and_then(|v| v.as_u64()) == Some(9),
                        "invalid schema 9 native header",
                    )?;
                    header.insert("schema_version".into(), 10.into());
                    if collection == "openings" {
                        let params = entity
                            .get_mut("parameters")
                            .and_then(|p| p.as_object_mut())
                            .ok_or_else(|| Error::Invalid("missing opening parameters".into()))?;
                        ensure(
                            !params.contains_key("hinge") && !params.contains_key("swing"),
                            "ambiguous schema 9 opening orientation",
                        )?;
                        params.insert("hinge".into(), "Start".into());
                        params.insert("swing".into(), "Left".into());
                    }
                }
            }
            object.insert("schema_version".into(), 10.into());
            migrate_inner(value, 10)
        }
        8 => {
            let object = value
                .as_object_mut()
                .ok_or_else(|| Error::Invalid("model must be an object".into()))?;
            ensure(!object.contains_key("floors"), "ambiguous schema 8 floors")?;
            for collection in [
                "project",
                "sites",
                "buildings",
                "levels",
                "walls",
                "openings",
                "opening_types",
                "rooms",
                "dimensions",
                "grids",
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
                        .ok_or_else(|| Error::Invalid("invalid collection".into()))?
                        .values_mut()
                        .collect()
                };
                for entity in entities {
                    let header = entity
                        .get_mut("header")
                        .and_then(|h| h.as_object_mut())
                        .ok_or_else(|| Error::Invalid("missing native header".into()))?;
                    ensure(
                        header.get("schema_version").and_then(|v| v.as_u64()) == Some(8),
                        "invalid schema 8 native header",
                    )?;
                    header.insert("schema_version".into(), 9.into());
                }
            }
            object.insert("floors".into(), serde_json::json!({}));
            object.insert("schema_version".into(), 9.into());
            migrate_inner(value, 9)
        }
        7 => {
            let object = value
                .as_object_mut()
                .ok_or_else(|| Error::Invalid("model must be an object".into()))?;
            ensure(
                !object.contains_key("opening_types"),
                "ambiguous schema 7 opening_types",
            )?;
            for collection in [
                "project",
                "sites",
                "buildings",
                "levels",
                "walls",
                "openings",
                "rooms",
                "dimensions",
                "grids",
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
                        header.get("schema_version").and_then(|v| v.as_u64()) == Some(7),
                        "unsupported schema 7 entity header",
                    )?;
                    header.insert("schema_version".into(), 8.into());
                    if collection == "openings" {
                        let p = entity
                            .get_mut("parameters")
                            .and_then(|v| v.as_object_mut())
                            .ok_or_else(|| Error::Invalid("missing opening parameters".into()))?;
                        ensure(
                            p.len() == 7
                                && ["name", "host", "offset", "kind", "width", "height", "sill"]
                                    .iter()
                                    .all(|key| p.contains_key(*key)),
                            "ambiguous schema 7 opening parameters",
                        )?;
                        let mut legacy = serde_json::Map::new();
                        for key in ["kind", "width", "height", "sill"] {
                            legacy.insert(
                                key.into(),
                                p.remove(key).expect("checked old opening field"),
                            );
                        }
                        p.insert("definition".into(), serde_json::json!({"Legacy": legacy}));
                    }
                }
            }
            object.insert("opening_types".into(), serde_json::json!({}));
            object.insert("schema_version".into(), 8.into());
            migrate_inner(value, 8)
        }
        6 => {
            let object = value
                .as_object_mut()
                .ok_or_else(|| Error::Invalid("model must be an object".into()))?;
            ensure(
                !object.contains_key("dimensions"),
                "ambiguous schema 6 dimensions",
            )?;
            for collection in [
                "project",
                "sites",
                "buildings",
                "levels",
                "walls",
                "openings",
                "rooms",
                "grids",
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
                        header.get("schema_version").and_then(|v| v.as_u64()) == Some(6),
                        "unsupported schema 6 entity header",
                    )?;
                    header.insert("schema_version".into(), 7.into());
                }
            }
            object.insert("dimensions".into(), serde_json::json!({}));
            object.insert("schema_version".into(), 7.into());
            migrate_inner(value, 7)
        }
        5 => {
            let object = value
                .as_object_mut()
                .ok_or_else(|| Error::Invalid("model must be an object".into()))?;
            ensure(!object.contains_key("rooms"), "ambiguous schema 5 rooms")?;
            for collection in [
                "project",
                "sites",
                "buildings",
                "levels",
                "walls",
                "openings",
                "grids",
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
                        header.get("schema_version").and_then(|v| v.as_u64()) == Some(5),
                        "unsupported schema 5 entity header",
                    )?;
                    header.insert("schema_version".into(), 6.into());
                }
            }
            object.insert("rooms".into(), serde_json::json!({}));
            object.insert("schema_version".into(), 6.into());
            migrate_inner(value, 6)
        }
        4 => {
            let object = value
                .as_object_mut()
                .ok_or_else(|| Error::Invalid("model must be an object".into()))?;
            ensure(
                !object.contains_key("openings"),
                "ambiguous schema 4 openings",
            )?;
            for collection in [
                "project",
                "sites",
                "buildings",
                "levels",
                "walls",
                "grids",
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
                        header.get("schema_version").and_then(|v| v.as_u64()) == Some(4),
                        "unsupported schema 4 entity header",
                    )?;
                    header.insert("schema_version".into(), 5.into());
                }
            }
            object.insert("openings".into(), serde_json::json!({}));
            object.insert("schema_version".into(), 5.into());
            migrate_inner(value, 5)
        }
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
            migrate_inner(value, 4)
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
#[path = "tests/openings.rs"]
mod opening_tests;

#[cfg(test)]
#[path = "tests/floors.rs"]
mod floor_tests;

#[cfg(test)]
#[path = "tests/columns.rs"]
mod column_tests;

#[cfg(test)]
#[path = "tests/sheets.rs"]
mod sheet_tests;

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
