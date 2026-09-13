//! Descriptor-driven command drafts, separate from committed model and UI widgets.
use os_core::{Id, Result, ensure};
use os_document::Document;
use os_plugin_api::generic::{CommandDescriptor, FieldKind, Mode, validate_values};
use os_plugin_host::{
    PluginHost,
    generic::{Invocation, Scope},
};
use serde_json::Value;
use std::{collections::BTreeMap, time::Duration};

/// One host-selected context. Changing level or selection requires a new draft.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToolContext {
    pub level: Id,
    pub selection: Option<Id>,
}

pub struct ToolDraft {
    owner: String,
    activation: Id,
    descriptor: CommandDescriptor,
    session: Id,
    revision: u64,
    context: ToolContext,
    values: BTreeMap<String, Value>,
}
impl ToolDraft {
    /// Reads validated descriptors and selected semantic values. Never invokes a
    /// plugin or mutates history. Fields absent from payload use command defaults.
    pub fn begin(
        host: &PluginHost,
        document: &Document,
        owner: &str,
        command: &str,
        context: ToolContext,
    ) -> Result<Self> {
        ensure(
            document.model().levels.contains_key(&context.level),
            "tool level is not in this document",
        )?;
        let descriptor = host
            .catalog(owner)
            .and_then(|c| c.commands.iter().find(|c| c.id == command))
            .ok_or_else(|| os_core::Error::Invalid("tool descriptor is unavailable".into()))?
            .clone();
        let payload = if descriptor.mode == Mode::Create {
            ensure(
                context.selection.is_none(),
                "creation draft must not retain a write selection",
            )?;
            Value::Null
        } else {
            let id = context.selection.ok_or_else(|| {
                os_core::Error::Invalid("select one element for this tool".into())
            })?;
            if let Some(e) = document.model().extensions.get(&id) {
                ensure(
                    e.owner == owner && e.type_id == descriptor.element_type,
                    "selected element belongs to another tool type",
                )?;
                e.payload.clone()
            } else if let Some(w) = document.model().walls.get(&id) {
                ensure(
                    owner == os_plugin_api::wall::OWNER
                        && descriptor.element_type == os_plugin_api::wall::TYPE
                        && w.header.type_id == descriptor.element_type,
                    "selected wall belongs to another tool type",
                )?;
                serde_json::to_value(os_plugin_api::wall::Parameters {
                    start_x: w.parameters.start.x,
                    start_y: w.parameters.start.y,
                    end_x: w.parameters.end.x,
                    end_y: w.parameters.end.y,
                    thickness: w.parameters.thickness,
                    height: w.parameters.height,
                    level: w.parameters.level.to_string(),
                })
                .map_err(|e| os_core::Error::Invalid(e.to_string()))?
            } else {
                return Err(os_core::Error::Invalid(
                    "selected element is unavailable".into(),
                ));
            }
        };
        let values = descriptor
            .fields
            .iter()
            .map(|field| {
                let default = match &field.kind {
                    FieldKind::Number { default, .. } => Value::from(*default),
                    FieldKind::Boolean { default } => Value::from(*default),
                    FieldKind::Choice { default, .. } | FieldKind::Text { default, .. } => {
                        Value::from(default.clone())
                    }
                };
                (
                    field.key.clone(),
                    payload.get(&field.key).cloned().unwrap_or(default),
                )
            })
            .collect();
        Ok(Self {
            owner: owner.into(),
            activation: host
                .activation_id(owner)
                .expect("validated catalog belongs to loaded plugin"),
            descriptor,
            session: document.session_id(),
            revision: document.revision(),
            context,
            values,
        })
    }
    pub fn descriptor(&self) -> &CommandDescriptor {
        &self.descriptor
    }
    pub fn values(&self) -> &BTreeMap<String, Value> {
        &self.values
    }
    /// Invalid edits stay in the draft so a form can display validation errors.
    pub fn set(&mut self, key: &str, value: Value) -> Result<()> {
        ensure(self.values.contains_key(key), "unknown tool input")?;
        self.values.insert(key.into(), value);
        Ok(())
    }
    pub fn validate(&self) -> Result<()> {
        ensure(
            self.descriptor.enabled,
            self.descriptor
                .disabled_reason
                .as_deref()
                .unwrap_or("tool disabled"),
        )?;
        validate_values(&self.descriptor.fields, &self.values, false)?;
        Ok(())
    }
    /// Freeze the reviewed draft and exact context into host authorization.
    pub fn invocation(
        &self,
        host: &PluginHost,
        document: &Document,
        context: ToolContext,
        timeout: Duration,
    ) -> Result<Invocation> {
        self.validate()?;
        ensure(
            host.activation_id(&self.owner) == Some(self.activation),
            "plugin activation changed; reload tool draft",
        )?;
        ensure(
            document.session_id() == self.session && document.revision() == self.revision,
            "tool draft is stale; reload committed values",
        )?;
        ensure(
            context == self.context,
            "tool selection or active level changed",
        )?;
        ensure(
            !timeout.is_zero() && timeout <= Duration::from_secs(30),
            "invalid tool timeout",
        )?;
        let migrating = self.descriptor.mode == Mode::Migrate;
        let write: std::collections::BTreeSet<_> = if migrating {
            document
                .model()
                .extensions
                .values()
                .filter(|e| e.owner == self.owner)
                .map(|e| e.id)
                .collect()
        } else {
            context.selection.into_iter().collect()
        };
        let mut read: std::collections::BTreeSet<_> = [context.level]
            .into_iter()
            .chain(write.iter().copied())
            .collect();
        if migrating {
            for id in &write {
                read.extend(document.model().extensions[id].references());
            }
        }
        Ok(Invocation {
            command_id: self.descriptor.id.clone(),
            inputs: self.values.clone(),
            timeout,
            scope: Scope {
                read,
                write,
                create_count: usize::from(self.descriptor.mode == Mode::Create),
            },
        })
    }
    /// UI callers use a Wasm worker, never the synchronous guest convenience API.
    /// Pass current context on each start; cancel pending jobs on intent changes.
    #[cfg(feature = "external-plugins")]
    pub fn start(
        &self,
        host: &PluginHost,
        document: &Document,
        context: ToolContext,
        view: Option<os_plugin_host::worker::ViewContext>,
        timeout: Duration,
    ) -> Result<os_plugin_host::worker::PendingJob> {
        host.start_generic_job(
            &self.owner,
            document,
            self.invocation(host, document, context, timeout)?,
            view,
        )
    }
    pub fn owner(&self) -> &str {
        &self.owner
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use os_plugin_api::{Manifest, Permission, Plugin, generic as wire};
    use serde_json::json;
    struct Fixture;
    impl Plugin for Fixture {
        fn manifest(&self) -> Manifest {
            Manifest::from_toml(include_str!("../../../fixtures/generic-column/plugin.toml"))
                .unwrap()
        }
        fn invoke_json(&self, text: &str) -> Result<String> {
            let request: wire::Request = serde_json::from_str(text).unwrap();
            assert!(
                matches!(request.operation, wire::Operation::Describe),
                "drafts must not invoke commands"
            );
            let mut catalog: wire::Catalog = serde_json::from_str(include_str!(
                "../../../fixtures/generic-column/catalog.json"
            ))
            .unwrap();
            catalog.commands[0].fields.extend([
                wire::Field {
                    key: "flag".into(),
                    label: "Flag".into(),
                    kind: FieldKind::Boolean { default: true },
                },
                wire::Field {
                    key: "label".into(),
                    label: "Label".into(),
                    kind: FieldKind::Text {
                        max_bytes: 8,
                        default: "Column".into(),
                    },
                },
                wire::Field {
                    key: "option".into(),
                    label: "Option".into(),
                    kind: FieldKind::Choice {
                        values: vec!["A".into(), "B".into()],
                        default: "A".into(),
                    },
                },
            ]);
            Ok(serde_json::to_string(&wire::Response {
                api_version: 2,
                request_id: request.request_id,
                context: None,
                result: wire::Reply::Catalog(catalog),
            })
            .unwrap())
        }
    }
    fn host() -> PluginHost {
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
        host
    }
    #[test]
    fn boolean_text_choice_and_disabled_states_are_not_silently_coerced() {
        let host = host();
        let doc = Document::new("Typed").unwrap();
        let context = ToolContext {
            level: *doc.model().levels.keys().next().unwrap(),
            selection: None,
        };
        let mut draft = ToolDraft::begin(
            &host,
            &doc,
            "org.example.columns",
            "org.example.columns.create",
            context,
        )
        .unwrap();
        assert_eq!(draft.values()["flag"], json!(true));
        assert_eq!(draft.values()["label"], json!("Column"));
        assert_eq!(draft.values()["option"], json!("A"));
        for (key, bad, good) in [
            ("flag", json!("true"), json!(false)),
            ("label", json!("too long label"), json!("Wall")),
            ("option", json!("C"), json!("B")),
        ] {
            draft.set(key, bad).unwrap();
            assert!(draft.validate().is_err());
            draft.set(key, good).unwrap();
            draft.validate().unwrap();
        }
        draft.descriptor.enabled = false;
        draft.descriptor.disabled_reason = Some("Missing dependency".into());
        assert!(
            draft
                .validate()
                .unwrap_err()
                .to_string()
                .contains("Missing dependency")
        );
    }

    #[test]
    fn drafts_use_descriptors_and_never_modify_document_or_history() {
        let host = host();
        let document = Document::new("Draft").unwrap();
        let context = ToolContext {
            level: *document.model().levels.keys().next().unwrap(),
            selection: None,
        };
        let mut draft = ToolDraft::begin(
            &host,
            &document,
            "org.example.columns",
            "org.example.columns.create",
            context,
        )
        .unwrap();
        assert_eq!(draft.values()["width"], json!(0.4));
        draft.set("width", json!(-1)).unwrap();
        assert!(draft.validate().is_err());
        assert!(
            draft
                .invocation(&host, &document, context, Duration::from_secs(5))
                .is_err()
        );
        assert!(draft.set("unregistered", json!(1)).is_err());
        draft.set("width", json!(0.8)).unwrap();
        let invocation = draft
            .invocation(&host, &document, context, Duration::from_secs(5))
            .unwrap();
        assert_eq!(invocation.inputs["width"], json!(0.8));
        assert_eq!(invocation.scope.read, [context.level].into());
        assert!(invocation.scope.write.is_empty());
        assert_eq!(document.revision(), 0);
        assert!(!document.can_undo());
    }
    #[test]
    fn plugin_reload_invalidates_reviewed_draft_even_with_identical_descriptors() {
        let mut host = host();
        let doc = Document::new("Reload").unwrap();
        let context = ToolContext {
            level: *doc.model().levels.keys().next().unwrap(),
            selection: None,
        };
        let draft = ToolDraft::begin(
            &host,
            &doc,
            "org.example.columns",
            "org.example.columns.create",
            context,
        )
        .unwrap();
        let activation = host.activation_id("org.example.columns");
        host.unload("org.example.columns").unwrap();
        assert!(
            draft
                .invocation(&host, &doc, context, Duration::from_secs(5))
                .is_err()
        );
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
        assert_ne!(host.activation_id("org.example.columns"), activation);
        assert!(
            draft
                .invocation(&host, &doc, context, Duration::from_secs(5))
                .is_err()
        );
    }

    #[test]
    fn changed_selection_level_session_and_undo_invalidate_drafts() {
        let host = host();
        let mut doc = Document::new("Context").unwrap();
        let context = ToolContext {
            level: *doc.model().levels.keys().next().unwrap(),
            selection: None,
        };
        let draft = ToolDraft::begin(
            &host,
            &doc,
            "org.example.columns",
            "org.example.columns.create",
            context,
        )
        .unwrap();
        for context in [
            ToolContext {
                selection: Some(Id::new()),
                ..context
            },
            ToolContext {
                level: Id::new(),
                ..context
            },
        ] {
            assert!(
                draft
                    .invocation(&host, &doc, context, Duration::from_secs(5))
                    .is_err()
            );
        }
        let reopened = Document::from_model(doc.model().clone()).unwrap();
        assert!(
            draft
                .invocation(&host, &reopened, context, Duration::from_secs(5))
                .is_err()
        );
        doc.execute(
            "Rename",
            vec![os_document::Command::RenameProject("Changed".into())],
        )
        .unwrap();
        assert!(doc.undo());
        assert!(
            draft
                .invocation(&host, &doc, context, Duration::from_secs(5))
                .is_err()
        );
    }
}
