//! Generate an original inert fixture, not an executable column plugin.
use os_document::Document;
use os_model::{ExtensionEntity, Model, PluginRequirement};
use os_storage::{StorageBackend, ZipJsonStorage};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("usage: preserved_extension <new-output.osb>")?,
    );
    let mut model = Model::new("Missing plugin preservation");
    let mut entity: ExtensionEntity =
        serde_json::from_str(include_str!("../../../fixtures/extension-envelope-v1.json"))?;
    entity
        .depends_on
        .insert(*model.levels.keys().next().ok_or("level missing")?);
    model.plugin_requirements.insert(
        entity.owner.clone(),
        PluginRequirement {
            version: "1.0.0".into(),
        },
    );
    model.extensions.insert(entity.id, entity);
    let document = Document::from_model(model)?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(std::path::Path::new("."));
    let temporary = tempfile::NamedTempFile::new_in(parent)?.into_temp_path();
    ZipJsonStorage.save(&document, &temporary)?;
    temporary.persist_noclobber(&path)?;
    println!(
        "Preserved plugin fixture: {} (no executable plugin)",
        path.display()
    );
    Ok(())
}
