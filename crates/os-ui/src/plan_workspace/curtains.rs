//! Two-click native curtain placement. Only the accepted transaction is published.
use super::*;
use os_core::ensure;
use os_model::{
    CurtainGrid, CurtainMullionType, CurtainMullionTypeParams, CurtainPanelKind, CurtainPanelType,
    CurtainPanelTypeParams, CurtainSystem, CurtainSystemParams,
};

pub(super) struct Draft {
    context: PlanContext,
    session: Id,
    revision: u64,
    providers: Vec<(String, Id)>,
    drawing: Id,
    id: Id,
    grids: [Id; 4],
    pub(super) first: Option<Point2>,
    pub height: f64,
    pub base_offset: f64,
}

impl Draft {
    fn current(&self, app: &DesktopApp) -> bool {
        app.plans.active == Some(self.context.view_id)
            && app.plans.active_sheet.is_none()
            && !app.editor.plugin_work_pending()
            && providers_ready(app, self.context.view_id)
            && app.editor.document.session_id() == self.session
            && app.editor.document.revision() == self.revision
            && app.editor.native_plan_context(self.context.view_id).ok() == Some(self.context)
            && plan_provider_signature(&app.editor) == self.providers
            && displayed_drawing(app, self.context.view_id).is_some_and(|drawing| {
                drawing.identity() == self.drawing && drawing.items(self.context).is_ok()
            })
    }

    fn commands(&self, editor: &Editor, end: Point2) -> Result<Vec<Command>> {
        let model = editor.document.model();
        let panel_parameters = CurtainPanelTypeParams {
            name: "Curtain glazing 20 mm".into(),
            kind: CurtainPanelKind::Glazing,
            thickness: 0.02,
            material: None,
        };
        let mullion_parameters = CurtainMullionTypeParams {
            name: "Curtain frame 50 × 100 mm".into(),
            width: 0.05,
            depth: 0.1,
            material: None,
        };
        let mut commands = Vec::new();
        let panel_type = model
            .curtain_panel_types
            .values()
            .find(|entity| entity.parameters == panel_parameters)
            .map(|entity| entity.id())
            .unwrap_or_else(|| {
                let entity = CurtainPanelType::new("core.curtain_panel_type", panel_parameters);
                let id = entity.id();
                commands.push(Command::AddCurtainPanelType(entity));
                id
            });
        let mullion_type = model
            .curtain_mullion_types
            .values()
            .find(|entity| entity.parameters == mullion_parameters)
            .map(|entity| entity.id())
            .unwrap_or_else(|| {
                let entity =
                    CurtainMullionType::new("core.curtain_mullion_type", mullion_parameters);
                let id = entity.id();
                commands.push(Command::AddCurtainMullionType(entity));
                id
            });
        let start = self
            .first
            .ok_or_else(|| Error::Invalid("Choose the curtain start".into()))?;
        let mut parameters = CurtainSystemParams {
            name: format!("Curtain {}", model.curtain_systems.len() + 1),
            level: model.views[&self.context.view_id]
                .parameters
                .level
                .ok_or_else(|| Error::Invalid("Curtains need a plan level".into()))?,
            start,
            end,
            base_offset: self.base_offset,
            height: self.height,
            normal_flip: false,
            panel_type,
            mullion_type,
            vertical: vec![
                CurtainGrid {
                    id: self.grids[0],
                    position: 0.0,
                },
                CurtainGrid {
                    id: self.grids[1],
                    position: start.distance(end),
                },
            ],
            horizontal: vec![
                CurtainGrid {
                    id: self.grids[2],
                    position: 0.0,
                },
                CurtainGrid {
                    id: self.grids[3],
                    position: self.height,
                },
            ],
            panels: Vec::new(),
            mullions: Vec::new(),
        };
        parameters.reconcile()?;
        let mut curtain = CurtainSystem::new("core.curtain_system", parameters);
        curtain.header.id = self.id;
        commands.push(Command::AddCurtainSystem(curtain));
        Ok(commands)
    }
}

pub(super) fn displayed_drawing(app: &DesktopApp, view: Id) -> Option<&PlanDrawing> {
    let native = app.plans.drawing.as_ref()?;
    #[cfg(feature = "external-plugins")]
    if app
        .editor
        .document
        .model()
        .views
        .get(&view)?
        .parameters
        .kind
        == os_model::ViewKind::Plan
    {
        return Some(
            app.plans
                .providers
                .drawing(&app.editor, view)
                .unwrap_or(native),
        );
    }
    let _ = view;
    Some(native)
}

pub(super) fn providers_ready(app: &DesktopApp, view: Id) -> bool {
    #[cfg(feature = "external-plugins")]
    {
        !app.plans.providers.busy() && app.plans.providers.current(&app.editor, view)
    }
    #[cfg(not(feature = "external-plugins"))]
    {
        let _ = (app, view);
        true
    }
}

