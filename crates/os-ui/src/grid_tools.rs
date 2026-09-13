//! Exact, revision-bound grid forms. Draft text/preview never enter the model.
use super::*;
use os_model::{Grid, GridParams};

const LABELS: [&str; 4] = ["Start X (m)", "Start Y (m)", "End X (m)", "End Y (m)"];

pub(super) struct GridDraft {
    session: Id,
    revision: u64,
    view: Option<Id>,
    id: Id,
    building: Id,
    editing: bool,
    pub(super) name: String,
    values: [String; 4],
    error: Option<String>,
}
impl GridDraft {
    fn begin(editor: &Editor, view: Option<Id>, target: Option<Id>) -> Result<Self> {
        let model = editor.document.model();
        let (id, parameters) = if let Some(id) = target {
            let grid = model
                .grids
                .get(&id)
                .ok_or_else(|| Error::Invalid("Grid missing".into()))?;
            (id, grid.parameters.clone())
        } else {
            let view =
                view.ok_or_else(|| Error::Invalid("Open a floor plan to create a grid".into()))?;
            let context = editor.native_plan_context(view)?;
            let level = model.views[&view].parameters.level.unwrap();
            let building = model.levels[&level].parameters.building;
            let names: std::collections::BTreeSet<_> = model
                .grids
                .values()
                .filter(|grid| grid.parameters.building == building)
                .map(|grid| grid.parameters.name.as_str())
                .collect();
            // At most n names cannot occupy all n+1 candidate labels.
            let index = (1..=names.len() + 1)
                .find(|i| !names.contains(format!("Grid {i}").as_str()))
                .unwrap();
            (
                Id::new(),
                GridParams {
                    name: format!("Grid {index}"),
                    building,
                    start: context.basis.plane_to_world(Point2::new(-3.0, 0.0))?,
                    end: context.basis.plane_to_world(Point2::new(3.0, 0.0))?,
                },
            )
        };
        Ok(Self {
            session: editor.document.session_id(),
            revision: editor.document.revision(),
            view,
            id,
            building: parameters.building,
            editing: target.is_some(),
            name: parameters.name,
            values: [
                parameters.start.x,
                parameters.start.y,
                parameters.end.x,
                parameters.end.y,
            ]
            .map(|n| n.to_string()),
            error: None,
        })
    }
    fn parameters(&self) -> Result<GridParams> {
        let mut numbers = [0.0; 4];
        for (i, text) in self.values.iter().enumerate() {
            numbers[i] = text
                .trim()
                .parse::<f64>()
                .map_err(|_| Error::Invalid(format!("{} needs a number", LABELS[i])))?;
            os_core::ensure(
                numbers[i].is_finite(),
                format!("{} must be finite", LABELS[i]),
            )?;
        }
        let params = GridParams {
            name: self.name.clone(),
            building: self.building,
            start: Point2::new(numbers[0], numbers[1]),
            end: Point2::new(numbers[2], numbers[3]),
        };
        params.validate()?;
        Ok(params)
    }
    fn apply(&self, editor: &mut Editor, active: Option<Id>) -> Result<Id> {
        os_core::ensure(
            self.session == editor.document.session_id()
                && self.revision == editor.document.revision()
                && active == self.view,
            "Grid draft is stale. Cancel and reopen the grid form.",
        )?;
        let parameters = self.parameters()?;
        let command = if self.editing {
            Command::UpdateGrid {
                id: self.id,
                parameters,
            }
        } else {
            let mut grid = Grid::new("core.grid", parameters);
            grid.header.id = self.id;
            Command::AddGrid(grid)
        };
        editor.command(
            if self.editing {
                "Edit architectural grid"
            } else {
                "Create architectural grid"
            },
            command,
        )?;
        Ok(self.id)
    }
}

