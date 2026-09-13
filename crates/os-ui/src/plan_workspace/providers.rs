use crate::{
    Editor,
    plan::{PreparedProviderPlan, ProviderPlanDrawing},
    plan_providers::PlanProviderBatch,
};
use os_core::{Id, Result};
use os_plugin_api::RegistrationKind;
use os_plugin_host::plan_graphics::PlanRequest;
use os_render::plan::{PlanContext, PlanDrawing};
use std::{collections::BTreeMap, thread::JoinHandle, time::Duration};

type Signature = (PlanContext, Vec<(String, Id)>);
#[derive(Default)]
pub(super) struct Providers {
    signature: Option<Signature>,
    attempted: bool,
    batch: Option<PlanProviderBatch>,
    pending: Option<(JoinHandle<Result<ProviderPlanDrawing>>, bool)>,
    drawing: Option<ProviderPlanDrawing>,
    pub diagnostics: BTreeMap<Id, String>,
    pub error: Option<String>,
}

impl Providers {
    pub fn busy(&self) -> bool {
        self.batch.is_some() || self.pending.is_some()
    }
    pub fn drawing<'a>(&'a self, editor: &Editor, view: Id) -> Option<&'a PlanDrawing> {
        self.drawing.as_ref()?.drawing(editor, view).ok()
    }
    fn spawn(&mut self, prepared: PreparedProviderPlan) {
        match std::thread::Builder::new()
            .name("provider-plan-drawing".into())
            .spawn(move || prepared.derive())
        {
            Ok(job) => self.pending = Some((job, true)),
            Err(error) => self.error = Some(error.to_string()),
        }
    }
    pub fn poll(&mut self, editor: &mut Editor, active: Option<Id>) {
        let context = active.and_then(|id| editor.native_plan_context(id).ok());
        let signature = context.map(|c| {
            (
                c,
                editor
                    .host
                    .manifests()
                    .filter_map(|m| editor.host.activation_id(&m.id).map(|a| (m.id.clone(), a)))
                    .collect(),
            )
        });
        if self.signature != signature {
            self.signature = signature;
            self.attempted = false;
            self.drawing = None;
            self.error = None;
            self.diagnostics.clear();
            if let Some(mut batch) = self.batch.take() {
                batch.cancel();
            }
            if let Some((_, valid)) = &mut self.pending {
                *valid = false;
            }
        }
        if self
            .pending
            .as_ref()
            .is_some_and(|(job, _)| job.is_finished())
        {
            let (job, valid) = self.pending.take().unwrap();
            let result = job.join().unwrap_or_else(|_| {
                Err(os_core::Error::Invalid(
                    "provider drawing worker failed".into(),
                ))
            });
            if valid {
                match result {
                    Ok(drawing) => {
                        if active.is_some_and(|id| drawing.drawing(editor, id).is_ok()) {
                            self.drawing = Some(drawing);
                        } else {
                            self.error =
                                Some("Provider plan became stale during derivation".into());
                        }
                    }
                    Err(error) => self.error = Some(error.to_string()),
                }
            }
        }
        if let Some(batch) = &mut self.batch {
            match batch.poll(editor, active) {
                Ok(false) => {}
                Ok(true) => {
                    let batch = self.batch.take().unwrap();
                    match batch.into_results(editor, active) {
                        Ok(output) => {
                            self.diagnostics.extend(output.failures);
                            if !output.graphics.is_empty() {
                                match editor.prepare_provider_plan(active.unwrap(), output.graphics)
                                {
                                    Ok(prepared) => self.spawn(prepared),
                                    Err(error) => self.error = Some(error.to_string()),
                                }
                            }
                        }
                        Err(error) => self.error = Some(error.to_string()),
                    }
                }
                Err(error) => {
                    self.batch.take();
                    self.error = Some(error.to_string());
                }
            }
        }
        if self.attempted || self.busy() {
            return;
        }
        let Some(context) = context else { return };
        self.attempted = true;
        if !context.show_extensions {
            return;
        }
        if editor.document.model().extensions.len() > os_render::plan::MAX_PLAN_ELEMENTS {
            self.error = Some("Plan exceeds 10000 plugin elements".into());
            return;
        }
        let level = editor.document.model().views[&context.view_id]
            .parameters
            .level
            .unwrap();
        let mut registrations: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (owner, registration) in editor.host.registrations(RegistrationKind::ViewProvider) {
            registrations
                .entry(owner.into())
                .or_default()
                .push(registration.id.clone());
        }
        let mut requests = Vec::new();
        for entity in editor.document.model().extensions.values() {
            let candidates = registrations
                .get(&entity.owner)
                .map_or(&[][..], Vec::as_slice);
            if candidates.len() != 1 || !editor.host.worker_supported(&entity.owner) {
                self.diagnostics.insert(
                    entity.id,
                    if candidates.len() > 1 {
                        "Ambiguous plan providers; no automatic choice"
                    } else {
                        "No callable Wasm plan provider"
                    }
                    .into(),
                );
                continue;
            }
            requests.push((
                entity.owner.clone(),
                PlanRequest {
                    provider: candidates[0].clone(),
                    element: entity.id,
                    view: context.view_id,
                    read: [entity.id, level].into(),
                    timeout: Duration::from_secs(5),
                },
            ));
        }
        if !requests.is_empty() {
            match PlanProviderBatch::begin(
                editor,
                context.view_id,
                requests,
                Duration::from_secs(300),
            ) {
                Ok(batch) => self.batch = Some(batch),
                Err(error) => self.error = Some(error.to_string()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::mpsc, time::Instant};

    #[test]
    fn missing_provider_is_diagnosed_and_visibility_change_clears_it() {
        let mut editor = Editor::new().unwrap();
        let level = *editor.document.model().levels.keys().next().unwrap();
        let view = editor.create_floor_plan("Plan", level).unwrap();
        let mut model = editor.document.model().clone();
        let entity: os_model::ExtensionEntity = serde_json::from_str(include_str!(
            "../../../../fixtures/extension-envelope-v1.json"
        ))
        .unwrap();
        let id = entity.id;
        model.extensions.insert(id, entity);
        model.plugin_requirements.insert(
            "org.example.columns".into(),
            os_model::PluginRequirement {
                version: "1.0.0".into(),
            },
        );
        editor.document = os_document::Document::from_model(model).unwrap();
        let before = editor.document.model().clone();
        let mut providers = Providers::default();
        providers.poll(&mut editor, Some(view));
        assert_eq!(providers.diagnostics[&id], "No callable Wasm plan provider");
        assert!(!providers.busy());
        assert!(providers.drawing(&editor, view).is_none());
        providers.poll(&mut editor, Some(view));
        assert_eq!(*editor.document.model(), before);
        let mut settings = editor.document.model().views[&view]
            .parameters
            .plan
            .unwrap();
        settings.visibility.extensions = false;
        editor
            .update_floor_plan(view, "Plan", level, settings)
            .unwrap();
        providers.poll(&mut editor, Some(view));
        assert!(providers.diagnostics.is_empty());
        assert!(!providers.busy());
    }

    #[test]
    fn retired_composition_cannot_revive_when_returning_to_view() {
        let mut editor = Editor::new().unwrap();
        let level = *editor.document.model().levels.keys().next().unwrap();
        let view = editor.create_floor_plan("Plan", level).unwrap();
        let mut providers = Providers::default();
        providers.poll(&mut editor, Some(view));
        let prepared = editor.prepare_provider_plan(view, vec![]).unwrap();
        let (tx, rx) = mpsc::channel();
        providers.pending = Some((
            std::thread::spawn(move || {
                rx.recv().unwrap();
                prepared.derive()
            }),
            true,
        ));
        providers.poll(&mut editor, None);
        assert!(!providers.pending.as_ref().unwrap().1);
        providers.poll(&mut editor, Some(view));
        assert!(!providers.pending.as_ref().unwrap().1);
        tx.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while providers.busy() {
            assert!(Instant::now() < deadline, "composition did not drain");
            providers.poll(&mut editor, Some(view));
            std::thread::yield_now();
        }
        assert!(providers.drawing(&editor, view).is_none());
        assert!(providers.error.is_none());
    }
}
