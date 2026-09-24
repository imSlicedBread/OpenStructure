//! Exact, revision-bound native doors/windows and their host-derived graphics.
use super::*;
use os_core::ensure;
use os_geometry::openings::{component_point, frame_offset, frame_thickness, pane_offset, world};
use os_model::{
    DoorHinge, DoorSwing, Opening, OpeningDefinition, OpeningKind, OpeningParams, OpeningType,
    OpeningTypeParams, ResolvedOpening,
};
use os_render::plan::{PlanContext, PlanLine};

pub(super) fn has_openings(model: &Model, host: Id) -> bool {
    model.openings.values().any(|o| o.parameters.host == host)
}

pub(super) fn host_mesh(model: &Model, id: Id) -> Result<os_geometry::Mesh> {
    os_geometry::walls::NativeWall::from_model(model, id)?.mesh()
}

pub(super) fn panel_mesh(model: &Model, id: Id) -> Result<os_geometry::Mesh> {
    let p = model.resolve_opening(&model.openings[&id].parameters)?;
    let host = &model.resolve_wall(p.host)?.parameters;
    os_geometry::openings::component_mesh(&p, host, model.levels[&host.level].parameters.elevation)
}

const SWING_SEGMENTS: usize = 16;

/// One press owns a same-host offset edit. All geometry is derived from a
/// disposable model; only `commit` is allowed to touch document history.
pub(super) struct OpeningMove {
    context: PlanContext,
    activation: Option<Id>,
    providers: Vec<(String, Id)>,
    drawing: Id,
    pub id: Id,
    original: OpeningParams,
    origin: egui::Pos2,
    anchor: Point2,
    offset: f64,
    moved: bool,
}

impl OpeningMove {
    pub fn current(&self, editor: &Editor, view: Option<Id>, selected: Option<Id>) -> bool {
        view == Some(self.context.view_id)
            && selected == Some(self.id)
            && editor.native_plan_context(self.context.view_id).ok() == Some(self.context)
            && editor.host.activation_id(os_walls::PLUGIN_ID) == self.activation
            && crate::plan_workspace::plan_provider_signature(editor) == self.providers
    }

    fn candidate(&self, editor: &Editor) -> Result<Model> {
        let mut model = editor.document.model().clone();
        let opening = model
            .openings
            .get_mut(&self.id)
            .ok_or_else(|| Error::Invalid("Opening no longer exists".into()))?;
        opening.parameters = self.original.clone();
        opening.parameters.offset = self.offset;
        model.validate()?;
        Ok(model)
    }

