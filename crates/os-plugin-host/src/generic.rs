//! Shared preparation/validation for synchronous and worker-based v2 commands.
use crate::{Loaded, PluginHost, invalid, require};
use os_core::{Error, Id, Result, ensure};
use os_document::{Command, Document};
use os_model::{EXTENSION_ENVELOPE_VERSION, ExtensionEntity, PluginRequirement};
use os_plugin_api::{Manifest, Permission, Plugin, generic as wire};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    time::{Duration, Instant},
};

/// Host-owned authorization, never accepted from a guest response.
#[derive(Clone, Debug, Default)]
pub struct Scope {
    pub read: BTreeSet<Id>,
    pub write: BTreeSet<Id>,
    pub create_count: usize,
}
#[derive(Clone, Debug)]
pub struct Invocation {
    pub command_id: String,
    pub inputs: BTreeMap<String, Value>,
    pub scope: Scope,
    /// Acceptance deadline, not OS termination of synchronous trusted Rust code.
    pub timeout: Duration,
}

/// Host-only frozen authority and correlation. Never deserialize from a guest.
pub(super) struct Prepared {
    migration_batch: bool,
    #[cfg(feature = "wasm")]
    pub(super) id: Id,
    pub(super) input: String,
    pub(super) deadline: Instant,
    request: wire::Request,
    invocation: Invocation,
    new_ids: BTreeSet<Id>,
    plugin_id: String,
    plugin_version: String,
}

pub(super) fn decode(output: &str, request: &wire::Request) -> Result<wire::Reply> {
    ensure(
        output.len() <= wire::MAX_BYTES,
        "generic response exceeds 1 MiB",
    )?;
    let response: wire::Response = serde_json::from_str(output).map_err(invalid)?;
    ensure(
        response.api_version == wire::VERSION,
        "generic response version mismatch",
    )?;
    ensure(
        response.request_id == request.request_id && response.context == request.context,
        "generic reply context/request mismatch",
    )?;
    if let wire::Reply::Error(error) = &response.result {
        ensure(
            error.message.len() <= 4096 && error.field.as_ref().is_none_or(|s| s.len() <= 64),
            "oversized structured plugin error",
        )?;
        return Err(Error::Invalid(format!(
            "Plugin {:?}: {}{}",
            error.code,
            error.message,
            error
                .field
                .as_ref()
                .map(|f| format!(" (field {f})"))
                .unwrap_or_default()
        )));
    }
    Ok(response.result)
}

pub(crate) fn describe(plugin: &dyn Plugin, manifest: &Manifest) -> Result<wire::Catalog> {
    let request = wire::Request {
        api_version: wire::VERSION,
        request_id: Id::new().to_string(),
        context: None,
        operation: wire::Operation::Describe,
    };
    let input = serde_json::to_string(&request).map_err(invalid)?;
    let reply = decode(&plugin.invoke_json(&input)?, &request)?;
    let wire::Reply::Catalog(catalog) = reply else {
        return Err(Error::Invalid("expected generic catalog".into()));
    };
    catalog.validate(manifest)?;
    Ok(catalog)
}

pub(super) fn entity_to_wire(entity: &ExtensionEntity) -> wire::Element {
    wire::Element {
        id: entity.id.to_string(),
        owner: entity.owner.clone(),
        type_id: entity.type_id.clone(),
        name: entity.name.clone(),
        payload_schema_version: entity.payload_schema_version,
        payload: entity.payload.clone(),
        relationships: entity
            .relationships
            .iter()
            .map(|(k, v)| (k.clone(), v.iter().map(ToString::to_string).collect()))
            .collect(),
        depends_on: entity.depends_on.iter().map(ToString::to_string).collect(),
    }
}
fn id(text: &str) -> Result<Id> {
    let id: Id = serde_json::from_value(Value::String(text.into())).map_err(invalid)?;
    ensure(
        !id.0.is_nil() && id.to_string() == text,
        "expected canonical non-nil UUID",
    )?;
    Ok(id)
}
fn checked_entity(
    p: &Loaded,
    entity: wire::Element,
    invocation: &Invocation,
    descriptor: &wire::CommandDescriptor,
) -> Result<ExtensionEntity> {
    ensure(
        entity.owner == p.manifest.id && entity.type_id == descriptor.element_type,
        "generic reply changes another owner/type",
    )?;
    let catalog = p
        .catalog
        .as_ref()
        .ok_or_else(|| Error::Invalid("catalog missing".into()))?;
    let kind = catalog
        .types
        .iter()
        .find(|t| t.id == entity.type_id)
        .ok_or_else(|| Error::Permission("unregistered type".into()))?;
    ensure(
        entity.payload_schema_version == kind.payload_schema_version,
        "unsupported generic payload schema",
    )?;
    let values: BTreeMap<_, _> = entity
        .payload
        .as_object()
        .ok_or_else(|| Error::Invalid("typed payload must be an object".into()))?
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    wire::validate_values(&kind.fields, &values, true)?;
    let relationships = entity
        .relationships
        .into_iter()
        .map(|(k, v)| Ok((k, v.iter().map(|s| id(s)).collect::<Result<Vec<_>>>()?)))
        .collect::<Result<BTreeMap<_, _>>>()?;
    let depends_on = entity
        .depends_on
        .iter()
        .map(|s| id(s))
        .collect::<Result<BTreeSet<_>>>()?;
    let result = ExtensionEntity {
        envelope_version: EXTENSION_ENVELOPE_VERSION,
        id: id(&entity.id)?,
        owner: entity.owner,
        type_id: entity.type_id,
        name: entity.name,
        payload_schema_version: entity.payload_schema_version,
        payload: entity.payload,
        relationships,
        depends_on,
    };
    ensure(
        result
            .references()
            .all(|id| invocation.scope.read.contains(id)),
        "generic reference outside read scope",
    )?;
    Ok(result)
}

