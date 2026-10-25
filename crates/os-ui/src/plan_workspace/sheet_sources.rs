//! Deduplicated sheet sources. One native derivation and one provider lane;
//! obsolete jobs drain before replacement, independent of placement count.
use super::*;

#[derive(Clone, Debug, PartialEq)]
struct Key {
    session: Id,
    revision: u64,
    sheet: Id,
    contexts: Vec<PlanContext>,
    providers: Vec<(String, Id)>,
}

#[derive(Default)]
pub(super) struct State {
    desired: Option<Key>,
    pending: Option<(Id, JoinHandle<Result<PlanDrawing>>, bool)>,
    drawings: BTreeMap<Id, PlanDrawing>,
    errors: BTreeMap<Id, String>,
    #[cfg(feature = "external-plugins")]
    providers: BTreeMap<Id, providers::Providers>,
    #[cfg(feature = "external-plugins")]
    retiring: Vec<providers::Providers>,
}

impl State {
    fn key(
        editor: &Editor,
        sheet: Option<Id>,
        active: Option<Id>,
        parameters: Option<&SheetParams>,
    ) -> Option<Key> {
        let sheet = sheet?;
        let parameters = parameters.or_else(|| {
            editor
                .document
                .model()
                .sheets
                .get(&sheet)
                .map(|s| &s.parameters)
        })?;
        let ids: std::collections::BTreeSet<_> = parameters
            .viewports
            .iter()
            .map(|v| v.view)
            .chain(
                editor
                    .document
                    .model()
                    .sheet_sets
                    .values()
                    .flat_map(|set| &set.sheets)
                    .filter_map(|id| editor.document.model().sheets.get(id))
                    .flat_map(|s| s.parameters.viewports.iter().map(|v| v.view)),
            )
            .filter(|id| Some(*id) != active)
            .collect();
        Some(Key {
            session: editor.document.session_id(),
            revision: editor.document.revision(),
            sheet,
            contexts: ids
                .into_iter()
                .filter_map(|id| editor.native_view_context(id).ok())
                .collect(),
            providers: plan_provider_signature(editor),
        })
    }

    pub(super) fn busy(&self) -> bool {
        #[cfg(feature = "external-plugins")]
        if self.providers.values().any(providers::Providers::busy)
            || self.retiring.iter().any(providers::Providers::busy)
        {
            return true;
        }
        self.pending.is_some()
    }

    pub(super) fn poll(
        &mut self,
        editor: &Editor,
        sheet: Option<Id>,
        active: Option<Id>,
        parameters: Option<&SheetParams>,
    ) {
        let key = Self::key(editor, sheet, active, parameters);
        if self.desired != key {
            self.desired = key;
            self.drawings.clear();
            self.errors.clear();
            if let Some((_, _, valid)) = &mut self.pending {
                *valid = false;
            }
            #[cfg(feature = "external-plugins")]
            self.retiring
                .extend(std::mem::take(&mut self.providers).into_values());
        }
        if self
            .pending
            .as_ref()
            .is_some_and(|(_, job, _)| job.is_finished())
        {
            let (id, job, valid) = self.pending.take().unwrap();
            let result = job
                .join()
                .unwrap_or_else(|_| Err(Error::Invalid("sheet source worker failed".into())));
            if valid {
                match result {
                    Ok(drawing) => {
                        self.drawings.insert(id, drawing);
                    }
                    Err(error) => {
                        self.errors.insert(id, error.to_string());
                    }
                }
            }
        }
        if self.pending.is_none()
            && let Some(context) = self
                .desired
                .as_ref()
                .and_then(|key| {
                    key.contexts.iter().find(|c| {
                        !self.drawings.contains_key(&c.view_id)
                            && !self.errors.contains_key(&c.view_id)
                    })
                })
                .copied()
        {
            match editor
                .native_view_snapshot(context.view_id)
                .and_then(|snapshot| {
                    std::thread::Builder::new()
                        .name("sheet-source".into())
                        .spawn(move || snapshot.derive())
                        .map_err(|e| {
                            Error::Invalid(format!("cannot start sheet source worker: {e}"))
                        })
                }) {
                Ok(job) => self.pending = Some((context.view_id, job, true)),
                Err(error) => {
                    self.errors.insert(context.view_id, error.to_string());
                }
            }
        }
    }

    #[cfg(feature = "external-plugins")]
    pub(super) fn poll_providers(&mut self, editor: &mut Editor) {
        for retired in &mut self.retiring {
            retired.poll(editor, None);
        }
        self.retiring.retain(providers::Providers::busy);
        if !self.retiring.is_empty() {
            return;
        }
        let Some(key) = &self.desired else { return };
        for context in &key.contexts {
            if editor.native_plan_context(context.view_id).is_err() {
                continue;
            }
            let provider = self.providers.entry(context.view_id).or_default();
            provider.poll(editor, Some(context.view_id));
            if provider.busy() {
                break;
            }
        }
    }

