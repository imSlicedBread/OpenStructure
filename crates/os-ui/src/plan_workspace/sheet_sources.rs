//! One auxiliary Section snapshot. Obsolete jobs drain before replacement.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Key {
    session: Id,
    revision: u64,
    sheet: Id,
    context: PlanContext,
}

#[derive(Default)]
pub(super) struct State {
    desired: Option<Key>,
    pending: Option<(JoinHandle<Result<PlanDrawing>>, bool)>,
    drawing: Option<PlanDrawing>,
    error: Option<String>,
    attempted: bool,
}

impl State {
    fn key(editor: &Editor, sheet: Option<Id>, active: Option<Id>) -> Option<Key> {
        let sheet = sheet?;
        let parameters = &editor.document.model().sheets.get(&sheet)?.parameters;
        if parameters.viewports.len() != 2 {
            return None;
        }
        let source = parameters
            .viewports
            .iter()
            .find(|v| Some(v.view) != active)?;
        if editor
            .document
            .model()
            .views
            .get(&source.view)?
            .parameters
            .kind
            != os_model::ViewKind::Section
        {
            return None;
        }
        Some(Key {
            session: editor.document.session_id(),
            revision: editor.document.revision(),
            sheet,
            context: editor.native_view_context(source.view).ok()?,
        })
    }

    pub(super) fn busy(&self) -> bool {
        self.pending.is_some()
    }

    pub(super) fn poll(&mut self, editor: &Editor, sheet: Option<Id>, active: Option<Id>) {
        let key = Self::key(editor, sheet, active);
        if self.desired != key {
            self.desired = key;
            self.drawing = None;
            self.error = None;
            self.attempted = false;
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
            let result = job
                .join()
                .unwrap_or_else(|_| Err(Error::Invalid("sheet Section worker failed".into())));
            if valid {
                match result {
                    Ok(drawing) => self.drawing = Some(drawing),
                    Err(error) => self.error = Some(error.to_string()),
                }
            }
        }
        if self.pending.is_none()
            && !self.attempted
            && let Some(key) = key
        {
            self.attempted = true;
            match editor
                .native_view_snapshot(key.context.view_id)
                .and_then(|snapshot| {
                    std::thread::Builder::new()
                        .name("sheet-section".into())
                        .spawn(move || snapshot.derive())
                        .map_err(|e| {
                            Error::Invalid(format!("cannot start sheet Section worker: {e}"))
                        })
                }) {
                Ok(job) => self.pending = Some((job, true)),
                Err(error) => self.error = Some(error.to_string()),
            }
        }
    }

    pub(super) fn drawing(
        &self,
        editor: &Editor,
        sheet: Option<Id>,
        active: Option<Id>,
        context: PlanContext,
    ) -> Result<&PlanDrawing> {
        let key = Self::key(editor, sheet, active);
        os_core::ensure(
            key.is_some() && self.desired == key && key.is_some_and(|k| k.context == context),
            "sheet Section source changed; wait for current graphics",
        )?;
        if let Some(error) = &self.error {
            return Err(Error::Invalid(error.clone()));
        }
        let drawing = self
            .drawing
            .as_ref()
            .ok_or_else(|| Error::Invalid("sheet Section graphics are still generating".into()))?;
        drawing.items(context)?;
        Ok(drawing)
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
    fn obsolete_section_worker_drains_without_publishing_after_cancel_and_return() {
        let mut editor = Editor::new().unwrap();
        let level = *editor.document.model().levels.keys().next().unwrap();
        let plan = editor.create_floor_plan("Plan", level).unwrap();
        let section = editor
            .create_section_view(
                "Section",
                level,
                os_model::SectionViewSettings::new(
                    Point2::new(0.0, 0.0),
                    Point2::new(8.0, 0.0),
                    -0.5,
                    4.0,
                ),
            )
            .unwrap();
        let mut parameters = SheetParams::new("A101", "Combined");
        for (view, x) in [(plan, 110.0), (section, 310.0)] {
            parameters.viewports.push(SheetViewport {
                id: Id::new(),
                view,
                model_center_m: Point2::default(),
                paper_center_mm: Point2::new(x, 128.0),
                width_mm: 184.0,
                height_mm: 220.0,
                scale_denominator: 100.0,
                title_override: None,
            });
        }
        let sheet = Sheet::new("core.sheet", parameters);
        let id = sheet.id();
        editor.command("Sheet", Command::AddSheet(sheet)).unwrap();
        let key = State::key(&editor, Some(id), Some(plan)).unwrap();
        let snapshot = editor.native_view_snapshot(section).unwrap();
        let (release, wait) = mpsc::channel();
        let job = std::thread::spawn(move || {
            wait.recv().unwrap();
            snapshot.derive()
        });
        let mut state = State {
            desired: Some(key),
            pending: Some((job, true)),
            attempted: true,
            ..Default::default()
        };
        state.poll(&editor, None, Some(plan));
        assert!(!state.pending.as_ref().unwrap().1);
        state.poll(&editor, Some(id), Some(plan));
        assert!(
            !state.pending.as_ref().unwrap().1,
            "returning to same sheet must not revive canceled work"
        );
        assert!(
            state
                .drawing(&editor, Some(id), Some(plan), key.context)
                .is_err()
        );
        release.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !state.pending.as_ref().unwrap().0.is_finished() {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        state.poll(&editor, Some(id), Some(plan));
        assert!(
            state.drawing.is_none(),
            "obsolete output must never publish"
        );
        assert!(
            state.pending.is_some(),
            "replacement starts only after obsolete work drains"
        );
        while state.busy() {
            assert!(Instant::now() < deadline);
            state.poll(&editor, Some(id), Some(plan));
            std::thread::yield_now();
        }
        assert!(
            state
                .drawing(&editor, Some(id), Some(plan), key.context)
                .is_ok()
        );
        let mut parameters = editor.document.model().sheets[&id].parameters.clone();
        parameters.name = "Changed".into();
        editor
            .command("Change", Command::UpdateSheet { id, parameters })
            .unwrap();
        assert!(
            state
                .drawing(&editor, Some(id), Some(plan), key.context)
                .is_err(),
            "revision fails closed even before poll"
        );
        editor.document =
            os_document::Document::from_model(editor.document.model().clone()).unwrap();
        assert!(
            state
                .drawing(&editor, Some(id), Some(plan), key.context)
                .is_err(),
            "session replacement fails closed"
        );
    }
}