    pub fn preview(&self, editor: &Editor) -> Result<os_render::plan::PlanDrawing> {
        let model = self.candidate(editor)?;
        let wall = os_geometry::walls::NativeWall::from_model(&model, self.original.host)?;
        let mut cells = Vec::new();
        for (layer, footprints) in
            wall.layer_plan_footprints(self.context.range, self.context.basis, self.context.crop)?
        {
            let surface = os_geometry::SurfaceIdentity {
                layer: layer.id,
                material: layer.material,
            };
            cells.extend(footprints.into_iter().map(|footprint| (surface, footprint)));
        }
        let seams = wall
            .seams()
            .into_iter()
            .map(|(a, b)| {
                Ok((
                    self.context.basis.world_to_plane(a)?,
                    self.context.basis.world_to_plane(b)?,
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        let resolved = model.resolve_opening(&model.openings[&self.id].parameters)?;
        os_render::plan::PlanDrawing::from_layered_footprints(
            self.context,
            &[(wall.entity, cells)].into(),
            vec![self.id],
        )?
        .without_wall_seams(&[(wall.entity, seams)].into())?
        .with_native_lines(
            [(
                self.id,
                plan_symbol(
                    self.id,
                    &resolved,
                    &wall.parameters,
                    wall.elevation,
                    self.context,
                )?,
            )]
            .into(),
        )
    }

    pub fn host(&self) -> Id {
        self.original.host
    }

    pub fn commit(self, editor: &mut Editor, view: Option<Id>, selected: Option<Id>) -> Result<()> {
        ensure(
            self.current(editor, view, selected),
            "Opening move is stale",
        )?;
        // Revalidate the final pointer candidate, never a cached last-valid one.
        self.candidate(editor)?;
        if self.moved && (self.offset - self.original.offset).abs() > 1e-6 {
            let mut parameters = self.original;
            parameters.offset = self.offset;
            editor.command(
                "Move opening",
                Command::UpdateOpening {
                    id: self.id,
                    parameters,
                },
            )?;
        }
        Ok(())
    }
}

fn opening_grip(
    editor: &Editor,
    id: Id,
    offset: Option<f64>,
    context: PlanContext,
    camera: os_render::plan::PlanCamera,
    rect: egui::Rect,
) -> Option<egui::Pos2> {
    let model = editor.document.model();
    let opening = model.openings.get(&id)?;
    let p = model.resolve_opening(&opening.parameters).ok()?;
    let wall = model.resolve_wall(p.host).ok()?;
    if !context.show_walls
        || model.views[&context.view_id].parameters.level != Some(wall.parameters.level)
    {
        return None;
    }
    let point = context
        .basis
        .world_to_plane(world(
            &wall.parameters,
            offset.unwrap_or(p.offset) + p.width * 0.5,
            0.0,
        ))
        .ok()?;
    if context.crop.is_some_and(|c| {
        point.x < c.min.x || point.x > c.max.x || point.y < c.min.y || point.y > c.max.y
    }) {
        return None;
    }
    let screen = camera
        .project(point, [f64::from(rect.width()), f64::from(rect.height())])
        .ok()?;
    let pos = rect.min + egui::vec2(screen.x as f32, screen.y as f32);
    (pos.is_finite() && rect.contains(pos)).then_some(pos)
}

/// Returns a release request. Keep pointer ownership until the workspace has
/// consumed that release, even when Escape or stale context discarded the draft.
#[allow(clippy::too_many_arguments)]
pub(super) fn move_input(
    editor: &Editor,
    draft: &mut Option<OpeningMove>,
    claimed: &mut bool,
    drawing: &os_render::plan::PlanDrawing,
    context: PlanContext,
    camera: os_render::plan::PlanCamera,
    response: &egui::Response,
    ctx: &egui::Context,
    selected: Option<Id>,
    allow: bool,
) -> bool {
    let rect = response.rect;
    let plane = |pos: egui::Pos2| {
        camera.unproject(
            Point2::new(
                f64::from(pos.x - rect.left()),
                f64::from(pos.y - rect.top()),
            ),
            [f64::from(rect.width()), f64::from(rect.height())],
        )
    };
    if draft
        .as_ref()
        .is_some_and(|d| d.drawing != drawing.identity() || !allow)
    {
        *draft = None;
    }
    if allow
        && !*claimed
        && response.contains_pointer()
        && ctx.input(|i| i.pointer.primary_pressed())
        && let Some(id) = selected
        && drawing
            .provider_lines(context)
            .is_ok_and(|lines| lines.iter().any(|l| l.entity == id))
        && let Some(grip) = opening_grip(editor, id, None, context, camera, rect)
        && let Some(pos) = ctx.input(|i| i.pointer.press_origin())
        && pos.distance(grip) <= 10.0
        && let Ok(anchor) = plane(pos)
    {
        let original = editor.document.model().openings[&id].parameters.clone();
        *draft = Some(OpeningMove {
            context,
            activation: editor.host.activation_id(os_walls::PLUGIN_ID),
            providers: crate::plan_workspace::plan_provider_signature(editor),
            drawing: drawing.identity(),
            id,
            offset: original.offset,
            original,
            origin: pos,
            anchor,
            moved: false,
        });
        *claimed = true;
    }
    if let Some(d) = draft {
        if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
            d.moved |= pos.distance(d.origin) > ctx.options(|o| o.input_options.max_click_dist);
            let wall = &editor.document.model().walls[&d.original.host].parameters;
            d.offset = plane(pos)
                .and_then(|point| context.basis.plane_to_world(point))
                .map_or(f64::NAN, |point| {
                    let anchor = context.basis.plane_to_world(d.anchor).unwrap();
                    d.original.offset
                        + ((point.x - anchor.x) * (wall.end.x - wall.start.x)
                            + (point.y - anchor.y) * (wall.end.y - wall.start.y))
                            / wall.length()
                });
        }
        if ctx.input(|i| i.pointer.primary_released()) {
            if response.contains_pointer()
                && ctx
                    .input(|i| i.pointer.interact_pos())
                    .is_some_and(|p| rect.contains(p))
            {
                return true;
            }
            *draft = None;
        }
    }
    false
}

#[allow(clippy::too_many_arguments)]
pub(super) fn paint_move_grip(
    editor: &Editor,
    draft: Option<&OpeningMove>,
    selected: Option<Id>,
    drawing: &os_render::plan::PlanDrawing,
    context: PlanContext,
    camera: os_render::plan::PlanCamera,
    response: &egui::Response,
    painter: &egui::Painter,
    allow: bool,
    error: Option<&str>,
) {
    if !allow {
        return;
    }
    if let Some(id) = selected
        && drawing
            .provider_lines(context)
            .is_ok_and(|lines| lines.iter().any(|l| l.entity == id))
        && let Some(pos) = opening_grip(
            editor,
            id,
            draft.map(|d| d.offset),
            context,
            camera,
            response.rect,
        )
    {
        painter.circle_filled(
            pos,
            6.0,
            if error.is_some() {
                theme::ERROR
            } else {
                theme::ACCENT
            },
        );
        painter.circle_stroke(pos, 6.0, egui::Stroke::new(1.0, theme::TEXT));
        if response
            .hover_pos()
            .is_some_and(|p| p.distance(pos) <= 10.0)
        {
            response.ctx.set_cursor_icon(egui::CursorIcon::Grab);
        }
    }
    if let Some(error) = error {
        painter.text(
            response.rect.left_top() + egui::vec2(12.0, 12.0),
            egui::Align2::LEFT_TOP,
            format!("Cannot move: {error}"),
            egui::FontId::proportional(12.0),
            theme::ERROR,
        );
    }
}

pub(super) fn plan_symbol(
    id: Id,
    p: &ResolvedOpening,
    wall: &WallParams,
    elevation: f64,
    context: PlanContext,
) -> Result<Vec<PlanLine>> {
    let mut aperture = os_walls::wall_solid(wall, elevation + p.sill)?;
    aperture.height = p.height;
    aperture.profile.vertices[0].x = p.offset;
    aperture.profile.vertices[3].x = p.offset;
    aperture.profile.vertices[1].x = p.offset + p.width;
    aperture.profile.vertices[2].x = p.offset + p.width;
    let Some(footprint) =
        os_geometry::plan::rectangular_plan(&aperture, context.range, context.basis, None)?
    else {
        return Ok(vec![]);
    };
    let half = wall.thickness / 2.0;
    let spans =
        os_geometry::openings::plan_spans(p, elevation, context.range.cut, context.range.depth)?;
    let mut pairs = Vec::new();
    for (a, b) in
        os_geometry::openings::cut_plan_spans(p, elevation, context.range.cut, context.range.depth)?
    {
        for u in [a, b] {
            let x = p.offset + u * p.width;
            pairs.push((Point2::new(x, -half), Point2::new(x, half)));
        }
    }
    match p.kind {
        OpeningKind::Window => {
            for y in [-half * 0.5, pane_offset(p, wall), half * 0.5] {
                for &(a, b) in &spans {
                    pairs.push((
                        Point2::new(p.offset + a * p.width, y),
                        Point2::new(p.offset + b * p.width, y),
                    ));
                }
            }
        }
        OpeningKind::Door => {
            let hinge = component_point(p, wall, 0.);
            for &(a, b) in &spans {
                pairs.push((component_point(p, wall, a), component_point(p, wall, b)));
            }
            let radius = spans.last().map_or(0., |s| s.1 * p.width);
            let tip = component_point(p, wall, radius / p.width);
            let closed = if p.hinge == DoorHinge::Start {
                1.0
            } else {
                -1.0
            };
            let side = if p.swing == DoorSwing::Left {
                1.0
            } else {
                -1.0
            };
            let point = |i: usize| {
                let angle = std::f64::consts::FRAC_PI_2 * i as f64 / SWING_SEGMENTS as f64;
                if i == SWING_SEGMENTS {
                    tip
                } else {
                    Point2::new(
                        hinge.x + closed * radius * angle.cos(),
                        hinge.y + side * radius * angle.sin(),
                    )
                }
            };
            for i in 0..if radius > 0. { SWING_SEGMENTS } else { 0 } {
                pairs.push((point(i), point(i + 1)));
            }
        }
    }
    if p.family.frame_width > 0.0 {
        let half_frame_depth = frame_thickness(p, wall) / 2.;
        let frame_center = frame_offset(p, wall);
        let frame_y = [
            frame_center - half_frame_depth,
            frame_center + half_frame_depth,
        ];
        let frame_width = p.family.frame_width;
        for (start, end) in [(0.0, frame_width), (p.width - frame_width, p.width)] {
            let x0 = p.offset + start;
            let x1 = p.offset + end;
            pairs.extend([
                (Point2::new(x0, frame_y[0]), Point2::new(x1, frame_y[0])),
                (Point2::new(x1, frame_y[0]), Point2::new(x1, frame_y[1])),
                (Point2::new(x1, frame_y[1]), Point2::new(x0, frame_y[1])),
                (Point2::new(x0, frame_y[1]), Point2::new(x0, frame_y[0])),
            ]);
        }
    }
    pairs
        .into_iter()
        .enumerate()
        .map(|(feature, (a, b))| {
            let start = context.basis.world_to_plane(world(wall, a.x, a.y))?;
            let end = context.basis.world_to_plane(world(wall, b.x, b.y))?;
            Ok(
                crate::plan_workspace::clip_plan_segment(start, end, context.crop).map(
                    |(start, end)| PlanLine {
                        entity: id,
                        feature: feature as u32,
                        start,
                        end,
                        role: footprint.role,
                    },
                ),
            )
        })
        .collect::<Result<Vec<_>>>()
        .map(|lines| lines.into_iter().flatten().collect())
}

const LABELS: [&str; 4] = [
    "Offset from wall start (m)",
    "Width (m)",
    "Height (m)",
    "Sill above floor (m)",
];
pub(super) struct OpeningDraft {
    session: Id,
    revision: u64,
    view: Id,
    selection: Id,
    activation: Option<Id>,
    id: Id,
    editing: bool,
    kind: OpeningKind,
    type_id: Option<Id>,
    params: OpeningParams,
    values: [String; 4],
    error: Option<String>,
}
impl OpeningDraft {
    fn begin(
        editor: &Editor,
        view: Option<Id>,
        selection: Option<Id>,
        kind: Option<OpeningKind>,
        preferred_type: Option<Id>,
    ) -> Result<Self> {
        let view = view.ok_or_else(|| Error::Invalid("Open a floor plan first".into()))?;
        let context = editor.native_plan_context(view)?;
        let selection = selection
            .ok_or_else(|| Error::Invalid("Select a native wall or opening first".into()))?;
        let model = editor.document.model();
        let selected_opening = model.openings.get(&selection);
        let host = selected_opening.map_or(selection, |o| o.parameters.host);
        let wall = model
            .walls
            .get(&host)
            .ok_or_else(|| Error::Invalid("Select a native straight wall".into()))?;
        ensure(
            model.views[&view].parameters.level == Some(wall.parameters.level),
            "Use a floor plan on the host wall level",
        )?;
        ensure(
            context.show_walls
                && os_geometry::plan::rectangular_plan(
                    &os_walls::wall_solid(
                        &wall.parameters,
                        model.levels[&wall.parameters.level].parameters.elevation,
                    )?,
                    context.range,
                    context.basis,
                    context.crop,
                )?
                .is_some(),
            "Host wall is hidden in this plan",
        )?;
        let editing = kind.is_none();
        let (id, params, resolved) = if editing {
            let opening = selected_opening
                .ok_or_else(|| Error::Invalid("Select a door or window to edit".into()))?;
            (
                opening.id(),
                opening.parameters.clone(),
                model.resolve_opening(&opening.parameters)?,
            )
        } else {
            let kind = kind.unwrap();
            let type_id = preferred_type
                .filter(|id| {
                    model
                        .opening_types
                        .get(id)
                        .is_some_and(|t| t.parameters.kind == kind)
                })
                .or_else(|| {
                    model
                        .opening_types
                        .iter()
                        .find(|(_, t)| t.parameters.kind == kind)
                        .map(|(id, _)| *id)
                });
            let dimensions =
                type_id.and_then(|id| model.opening_types.get(&id).map(|t| t.parameters.clone()));
            let width = dimensions.as_ref().map_or_else(
                || if kind == OpeningKind::Door { 0.9 } else { 1.2 },
                |p| p.width,
            );
            let height = dimensions.as_ref().map_or_else(
                || if kind == OpeningKind::Door { 2.1 } else { 1.2 },
                |p| p.height,
            );
            let sill = dimensions.as_ref().map_or_else(
                || if kind == OpeningKind::Door { 0.0 } else { 0.9 },
                |p| p.sill,
            );
            let definition = type_id.map_or(
                OpeningDefinition::Legacy {
                    kind,
                    width,
                    height,
                    sill,
                },
                |type_id| OpeningDefinition::Typed { type_id },
            );
            let parameters = OpeningParams {
                hinge: Default::default(),
                swing: Default::default(),
                name: format!("{kind:?}"),
                host,
                offset: ((wall.parameters.length() - width) * 0.5).max(0.001),
                definition,
            };
            let resolved = model.resolve_opening(&parameters)?;
            (Id::new(), parameters, resolved)
        };
        let values = [
            params.offset,
            resolved.width,
            resolved.height,
            resolved.sill,
        ]
        .map(|n| n.to_string());
        Ok(Self {
            session: editor.document.session_id(),
            revision: editor.document.revision(),
            view,
            selection,
            activation: editor.host.activation_id(os_walls::PLUGIN_ID),
            id,
            editing,
            kind: resolved.kind,
            type_id: resolved.type_id,
            params,
            values,
            error: None,
        })
    }
    fn current(&self, editor: &Editor, view: Option<Id>, selected: Option<Id>) -> bool {
        self.session == editor.document.session_id()
            && self.revision == editor.document.revision()
            && view == Some(self.view)
            && selected == Some(self.selection)
            && self.activation == editor.host.activation_id(os_walls::PLUGIN_ID)
    }
    fn parameters(&self) -> Result<OpeningParams> {
        let mut values = [0.0; 4];
        for (i, text) in self.values.iter().enumerate() {
            values[i] = text
                .trim()
                .parse()
                .map_err(|_| Error::Invalid(format!("{} needs a number", LABELS[i])))?;
        }
        let mut p = self.params.clone();
        p.offset = values[0];
        if let OpeningDefinition::Legacy {
            kind,
            width,
            height,
            sill,
        } = &mut p.definition
        {
            *width = values[1];
            *height = values[2];
            *sill = if *kind == OpeningKind::Door {
                0.0
            } else {
                values[3]
            };
        }
        Ok(p)
    }

    /// Validate a disposable candidate. Preview never changes model or history.
    fn preview(
        &self,
        editor: &Editor,
        view: Option<Id>,
        selected: Option<Id>,
    ) -> Result<Vec<PlanLine>> {
        ensure(
            self.current(editor, view, selected),
            "Opening draft is stale; reopen it",
        )?;
        let mut model = editor.document.model().clone();
        let params = self.parameters()?;
        let resolved = model.resolve_opening(&params)?;
        if self.editing {
            model.openings.get_mut(&self.id).unwrap().parameters = params;
        } else {
            let mut opening = Opening::new("core.opening", params);
            opening.header.id = self.id;
            model.openings.insert(self.id, opening);
        }
        model.validate()?;
        let wall = &model.walls[&resolved.host].parameters;
        let context = editor.native_plan_context(self.view)?;
        plan_symbol(
            self.id,
            &resolved,
            wall,
            model.levels[&wall.level].parameters.elevation,
            context,
        )
    }
    fn apply(
        &self,
        editor: &mut Editor,
        view: Option<Id>,
        selected: Option<Id>,
        delete: bool,
    ) -> Result<Id> {
        ensure(
            self.current(editor, view, selected),
            "Opening draft is stale; reopen it",
        )?;
        if delete {
            ensure(self.editing, "Only an existing opening can be deleted")?;
            editor.command("Delete opening", Command::RemoveOpening(self.id))?;
        } else if self.editing {
            editor.command(
                "Edit opening",
                Command::UpdateOpening {
                    id: self.id,
                    parameters: self.parameters()?,
                },
            )?;
        } else {
            let mut parameters = self.parameters()?;
            let mut commands = Vec::new();
            if self.type_id.is_none() {
                let resolved = editor.document.model().resolve_opening(&parameters)?;
                let opening_type = OpeningType::new(
                    "core.opening_type",
                    OpeningTypeParams {
                        family: Default::default(),
                        name: match resolved.kind {
                            OpeningKind::Door => "Basic Door 900 × 2100".into(),
                            OpeningKind::Window => "Basic Window 1200 × 1200".into(),
                        },
                        kind: resolved.kind,
                        width: resolved.width,
                        height: resolved.height,
                        sill: resolved.sill,
                        pane_position: resolved.pane_position,
                    },
                );
                parameters.definition = OpeningDefinition::Typed {
                    type_id: opening_type.id(),
                };
                commands.push(Command::AddOpeningType(opening_type));
            }
            let mut opening = Opening::new("core.opening", parameters);
            opening.header.id = self.id;
            commands.push(Command::AddOpening(opening));
            editor.document.execute("Create opening", commands)?;
            editor.regenerate()?;
        }
        Ok(if delete { self.params.host } else { self.id })
    }
}

impl DesktopApp {
    pub(super) fn begin_opening(&mut self, kind: Option<OpeningKind>) {
        match OpeningDraft::begin(
            &self.editor,
            self.plans.active,
            self.selected,
            kind,
            kind.and_then(|kind| self.preferred_type(kind)),
        ) {
            Ok(draft) => {
                self.cancel_plan_wall();
                self.cancel_opening_placement();
                self.cancel_aligned_dimension();
                self.plans.room_placement_active = false;
                self.opening_draft = Some(draft);
            }
            Err(error) => self.report(Err(error), ""),
        }
    }
    pub(super) fn opening_commands(&mut self, ui: &mut egui::Ui) {
        let types: Vec<_> = self
            .editor
            .document
            .model()
            .opening_types
            .iter()
            .map(|(id, ty)| (*id, ty.parameters.kind, ty.parameters.name.clone()))
            .collect();
        ui.add_enabled_ui(self.plans.active.is_some(), |ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    if ui
                        .button("Door")
                        .on_hover_text("Place a door by clicking a visible wall")
                        .clicked()
                    {
                        self.begin_opening_placement(OpeningKind::Door);
                    }
                    if ui
                        .button("Window")
                        .on_hover_text("Place a window by clicking a visible wall")
                        .clicked()
                    {
                        self.begin_opening_placement(OpeningKind::Window);
                    }
                });
                ui.horizontal(|ui| {
                    ui.menu_button("Exact new…", |ui| {
                        if ui.button("Door dimensions…").clicked() {
                            self.begin_opening(Some(OpeningKind::Door));
                            ui.close();
                        }
                        if ui.button("Window dimensions…").clicked() {
                            self.begin_opening(Some(OpeningKind::Window));
                            ui.close();
                        }
                    });
                    for (kind, label) in [
                        (OpeningKind::Door, "Door type…"),
                        (OpeningKind::Window, "Window type…"),
                    ] {
                        ui.menu_button(label, |ui| {
                            for (id, candidate_kind, name) in &types {
                                if *candidate_kind == kind
                                    && ui
                                        .selectable_label(
                                            self.preferred_type(kind) == Some(*id),
                                            name,
                                        )
                                        .clicked()
                                {
                                    self.remember_type(kind, Some(*id));
                                    ui.close();
                                }
                            }
                            if ui.button("New type…").clicked() {
                                self.begin_new_opening_type(kind);
                                ui.close();
                            }
                        });
                    }
                    if ui.button("Edit opening").clicked() {
                        self.begin_opening(None);
                    }
                });
            });
        });
    }
    pub(super) fn opening_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut draft) = self.opening_draft.take() else {
            return;
        };
        if !draft.current(&self.editor, self.plans.active, self.selected) {
            self.report(Err(Error::Invalid("Opening draft cancelled because its document, view, selection or provider changed".into())), "");
            return;
        }
        let mut close = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        let mut edit_type = None;
        let mut make_type = false;
        egui::Modal::new(egui::Id::new("opening_dialog")).show(ctx, |ui| {
            ui.set_width(380.0);
            ui.heading(format!("{} {:?}", if draft.editing { "Edit" } else { "New" }, draft.kind));
            let host = &self.editor.document.model().walls[&draft.params.host].parameters;
            ui.label(format!("Host: {} · {:.3} m long · {:.3} m high", host.name, host.length(), host.height));
            ui.label("Offset, hinge and swing belong to this instance. Width, height and sill belong to the reusable type. All dimensions are in metres.");
            egui::ScrollArea::vertical().max_height((ctx.content_rect().height()-260.0).max(100.0)).show(ui, |ui| {
                egui::Grid::new("opening_fields").num_columns(2).show(ui, |ui| {
                    ui.label("Name"); ui.add(egui::TextEdit::singleline(&mut draft.params.name).char_limit(256).desired_width(170.0)); ui.end_row();
                    ui.label(LABELS[0]);
                    ui.add(egui::TextEdit::singleline(&mut draft.values[0]).char_limit(64).desired_width(170.0));
                    ui.end_row();
                    match self.editor.document.model().resolve_opening(&draft.params) {
                        Ok(resolved) if resolved.type_id.is_some() => {
                            ui.label("Assigned type");
                            ui.label(format!("{} · {:.3} × {:.3} m · sill {:.3} m",
                                resolved.type_name.unwrap_or_else(|| "Opening type".into()),
                                resolved.width, resolved.height, resolved.sill));
                            ui.end_row();
                            if let Some(type_id) = resolved.type_id
                                && ui.button("Edit shared type dimensions…").clicked() {
                                edit_type = Some(type_id);
                            }
                        }
                        Ok(resolved) => {
                            for (index, label) in LABELS.iter().enumerate().skip(1) {
                                ui.label(*label);
                                ui.add_enabled(index != 3 || resolved.kind == OpeningKind::Window,
                                    egui::TextEdit::singleline(&mut draft.values[index]).char_limit(64).desired_width(170.0));
                                ui.end_row();
                            }
                            if draft.editing && ui.button("Create reusable type from this opening…").clicked() {
                                make_type = true;
                            }
                        }
                        Err(error) => { ui.colored_label(theme::ERROR, error.to_string()); }
                    }
                    if draft.kind == OpeningKind::Door {
                        ui.end_row();
                        ui.label("Hinge");
                        ui.horizontal(|ui| {
                            ui.selectable_value(&mut draft.params.hinge, DoorHinge::Start, "Wall start");
                            ui.selectable_value(&mut draft.params.hinge, DoorHinge::End, "Wall end");
                        });
                        ui.end_row();
                        ui.label("Swing");
                        ui.horizontal(|ui| {
                            ui.selectable_value(&mut draft.params.swing, DoorSwing::Left, "Left of wall");
                            ui.selectable_value(&mut draft.params.swing, DoorSwing::Right, "Right of wall");
                        });
                        ui.end_row();
                    }
                });
                ui.label("Keep at least 1 mm at wall ends, above the opening and between openings. Left/right are viewed along wall start → end. Reversing the wall reverses this frame; doors open at 90°.");
                match draft.preview(&self.editor, self.plans.active, self.selected) {
                    Ok(lines) if !lines.is_empty() => {
                        ui.label("Opening preview");
                        let (rect, _) = ui.allocate_exact_size(egui::vec2(300.0, 110.0), egui::Sense::hover());
                        let mut min = Point2::new(f64::INFINITY, f64::INFINITY);
                        let mut max = Point2::new(f64::NEG_INFINITY, f64::NEG_INFINITY);
                        for p in lines.iter().flat_map(|line| [line.start, line.end]) {
                            min.x = min.x.min(p.x); min.y = min.y.min(p.y);
                            max.x = max.x.max(p.x); max.y = max.y.max(p.y);
                        }
                        let scale = (280.0 / (max.x-min.x).max(0.001)).min(90.0 / (max.y-min.y).max(0.001));
                        let screen = |p: Point2| rect.center() + egui::vec2(
                            ((p.x - (min.x+max.x)*0.5)*scale) as f32,
                            (-(p.y - (min.y+max.y)*0.5)*scale) as f32);
                        for line in lines {
                            ui.painter().line_segment([screen(line.start), screen(line.end)], egui::Stroke::new(2.0, theme::ACCENT));
                        }
                    }
                    Ok(_) => { ui.label("Opening is outside the current plan crop/range."); }
                    Err(error) => { ui.colored_label(theme::ERROR, error.to_string()); }
                }
            });
            if let Some(error) = &draft.error { ui.colored_label(theme::ERROR, error); }
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Cancel opening").clicked() { close = true; }
                let apply = ui.button("Apply opening").clicked();
                let delete = draft.editing && ui.button("Delete opening").clicked();
                if (apply || delete) && !close && edit_type.is_none() && !make_type {
                    match draft.apply(&mut self.editor, self.plans.active, self.selected, delete) {
                        Ok(id) => {
                            close = true;
                            if let Some(opening) = self.editor.document.model().openings.get(&id)
                                && let Ok(resolved) = self.editor.document.model().resolve_opening(&opening.parameters)
                            {
                                self.remember_type(resolved.kind, resolved.type_id);
                            }
                            self.select(Some(id));
                            self.report(Ok(()), "Opening applied.");
                            ctx.request_repaint();
                        }
                        Err(error) => draft.error = Some(error.to_string()),
                    }
                }
            });
        });
        if let Some(id) = edit_type {
            self.begin_edit_opening_type(id);
        } else if make_type {
            self.begin_type_from_opening(draft.id);
        } else if !close {
            self.opening_draft = Some(draft);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn orientation_edit_preview_apply_invalid_cancel_stale_and_undo_redo() {
        let mut e = Editor::new().unwrap();
        let level = *e.document.model().levels.keys().next().unwrap();
        let view = e.create_floor_plan("Plan", level).unwrap();
        let wall = os_model::Wall::new(os_walls::WALL_TYPE, default_wall(level));
        let host = wall.id();
        e.command("Wall", Command::AddWall(wall)).unwrap();
        let draft =
            OpeningDraft::begin(&e, Some(view), Some(host), Some(OpeningKind::Door), None).unwrap();
        assert_eq!(
            (draft.params.hinge, draft.params.swing),
            (DoorHinge::Start, DoorSwing::Left)
        );
        let id = draft.apply(&mut e, Some(view), Some(host), false).unwrap();
        let initial = e.document.model().clone();
        let history = e.document.history_stats();
        let scene = e.scene.clone();
        let mut edit = OpeningDraft::begin(&e, Some(view), Some(id), None, None).unwrap();
        let baseline = edit.preview(&e, Some(view), Some(id)).unwrap();
        edit.params.hinge = DoorHinge::End;
        edit.params.swing = DoorSwing::Right;
        let changed = edit.preview(&e, Some(view), Some(id)).unwrap();
        assert_ne!(baseline, changed);
        assert_eq!(e.document.model(), &initial);
        assert_eq!(e.document.history_stats(), history);
        assert_eq!(e.scene, scene);
        drop(edit); // Cancel drops the draft; no command or history entry.
        let mut edit = OpeningDraft::begin(&e, Some(view), Some(id), None, None).unwrap();
        assert_eq!(edit.preview(&e, Some(view), Some(id)).unwrap(), baseline);
        edit.params.hinge = DoorHinge::End;
        edit.params.swing = DoorSwing::Right;
        edit.values[0] = "NaN".into();
        assert!(edit.preview(&e, Some(view), Some(id)).is_err());
        assert!(edit.apply(&mut e, Some(view), Some(id), false).is_err());
        assert_eq!(e.document.model(), &initial);
        assert_eq!(e.document.history_stats(), history);
        edit.values[0] = initial.openings[&id].parameters.offset.to_string();
        assert!(edit.preview(&e, None, Some(id)).is_err());
        assert!(edit.preview(&e, Some(view), Some(host)).is_err());
        edit.apply(&mut e, Some(view), Some(id), false).unwrap();
        assert_eq!(
            e.document.history_stats().undo_entries,
            history.undo_entries + 1
        );
        let applied = e.document.model().clone();
        let applied_scene = e.scene.clone();
        assert_eq!(
            (
                applied.openings[&id].parameters.hinge,
                applied.openings[&id].parameters.swing
            ),
            (DoorHinge::End, DoorSwing::Right)
        );
        assert_eq!(applied.opening_types, initial.opening_types);
        assert!(edit.preview(&e, Some(view), Some(id)).is_err());
        e.undo().unwrap();
        assert_eq!(e.document.model(), &initial);
        assert_eq!(e.scene, scene);
        e.redo().unwrap();
        assert_eq!(e.document.model(), &applied);
        assert_eq!(e.scene, applied_scene);
    }

    #[test]
    fn exact_drafts_reject_invalid_stale_selection_view_document_and_provider() {
        let mut e = Editor::new().unwrap();
        let level = *e.document.model().levels.keys().next().unwrap();
        let view = e.create_floor_plan("Plan", level).unwrap();
        let wall = os_model::Wall::new(os_walls::WALL_TYPE, default_wall(level));
        let id = wall.id();
        e.command("Wall", Command::AddWall(wall)).unwrap();
        assert!(OpeningDraft::begin(&e, None, Some(id), Some(OpeningKind::Door), None).is_err());
        let mut draft =
            OpeningDraft::begin(&e, Some(view), Some(id), Some(OpeningKind::Door), None).unwrap();
        assert!((draft.params.offset - 2.05).abs() < 1e-12);
        let before = e.document.model().clone();
        let stats = e.document.history_stats();
        for invalid in ["NaN", "inf", "-1", "6"] {
            draft.values[0] = invalid.into();
            assert!(draft.apply(&mut e, Some(view), Some(id), false).is_err());
            assert_eq!(e.document.model(), &before);
            assert_eq!(e.document.history_stats(), stats);
        }
        draft.values[0] = "0.5".into();
        assert!(draft.apply(&mut e, None, Some(id), false).is_err());
        assert!(draft.apply(&mut e, Some(view), None, false).is_err());
        let opening = draft.apply(&mut e, Some(view), Some(id), false).unwrap();
        assert!(draft.apply(&mut e, Some(view), Some(id), false).is_err());
        let created = e.document.model().clone();
        let mut edit = OpeningDraft::begin(&e, Some(view), Some(opening), None, None).unwrap();
        edit.values[0] = "0.75".into();
        edit.apply(&mut e, Some(view), Some(opening), false)
            .unwrap();
        assert_eq!(
            e.document.model().openings[&opening].header,
            created.openings[&opening].header
        );
        e.undo().unwrap();
        assert_eq!(e.document.model(), &created);
        assert!(
            edit.apply(&mut e, Some(view), Some(opening), false)
                .is_err()
        );
        let stale = OpeningDraft::begin(&e, Some(view), Some(opening), None, None).unwrap();
        e.document = Document::from_model(created).unwrap();
        assert!(!stale.current(&e, Some(view), Some(opening)));
        let mut stale = OpeningDraft::begin(&e, Some(view), Some(opening), None, None).unwrap();
        stale.activation = Some(Id::new());
        assert!(!stale.current(&e, Some(view), Some(opening)));
    }
}