impl DesktopApp {
    pub(crate) fn begin_curtain(&mut self, view: Id) {
        let result = (|| {
            ensure(
                self.plans.active == Some(view)
                    && self.plans.active_sheet.is_none()
                    && !self.editor.plugin_work_pending()
                    && self.plans.pending.is_none()
                    && providers_ready(self, view),
                "Open a ready floor plan before placing a curtain",
            )?;
            let context = self.editor.native_plan_context(view)?;
            let drawing = displayed_drawing(self, view)
                .ok_or_else(|| Error::Invalid("Wait for the plan drawing".into()))?;
            drawing.items(context)?;
            let drawing = drawing.identity();
            self.select(None);
            self.cancel_plan_wall();
            self.cancel_aligned_dimension();
            self.cancel_opening_placement();
            self.cancel_opening_controls();
            self.plans.floor_sketch = None;
            self.plans.floor_hole_sketch = None;
            self.plans.floor_vertex_drag = None;
            self.plans.floor_properties = None;
            self.plans.ceiling_draft = None;
            self.plans.section_placement = None;
            self.plans.detail_line_draft = None;
            self.plans.room_separation_line_draft = None;
            self.plans.room_tag_draft = None;
            self.plans.opening_tag_draft = None;
            self.plans.room_placement_active = false;
            self.plans.area_selection.cancel();
            self.plans.crop.mode = None;
            self.plans.crop.cancel();
            self.grid_draft = None;
            self.plan_draft = None;
            self.plans.curtain_draft = Some(Draft {
                context,
                session: self.editor.document.session_id(),
                revision: self.editor.document.revision(),
                providers: plan_provider_signature(&self.editor),
                drawing,
                id: Id::new(),
                grids: std::array::from_fn(|_| Id::new()),
                first: None,
                height: 3.0,
                base_offset: 0.0,
            });
            Ok(())
        })();
        self.report(result, "Curtain · click start, then end · Escape cancels.");
    }

    pub(super) fn validate_curtain(&mut self, ctx: &egui::Context) {
        if self.plans.curtain_draft.is_some() {
            // Keep this frame and any held press owned even if validation cancels.
            self.plans.curtain_pointer_claimed = true;
        }
        let cancel = ctx.input(|input| {
            input.key_pressed(egui::Key::Escape)
                || input
                    .events
                    .iter()
                    .any(|event| matches!(event, egui::Event::PointerGone))
        });
        if self
            .plans
            .curtain_draft
            .as_ref()
            .is_some_and(|draft| cancel || !draft.current(self))
        {
            self.plans.curtain_draft = None;
            self.report(Ok(()), "Curtain placement canceled.");
        }
    }

    pub(super) fn curtain_click(&mut self, target: Point2) -> Result<()> {
        let draft = self
            .plans
            .curtain_draft
            .as_ref()
            .ok_or_else(|| Error::Invalid("Curtain placement canceled".into()))?;
        ensure(draft.current(self), "Curtain placement context changed")?;
        ensure(
            point_in_plan_crop(draft.context, draft.context.basis.world_to_plane(target)?),
            "Place curtain endpoints inside the plan crop",
        )?;
        if draft.first.is_none() {
            self.plans.curtain_draft.as_mut().unwrap().first = Some(target);
            return Ok(());
        }
        let commands = draft.commands(&self.editor, target)?;
        // Prepare the exact transaction and all geometry on a disposable editor.
        // Editor::command itself cannot roll back a regeneration failure.
        let mut staged = Editor::new()?;
        staged.document = Document::from_model(self.editor.document.model().clone())?;
        staged.scene = self.editor.scene.clone();
        staged.pending_geometry = self.editor.pending_geometry.clone();
        staged.document.execute("Place curtain", commands.clone())?;
        staged.regenerate()?;
        staged.native_drawing(draft.context.view_id)?;
        ensure(draft.current(self), "Curtain placement context changed")?;
        let id = draft.id;
        self.editor.document.execute("Place curtain", commands)?;
        self.editor.scene = staged.scene;
        self.editor.pending_geometry = staged.pending_geometry;
        self.plans.curtain_draft = None;
        self.select(Some(id));
        Ok(())
    }
}

pub(super) fn target(
    drawing: &PlanDrawing,
    context: PlanContext,
    camera: PlanCamera,
    rect: egui::Rect,
    pointer: egui::Pos2,
    snaps: SnapOptions,
) -> Result<Point2> {
    let point = floor_snap_target(
        drawing,
        context,
        camera,
        [f64::from(rect.width()), f64::from(rect.height())],
        Point2::new(
            f64::from(pointer.x - rect.left()),
            f64::from(pointer.y - rect.top()),
        ),
        snaps,
    )?;
    ensure(
        point_in_plan_crop(context, point),
        "Curtain endpoint is outside the plan crop",
    )?;
    context.basis.plane_to_world(point)
}

pub(super) fn paint(
    draft: &Draft,
    target: Point2,
    painter: &egui::Painter,
    camera: PlanCamera,
    rect: egui::Rect,
) {
    let project = |point| {
        draft
            .context
            .basis
            .world_to_plane(point)
            .and_then(|point| {
                camera.project(point, [f64::from(rect.width()), f64::from(rect.height())])
            })
            .map(|point| rect.min + egui::vec2(point.x as f32, point.y as f32))
    };
    if let Ok(end) = project(target) {
        painter.circle_stroke(end, 5.0, egui::Stroke::new(1.5, theme::ACCENT));
        if let Some(start) = draft.first.and_then(|point| project(point).ok()) {
            painter.line_segment([start, end], egui::Stroke::new(2.0, theme::ACCENT));
            painter.circle_filled(start, 4.0, theme::ACCENT);
        }
    }
}
