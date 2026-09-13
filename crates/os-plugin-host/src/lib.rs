//! Permission-checked transport and document command routing.
use os_core::{Error, Result, ensure};
use os_document::{Command, Document};
use os_model::Model;
use os_plugin_api::*;
use std::collections::{BTreeMap, BTreeSet};
pub mod generic;
pub mod generic_geometry;
mod native_wall;
pub mod plan_graphics;

#[cfg(feature = "wasm")]
pub mod loading;
#[cfg(feature = "wasm")]
pub mod migration;
#[cfg(feature = "wasm")]
pub mod wasm;
#[cfg(feature = "wasm")]
pub mod worker;

struct Loaded {
    activation_id: os_core::Id,
    catalog: Option<os_plugin_api::generic::Catalog>,
    #[cfg(feature = "wasm")]
    worker: Option<std::sync::Arc<worker::Runtime>>,
    plugin: Box<dyn Plugin>,
    manifest: Manifest,
    grants: BTreeSet<Permission>,
}
#[derive(Default)]
pub struct PluginHost {
    #[cfg(feature = "wasm")]
    load_busy: std::sync::Arc<std::sync::atomic::AtomicBool>,
    #[cfg(feature = "wasm")]
    worker_count: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    loaded: BTreeMap<String, Loaded>,
    registrations: BTreeMap<String, (String, Registration)>,
}
impl PluginHost {
    pub fn load(&mut self, plugin: Box<dyn Plugin>, grants: BTreeSet<Permission>) -> Result<()> {
        ensure(
            plugin.manifest().entrypoint.starts_with("builtin:"),
            "use the explicit Wasm loader for external artifacts",
        )?;
        self.load_checked(plugin, grants)
    }
    fn load_checked(
        &mut self,
        plugin: Box<dyn Plugin>,
        grants: BTreeSet<Permission>,
    ) -> Result<()> {
        let manifest = plugin.manifest();
        self.validate_load(&manifest, &grants)?;
        let catalog = if manifest.api_version == os_plugin_api::generic::VERSION {
            Some(generic::describe(plugin.as_ref(), &manifest)?)
        } else {
            None
        };
        self.install_validated(plugin, manifest, catalog);
        Ok(())
    }
    fn validate_load(&self, manifest: &Manifest, grants: &BTreeSet<Permission>) -> Result<()> {
        manifest.validate()?;
        ensure(
            !self.loaded.contains_key(&manifest.id),
            "plugin already loaded",
        )?;
        if !manifest.permissions.is_subset(grants) {
            return Err(Error::Permission(
                "plugin requested permissions not granted by host".into(),
            ));
        }
        for dependency in &manifest.dependencies {
            ensure(
                self.loaded
                    .get(&dependency.id)
                    .is_some_and(|p| p.manifest.version == dependency.version),
                format!(
                    "unsatisfied dependency {}@{}",
                    dependency.id, dependency.version
                ),
            )?;
        }
        // Plugin namespaces can be nested, so validating each manifest alone
        // cannot establish global ownership of a registration ID.
        for registration in &manifest.registrations {
            ensure(
                !self.registrations.contains_key(&registration.id),
                format!(
                    "registration {} is already owned by another plugin",
                    registration.id
                ),
            )?;
        }
        Ok(())
    }
    fn install_validated(
        &mut self,
        plugin: Box<dyn Plugin>,
        manifest: Manifest,
        catalog: Option<os_plugin_api::generic::Catalog>,
    ) {
        let grants = manifest.permissions.clone();
        for registration in &manifest.registrations {
            self.registrations.insert(
                registration.id.clone(),
                (manifest.id.clone(), registration.clone()),
            );
        }
        self.loaded.insert(
            manifest.id.clone(),
            Loaded {
                activation_id: os_core::Id::new(),
                catalog,
                #[cfg(feature = "wasm")]
                worker: None,
                plugin,
                manifest,
                grants,
            },
        );
    }
    /// Ephemeral activation identity for invalidating reviewed UI drafts on reload.
    pub fn activation_id(&self, plugin: &str) -> Option<os_core::Id> {
        self.loaded.get(plugin).map(|p| p.activation_id)
    }
    pub fn manifests(&self) -> impl Iterator<Item = &Manifest> {
        self.loaded.values().map(|p| &p.manifest)
    }
    /// Effective grants, restricted to the loaded manifest's requested permissions.
    pub fn granted_permissions(&self, id: &str) -> Option<&BTreeSet<Permission>> {
        self.loaded.get(id).map(|p| &p.grants)
    }
    /// Remove callable registrations without altering any document data.
    /// In-flight Wasm jobs retain their runtime but their replies become invalid.
    pub fn unload(&mut self, id: &str) -> Result<()> {
        self.plugin(id)?;
        ensure(
            !self
                .loaded
                .values()
                .any(|p| p.manifest.dependencies.iter().any(|d| d.id == id)),
            "plugin is required by a loaded dependency",
        )?;
        self.loaded.remove(id);
        self.registrations.retain(|_, (owner, _)| owner != id);
        Ok(())
    }
    /// Discover registered extensions and their owning plugin. Registration does
    /// not imply a future service category has an executable transport yet.
    pub fn registrations(
        &self,
        kind: RegistrationKind,
    ) -> impl Iterator<Item = (&str, &Registration)> {
        self.registrations
            .values()
            .filter(move |(_, registration)| registration.kind == kind)
            .map(|(owner, registration)| (owner.as_str(), registration))
    }
    fn plugin(&self, id: &str) -> Result<&Loaded> {
        self.loaded
            .get(id)
            .ok_or_else(|| Error::Invalid(format!("plugin {id} is not loaded")))
    }
    pub fn request(&self, plugin_id: &str, model: &Model, request: Request) -> Result<Response> {
        let p = self.plugin(plugin_id)?;
        #[cfg(feature = "wasm")]
        ensure(
            !p.worker.as_ref().is_some_and(|r| r.is_busy()),
            "plugin already has a live worker",
        )?;
        let geometry = matches!(request, Request::GenerateWall { .. });
        let input = Self::prepare_input(p, model, request)?;
        let output = p.plugin.invoke_json(&input)?;
        Self::validate_output(p, model, geometry, &output)
    }
    fn prepare_input(p: &Loaded, model: &Model, request: Request) -> Result<String> {
        ensure(
            p.manifest.api_version == API_VERSION,
            "legacy request requires API 1",
        )?;
        require(p, Permission::ModelRead)?;
        let geometry = matches!(request, Request::GenerateWall { .. });
        if !geometry {
            require(p, Permission::ModelWrite)?;
        }
        let capability = if geometry {
            Capability::Geometry
        } else {
            Capability::Modeling
        };
        ensure(
            p.manifest.capabilities.contains(&capability),
            "plugin lacks requested capability",
        )?;
        let input = serde_json::to_string(&RequestEnvelope {
            api_version: API_VERSION,
            request,
            model: Some(model.clone()),
        })
        .map_err(invalid)?;
        ensure(input.len() <= MAX_MESSAGE_BYTES, "plugin request too large")?;
        Ok(input)
    }
    fn validate_output(
        p: &Loaded,
        model: &Model,
        geometry: bool,
        output: &str,
    ) -> Result<Response> {
        ensure(
            output.len() <= MAX_MESSAGE_BYTES,
            "plugin response too large",
        )?;
        let result: ResponseEnvelope = serde_json::from_str(output).map_err(invalid)?;
        ensure(
            result.api_version == API_VERSION,
            "plugin response version mismatch",
        )?;
        match &result.response {
            Response::Commands(commands) => {
                ensure(!geometry, "geometry request cannot emit commands")?;
                require(p, Permission::ModelWrite)?;
                ensure(
                    !commands.is_empty() && commands.len() <= 1000,
                    "invalid plugin command count",
                )?;
                for command in commands {
                    authorize_command(p, model, command)?;
                }
            }
            Response::Solid(_) => ensure(geometry, "modeling request must emit commands")?,
        }
        Ok(result.response)
    }
    pub fn execute(
        &self,
        plugin: &str,
        document: &mut Document,
        label: &str,
        request: Request,
    ) -> Result<()> {
        let response = self.request(plugin, document.model(), request)?;
        match response {
            Response::Commands(commands) => document.execute(label, commands),
            Response::Solid(_) => Err(Error::Invalid("expected a modeling response".into())),
        }
    }
}
fn invalid(error: impl std::fmt::Display) -> Error {
    Error::Invalid(error.to_string())
}
fn require(plugin: &Loaded, permission: Permission) -> Result<()> {
    if plugin.grants.contains(&permission) {
        Ok(())
    } else {
        Err(Error::Permission(format!("{permission:?}")))
    }
}
fn authorize_command(plugin: &Loaded, model: &Model, command: &Command) -> Result<()> {
    let type_id = match command {
        Command::AddWall(wall) => wall.header.type_id.as_str(),
        Command::UpdateWall { id, .. } | Command::RemoveWall(id) => model
            .walls
            .get(id)
            .ok_or_else(|| Error::Invalid("target wall missing".into()))?
            .header
            .type_id
            .as_str(),
        _ => {
            return Err(Error::Permission(
                "plugin may only edit its registered element types".into(),
            ));
        }
    };
    if plugin
        .manifest
        .registrations
        .iter()
        .any(|r| r.kind == RegistrationKind::ElementType && r.id == type_id)
    {
        Ok(())
    } else {
        Err(Error::Permission("unregistered element type".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, rc::Rc};
    struct Probe {
        manifest: Manifest,
        calls: Rc<Cell<usize>>,
    }
    impl Plugin for Probe {
        fn manifest(&self) -> Manifest {
            self.manifest.clone()
        }
        fn invoke_json(&self, _: &str) -> Result<String> {
            self.calls.set(self.calls.get() + 1);
            Err(Error::Unsupported("registration-only test plugin".into()))
        }
    }
    fn bundled_manifest() -> Manifest {
        Manifest::from_toml(include_str!("../../../plugins/walls/plugin.toml")).unwrap()
    }
    #[test]
    fn every_required_registration_category_is_discoverable_with_its_owner() {
        let mut manifest = bundled_manifest();
        manifest.capabilities = vec![
            Capability::Modeling,
            Capability::Geometry,
            Capability::Views,
            Capability::Exchange,
            Capability::Reports,
            Capability::Analysis,
        ];
        let kinds = [
            RegistrationKind::ElementType,
            RegistrationKind::Tool,
            RegistrationKind::PropertyPanel,
            RegistrationKind::ViewProvider,
            RegistrationKind::Importer,
            RegistrationKind::Exporter,
            RegistrationKind::Schedule,
            RegistrationKind::Report,
            RegistrationKind::AnalysisService,
        ];
        manifest.registrations = kinds
            .iter()
            .enumerate()
            .map(|(i, kind)| Registration {
                id: format!("{}.extension{i}", manifest.id),
                name: format!("Extension {i}"),
                kind: kind.clone(),
            })
            .collect();
        let owner = manifest.id.clone();
        let mut host = PluginHost::default();
        host.load(
            Box::new(Probe {
                manifest,
                calls: Rc::new(Cell::new(0)),
            }),
            grants(),
        )
        .unwrap();
        for kind in kinds {
            let registered: Vec<_> = host.registrations(kind.clone()).collect();
            assert_eq!(registered.len(), 1);
            assert_eq!(registered[0].0, owner);
            assert_eq!(registered[0].1.kind, kind);
        }
    }
    #[test]
    fn overlapping_namespaces_cannot_claim_an_existing_registration() {
        let mut host = PluginHost::default();
        host.load(
            Box::new(Probe {
                manifest: bundled_manifest(),
                calls: Rc::new(Cell::new(0)),
            }),
            grants(),
        )
        .unwrap();
        let mut overlapping = bundled_manifest();
        overlapping.id = "org.openstructure".into();
        overlapping.validate().unwrap();
        assert!(
            host.load(
                Box::new(Probe {
                    manifest: overlapping,
                    calls: Rc::new(Cell::new(0))
                }),
                grants()
            )
            .is_err()
        );
        assert_eq!(host.manifests().count(), 1);
        assert_eq!(
            host.registrations(RegistrationKind::ElementType)
                .next()
                .unwrap()
                .0,
            "org.openstructure.walls"
        );
    }
    #[test]
    fn dependencies_are_checked_before_any_registration_is_published() {
        let mut host = PluginHost::default();
        let mut dependent = bundled_manifest();
        dependent.dependencies = vec![Dependency {
            id: "org.example.foundation".into(),
            version: "1.2.3".into(),
        }];
        assert!(
            host.load(
                Box::new(Probe {
                    manifest: dependent.clone(),
                    calls: Rc::new(Cell::new(0))
                }),
                grants()
            )
            .is_err()
        );
        assert_eq!(host.registrations(RegistrationKind::Tool).count(), 0);
        let mut foundation = bundled_manifest();
        foundation.id = "org.example.foundation".into();
        foundation.version = "1.2.2".into();
        foundation.registrations.clear();
        host.load(
            Box::new(Probe {
                manifest: foundation,
                calls: Rc::new(Cell::new(0)),
            }),
            grants(),
        )
        .unwrap();
        assert!(
            host.load(
                Box::new(Probe {
                    manifest: dependent.clone(),
                    calls: Rc::new(Cell::new(0))
                }),
                grants()
            )
            .is_err()
        );
        dependent.dependencies[0].version = "1.2.2".into();
        host.load(
            Box::new(Probe {
                manifest: dependent,
                calls: Rc::new(Cell::new(0)),
            }),
            grants(),
        )
        .unwrap();
        assert_eq!(host.manifests().count(), 2);
    }
    #[test]
    fn extra_host_grants_do_not_authorize_unrequested_model_access() {
        for permission in [Permission::ModelRead, Permission::ModelWrite] {
            let calls = Rc::new(Cell::new(0));
            let mut manifest = bundled_manifest();
            manifest.permissions.remove(&permission);
            let mut host = PluginHost::default();
            host.load(
                Box::new(Probe {
                    manifest,
                    calls: calls.clone(),
                }),
                grants(),
            )
            .unwrap();
            let mut document = Document::new("Test").unwrap();
            let model = document.model().clone();
            let request = Request::CreateWall(os_model::WallParams {
                name: "Wall".into(),
                start: os_core::Point2::new(0.0, 0.0),
                end: os_core::Point2::new(5.0, 0.0),
                thickness: 0.2,
                height: 3.0,
                level: *model.levels.keys().next().unwrap(),
                material: None,
            });
            assert!(matches!(
                host.execute("org.openstructure.walls", &mut document, "Denied", request),
                Err(Error::Permission(_))
            ));
            assert_eq!(calls.get(), 0);
            assert_eq!(document.model(), &model);
            assert_eq!(document.revision(), 0);
        }
    }
    struct Rogue {
        response: String,
        writable: bool,
    }
    impl Plugin for Rogue {
        fn manifest(&self) -> Manifest {
            let mut m =
                Manifest::from_toml(include_str!("../../../plugins/walls/plugin.toml")).unwrap();
            if !self.writable {
                m.permissions.remove(&Permission::ModelWrite);
            }
            m
        }
        fn invoke_json(&self, _: &str) -> Result<String> {
            Ok(self.response.clone())
        }
    }
    fn grants() -> BTreeSet<Permission> {
        [
            Permission::ModelRead,
            Permission::ModelWrite,
            Permission::UiTool,
            Permission::UiPanel,
        ]
        .into()
    }
    #[test]
    fn refuses_missing_grants() {
        assert!(
            PluginHost::default()
                .load(
                    Box::new(Rogue {
                        response: String::new(),
                        writable: true
                    }),
                    BTreeSet::new()
                )
                .is_err()
        );
    }
    #[test]
    fn rogue_geometry_response_cannot_mutate_document() {
        let mut host = PluginHost::default();
        let mut doc = Document::new("Original").unwrap();
        let response = serde_json::to_string(&ResponseEnvelope {
            api_version: API_VERSION,
            response: Response::Commands(vec![Command::RenameProject("Hacked".into())]),
        })
        .unwrap();
        host.load(
            Box::new(Rogue {
                response,
                writable: false,
            }),
            grants(),
        )
        .unwrap();
        assert!(
            host.execute(
                "org.openstructure.walls",
                &mut doc,
                "bad",
                Request::GenerateWall {
                    id: os_core::Id::new()
                }
            )
            .is_err()
        );
        assert_eq!(doc.model().project.parameters.name, "Original");
    }
    #[test]
    fn denies_cross_type_commands_and_future_protocol() {
        for response in [
            ResponseEnvelope {
                api_version: 1,
                response: Response::Commands(vec![Command::RenameProject("Hacked".into())]),
            },
            ResponseEnvelope {
                api_version: 999,
                response: Response::Commands(vec![]),
            },
        ] {
            let mut host = PluginHost::default();
            let mut doc = Document::new("Original").unwrap();
            host.load(
                Box::new(Rogue {
                    response: serde_json::to_string(&response).unwrap(),
                    writable: true,
                }),
                grants(),
            )
            .unwrap();
            let params = os_model::WallParams {
                name: "W".into(),
                start: os_core::Point2::new(0.0, 0.0),
                end: os_core::Point2::new(1.0, 0.0),
                thickness: 0.2,
                height: 3.0,
                level: *doc.model().levels.keys().next().unwrap(),
                material: None,
            };
            assert!(
                host.execute(
                    "org.openstructure.walls",
                    &mut doc,
                    "bad",
                    Request::CreateWall(params)
                )
                .is_err()
            );
            assert!(!doc.can_undo());
        }
    }

    #[test]
    fn v1_registered_owner_still_cannot_modify_extension_envelopes_or_requirements() {
        let mut model = Model::new("Protected extension");
        let mut entity: os_model::ExtensionEntity =
            serde_json::from_str(include_str!("../../../fixtures/extension-envelope-v1.json"))
                .unwrap();
        entity.owner = "org.openstructure.walls".into();
        entity.type_id = "org.openstructure.walls.wall".into();
        model.plugin_requirements.insert(
            entity.owner.clone(),
            os_model::PluginRequirement {
                version: "0.1.0".into(),
            },
        );
        model.extensions.insert(entity.id, entity.clone());
        let mut doc = Document::from_model(model.clone()).unwrap();
        for command in [
            Command::AddExtension(entity.clone()),
            Command::ReplaceExtension {
                expected_schema_version: 1,
                entity: entity.clone(),
            },
            Command::RemoveExtension(entity.id),
            Command::SetPluginRequirement {
                plugin_id: entity.owner.clone(),
                requirement: None,
            },
        ] {
            let mut host = PluginHost::default();
            host.load(
                Box::new(Rogue {
                    response: serde_json::to_string(&ResponseEnvelope {
                        api_version: 1,
                        response: Response::Commands(vec![command]),
                    })
                    .unwrap(),
                    writable: true,
                }),
                grants(),
            )
            .unwrap();
            let parameters = os_model::WallParams {
                name: "W".into(),
                start: os_core::Point2::new(0., 0.),
                end: os_core::Point2::new(1., 0.),
                thickness: 0.2,
                height: 3.,
                level: *model.levels.keys().next().unwrap(),
                material: None,
            };
            assert!(matches!(
                host.execute(
                    "org.openstructure.walls",
                    &mut doc,
                    "Denied",
                    Request::CreateWall(parameters)
                ),
                Err(Error::Permission(_))
            ));
            assert_eq!(doc.model(), &model);
            assert_eq!(doc.revision(), 0);
            assert!(!doc.can_undo());
        }
    }
}