impl PluginHost {
    /// Installed code cannot regenerate/edit incompatible preserved payloads.
    /// This is an unavailable-state check, not permission to migrate automatically.
    pub fn extension_needs_migration(&self, model: &os_model::Model, id: Id) -> bool {
        let Some(entity) = model.extensions.get(&id) else {
            return false;
        };
        let Some(plugin) = self.loaded.get(&entity.owner) else {
            return false;
        };
        model
            .plugin_requirements
            .get(&entity.owner)
            .is_some_and(|r| r.version != plugin.manifest.version)
            || plugin.catalog.as_ref().is_some_and(|c| {
                c.types
                    .iter()
                    .find(|t| t.id == entity.type_id)
                    .is_none_or(|t| t.payload_schema_version != entity.payload_schema_version)
            })
    }
    /// Validated immutable descriptors, not an assertion that a UI/provider exists.
    pub fn catalog(&self, plugin: &str) -> Option<&wire::Catalog> {
        self.loaded.get(plugin).and_then(|p| p.catalog.as_ref())
    }

    /// Invoke a registered v2 tool with explicit host scope and commit atomically.
    /// Supports preserved extensions, native straight walls and scoped levels.
    pub fn execute_generic(
        &self,
        plugin: &str,
        doc: &mut Document,
        label: &str,
        invocation: Invocation,
    ) -> Result<()> {
        let p = self.plugin(plugin)?;
        let prepared = Self::prepare_generic(p, doc, invocation)?;
        let output = p.plugin.invoke_json(&prepared.input)?;
        ensure(
            Instant::now() < prepared.deadline,
            "generic acceptance deadline expired",
        )?;
        let commands = Self::generic_commands(p, doc, &prepared, &output)?;
        ensure(
            Instant::now() < prepared.deadline,
            "generic acceptance deadline expired",
        )?;
        doc.execute(label, commands)
    }

