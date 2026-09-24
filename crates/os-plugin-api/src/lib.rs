//! Independent plugin metadata; wall-specific v1 messages require `legacy-v1`.
//!
//! Contract-only consumers use `default-features = false`. That configuration
//! has no dependency on model, document, geometry, UI, or the native UUID runtime.
#[cfg(feature = "legacy-v1")]
use os_core::Id;
#[cfg(feature = "legacy-v1")]
use os_document::Command;
#[cfg(feature = "legacy-v1")]
use os_geometry::Solid;
#[cfg(feature = "legacy-v1")]
use os_model::{Model, WallParams};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Full native Model transport, schema 28. Older native guests require rebuilding.
/// Generic API 2 uses independent DTOs and remains compatible.
pub const API_VERSION: u32 = 9;
pub const REQUIRED_MODEL_SCHEMA_VERSION: u32 = 28;
pub mod generic;
pub mod geometry;
pub mod plan;
pub mod wall;
pub const MAX_MESSAGE_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_MANIFEST_BYTES: usize = 64 * 1024;
pub const MAX_REGISTRATIONS: usize = 256;
pub const MAX_DEPENDENCIES: usize = 128;

/// Contract errors must not depend on a host's document or storage error types.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProtocolError(pub String);
impl std::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for ProtocolError {}
pub type ProtocolResult<T> = std::result::Result<T, ProtocolError>;
fn ensure(condition: bool, message: impl Into<String>) -> ProtocolResult<()> {
    if condition {
        Ok(())
    } else {
        Err(ProtocolError(message.into()))
    }
}

