//! Explicit per-type migration review; no command is selected implicitly.
use crate::plugin_tools::{ToolContext, ToolDraft};
use os_core::{Id, Result, ensure};
use os_document::Document;
use os_plugin_api::generic::Mode;
use os_plugin_host::{
    PluginHost,
    migration::{MigrationCommand, MigrationSession},
};
use std::{collections::BTreeMap, time::Duration};

pub struct MigrationDraft {
    owner: String,
    context: ToolContext,
    session: Id,
    revision: u64,
    activation: Id,
    representatives: BTreeMap<String, Id>,
    choices: BTreeMap<String, ToolDraft>,
}

impl MigrationDraft {
    pub fn begin(
        host: &PluginHost,
        document: &Document,
        owner: &str,
        context: ToolContext,
    ) -> Result<Self> {
        let activation = host
            .activation_id(owner)
            .ok_or_else(|| os_core::Error::Invalid("migration provider unavailable".into()))?;
        ensure(
            document.model().levels.contains_key(&context.level),
            "migration level unavailable",
        )?;
        let mut representatives = BTreeMap::new();
        for entity in document
            .model()
            .extensions
            .values()
            .filter(|e| e.owner == owner)
        {
            representatives
                .entry(entity.type_id.clone())
                .or_insert(entity.id);
        }
        ensure(!representatives.is_empty(), "provider owns no extensions")?;
        Ok(Self {
            owner: owner.into(),
            context,
            session: document.session_id(),
            revision: document.revision(),
            activation,
            representatives,
            choices: BTreeMap::new(),
        })
    }
    pub fn owner(&self) -> &str {
        &self.owner
    }
    pub fn types(&self) -> impl Iterator<Item = &String> {
        self.representatives.keys()
    }
    pub fn choice_mut(&mut self, kind: &str) -> Option<&mut ToolDraft> {
        self.choices.get_mut(kind)
    }
    pub fn choice(&self, kind: &str) -> Option<&ToolDraft> {
        self.choices.get(kind)
    }
    fn check(&self, host: &PluginHost, document: &Document, context: ToolContext) -> Result<()> {
        ensure(
            self.context == context,
            "migration review selection or level changed",
        )?;
        ensure(
            self.session == document.session_id() && self.revision == document.revision(),
            "migration review is stale",
        )?;
        ensure(
            host.activation_id(&self.owner) == Some(self.activation),
            "migration provider changed",
        )
    }
    pub fn choose(
        &mut self,
        host: &PluginHost,
        document: &Document,
        context: ToolContext,
        kind: &str,
        command: &str,
    ) -> Result<()> {
        self.check(host, document, context)?;
        let id = *self
            .representatives
            .get(kind)
            .ok_or_else(|| os_core::Error::Invalid("type is not in migration review".into()))?;
        let draft = ToolDraft::begin(
            host,
            document,
            &self.owner,
            command,
            ToolContext {
                selection: Some(id),
                ..context
            },
        )?;
        ensure(
            draft.descriptor().mode == Mode::Migrate && draft.descriptor().element_type == kind,
            "choose a migration command for this type",
        )?;
        ensure(draft.descriptor().enabled, "migration command is disabled")?;
        // Existing payload values may need correction; retain them in the typed
        // draft and block Apply through validate(), not command selection.
        self.choices.insert(kind.into(), draft);
        Ok(())
    }
    pub fn validate(&self) -> Result<()> {
        ensure(
            self.choices.len() == self.representatives.len(),
            "review a migration command for every owned type",
        )?;
        for draft in self.choices.values() {
            draft.validate()?;
        }
        Ok(())
    }
    pub fn start(
        &self,
        host: &PluginHost,
        document: &Document,
        context: ToolContext,
    ) -> Result<MigrationSession> {
        self.check(host, document, context)?;
        self.validate()?;
        let mut plan = BTreeMap::new();
        for (kind, draft) in &self.choices {
            let invocation = draft.invocation(
                host,
                document,
                ToolContext {
                    selection: Some(self.representatives[kind]),
                    ..context
                },
                Duration::from_secs(5),
            )?;
            plan.insert(
                kind.clone(),
                MigrationCommand {
                    command_id: invocation.command_id,
                    inputs: invocation.inputs,
                },
            );
        }
        host.start_migration(document, &self.owner, plan, Duration::from_secs(300))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use os_plugin_api::{Manifest, Permission, Plugin, generic as wire};
    const OWNER: &str = "org.example.columns";
    const A: &str = "org.example.columns.rectangular";
    const B: &str = "org.example.columns.other";
    struct Fixture;
    impl Plugin for Fixture {
        fn manifest(&self) -> Manifest {
            let mut manifest =
                Manifest::from_toml(include_str!("../../../fixtures/generic-column/plugin.toml"))
                    .unwrap();
            let mut kind = manifest.registrations[0].clone();
            kind.id = B.into();
            let mut command = manifest.registrations[2].clone();
            command.id = "org.example.columns.other-migrate".into();
            manifest.registrations.extend([kind, command]);
            manifest
        }
        fn invoke_json(&self, input: &str) -> Result<String> {
            let request: wire::Request = serde_json::from_str(input).unwrap();
            assert!(
                matches!(request.operation, wire::Operation::Describe),
                "review must not invoke guest commands"
            );
            let mut catalog: wire::Catalog = serde_json::from_str(include_str!(
                "../../../fixtures/generic-column/catalog.json"
            ))
            .unwrap();
            let mut other = catalog.types[0].clone();
            other.id = B.into();
            catalog.types.push(other);
            catalog.commands[1].mode = Mode::Migrate;
            let mut command = catalog.commands[1].clone();
            command.id = "org.example.columns.other-migrate".into();
            command.element_type = B.into();
            catalog.commands.push(command);
            Ok(serde_json::to_string(&wire::Response {
                api_version: 2,
                request_id: request.request_id,
                context: None,
                result: wire::Reply::Catalog(catalog),
            })
            .unwrap())
        }
    }
    #[test]
    fn every_type_requires_explicit_valid_review() {
        let mut host = PluginHost::default();
        host.load(
            Box::new(Fixture),
            [
                Permission::ModelRead,
                Permission::ModelWrite,
                Permission::UiTool,
            ]
            .into(),
        )
        .unwrap();
        let mut model = Document::new("Mixed review").unwrap().model().clone();
        let entity: os_model::ExtensionEntity =
            serde_json::from_str(include_str!("../../../fixtures/extension-envelope-v1.json"))
                .unwrap();
        let mut other = entity.clone();
        other.id = Id::new();
        other.type_id = B.into();
        model.plugin_requirements.insert(
            OWNER.into(),
            os_model::PluginRequirement {
                version: "1.0.0".into(),
            },
        );
        model.extensions.insert(entity.id, entity);
        model.extensions.insert(other.id, other);
        let mut doc = Document::from_model(model).unwrap();
        let context = ToolContext {
            level: *doc.model().levels.keys().next().unwrap(),
            selection: None,
        };
        let mut plan = MigrationDraft::begin(&host, &doc, OWNER, context).unwrap();
        assert_eq!(plan.types().count(), 2);
        assert!(plan.validate().is_err());
        assert!(
            plan.choose(&host, &doc, context, A, "org.example.columns.create")
                .is_err()
        );
        plan.choose(&host, &doc, context, A, "org.example.columns.edit")
            .unwrap();
        assert!(plan.validate().is_err());
        assert!(
            plan.choose(&host, &doc, context, B, "org.example.columns.edit")
                .is_err()
        );
        plan.choose(&host, &doc, context, B, "org.example.columns.other-migrate")
            .unwrap();
        plan.validate().unwrap();
        plan.choice_mut(B)
            .unwrap()
            .set("width", serde_json::json!(-1))
            .unwrap();
        assert!(plan.validate().is_err());
        plan.choice_mut(B)
            .unwrap()
            .set("width", serde_json::json!(0.8))
            .unwrap();
        plan.validate().unwrap();
        assert!(!doc.can_undo());
        assert!(
            plan.start(
                &host,
                &doc,
                ToolContext {
                    selection: Some(Id::new()),
                    ..context
                }
            )
            .is_err()
        );
        doc.execute(
            "Rename",
            vec![os_document::Command::RenameProject("Changed".into())],
        )
        .unwrap();
        assert!(
            plan.choose(&host, &doc, context, B, "org.example.columns.other-migrate")
                .is_err()
        );
        assert!(plan.start(&host, &doc, context).is_err());
    }
}
