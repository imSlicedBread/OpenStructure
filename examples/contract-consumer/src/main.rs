//! Independent native contract consumer. Not an executable modeling plugin.
use os_plugin_api::{Manifest, Permission, RegistrationKind};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let path = args
        .next()
        .ok_or("usage: contract-consumer <plugin.toml>")?;
    let text = std::fs::read_to_string(path)?;
    let manifest = Manifest::from_toml(&text)?;
    if let Some(path) = args.next() {
        let catalog =
            os_plugin_api::generic::Catalog::from_json(&std::fs::read_to_string(path)?, &manifest)?;
        println!(
            "PASS: {} validated generic command descriptors without host dependencies",
            catalog.commands.len()
        );
    }
    assert!(manifest.permissions.contains(&Permission::ModelRead));
    assert!(
        manifest
            .registrations
            .iter()
            .any(|r| r.kind == RegistrationKind::ElementType)
    );
    println!(
        "PASS: independently resolved {} @ {} metadata (API {})",
        manifest.id, manifest.version, manifest.api_version
    );
    Ok(())
}