#[cfg(feature = "legacy-v1")]
impl From<ProtocolError> for os_core::Error {
    fn from(value: ProtocolError) -> Self {
        Self::Invalid(value.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Permission {
    #[serde(rename = "model.read")]
    ModelRead,
    #[serde(rename = "model.write")]
    ModelWrite,
    #[serde(rename = "ui.tool")]
    UiTool,
    #[serde(rename = "ui.panel")]
    UiPanel,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Capability {
    Modeling,
    Geometry,
    Views,
    Exchange,
    Reports,
    Analysis,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegistrationKind {
    ElementType,
    Tool,
    PropertyPanel,
    ViewProvider,
    Importer,
    Exporter,
    Schedule,
    Report,
    AnalysisService,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Registration {
    pub id: String,
    pub name: String,
    pub kind: RegistrationKind,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dependency {
    pub id: String,
    pub version: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub api_version: u32,
    pub entrypoint: String,
    pub dependencies: Vec<Dependency>,
    pub capabilities: Vec<Capability>,
    pub permissions: BTreeSet<Permission>,
    pub registrations: Vec<Registration>,
}
impl Manifest {
    pub fn from_toml(text: &str) -> ProtocolResult<Self> {
        ensure(
            text.len() <= MAX_MANIFEST_BYTES,
            "plugin manifest exceeds 64 KiB",
        )?;
        let m: Self = toml::from_str(text).map_err(|e| ProtocolError(e.to_string()))?;
        m.validate()?;
        Ok(m)
    }
    pub fn validate(&self) -> ProtocolResult<()> {
        ensure(
            matches!(self.api_version, API_VERSION | generic::VERSION),
            "plugin API version mismatch: native Model guests require API 9/schema 28; rebuild and reinstall API 1/API 3/API 4/API 5/API 6/API 7/API 8 guests",
        )?;
        ensure(
            self.id.len() <= 128 && valid_id(&self.id),
            "invalid namespaced plugin ID",
        )?;
        ensure(valid_name(&self.name), "invalid plugin name")?;
        ensure(
            valid_version(&self.version),
            "expected numeric major.minor.patch plugin version",
        )?;
        ensure(
            !self.entrypoint.trim().is_empty() && self.entrypoint.len() <= 1024,
            "missing plugin entrypoint",
        )?;
        let mut dependencies = BTreeSet::new();
        ensure(
            self.dependencies.len() <= MAX_DEPENDENCIES,
            "too many plugin dependencies",
        )?;
        ensure(
            self.registrations.len() <= MAX_REGISTRATIONS,
            "too many plugin registrations",
        )?;
        ensure(self.capabilities.len() <= 6, "too many plugin capabilities")?;
        for d in &self.dependencies {
            ensure(
                d.id.len() <= 128
                    && valid_id(&d.id)
                    && d.id != self.id
                    && valid_version(&d.version),
                "invalid plugin dependency",
            )?;
            ensure(dependencies.insert(&d.id), "duplicate plugin dependency")?;
        }
        let mut ids = BTreeSet::new();
        for r in &self.registrations {
            ensure(
                r.id.starts_with(&format!("{}.", self.id)) && valid_id(&r.id),
                "registration outside plugin namespace",
            )?;
            ensure(
                ids.insert(&r.id) && valid_name(&r.name),
                "duplicate or unnamed registration",
            )?;
            let required = match r.kind {
                RegistrationKind::Tool => Some(Permission::UiTool),
                RegistrationKind::PropertyPanel => Some(Permission::UiPanel),
                _ => None,
            };
            if let Some(p) = required {
                ensure(
                    self.permissions.contains(&p),
                    "registration lacks required permission",
                )?;
            }
            let capability = match r.kind {
                RegistrationKind::ElementType
                | RegistrationKind::Tool
                | RegistrationKind::PropertyPanel => Capability::Modeling,
                RegistrationKind::ViewProvider => Capability::Views,
                RegistrationKind::Importer | RegistrationKind::Exporter => Capability::Exchange,
                RegistrationKind::Schedule | RegistrationKind::Report => Capability::Reports,
                RegistrationKind::AnalysisService => Capability::Analysis,
            };
            ensure(
                self.capabilities.contains(&capability),
                "registration lacks required capability",
            )?;
        }
        Ok(())
    }
}
fn valid_id(id: &str) -> bool {
    id.len() <= 256
        && id.contains('.')
        && id.split('.').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
        })
}
fn valid_name(name: &str) -> bool {
    !name.trim().is_empty() && name.len() <= 256 && !name.chars().any(char::is_control)
}
fn valid_version(version: &str) -> bool {
    let parts: Vec<_> = version.split('.').collect();
    version.len() <= 32
        && parts.len() == 3
        && parts.iter().all(|p| {
            !p.is_empty()
                && (p.len() == 1 || !p.starts_with('0'))
                && p.bytes().all(|c| c.is_ascii_digit())
                && p.parse::<u32>().is_ok()
        })
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "operation", content = "data")]
#[cfg(feature = "legacy-v1")]
pub enum Request {
    CreateWall(WallParams),
    EditWall { id: Id, parameters: WallParams },
    GenerateWall { id: Id },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg(feature = "legacy-v1")]
pub struct RequestEnvelope {
    pub api_version: u32,
    pub request: Request,
    pub model: Option<Model>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "result", content = "data")]
#[cfg(feature = "legacy-v1")]
pub enum Response {
    Commands(Vec<Command>),
    Solid(Solid),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg(feature = "legacy-v1")]
pub struct ResponseEnvelope {
    pub api_version: u32,
    pub response: Response,
}

/// Rust trait for the built-in transport only, never a dynamic-library ABI.
#[cfg(feature = "legacy-v1")]
pub trait Plugin {
    fn manifest(&self) -> Manifest;
    fn invoke_json(&self, request: &str) -> os_core::Result<String>;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_incompatible_versions_and_permissions() {
        let mut m =
            Manifest::from_toml(include_str!("../../../plugins/walls/plugin.toml")).unwrap();
        m.api_version = 999;
        assert!(m.validate().is_err());
        for version in [1, 3, 4, 5, 6, 7, 8] {
            m.api_version = version;
            let error = m.validate().unwrap_err().to_string();
            assert!(
                error.contains("API 9/schema 28")
                    && error.contains("rebuild")
                    && error.contains(&format!("API {version}"))
            );
        }
        m.api_version = generic::VERSION;
        m.validate().unwrap();
        assert_eq!(generic::VERSION, 2);
        #[cfg(feature = "legacy-v1")]
        assert_eq!(REQUIRED_MODEL_SCHEMA_VERSION, os_model::SCHEMA_VERSION);
        m.api_version = API_VERSION;
        m.permissions.clear();
        assert!(m.validate().is_err());
        assert!(!valid_version(".."));
        assert!(!valid_id("org..walls"));
    }

    #[test]
    fn contract_only_metadata_bounds_and_unknown_fields_are_enforced() {
        let source = include_str!("../../../plugins/walls/plugin.toml");
        assert!(Manifest::from_toml(&" ".repeat(MAX_MANIFEST_BYTES + 1)).is_err());
        assert!(Manifest::from_toml(&format!("misspelled_permission = true\n{source}")).is_err());
        for case in [
            "name",
            "id",
            "entry",
            "version",
            "dependencies",
            "registrations",
            "capabilities",
        ] {
            let mut m = Manifest::from_toml(source).unwrap();
            match case {
                "name" => m.name = "x".repeat(257),
                "id" => m.id = format!("org.{}", "x".repeat(128)),
                "entry" => m.entrypoint = "x".repeat(1025),
                "version" => m.version = "01.0.0".into(),
                "dependencies" => {
                    m.dependencies = vec![
                        Dependency {
                            id: "org.example.dependency".into(),
                            version: "1.0.0".into()
                        };
                        MAX_DEPENDENCIES + 1
                    ]
                }
                "registrations" => {
                    m.registrations = vec![m.registrations[0].clone(); MAX_REGISTRATIONS + 1]
                }
                "capabilities" => m.capabilities = vec![Capability::Modeling; 7],
                _ => unreachable!(),
            }
            assert!(m.validate().is_err(), "accepted {case}");
        }
        assert!(Manifest::from_toml(&format!("{source}\nunknown = 'value'\n")).is_err());
    }
}