    pub(super) fn drawing(
        &self,
        editor: &Editor,
        sheet: Option<Id>,
        _active: Option<Id>,
        context: PlanContext,
    ) -> Result<&PlanDrawing> {
        os_core::ensure(
            self.desired.as_ref().is_some_and(|k| {
                Some(k.sheet) == sheet
                    && k.session == editor.document.session_id()
                    && k.revision == editor.document.revision()
                    && k.providers == plan_provider_signature(editor)
                    && k.contexts.contains(&context)
            }),
            "sheet source changed; wait for current graphics",
        )?;
        if let Some(error) = self.errors.get(&context.view_id) {
            return Err(Error::Invalid(error.clone()));
        }
        let native = self
            .drawings
            .get(&context.view_id)
            .ok_or_else(|| Error::Invalid("sheet source graphics are still generating".into()))?;
        native.items(context)?;
        #[cfg(feature = "external-plugins")]
        if editor.native_plan_context(context.view_id).is_ok()
            && context.show_extensions
            && !editor.document.model().extensions.is_empty()
        {
            let provider = self.providers.get(&context.view_id).ok_or_else(|| {
                Error::Invalid("sheet plan providers are still generating".into())
            })?;
            os_core::ensure(
                provider.current(editor, context.view_id)
                    && !provider.busy()
                    && provider.error.is_none()
                    && provider.diagnostics.is_empty(),
                "sheet provider graphics are incomplete; resolve diagnostics before export",
            )?;
            return provider
                .drawing(editor, context.view_id)
                .ok_or_else(|| Error::Invalid("sheet provider graphics are unavailable".into()));
        }
        Ok(native)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::mpsc,
        time::{Duration, Instant},
    };

    #[test]
    fn duplicate_sources_share_one_cache_and_obsolete_jobs_drain_without_publishing() {
        let (mut app, section) = super::super::sheet_tests::app_with_section();
        app.create_sheet_from_active_view();
        let sheet = app.plans.active_sheet.unwrap();
        let mut parameters = app.editor.document.model().sheets[&sheet]
            .parameters
            .clone();
        let mut duplicate = parameters.viewports[0].clone();
        duplicate.id = Id::new();
        duplicate.detail_number = super::super::next_sheet_detail_number(&parameters.viewports);
        parameters.viewports.push(duplicate);
        let key = State::key(&app.editor, Some(sheet), None, Some(&parameters)).unwrap();
        assert_eq!(
            key.contexts.len(),
            1,
            "placement count must not multiply source work"
        );
        let context = key.contexts[0];
        let snapshot = app.editor.native_view_snapshot(section).unwrap();
        let (release, wait) = mpsc::channel();
        let job = std::thread::spawn(move || {
            wait.recv().unwrap();
            snapshot.derive()
        });
        let mut state = State {
            desired: Some(key),
            pending: Some((section, job, true)),
            ..Default::default()
        };
        state.poll(&app.editor, None, None, None);
        assert!(!state.pending.as_ref().unwrap().2);
        state.poll(&app.editor, Some(sheet), None, Some(&parameters));
        assert!(
            !state.pending.as_ref().unwrap().2,
            "returning cannot revive an obsolete job"
        );
        assert!(
            state
                .drawing(&app.editor, Some(sheet), None, context)
                .is_err()
        );
        release.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !state.pending.as_ref().unwrap().1.is_finished() {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        state.poll(&app.editor, Some(sheet), None, Some(&parameters));
        assert!(state.drawings.is_empty(), "obsolete output never publishes");
        assert!(
            state.pending.is_some(),
            "replacement begins only after draining"
        );
        while state.busy() {
            assert!(Instant::now() < deadline);
            state.poll(&app.editor, Some(sheet), None, Some(&parameters));
            std::thread::yield_now();
        }
        assert_eq!(state.drawings.len(), 1);
        assert!(
            state
                .drawing(&app.editor, Some(sheet), None, context)
                .is_ok()
        );
        parameters.name = "Changed".into();
        app.editor
            .command(
                "Edit",
                Command::UpdateSheet {
                    id: sheet,
                    parameters,
                },
            )
            .unwrap();
        assert!(
            state
                .drawing(&app.editor, Some(sheet), None, context)
                .is_err()
        );
        app.editor.document =
            os_document::Document::from_model(app.editor.document.model().clone()).unwrap();
        assert!(
            state
                .drawing(&app.editor, Some(sheet), None, context)
                .is_err()
        );
    }

    #[cfg(feature = "external-plugins")]
    #[test]
    fn inactive_plan_provider_failure_blocks_native_only_sheet_export() {
        let (mut app, section) = super::super::sheet_tests::app_with_section();
        let plan = app
            .editor
            .create_floor_plan("Provider plan", app.active_level)
            .unwrap();
        app.create_sheet_from_active_view();
        let sheet = app.plans.active_sheet.unwrap();
        let mut model = app.editor.document.model().clone();
        let entity: os_model::ExtensionEntity = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/extension-envelope-v1.json"
        )))
        .unwrap();
        model.extensions.insert(entity.id, entity);
        model.plugin_requirements.insert(
            "org.example.columns".into(),
            os_model::PluginRequirement {
                version: "1.0.0".into(),
            },
        );
        let parameters = &mut model.sheets.get_mut(&sheet).unwrap().parameters;
        let mut vp = parameters.viewports[0].clone();
        vp.id = Id::new();
        vp.view = plan;
        vp.detail_number = super::super::next_sheet_detail_number(&parameters.viewports);
        parameters.viewports.push(vp);
        app.editor.document = os_document::Document::from_model(model).unwrap();
        let context = app.editor.native_view_context(plan).unwrap();
        let mut state = State::default();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            state.poll(&app.editor, Some(sheet), Some(section), None);
            state.poll_providers(&mut app.editor);
            if !state.busy() {
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert!(
            state.drawings.contains_key(&plan),
            "native fallback is available but forbidden"
        );
        assert!(
            state
                .drawing(&app.editor, Some(sheet), Some(section), context)
                .err()
                .unwrap()
                .to_string()
                .contains("incomplete")
        );
        state
            .desired
            .as_mut()
            .unwrap()
            .providers
            .push(("changed".into(), Id::new()));
        assert!(
            state
                .drawing(&app.editor, Some(sheet), Some(section), context)
                .is_err()
        );
    }
}