    pub(super) fn prepare_generic(
        p: &Loaded,
        doc: &Document,
        invocation: Invocation,
    ) -> Result<Prepared> {
        Self::prepare_generic_inner(p, doc, invocation, false)
    }
    pub(super) fn prepare_generic_inner(
        p: &Loaded,
        doc: &Document,
        invocation: Invocation,
        migration_batch: bool,
    ) -> Result<Prepared> {
        let started = Instant::now();
        ensure(
            !invocation.timeout.is_zero() && invocation.timeout <= Duration::from_secs(30),
            "invalid generic acceptance timeout",
        )?;
        let plugin = p.manifest.id.as_str();
        ensure(
            p.manifest.api_version == wire::VERSION,
            "generic command requires API 2",
        )?;
        require(p, Permission::ModelRead)?;
        require(p, Permission::ModelWrite)?;
        require(p, Permission::UiTool)?;
        #[cfg(feature = "wasm")]
        ensure(
            !p.worker.as_ref().is_some_and(|r| r.is_busy()),
            "plugin already has a live worker",
        )?;
        let descriptor = p
            .catalog
            .as_ref()
            .and_then(|c| c.commands.iter().find(|c| c.id == invocation.command_id))
            .ok_or_else(|| Error::Permission("unregistered generic command".into()))?;
        ensure(
            descriptor.enabled,
            descriptor
                .disabled_reason
                .as_deref()
                .unwrap_or("command disabled"),
        )?;
        wire::validate_values(&descriptor.fields, &invocation.inputs, false)?;
        let scope = &invocation.scope;
        let migrating = descriptor.mode == wire::Mode::Migrate;
        ensure(
            !migration_batch || migrating,
            "staged batches require a migration descriptor",
        )?;
        ensure(
            scope.read.len() <= wire::MAX_SCOPE && scope.write.is_subset(&scope.read),
            "invalid generic object scope",
        )?;
        match descriptor.mode {
            wire::Mode::Create => ensure(
                scope.write.is_empty() && (1..=wire::MAX_EDITS).contains(&scope.create_count),
                "invalid create scope",
            )?,
            _ => ensure(
                !scope.write.is_empty()
                    && scope.write.len() <= wire::MAX_EDITS
                    && scope.create_count == 0,
                "invalid edit selection",
            )?,
        }
        if migrating {
            let owned: BTreeSet<_> = doc
                .model()
                .extensions
                .values()
                .filter(|e| e.owner == plugin)
                .map(|e| e.id)
                .collect();
            ensure(
                (scope.write == owned || (migration_batch && scope.write.is_subset(&owned)))
                    && !owned.is_empty(),
                "migration must include every extension owned by this plugin",
            )?;
            ensure(
                plugin != os_plugin_api::wall::OWNER,
                "native Wall migrations are not supported by the extension migration route",
            )?;
        }
        if !migrating && let Some(requirement) = doc.model().plugin_requirements.get(plugin) {
            ensure(
                requirement.version == p.manifest.version,
                "document requires another plugin version; explicit migration required",
            )?;
        }
        let mut snapshot = wire::Snapshot::default();
        for key in &scope.read {
            if let Some(entity) = doc.model().extensions.get(key) {
                snapshot.elements.push(entity_to_wire(entity));
            } else if let Some(wall) = doc.model().walls.get(key) {
                snapshot
                    .elements
                    .push(crate::native_wall::project_checked(doc.model(), wall)?);
            } else if let Some(level) = doc.model().levels.get(key) {
                snapshot.levels.push(wire::Level {
                    id: key.to_string(),
                    name: level.parameters.name.clone(),
                    elevation_metres: level.parameters.elevation,
                });
            } else {
                return Err(Error::Invalid(
                    "generic read scope must contain existing extensions, native walls or levels"
                        .into(),
                ));
            }
        }
        for key in &scope.write {
            if let Some(wall) = doc.model().walls.get(key) {
                ensure(
                    plugin == os_plugin_api::wall::OWNER
                        && descriptor.element_type == os_plugin_api::wall::TYPE
                        && wall.header.type_id == os_plugin_api::wall::TYPE,
                    "native wall outside registered owner/type",
                )?;
                ensure(
                    p.catalog.as_ref().unwrap().types.iter().any(|t| {
                        t.id == descriptor.element_type
                            && t.payload_schema_version == os_plugin_api::wall::PAYLOAD_VERSION
                    }),
                    "native wall payload version mismatch",
                )?;
                continue;
            }
            let entity = doc.model().extensions.get(key).ok_or_else(|| {
                Error::Permission("generic write target is not an extension".into())
            })?;
            ensure(
                entity.owner == plugin && entity.type_id == descriptor.element_type,
                "generic write target outside owned command type",
            )?;
            let kind = p
                .catalog
                .as_ref()
                .unwrap()
                .types
                .iter()
                .find(|t| t.id == descriptor.element_type)
                .unwrap();
            ensure(
                entity.payload_schema_version == kind.payload_schema_version
                    || (migrating && entity.payload_schema_version < kind.payload_schema_version),
                "write target needs explicit payload migration",
            )?;
        }
        let new_ids: BTreeSet<Id> = (0..scope.create_count).map(|_| Id::new()).collect();
        let request_id = Id::new();
        let request = wire::Request {
            api_version: wire::VERSION,
            request_id: request_id.to_string(),
            context: Some(wire::Stamp {
                project: doc.model().project.id().to_string(),
                session: doc.session_id().to_string(),
                revision: doc.revision(),
            }),
            operation: wire::Operation::Invoke {
                command_id: invocation.command_id.clone(),
                inputs: invocation.inputs.clone(),
                snapshot,
                selection: scope.write.iter().map(ToString::to_string).collect(),
                new_ids: new_ids.iter().map(ToString::to_string).collect(),
            },
        };
        let input = serde_json::to_string(&request).map_err(invalid)?;
        ensure(
            input.len() <= wire::MAX_BYTES,
            "generic request exceeds 1 MiB",
        )?;
        ensure(
            started.elapsed() < invocation.timeout,
            "generic acceptance deadline expired",
        )?;
        Ok(Prepared {
            migration_batch,
            #[cfg(feature = "wasm")]
            id: request_id,
            input,
            deadline: started + invocation.timeout,
            request,
            invocation,
            new_ids,
            plugin_id: plugin.into(),
            plugin_version: p.manifest.version.clone(),
        })
    }