impl DesktopApp {
    pub(super) fn begin_grid_form(&mut self, target: Option<Id>) {
        match GridDraft::begin(&self.editor, self.plans.active, target) {
            Ok(draft) => {
                self.wall_gesture = None;
                self.grid_draft = Some(draft);
            }
            Err(error) => self.report(Err(error), ""),
        }
    }
    pub(super) fn grid_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut draft) = self.grid_draft.take() else {
            return;
        };
        let mut close = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        let mut preview_error = None;
        egui::Modal::new(egui::Id::new("architectural_grid_dialog")).show(ctx,|ui| {
            ui.set_width(400.0);
            ui.heading(if draft.editing {"Edit architectural grid"} else {"New architectural grid"});
            ui.label("Model XY coordinates in metres; +X right, +Y up. One Apply is one undoable edit.");
            egui::ScrollArea::vertical().id_salt("grid_form_scroll")
                .max_height((ctx.content_rect().height()-230.0).max(120.0)).show(ui,|ui| {
                let building=self.editor.document.model().buildings.get(&draft.building);
                ui.label(format!("Building: {}",building.map_or("Missing",|b| b.parameters.name.as_str())));
                egui::Grid::new("architectural_grid_fields").num_columns(2).show(ui,|ui| {
                    ui.label("Grid name");
                    ui.add(egui::TextEdit::singleline(&mut draft.name).char_limit(256).desired_width(210.0));ui.end_row();
                    for (i,label) in LABELS.iter().enumerate() {
                        ui.label(*label);
                        ui.add(egui::TextEdit::singleline(&mut draft.values[i]).char_limit(128).desired_width(210.0));ui.end_row();
                    }
                });
                match draft.parameters() {
                    Ok(p) => {
                        let length=p.start.distance(p.end);
                        ui.label(format!("Preview extent: {length:.6} m"));
                        let (response,painter)=ui.allocate_painter(egui::vec2(360.0,100.0),egui::Sense::hover());
                        let center=response.rect.center();
                        let unit=egui::vec2(((p.end.x-p.start.x)/length) as f32, (-(p.end.y-p.start.y)/length) as f32);
                        let a=center-unit*40.0; let b=center+unit*40.0;
                        painter.line_segment([a,b],egui::Stroke::new(2.0_f32,theme::ACCENT));
                        painter.circle_filled(a,3.0,theme::ACCENT);
                        painter.text(a+egui::vec2(5.0,0.0),egui::Align2::LEFT_CENTER,"Start",egui::FontId::proportional(12.0),theme::TEXT);
                        painter.text(b+egui::vec2(5.0,0.0),egui::Align2::LEFT_CENTER,"End",egui::FontId::proportional(12.0),theme::TEXT);
                        ui.label("Direction preview only; not to scale. Crop/visibility may hide the grid in the plan.");
                    }
                    Err(error)=>{preview_error=Some(error.to_string());}
                }
            });
            if let Some(error)=&preview_error {ui.colored_label(theme::ERROR,error);}
            if let Some(error)=&draft.error && Some(error) != preview_error.as_ref() {ui.colored_label(theme::ERROR,error);}
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Cancel grid").clicked() {close=true;}
                if ui.button("Apply grid").clicked() && !close {
                    match draft.apply(&mut self.editor,self.plans.active) {
                        Ok(id)=>{close=true;self.select(Some(id));self.report(Ok(()),"Grid applied.");ctx.request_repaint();}
                        Err(error)=>draft.error=Some(error.to_string()),
                    }
                }
            });
        });
        if !close {
            self.grid_draft = Some(draft);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn editor() -> (Editor, Id) {
        let mut editor = Editor::new().unwrap();
        let level = *editor.document.model().levels.keys().next().unwrap();
        let view = editor.create_floor_plan("Plan", level).unwrap();
        (editor, view)
    }
    #[test]
    fn create_edit_invalid_values_and_stale_grid_drafts_are_atomic() {
        let (mut editor, view) = editor();
        let mut draft = GridDraft::begin(&editor, Some(view), None).unwrap();
        let original = editor.document.model().clone();
        let stats = editor.document.history_stats();
        for value in ["-", "NaN", "inf", "1e999"] {
            draft.values[0] = value.into();
            assert!(draft.apply(&mut editor, Some(view)).is_err());
            assert_eq!(editor.document.model(), &original);
            assert_eq!(editor.document.history_stats(), stats);
        }
        draft.values = ["1", "2", "1", "2"].map(String::from);
        assert!(draft.apply(&mut editor, Some(view)).is_err());
        draft.values = ["1", "2", "4", "6"].map(String::from);
        assert!(draft.apply(&mut editor, None).is_err());
        let id = draft.apply(&mut editor, Some(view)).unwrap();
        let created = editor.document.model().clone();
        assert_eq!(
            created.grids[&id]
                .parameters
                .start
                .distance(created.grids[&id].parameters.end),
            5.0
        );
        assert!(draft.apply(&mut editor, Some(view)).is_err());
        let mut edit = GridDraft::begin(&editor, Some(view), Some(id)).unwrap();
        edit.name = "B".into();
        edit.values[3] = "8".into();
        edit.apply(&mut editor, Some(view)).unwrap();
        assert_eq!(
            editor.document.model().grids[&id].header,
            created.grids[&id].header
        );
        assert!(editor.document.undo());
        assert_eq!(editor.document.model(), &created);
        assert!(edit.apply(&mut editor, Some(view)).is_err());
        let noop = GridDraft::begin(&editor, Some(view), Some(id)).unwrap();
        let stats = editor.document.history_stats();
        noop.apply(&mut editor, Some(view)).unwrap();
        assert_eq!(editor.document.history_stats(), stats);
        assert!(editor.document.can_redo());
        let stale = GridDraft::begin(&editor, Some(view), Some(id)).unwrap();
        editor.document = os_document::Document::from_model(created).unwrap();
        assert!(stale.apply(&mut editor, Some(view)).is_err());
    }
    #[test]
    fn naming_and_duplicate_rejection_are_building_scoped() {
        let (mut editor, view) = editor();
        let first = GridDraft::begin(&editor, Some(view), None).unwrap();
        let id = first.apply(&mut editor, Some(view)).unwrap();
        let mut second = GridDraft::begin(&editor, Some(view), None).unwrap();
        assert_eq!(second.name, "Grid 2");
        second.name = "Grid 1".into();
        let before = editor.document.model().clone();
        assert!(second.apply(&mut editor, Some(view)).is_err());
        assert_eq!(editor.document.model(), &before);
        let edit = GridDraft::begin(&editor, None, Some(id)).unwrap();
        edit.apply(&mut editor, None).unwrap(); // Existing datums editable from Browser in 3D.
        assert!(GridDraft::begin(&editor, None, None).is_err());
    }
}
