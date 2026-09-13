//! Revision-bound settings drafts: incomplete text never mutates the document.
use super::*;
use os_model::{PlanSettings, PlanViewBasis, PlanViewCrop, PlanViewRange, PlanVisibility};

const LABELS: [&str; 12] = [
    "Top offset (m)",
    "Cut offset (m)",
    "Bottom offset (m)",
    "Depth offset (m)",
    "Origin X (m)",
    "Origin Y (m)",
    "Rotation (rad)",
    "Scale denominator",
    "Crop min X (m)",
    "Crop min Y (m)",
    "Crop max X (m)",
    "Crop max Y (m)",
];

pub(super) struct PlanDraft {
    session: Id,
    revision: u64,
    view: Id,
    pub(super) name: String,
    level: Option<Id>,
    values: [String; 12],
    crop: bool,
    visibility: PlanVisibility,
    error: Option<String>,
}
impl PlanDraft {
    pub(super) fn begin(editor: &Editor, id: Id) -> Result<Self> {
        let view = editor
            .document
            .model()
            .views
            .get(&id)
            .ok_or_else(|| Error::Invalid("plan view missing".into()))?;
        os_core::ensure(
            view.parameters.kind == os_model::ViewKind::Plan,
            "view is not a floor plan",
        )?;
        let s = view
            .parameters
            .plan
            .ok_or_else(|| Error::Invalid("plan settings missing".into()))?;
        let crop = s.crop.unwrap_or(PlanViewCrop {
            min: Point2::new(-10.0, -10.0),
            max: Point2::new(10.0, 10.0),
        });
        Ok(Self {
            session: editor.document.session_id(),
            revision: editor.document.revision(),
            view: id,
            name: view.parameters.name.clone(),
            level: view.parameters.level,
            values: [
                s.range.top,
                s.range.cut,
                s.range.bottom,
                s.range.depth,
                s.basis.origin.x,
                s.basis.origin.y,
                s.basis.rotation,
                s.scale_denominator,
                crop.min.x,
                crop.min.y,
                crop.max.x,
                crop.max.y,
            ]
            .map(|v| v.to_string()),
            crop: s.crop.is_some(),
            visibility: s.visibility,
            error: None,
        })
    }
    fn apply(&self, editor: &mut Editor, active: Option<Id>) -> Result<()> {
        os_core::ensure(
            self.session == editor.document.session_id()
                && self.revision == editor.document.revision()
                && active == Some(self.view),
            "Plan draft is stale. Cancel and reopen Plan settings.",
        )?;
        let level = self
            .level
            .ok_or_else(|| Error::Invalid("Choose an associated level.".into()))?;
        let mut numbers = [0.0; 12];
        for (i, value) in self
            .values
            .iter()
            .enumerate()
            .take(if self.crop { 12 } else { 8 })
        {
            numbers[i] = value
                .trim()
                .parse::<f64>()
                .map_err(|_| Error::Invalid(format!("{} needs a number", LABELS[i])))?;
            os_core::ensure(
                numbers[i].is_finite(),
                format!("{} must be finite", LABELS[i]),
            )?;
        }
        let s = PlanSettings {
            range: PlanViewRange {
                top: numbers[0],
                cut: numbers[1],
                bottom: numbers[2],
                depth: numbers[3],
            },
            basis: PlanViewBasis {
                origin: Point2::new(numbers[4], numbers[5]),
                rotation: numbers[6],
            },
            scale_denominator: numbers[7],
            crop: self.crop.then_some(PlanViewCrop {
                min: Point2::new(numbers[8], numbers[9]),
                max: Point2::new(numbers[10], numbers[11]),
            }),
            visibility: self.visibility,
            ..Default::default()
        };
        editor.update_floor_plan(self.view, &self.name, level, s)
    }
}

