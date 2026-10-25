//! Annotation-only frozen snapshot: never requests model/provider projection.
use super::*;
pub(crate) struct Snapshot {
    context: PlanContext,
    lines: Vec<os_render::plan::PlanDetailLine>,
    notes: Vec<os_render::plan::NoteGraphic>,
}
impl Snapshot {
    pub(crate) fn derive(self) -> Result<PlanDrawing> {
        PlanDrawing::from_prisms(self.context, &BTreeMap::new(), Vec::new())?
            .with_detail_lines(self.lines)?
            .with_text_notes(self.notes)?
            .with_snap_segments(Vec::new())
    }
}
impl Editor {
    pub fn create_drafting_view(&mut self, name: &str) -> Result<Id> {
        let view = View::new("core.view", ViewParams::drafting(name));
        let id = view.id();
        self.command("Create drafting view", Command::AddView(view))?;
        Ok(id)
    }
    pub fn native_drafting_context(&self, id: Id) -> Result<PlanContext> {
        let view = self
            .document
            .model()
            .views
            .get(&id)
            .ok_or_else(|| os_core::Error::Invalid("drafting view missing".into()))?;
        ensure(
            view.parameters.kind == ViewKind::Drafting,
            "not a Drafting view",
        )?;
        view.parameters.validate_edit()?;
        let settings = view.parameters.drafting.unwrap();
        Ok(PlanContext {
            annotation_crop: None,
            session_id: self.document.session_id(),
            model_revision: self.document.revision(),
            view_id: id,
            settings_revision: view.parameters.settings_revision,
            basis: HorizontalBasis::default(),
            range: PlanRange {
                top: 1.,
                cut: 0.,
                bottom: 0.,
                depth: -1.,
            },
            crop: Some(PlanCrop {
                min: settings.crop.min,
                max: settings.crop.max,
            }),
            scale_denominator: settings.scale_denominator,
            show_walls: false,
            show_extensions: false,
        })
    }
    pub(crate) fn drafting_snapshot(&self, id: Id) -> Result<Snapshot> {
        let context = self.native_drafting_context(id)?;
        let model = self.document.model();
        let lines = model
            .detail_lines
            .values()
            .filter(|l| l.parameters.view == id)
            .map(|l| os_render::plan::PlanDetailLine {
                entity: l.id(),
                view: id,
                start: l.parameters.start,
                end: l.parameters.end,
            })
            .collect();
        Ok(Snapshot {
            context,
            lines,
            notes: note_graphics(model, context)?,
        })
    }
}