    pub(super) fn generic_commands(
        p: &Loaded,
        doc: &Document,
        prepared: &Prepared,
        output: &str,
    ) -> Result<Vec<Command>> {
        ensure(
            p.manifest.id == prepared.plugin_id && p.manifest.version == prepared.plugin_version,
            "generic plugin identity changed",
        )?;
        let current = wire::Stamp {
            project: doc.model().project.id().to_string(),
            session: doc.session_id().to_string(),
            revision: doc.revision(),
        };
        ensure(
            prepared.request.context.as_ref() == Some(&current),
            "generic document context changed",
        )?;
        require(p, Permission::ModelRead)?;
        require(p, Permission::ModelWrite)?;
        require(p, Permission::UiTool)?;
        let invocation = &prepared.invocation;
        let scope = &invocation.scope;
        let new_ids = &prepared.new_ids;
        let plugin = p.manifest.id.as_str();
        let descriptor = p
            .catalog
            .as_ref()
            .and_then(|c| c.commands.iter().find(|c| c.id == invocation.command_id))
            .ok_or_else(|| Error::Permission("generic command no longer registered".into()))?;
        ensure(descriptor.enabled, "generic command disabled")?;
        let wire::Reply::Edits(edits) = decode(output, &prepared.request)? else {
            return Err(Error::Invalid("expected generic edits".into()));
        };
        ensure(
            !edits.is_empty() && edits.len() <= wire::MAX_EDITS,
            "invalid generic edit count",
        )?;
        let mut commands = Vec::new();
        let mut touched = BTreeSet::new();
        for edit in edits {
            let (key, command) = match edit {
                wire::Edit::Create(entity) => {
                    ensure(
                        descriptor.mode == wire::Mode::Create,
                        "command cannot create",
                    )?;
                    let entity = checked_entity(p, entity, invocation, descriptor)?;
                    ensure(new_ids.contains(&entity.id), "unreserved creation identity")?;
                    let key = entity.id;
                    let command = if entity.type_id == os_plugin_api::wall::TYPE {
                        crate::native_wall::command(doc, entity, false)?
                    } else {
                        Command::AddExtension(entity)
                    };
                    (key, command)
                }
                wire::Edit::Replace {
                    expected_schema_version,
                    element,
                } => {
                    ensure(
                        matches!(descriptor.mode, wire::Mode::Edit | wire::Mode::Migrate),
                        "command cannot replace",
                    )?;
                    let entity = checked_entity(p, element, invocation, descriptor)?;
                    ensure(
                        scope.write.contains(&entity.id),
                        "replacement outside write scope",
                    )?;
                    if descriptor.mode == wire::Mode::Migrate {
                        let old = &doc.model().extensions[&entity.id];
                        ensure(
                            old.name == entity.name
                                && old.relationships == entity.relationships
                                && old.depends_on == entity.depends_on,
                            "migration cannot change names, relationships or dependencies",
                        )?;
                    }
                    if entity.type_id == os_plugin_api::wall::TYPE {
                        ensure(
                            expected_schema_version == os_plugin_api::wall::PAYLOAD_VERSION,
                            "native wall expected schema mismatch",
                        )?;
                        (entity.id, crate::native_wall::command(doc, entity, true)?)
                    } else {
                        (
                            entity.id,
                            Command::ReplaceExtension {
                                expected_schema_version,
                                entity,
                            },
                        )
                    }
                }
                wire::Edit::Delete { id: key } => {
                    ensure(
                        descriptor.mode == wire::Mode::Delete,
                        "command cannot delete",
                    )?;
                    let key = id(&key)?;
                    ensure(scope.write.contains(&key), "deletion outside write scope")?;
                    (
                        key,
                        if doc.model().walls.contains_key(&key) {
                            ensure(
                                !doc.model().wall_type_assignments.contains_key(&key),
                                "generic API 2 cannot delete typed walls",
                            )?;
                            Command::RemoveWall(key)
                        } else {
                            Command::RemoveExtension(key)
                        },
                    )
                }
            };
            ensure(touched.insert(key), "duplicate edit target")?;
            commands.push(command);
        }
        if descriptor.mode == wire::Mode::Migrate {
            ensure(
                touched == scope.write,
                "migration must replace every authorized extension exactly once",
            )?;
        }
        if !prepared.migration_batch
            && (descriptor.mode == wire::Mode::Migrate
                || !doc.model().plugin_requirements.contains_key(plugin))
        {
            commands.push(Command::SetPluginRequirement {
                plugin_id: plugin.into(),
                requirement: Some(PluginRequirement {
                    version: p.manifest.version.clone(),
                }),
            });
        }
        Ok(commands)
    }
}