impl DesktopApp {
    pub(super) fn plan_settings_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut draft) = self.plan_draft.take() else {
            return;
        };
        let mut close = false;
        egui::Modal::new(egui::Id::new("plan_settings_dialog")).show(ctx, |ui| {
            ui.set_width(410.0);
            ui.heading("Plan settings");
            ui.label("Apply changes as one undoable edit. Cancel keeps the saved settings.");
            egui::ScrollArea::vertical().id_salt("plan_settings_scroll")
                .max_height((ctx.content_rect().height()-230.0).max(120.0)).show(ui, |ui| {
                egui::Grid::new("plan_settings_fields").num_columns(2).show(ui, |ui| {
                    ui.label("Plan name");
                    ui.add(egui::TextEdit::singleline(&mut draft.name).char_limit(256).desired_width(220.0)); ui.end_row();
                    ui.label("Associated level");
                    egui::ComboBox::from_id_salt("plan_settings_level")
                        .selected_text(draft.level.and_then(|id| self.editor.document.model().levels.get(&id))
                            .map_or("Choose level", |l| l.parameters.name.as_str()))
                        .show_ui(ui, |ui| {
                            for (id, level) in &self.editor.document.model().levels {
                                ui.selectable_value(&mut draft.level,Some(*id),&level.parameters.name);
                            }
                        }); ui.end_row();
                    for (i,label) in LABELS.iter().enumerate().take(8) {
                        ui.label(*label);
                        ui.add(egui::TextEdit::singleline(&mut draft.values[i]).char_limit(128).desired_width(220.0)); ui.end_row();
                    }
                });
                ui.label("Offsets are relative to the level. Rotation is horizontal yaw; scale is 1:N, not screen zoom.");
                ui.checkbox(&mut draft.visibility.walls,"Show native walls");
                ui.checkbox(&mut draft.visibility.extensions,"Show extension elements / unavailable warnings");
                ui.checkbox(&mut draft.crop,"Enable rectangular crop");
                if draft.crop {
                    egui::Grid::new("plan_crop_fields").num_columns(2).show(ui, |ui| {
                        for (i,label) in LABELS.iter().enumerate().skip(8) {
                            ui.label(*label);
                            ui.add(egui::TextEdit::singleline(&mut draft.values[i]).char_limit(128).desired_width(220.0)); ui.end_row();
                        }
                    });
                    ui.label("Crop coordinates use the rotated view plane, in metres.");
                }
            });
            if let Some(error) = &draft.error { ui.colored_label(theme::ERROR,error); }
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Cancel plan settings").clicked() { close = true; }
                if ui.button("Apply plan settings").clicked() {
                    match draft.apply(&mut self.editor,self.plans.active) {
                        Ok(()) => {
                            close = true;
                            self.focus_plan(Some(draft.view));
                            self.report(Ok(()),"Plan settings applied.");
                            ctx.request_repaint();
                        }
                        Err(error) => draft.error = Some(error.to_string()),
                    }
                }
            });
        });
        if !close {
            self.plan_draft = Some(draft);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn plan_draft_is_atomic_and_rejects_invalid_and_stale_values() {
        let mut editor = Editor::new().unwrap();
        let level = *editor.document.model().levels.keys().next().unwrap();
        let view = editor.create_floor_plan("Plan", level).unwrap();
        let original = editor.document.model().clone();
        let mut draft = PlanDraft::begin(&editor, view).unwrap();
        draft.name = "Renamed plan".into();
        for invalid in ["-", "NaN", "inf", "1e999", "-10"] {
            draft.values[0] = invalid.into();
            assert!(draft.apply(&mut editor, Some(view)).is_err());
            assert_eq!(editor.document.model(), &original);
        }
        draft.values = [
            "4", "2", "0.2", "-2", "12.3", "45.6", "0.3", "50", "-1", "-2", "7", "8",
        ]
        .map(str::to_owned);
        draft.crop = true;
        draft.visibility.walls = false;
        draft.apply(&mut editor, Some(view)).unwrap();
        let changed = editor.document.model().clone();
        assert_eq!(changed.views[&view].parameters.name, "Renamed plan");
        assert_eq!(changed.views[&view].parameters.settings_revision, 1);
        assert_eq!(
            changed.views[&view].parameters.plan.unwrap().basis.rotation,
            0.3
        );
        assert!(draft.apply(&mut editor, Some(view)).is_err());
        editor.undo().unwrap();
        assert_eq!(editor.document.model(), &original);
        assert!(draft.apply(&mut editor, Some(view)).is_err());
        // Opening and applying an unchanged form must not clear the redo branch.
        PlanDraft::begin(&editor, view)
            .unwrap()
            .apply(&mut editor, Some(view))
            .unwrap();
        editor.redo().unwrap();
        assert_eq!(editor.document.model(), &changed);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("plan-draft.osb");
        editor.save(&path).unwrap();
        editor.open(&path).unwrap();
        assert_eq!(editor.document.model(), &changed);
        let restored = PlanDraft::begin(&editor, view).unwrap();
        assert_eq!(restored.values, draft.values);
        assert!(restored.crop);
        assert!(!restored.visibility.walls);
        let draft = PlanDraft::begin(&editor, view).unwrap();
        assert!(draft.apply(&mut editor, None).is_err());
        editor.document = Document::from_model(changed).unwrap();
        assert!(draft.apply(&mut editor, Some(view)).is_err());
    }
}
