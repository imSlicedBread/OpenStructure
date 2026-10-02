//! Exact, revision-bound native doors/windows and their host-derived graphics.
use super::*;
use os_core::ensure;
use os_geometry::openings::{component_point, frame_offset, frame_thickness, pane_offset, world};
use os_model::{
    DoorHinge, DoorSwing, Opening, OpeningDefinition, OpeningKind, OpeningParams, OpeningType,
    OpeningTypeParams, ResolvedOpening, WindowPanePosition,
};
use os_render::plan::{PlanContext, PlanLine};

pub(super) mod spacing;

#[cfg(test)]
#[path = "opening_sill_tests.rs"]
mod sill_tests;

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

#[derive(Clone, Copy, PartialEq, Eq)]
enum OpeningAnchor {
    Center,
    Start,
    End,
}

impl OpeningAnchor {
    fn fraction(self) -> f64 {
        match self {
            Self::Center => 0.5,
            Self::Start => 0.0,
            Self::End => 1.0,
        }
    }
}

/// A selected instance and the exact plan snapshot in which Rehost began.
pub(super) struct OpeningRehost {
    context: PlanContext,
    session: Id,
    revision: u64,
    activation: Option<Id>,
    providers: Vec<(String, Id)>,
    drawing: Id,
    pub id: Id,
    original: OpeningParams,
}

impl OpeningRehost {
    pub fn begin(
        editor: &Editor,
        view: Option<Id>,
        selected: Option<Id>,
        drawing: &os_render::plan::PlanDrawing,
    ) -> Result<Self> {
        let view = view.ok_or_else(|| Error::Invalid("Open a floor plan first".into()))?;
        let context = editor.native_plan_context(view)?;
        let id =
            selected.ok_or_else(|| Error::Invalid("Select a door or window to rehost".into()))?;
        let opening = editor
            .document
            .model()
            .openings
            .get(&id)
            .ok_or_else(|| Error::Invalid("Select a door or window to rehost".into()))?;
        ensure(
            context.show_walls
                && drawing
                    .provider_lines(context)?
                    .iter()
                    .any(|line| line.entity == id),
            "Selected opening is hidden in this plan",
        )?;
        ensure(
            editor.document.model().views[&view].parameters.level
                == Some(
                    editor.document.model().walls[&opening.parameters.host]
                        .parameters
                        .level,
                ),
            "Use a floor plan on the host wall level",
        )?;
        Ok(Self {
            context,
            session: editor.document.session_id(),
            revision: editor.document.revision(),
            activation: editor.host.activation_id(os_walls::PLUGIN_ID),
            providers: crate::plan_workspace::plan_provider_signature(editor),
            drawing: drawing.identity(),
            id,
            original: opening.parameters.clone(),
        })
    }

    pub fn current(
        &self,
        editor: &Editor,
        view: Option<Id>,
        selected: Option<Id>,
        drawing: Option<&os_render::plan::PlanDrawing>,
    ) -> bool {
        view == Some(self.context.view_id)
            && selected == Some(self.id)
            && editor.document.session_id() == self.session
            && editor.document.revision() == self.revision
            && editor.native_plan_context(self.context.view_id).ok() == Some(self.context)
            && editor.host.activation_id(os_walls::PLUGIN_ID) == self.activation
            && crate::plan_workspace::plan_provider_signature(editor) == self.providers
            && drawing.is_some_and(|d| d.identity() == self.drawing)
    }

    pub fn host(&self) -> Id {
        self.original.host
    }

    pub fn candidate(
        &self,
        editor: &Editor,
        drawing: &os_render::plan::PlanDrawing,
        camera: os_render::plan::PlanCamera,
        viewport: [f64; 2],
        pointer: Point2,
    ) -> Result<(OpeningParams, os_render::plan::PlanDrawing)> {
        let (host, station) = crate::plan_workspace::opening_host_hit(
            editor.document.model(),
            drawing,
            self.context,
            camera,
            viewport,
            pointer,
        )?
        .ok_or_else(|| Error::Invalid("Choose a visible native wall on this plan level".into()))?;
        ensure(host != self.original.host, "Choose a different host wall")?;
        let mut parameters = self.original.clone();
        parameters.host = host;
        parameters.offset = station
            - editor
                .document
                .model()
                .resolve_opening(&self.original)?
                .width
                * 0.5;
        let mut model = editor.document.model().clone();
        model
            .openings
            .get_mut(&self.id)
            .ok_or_else(|| Error::Invalid("Opening no longer exists".into()))?
            .parameters = parameters.clone();
        model.settle_opening_clearances(editor.document.model())?;
        model.validate()?;
        let preview =
            opening_edit_preview(&model, self.id, &[self.original.host, host], self.context)?;
        Ok((parameters, preview))
    }
}

/// One press owns a same-host move or anchored width resize. Geometry comes from a
/// disposable model; only `commit` is allowed to touch document history.
pub(super) struct OpeningMove {
    context: PlanContext,
    session: Id,
    revision: u64,
    activation: Option<Id>,
    providers: Vec<(String, Id)>,
    drawing: Id,
    pub id: Id,
    original: OpeningParams,
    origin: egui::Pos2,
    anchor: Point2,
    handle: OpeningAnchor,
    offset: f64,
    width: f64,
    original_width: f64,
    moved: bool,
}

impl OpeningMove {
    pub fn current(
        &self,
        editor: &Editor,
        view: Option<Id>,
        selected: Option<Id>,
        drawing: Option<&os_render::plan::PlanDrawing>,
    ) -> bool {
        view == Some(self.context.view_id)
            && selected == Some(self.id)
            && editor.document.session_id() == self.session
            && editor.document.revision() == self.revision
            && drawing.is_some_and(|drawing| drawing.identity() == self.drawing)
            && editor.native_plan_context(self.context.view_id).ok() == Some(self.context)
            && editor.host.activation_id(os_walls::PLUGIN_ID) == self.activation
            && crate::plan_workspace::plan_provider_signature(editor) == self.providers
    }

    pub(super) fn parameters(&self) -> OpeningParams {
        let mut parameters = self.original.clone();
        parameters.offset = if (self.offset - self.original.offset).abs() <= 1e-6 {
            self.original.offset
        } else {
            self.offset
        };
        if self.handle != OpeningAnchor::Center && (self.width - self.original_width).abs() > 1e-6 {
            match &mut parameters.definition {
                OpeningDefinition::Legacy { width, .. } => *width = self.width,
                OpeningDefinition::Typed { .. } => parameters.width_override = Some(self.width),
            }
        }
        parameters
    }

    fn update_pointer(
        &mut self,
        editor: &Editor,
        drawing: &os_render::plan::PlanDrawing,
        query: os_render::snapping::SnapQuery,
        point: Point2,
    ) -> Result<()> {
        let wall = &editor.document.model().walls[&self.original.host].parameters;
        let point = self.context.basis.plane_to_world(point)?;
        let anchor = self.context.basis.plane_to_world(self.anchor)?;
        let station = |p: Point2| wall.path.project(p);
        // Retain the press-to-grip station offset, so an off-center grab never jumps.
        let delta = station(point) - station(anchor);
        self.offset = self.original.offset;
        self.width = self.original_width;
        if !self.moved || delta.abs() <= 1e-6 {
            return Ok(());
        }
        let target = self.original.offset + self.original_width * self.handle.fraction() + delta;
        let projected = self
            .context
            .basis
            .world_to_plane(world(wall, target, 0.0))?;
        let snapped = host_axis_snap(editor, self, drawing, query, projected)?;
        let target = station(self.context.basis.plane_to_world(snapped)?);
        if self.handle == OpeningAnchor::Center {
            self.offset = target - self.original_width * 0.5;
        } else if self.handle == OpeningAnchor::Start {
            self.offset = target;
            self.width = self.original.offset + self.original_width - target;
        } else {
            self.width = target - self.original.offset;
        }
        // A jamb grip authors width. The persistent endpoint owns the resulting
        // offset; a center grip remains an explicit move and must conflict.
        if self.handle != OpeningAnchor::Center
            && editor
                .document
                .model()
                .opening_clearances
                .contains_key(&self.id)
        {
            self.offset = self.original.offset;
        }
        Ok(())
    }

    fn candidate(&self, editor: &Editor) -> Result<Model> {
        let mut model = editor.document.model().clone();
        let opening = model
            .openings
            .get_mut(&self.id)
            .ok_or_else(|| Error::Invalid("Opening no longer exists".into()))?;
        opening.parameters = self.parameters();
        model.settle_opening_clearances(editor.document.model())?;
        model.validate()?;
        Ok(model)
    }

    pub fn preview(&self, editor: &Editor) -> Result<os_render::plan::PlanDrawing> {
        let model = self.candidate(editor)?;
        opening_edit_preview(&model, self.id, &[self.original.host], self.context)
    }

    pub fn host(&self) -> Id {
        self.original.host
    }

    pub fn commit(
        self,
        editor: &mut Editor,
        view: Option<Id>,
        selected: Option<Id>,
        drawing: Option<&os_render::plan::PlanDrawing>,
    ) -> Result<()> {
        ensure(
            self.current(editor, view, selected, drawing),
            "Opening move is stale",
        )?;
        // Revalidate the final pointer candidate, never a cached last-valid one.
        self.candidate(editor)?;
        let parameters = self.parameters();
        if self.moved && parameters != self.original {
            editor.command(
                if self.handle == OpeningAnchor::Center {
                    "Move opening"
                } else {
                    "Resize opening"
                },
                Command::UpdateOpening {
                    id: self.id,
                    parameters,
                },
            )?;
        }
        Ok(())
    }
}

/// Derive replacement host graphics and the selected symbol from a disposable model.
pub(super) fn opening_edit_preview(
    model: &Model,
    id: Id,
    hosts: &[Id],
    context: PlanContext,
) -> Result<os_render::plan::PlanDrawing> {
    opening_batch_preview(model, &[id], hosts, context)
}

pub(super) fn opening_batch_preview(
    model: &Model,
    ids: &[Id],
    hosts: &[Id],
    context: PlanContext,
) -> Result<os_render::plan::PlanDrawing> {
    let mut host_cells = std::collections::BTreeMap::new();
    let mut host_seams = std::collections::BTreeMap::new();
    for &host in hosts {
        let wall = os_geometry::walls::NativeWall::from_model(model, host)?;
        let mut cells = Vec::new();
        for (layer, footprints) in
            wall.layer_plan_footprints(context.range, context.basis, context.crop)?
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
                    context.basis.world_to_plane(a)?,
                    context.basis.world_to_plane(b)?,
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        host_cells.insert(host, cells);
        host_seams.insert(host, seams);
    }
    let mut lines = std::collections::BTreeMap::new();
    for &id in ids {
        let resolved = model.resolve_opening(&model.openings[&id].parameters)?;
        let wall = os_geometry::walls::NativeWall::from_model(model, resolved.host)?;
        lines.insert(
            id,
            plan_symbol(id, &resolved, &wall.parameters, wall.elevation, context)?,
        );
    }
    os_render::plan::PlanDrawing::from_layered_footprints(context, &host_cells, ids.to_vec())?
        .without_wall_seams(&host_seams)?
        .with_native_lines(lines)
}

#[allow(clippy::too_many_arguments)]
fn opening_grip(
    editor: &Editor,
    id: Id,
    dimensions: Option<(f64, f64)>,
    handle: OpeningAnchor,
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
            dimensions.map_or(p.offset, |p| p.0)
                + dimensions.map_or(p.width, |p| p.1) * handle.fraction(),
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

fn opening_jambs(
    editor: &Editor,
    id: Id,
    dimensions: Option<(f64, f64)>,
    context: PlanContext,
    camera: os_render::plan::PlanCamera,
    rect: egui::Rect,
) -> Option<[(OpeningAnchor, egui::Pos2); 2]> {
    let center = opening_grip(
        editor,
        id,
        dimensions,
        OpeningAnchor::Center,
        context,
        camera,
        rect,
    )?;
    let start = opening_grip(
        editor,
        id,
        dimensions,
        OpeningAnchor::Start,
        context,
        camera,
        rect,
    )?;
    let end = opening_grip(
        editor,
        id,
        dimensions,
        OpeningAnchor::End,
        context,
        camera,
        rect,
    )?;
    // Suppress both jambs when acquisition discs touch. Keep the center grip usable.
    (start.distance(end) > 20.0 && start.distance(center) > 20.0 && end.distance(center) > 20.0)
        .then_some([(OpeningAnchor::Start, start), (OpeningAnchor::End, end)])
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum DoorFlip {
    Hinge,
    Swing,
    Pane,
    Lite,
}

impl DoorFlip {
    pub(super) fn available(self, resolved: &ResolvedOpening) -> bool {
        match self {
            Self::Lite => resolved.type_id.is_some() && resolved.family.side_lite.is_some(),
            Self::Pane => {
                resolved.kind == OpeningKind::Window
                    && resolved.type_id.is_some()
                    && resolved.pane_position != WindowPanePosition::Center
            }
            Self::Hinge | Self::Swing => resolved.kind == OpeningKind::Door,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Hinge => "Flip door hinge",
            Self::Swing => "Flip door swing",
            Self::Pane => "Flip window side",
            Self::Lite => "Flip lite",
        }
    }

    pub(super) fn parameters(
        self,
        original: &OpeningParams,
        resolved: &ResolvedOpening,
    ) -> Result<OpeningParams> {
        let mut parameters = original.clone();
        ensure(
            self.available(resolved),
            "Select an opening with an available flip action",
        )?;
        match self {
            Self::Lite => {
                parameters.lite_side_override =
                    Some(match resolved.family.side_lite.as_ref().unwrap().side {
                        os_model::LiteSide::Start => os_model::LiteSide::End,
                        os_model::LiteSide::End => os_model::LiteSide::Start,
                    });
            }
            Self::Pane => {
                parameters.pane_position_override = Some(match resolved.pane_position {
                    WindowPanePosition::LeftFace => WindowPanePosition::RightFace,
                    WindowPanePosition::RightFace => WindowPanePosition::LeftFace,
                    WindowPanePosition::Center => unreachable!("center has no flip action"),
                });
            }
            Self::Hinge => {
                parameters.hinge = match parameters.hinge {
                    DoorHinge::Start => DoorHinge::End,
                    DoorHinge::End => DoorHinge::Start,
                };
            }
            Self::Swing => {
                parameters.swing = match parameters.swing {
                    DoorSwing::Left => DoorSwing::Right,
                    DoorSwing::Right => DoorSwing::Left,
                };
            }
        }
        Ok(parameters)
    }
}

/// Reuse the revision/selection/provider snapshot used by Rehost. A flip owns
/// its entire press, even if the context changes or the click becomes a drag.
pub(super) struct OpeningFlip {
    snapshot: OpeningRehost,
    control: DoorFlip,
    rect: egui::Rect,
    camera: os_render::plan::PlanCamera,
    canvas: egui::Rect,
    origin: egui::Pos2,
}

impl OpeningFlip {
    pub fn current(
        &self,
        editor: &Editor,
        view: Option<Id>,
        selected: Option<Id>,
        drawing: Option<&os_render::plan::PlanDrawing>,
    ) -> bool {
        self.snapshot.current(editor, view, selected, drawing)
    }

    pub fn commit(
        self,
        editor: &mut Editor,
        view: Option<Id>,
        selected: Option<Id>,
        drawing: Option<&os_render::plan::PlanDrawing>,
    ) -> Result<()> {
        ensure(
            self.current(editor, view, selected, drawing),
            "Opening flip is stale",
        )?;
        let resolved = editor
            .document
            .model()
            .resolve_opening(&self.snapshot.original)?;
        let parameters = self
            .control
            .parameters(&self.snapshot.original, &resolved)?;
        if matches!(
            editor
                .document
                .model()
                .resolve_wall(resolved.host)?
                .parameters
                .path,
            os_model::WallPath::CircularArc { .. }
        ) {
            // Arc flips can change the tessellated component's radius or bay
            // placement. Check its geometry budget before command commits history.
            let mut candidate = editor.document.model().clone();
            candidate
                .openings
                .get_mut(&self.snapshot.id)
                .unwrap()
                .parameters = parameters.clone();
            candidate.validate()?;
            panel_mesh(&candidate, self.snapshot.id)?;
        }
        editor.command(
            self.control.label(),
            Command::UpdateOpening {
                id: self.snapshot.id,
                parameters,
            },
        )
    }
}

fn flip_rects(
    editor: &Editor,
    id: Id,
    drawing: &os_render::plan::PlanDrawing,
    context: PlanContext,
    camera: os_render::plan::PlanCamera,
    canvas: egui::Rect,
) -> Option<Vec<(DoorFlip, egui::Rect)>> {
    let model = editor.document.model();
    let resolved = model
        .resolve_opening(&model.openings.get(&id)?.parameters)
        .ok()?;
    if !drawing
        .provider_lines(context)
        .ok()?
        .iter()
        .any(|l| l.entity == id)
    {
        return None;
    }
    // Flip buttons sit away from the axis, so overlapping resize grips alone
    // must not hide them. Project each visible, uncropped jamb independently.
    let start = opening_grip(
        editor,
        id,
        None,
        OpeningAnchor::Start,
        context,
        camera,
        canvas,
    )?;
    let end = opening_grip(
        editor,
        id,
        None,
        OpeningAnchor::End,
        context,
        camera,
        canvas,
    )?;
    if matches!(
        model.resolve_wall(resolved.host).ok()?.parameters.path,
        os_model::WallPath::CircularArc { .. }
    ) {
        let center = opening_grip(
            editor,
            id,
            None,
            OpeningAnchor::Center,
            context,
            camera,
            canvas,
        )?;
        return arc_flip_rects(&resolved, [start, center, end], canvas);
    }
    let center = start.lerp(end, 0.5);
    let row = egui::pos2(center.x, start.y.min(end.y) - 30.0);
    let mut controls = if resolved.kind == OpeningKind::Window {
        if DoorFlip::Pane.available(&resolved) {
            vec![(
                DoorFlip::Pane,
                egui::Rect::from_center_size(row, egui::vec2(58.0, 24.0)),
            )]
        } else {
            vec![]
        }
    } else {
        vec![
            (
                DoorFlip::Hinge,
                egui::Rect::from_center_size(row + egui::vec2(-32.0, 0.0), egui::vec2(58.0, 24.0)),
            ),
            (
                DoorFlip::Swing,
                egui::Rect::from_center_size(row + egui::vec2(32.0, 0.0), egui::vec2(58.0, 24.0)),
            ),
        ]
    };
    if DoorFlip::Lite.available(&resolved) {
        let position = if controls.is_empty() {
            row
        } else {
            row + egui::vec2(0.0, -28.0)
        };
        controls.push((
            DoorFlip::Lite,
            egui::Rect::from_center_size(position, egui::vec2(68.0, 24.0)),
        ));
    }
    (controls
        .windows(2)
        .all(|pair| !pair[0].1.intersects(pair[1].1))
        && controls.iter().all(|(_, rect)| {
            canvas.contains_rect(rect.expand(4.0))
                && [start, center, end].iter().all(|p| {
                    !rect
                        .expand(4.0)
                        .intersects(egui::Rect::from_center_size(*p, egui::vec2(20.0, 20.0)))
                })
        }))
    .then_some(controls)
}

fn arc_flip_rects(
    resolved: &ResolvedOpening,
    anchors: [egui::Pos2; 3],
    canvas: egui::Rect,
) -> Option<Vec<(DoorFlip, egui::Rect)>> {
    let [start, center, end] = anchors;
    // Use the analytic center grip and clear every anchor, including the arc's
    // bulge. Rows and padding are logical pixels, independent of display scale.
    for (y, outward) in [
        (start.y.min(center.y).min(end.y) - 30.0, -1.0),
        (start.y.max(center.y).max(end.y) + 30.0, 1.0),
    ] {
        let row = egui::pos2(center.x, y);
        let mut controls = if resolved.kind == OpeningKind::Window {
            if DoorFlip::Pane.available(resolved) {
                vec![(
                    DoorFlip::Pane,
                    egui::Rect::from_center_size(row, egui::vec2(58.0, 24.0)),
                )]
            } else {
                vec![]
            }
        } else {
            vec![
                (
                    DoorFlip::Hinge,
                    egui::Rect::from_center_size(
                        row + egui::vec2(-32.0, 0.0),
                        egui::vec2(58.0, 24.0),
                    ),
                ),
                (
                    DoorFlip::Swing,
                    egui::Rect::from_center_size(
                        row + egui::vec2(32.0, 0.0),
                        egui::vec2(58.0, 24.0),
                    ),
                ),
            ]
        };
        if DoorFlip::Lite.available(resolved) {
            let position = if controls.is_empty() {
                row
            } else {
                row + egui::vec2(0.0, outward * 28.0)
            };
            controls.push((
                DoorFlip::Lite,
                egui::Rect::from_center_size(position, egui::vec2(68.0, 24.0)),
            ));
        }
        if controls.iter().enumerate().all(|(i, (_, rect))| {
            controls[i + 1..]
                .iter()
                .all(|(_, other)| !rect.intersects(*other))
                && canvas.contains_rect(rect.expand(4.0))
                && anchors.iter().all(|p| {
                    !rect
                        .expand(4.0)
                        .intersects(egui::Rect::from_center_size(*p, egui::vec2(20.0, 20.0)))
                })
        }) {
            return Some(controls);
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
pub(super) fn flip_input(
    editor: &Editor,
    draft: &mut Option<OpeningFlip>,
    claimed: &mut bool,
    drawing: &os_render::plan::PlanDrawing,
    context: PlanContext,
    camera: os_render::plan::PlanCamera,
    canvas: egui::Rect,
    ui: &mut egui::Ui,
    selected: Option<Id>,
    allow: bool,
) -> (Vec<(DoorFlip, egui::Response)>, bool) {
    let controls = if allow {
        selected.and_then(|id| flip_rects(editor, id, drawing, context, camera, canvas))
    } else {
        None
    };
    let max_click_dist = ui.ctx().options(|o| o.input_options.max_click_dist);
    if draft.as_ref().is_some_and(|d| {
        !d.current(editor, Some(context.view_id), selected, Some(drawing))
            || d.camera != camera
            || d.canvas != canvas
            || !controls
                .as_ref()
                .is_some_and(|controls| controls.contains(&(d.control, d.rect)))
    }) || ui.input(|i| {
        i.key_pressed(egui::Key::Escape)
            || i.events
                .iter()
                .any(|e| matches!(e, egui::Event::PointerGone))
            || draft.as_ref().is_some_and(|d| {
                i.pointer
                    .interact_pos()
                    .is_some_and(|p| p.distance(d.origin) > max_click_dist)
            })
    }) {
        *draft = None;
    }
    let mut responses = Vec::new();
    let mut release = false;
    for (control, rect) in controls.into_iter().flatten() {
        let response = ui.interact(
            rect,
            ui.id().with(("door_flip", selected, control)),
            egui::Sense::click(),
        );
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, control.label())
        });
        if !*claimed
            && (response.is_pointer_button_down_on() || response.clicked())
            && ui.input(|i| i.pointer.primary_pressed())
            // egui clears press_origin on release, including a same-frame click.
            && let Some(origin) = ui.input(|i| {
                i.events.iter().find_map(|event| match event {
                    egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, .. } => Some(*pos),
                    _ => None,
                })
            })
            && let Ok(snapshot) =
                OpeningRehost::begin(editor, Some(context.view_id), selected, drawing)
        {
            *draft = Some(OpeningFlip {
                snapshot,
                control,
                rect,
                camera,
                canvas,
                origin,
            });
            *claimed = true;
        }
        release |= response.clicked() && draft.as_ref().is_some_and(|d| d.control == control);
        responses.push((control, response));
    }
    (responses, release)
}

pub(super) fn paint_flip_controls(
    ui: &egui::Ui,
    painter: &egui::Painter,
    controls: Vec<(DoorFlip, egui::Response)>,
) {
    for (control, response) in controls {
        let visuals = ui.style().interact(&response);
        painter.rect(
            response.rect,
            3.0,
            visuals.bg_fill,
            visuals.bg_stroke,
            egui::StrokeKind::Inside,
        );
        painter.text(
            response.rect.center(),
            egui::Align2::CENTER_CENTER,
            match control {
                DoorFlip::Hinge => "Hinge",
                DoorFlip::Swing => "Swing",
                DoorFlip::Pane => "Side",
                DoorFlip::Lite => "Flip lite",
            },
            egui::FontId::proportional(12.0),
            visuals.text_color(),
        );
        response.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text(match control {
            DoorFlip::Hinge => "Flip door hinge: wall start ↔ wall end. Uses the host's stored start → end direction.",
            DoorFlip::Swing => "Flip door swing: left ↔ right of wall. Uses the host's stored start → end direction.",
            DoorFlip::Pane => "Flip window side: left ↔ right of wall. Pins this instance's pane position.",
            DoorFlip::Lite => "Flip fixed lite: start ↔ end along the host wall. Pins this instance's lite side.",
        });
    }
}

/// Restrict the existing snap solver to the host axis and other visible jambs.
/// Never use the selected opening's aperture edges as references for its own move.
fn host_axis_snap(
    editor: &Editor,
    draft: &OpeningMove,
    drawing: &os_render::plan::PlanDrawing,
    query: os_render::snapping::SnapQuery,
    point: Point2,
) -> Result<Point2> {
    opening_axis_snap(
        editor.document.model(),
        draft.original.host,
        Some(draft.id),
        draft.context,
        drawing,
        query,
        point,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn opening_axis_snap(
    model: &Model,
    host: Id,
    exclude: Option<Id>,
    context: PlanContext,
    drawing: &os_render::plan::PlanDrawing,
    mut query: os_render::snapping::SnapQuery,
    point: Point2,
) -> Result<Point2> {
    use os_render::snapping::{SnapArc, SnapScene, SnapSegment};
    let wall = &model.walls[&host].parameters;
    let mut segments = Vec::new();
    let mut arcs = Vec::new();
    match wall.path {
        os_model::WallPath::Straight { start, end } => segments.push(SnapSegment {
            entity: host,
            feature: 0,
            start: context.basis.world_to_plane(start)?,
            end: context.basis.world_to_plane(end)?,
        }),
        os_model::WallPath::CircularArc {
            center,
            radius,
            start_angle_rad,
            signed_sweep_rad,
        } => arcs.push(SnapArc {
            entity: host,
            center: context.basis.world_to_plane(center)?,
            radius,
            start_angle_rad: start_angle_rad - context.basis.rotation,
            signed_sweep_rad,
        }),
    }
    segments.extend(drawing.provider_lines(context)?.iter().filter_map(|line| {
        let opening = model.openings.get(&line.entity)?;
        (Some(line.entity) != exclude && opening.parameters.host == host && line.feature < 2)
            .then_some(SnapSegment {
                entity: line.entity,
                feature: line.feature,
                start: line.start,
                end: line.end,
            })
    }));
    query.pointer = query.camera.project(point, query.viewport)?;
    query.exclude_entity = exclude;
    Ok(SnapScene::new(context, segments)?
        .with_arcs(arcs)?
        .query(context, query)?
        .candidate(context, query)?
        .map_or(point, |candidate| candidate.point))
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
    snap_query: os_render::snapping::SnapQuery,
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
    if draft.as_ref().is_some_and(|d| {
        !d.current(editor, Some(context.view_id), selected, Some(drawing)) || !allow
    }) || ctx.input(|i| {
        i.events
            .iter()
            .any(|e| matches!(e, egui::Event::PointerGone))
    }) {
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
        && let Some(pos) = ctx.input(|i| i.pointer.press_origin())
        && let Some(handle) = opening_jambs(editor, id, None, context, camera, rect)
            .into_iter()
            .flatten()
            .find_map(|(handle, grip)| (pos.distance(grip) <= 10.0).then_some(handle))
            .or_else(|| {
                opening_grip(
                    editor,
                    id,
                    None,
                    OpeningAnchor::Center,
                    context,
                    camera,
                    rect,
                )
                .filter(|grip| pos.distance(*grip) <= 10.0)
                .map(|_| OpeningAnchor::Center)
            })
        && let Ok(anchor) = plane(pos)
    {
        let original = editor.document.model().openings[&id].parameters.clone();
        let Ok(resolved) = editor.document.model().resolve_opening(&original) else {
            return false;
        };
        *draft = Some(OpeningMove {
            context,
            session: editor.document.session_id(),
            revision: editor.document.revision(),
            activation: editor.host.activation_id(os_walls::PLUGIN_ID),
            providers: crate::plan_workspace::plan_provider_signature(editor),
            drawing: drawing.identity(),
            id,
            offset: original.offset,
            width: resolved.width,
            original_width: resolved.width,
            original,
            origin: pos,
            anchor,
            handle,
            moved: false,
        });
        *claimed = true;
    }
    if let Some(d) = draft {
        if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
            d.moved |= pos.distance(d.origin) > ctx.options(|o| o.input_options.max_click_dist);
            if plane(pos)
                .and_then(|point| d.update_pointer(editor, drawing, snap_query, point))
                .is_err()
            {
                d.offset = f64::NAN;
                d.width = f64::NAN;
            }
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
            draft.map(|d| (d.offset, d.width)),
            OpeningAnchor::Center,
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
    if let Some(id) = selected
        && drawing
            .provider_lines(context)
            .is_ok_and(|lines| lines.iter().any(|l| l.entity == id))
        && let Some(jambs) = opening_jambs(
            editor,
            id,
            draft.map(|d| (d.offset, d.width)),
            context,
            camera,
            response.rect,
        )
    {
        let axis = jambs[1].1 - jambs[0].1;
        let cursor = if axis.x.abs() > 2.0 * axis.y.abs() {
            egui::CursorIcon::ResizeHorizontal
        } else if axis.y.abs() > 2.0 * axis.x.abs() {
            egui::CursorIcon::ResizeVertical
        } else if axis.x * axis.y > 0.0 {
            egui::CursorIcon::ResizeNwSe
        } else {
            egui::CursorIcon::ResizeNeSw
        };
        for (handle, pos) in jambs {
            let hovered = response
                .hover_pos()
                .is_some_and(|p| p.distance(pos) <= 10.0);
            let active = draft.is_some_and(|d| d.handle == handle);
            let color = if error.is_some() {
                theme::ERROR
            } else {
                theme::ACCENT
            };
            painter.add(egui::Shape::convex_polygon(
                vec![
                    pos + egui::vec2(0.0, -6.0),
                    pos + egui::vec2(6.0, 0.0),
                    pos + egui::vec2(0.0, 6.0),
                    pos + egui::vec2(-6.0, 0.0),
                ],
                if active || hovered {
                    color
                } else {
                    egui::Color32::TRANSPARENT
                },
                egui::Stroke::new(2.0, color),
            ));
            if active || hovered {
                response.ctx.set_cursor_icon(cursor);
                painter.text(
                    pos + egui::vec2(0.0, -14.0),
                    egui::Align2::CENTER_BOTTOM,
                    "Resize opening · opposite jamb fixed",
                    egui::FontId::proportional(12.0),
                    theme::TEXT,
                );
            }
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
    // Draft symbols must still paint at an invalid offset/clearance so the UI
    // can show the candidate in error color. Model commits validate the host.
    context.range.validate()?;
    context.basis.validate()?;
    let base = elevation + p.sill;
    let top = base + p.height;
    let tolerance = os_geometry::plan::PLAN_TOLERANCE;
    let role = if top <= context.range.depth + tolerance || base >= context.range.top - tolerance {
        None
    } else if base <= context.range.cut + tolerance && top > context.range.cut + tolerance {
        Some(os_geometry::plan::PlanRole::Cut)
    } else if top <= context.range.cut + tolerance && top > context.range.bottom + tolerance {
        Some(os_geometry::plan::PlanRole::Projected)
    } else if top <= context.range.bottom + tolerance && top > context.range.depth + tolerance {
        Some(os_geometry::plan::PlanRole::Depth)
    } else {
        None
    };
    let Some(role) = role else {
        return Ok(vec![]);
    };
    let half = wall.thickness / 2.0;
    let bays = p.family.bays(p.width)?;
    let primary = os_geometry::openings::primary_bay(p)?;
    let posed_sash = p.supports_open_state(wall)
        && matches!(p.open_state,
        os_model::OpeningState::SlidingFraction(v) | os_model::OpeningState::CasementAngle(v) if v != 0.);
    let spans =
        os_geometry::openings::plan_spans(p, elevation, context.range.cut, context.range.depth)?;
    let mut pairs = Vec::new();
    let mut rigid_pairs = std::collections::BTreeMap::<usize, (Point2, Point2)>::new();
    for (a, b) in
        os_geometry::openings::cut_plan_spans(p, elevation, context.range.cut, context.range.depth)?
    {
        for u in [a, b] {
            let x = p.offset + u * p.width;
            pairs.push((Point2::new(x, -half), Point2::new(x, half)));
        }
    }
    match p.kind {
        OpeningKind::Door
            if matches!(p.family.door_leaves, os_model::DoorLeaves::Paired { .. }) =>
        {
            if !spans.is_empty() {
                for leaf in os_geometry::openings::door_pair(p)?.unwrap() {
                    pairs.push((
                        component_point(&leaf, wall, 0.),
                        component_point(&leaf, wall, 1.),
                    ));
                    for i in 0..SWING_SEGMENTS {
                        let point = |step: usize| {
                            let mut pose = leaf.clone();
                            pose.open_state = os_model::OpeningState::DoorAngle(
                                leaf.door_angle(wall) * step as f64 / SWING_SEGMENTS as f64,
                            );
                            component_point(&pose, wall, 1.)
                        };
                        pairs.push((point(i), point(i + 1)));
                    }
                }
            }
        }
        OpeningKind::Window => {
            let spans = if spans.is_empty() {
                spans
            } else {
                bays.map_or_else(
                    || spans.clone(),
                    |b| {
                        vec![
                            (b.primary.0 / p.width, b.primary.1 / p.width),
                            (b.lite.0 / p.width, b.lite.1 / p.width),
                        ]
                    },
                )
            };
            for (row, y) in [-half * 0.5, pane_offset(p, wall), half * 0.5]
                .into_iter()
                .enumerate()
            {
                for (span, &(a, b)) in spans.iter().enumerate() {
                    let mut ends = (
                        Point2::new(p.offset + a * p.width, y),
                        Point2::new(p.offset + b * p.width, y),
                    );
                    let is_primary = bays.is_none() || span == 0;
                    if posed_sash && is_primary {
                        let bay = os_geometry::openings::sash_bay(p)?;
                        let d = os_geometry::openings::depth(p, wall);
                        let center = pane_offset(p, wall);
                        if p.window_operation == os_model::WindowOperation::Sliding && row < 2 {
                            let rail = 0.05_f64.min(bay.width * 0.1).min(bay.height * 0.1);
                            let (start, end, track_y) = if row == 0 {
                                (0., (bay.width + rail) / 2., center - d * 0.275)
                            } else {
                                ((bay.width - rail) / 2., bay.width, center + d * 0.275)
                            };
                            ends = (
                                os_geometry::openings::sash_point(
                                    p,
                                    &bay,
                                    wall,
                                    row,
                                    bay.offset + start,
                                    track_y,
                                ),
                                os_geometry::openings::sash_point(
                                    p,
                                    &bay,
                                    wall,
                                    row,
                                    bay.offset + end,
                                    track_y,
                                ),
                            );
                        } else if p.window_operation == os_model::WindowOperation::Casement {
                            let y = center + (row as f64 - 1.) * d / 2.;
                            ends = (
                                os_geometry::openings::sash_point(p, &bay, wall, 0, bay.offset, y),
                                os_geometry::openings::sash_point(
                                    p,
                                    &bay,
                                    wall,
                                    0,
                                    bay.offset + bay.width,
                                    y,
                                ),
                            );
                        }
                    }
                    pairs.push(ends);
                }
            }
        }
        OpeningKind::Door => {
            let p = &primary;
            let spans = if bays.is_some() {
                os_geometry::openings::plan_spans(
                    p,
                    elevation,
                    context.range.cut,
                    context.range.depth,
                )?
            } else {
                spans
            };
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
            let is_arc = matches!(wall.path, os_model::WallPath::CircularArc { .. });
            let station = p.offset
                + if p.hinge == DoorHinge::End {
                    p.width
                } else {
                    0.0
                };
            let hinge = component_point(p, wall, 0.);
            let tangent = wall.path.tangent(station);
            let normal = Point2::new(-tangent.y, tangent.x);
            let center = wall.path.point(station);
            let hinge_world = Point2::new(
                center.x - side * half * normal.x,
                center.y - side * half * normal.y,
            );
            let leaf_point = |u: f64| {
                Point2::new(
                    hinge_world.x + side * u * p.width * normal.x,
                    hinge_world.y + side * u * p.width * normal.y,
                )
            };
            for &(a, b) in &spans {
                if is_arc {
                    let index = pairs.len();
                    pairs.push((Point2::default(), Point2::default()));
                    rigid_pairs.insert(index, (leaf_point(a), leaf_point(b)));
                } else {
                    pairs.push((component_point(p, wall, a), component_point(p, wall, b)));
                }
            }
            let radius = spans.last().map_or(0., |s| s.1 * p.width);
            let tip = if is_arc {
                leaf_point(radius / p.width)
            } else {
                component_point(p, wall, radius / p.width)
            };
            let point = |i: usize| {
                let angle = p.door_angle(wall).to_radians() * i as f64 / SWING_SEGMENTS as f64;
                if i == SWING_SEGMENTS {
                    tip
                } else if is_arc {
                    Point2::new(
                        hinge_world.x
                            + closed * radius * angle.cos() * tangent.x
                            + side * radius * angle.sin() * normal.x,
                        hinge_world.y
                            + closed * radius * angle.cos() * tangent.y
                            + side * radius * angle.sin() * normal.y,
                    )
                } else {
                    Point2::new(
                        hinge.x + closed * radius * angle.cos(),
                        hinge.y + side * radius * angle.sin(),
                    )
                }
            };
            for i in 0..if radius > 0. { SWING_SEGMENTS } else { 0 } {
                if is_arc {
                    let index = pairs.len();
                    pairs.push((Point2::default(), Point2::default()));
                    rigid_pairs.insert(index, (point(i), point(i + 1)));
                } else {
                    pairs.push((point(i), point(i + 1)));
                }
            }
        }
    }
    if let Some(bays) = bays {
        let bottom = elevation
            + p.sill
            + if p.kind == OpeningKind::Window {
                p.family.frame_width
            } else {
                0.
            };
        let top = elevation + p.sill + p.height - p.family.frame_width;
        if context.range.cut >= bottom && context.range.depth <= top {
            if p.kind == OpeningKind::Door {
                pairs.push((
                    Point2::new(p.offset + bays.lite.0, pane_offset(p, wall)),
                    Point2::new(p.offset + bays.lite.1, pane_offset(p, wall)),
                ));
            }
            let y = frame_offset(p, wall);
            let d = frame_thickness(p, wall) / 2.;
            let (a, b) = (p.offset + bays.mullion.0, p.offset + bays.mullion.1);
            pairs.extend([
                (Point2::new(a, y - d), Point2::new(b, y - d)),
                (Point2::new(b, y - d), Point2::new(b, y + d)),
                (Point2::new(b, y + d), Point2::new(a, y + d)),
                (Point2::new(a, y + d), Point2::new(a, y - d)),
            ]);
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
    // Append marks so existing frame, lite and pane feature identities stay stable.
    // These are plan symbols only; the primary bay's visible spans bound every mark.
    if p.kind == OpeningKind::Window {
        use os_model::WindowOperation;
        for (a, b) in os_geometry::openings::plan_spans(
            &primary,
            elevation,
            context.range.cut,
            context.range.depth,
        )? {
            let x0 = primary.offset + a * primary.width;
            let x1 = primary.offset + b * primary.width;
            let at = |t: f64, y: f64| Point2::new(x0 + t * (x1 - x0), y * half);
            let marks_start = pairs.len();
            match p.window_operation {
                WindowOperation::Fixed => {}
                WindowOperation::Sliding => pairs.extend([
                    (at(0., -0.35), at(0.6, -0.35)),
                    (at(0.6, -0.35), at(0.6, 0.)),
                    (at(0.4, 0.35), at(1., 0.35)),
                    (at(0.4, 0.), at(0.4, 0.35)),
                ]),
                // Start-side convention in the stored host frame: -normal at
                // the primary start jamb, +normal at its end jamb.
                WindowOperation::Casement => pairs.push((at(0., -0.5), at(1., 0.5))),
            }
            if posed_sash {
                let bay = os_geometry::openings::sash_bay(p)?;
                for (i, (a, b)) in pairs[marks_start..].iter_mut().enumerate() {
                    let track = usize::from(i >= 2);
                    let pose = |v: Point2| {
                        let x = bay.offset + (v.x - x0) / (x1 - x0) * bay.width;
                        let y = pane_offset(p, wall)
                            + v.y / half * os_geometry::openings::depth(p, wall);
                        os_geometry::openings::sash_point(p, &bay, wall, track, x, y)
                    };
                    *a = pose(*a);
                    *b = pose(*b);
                }
            }
        }
    }
    let mut lines = Vec::new();
    let mut feature = 0u32;
    for (index, (a, b)) in pairs.into_iter().enumerate() {
        if let Some((start, end)) = rigid_pairs.get(&index).copied() {
            let start = context.basis.world_to_plane(start)?;
            let end = context.basis.world_to_plane(end)?;
            let line_feature = feature;
            feature = feature
                .checked_add(1)
                .ok_or_else(|| Error::Invalid("opening symbol feature limit exceeded".into()))?;
            if start.distance(end) <= 1e-10 {
                continue;
            }
            if let Some((start, end)) =
                crate::plan_workspace::clip_plan_segment(start, end, context.crop)
            {
                lines.push(PlanLine {
                    entity: id,
                    feature: line_feature,
                    start,
                    end,
                    role,
                });
            }
            continue;
        }
        let steps = match wall.path {
            os_model::WallPath::Straight { .. } => 1,
            path @ os_model::WallPath::CircularArc { .. } => {
                let max_offset = a.y.abs().max(b.y.abs());
                let total = path.display_segments(max_offset)?;
                let fraction = (b.x - a.x).abs() / path.length();
                ((total as f64 * fraction).ceil() as usize).clamp(1, total)
            }
        };
        for step in 0..steps {
            let t0 = step as f64 / steps as f64;
            let t1 = (step + 1) as f64 / steps as f64;
            let local = |t: f64| Point2::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
            let start = context
                .basis
                .world_to_plane(world(wall, local(t0).x, local(t0).y))?;
            let end = context
                .basis
                .world_to_plane(world(wall, local(t1).x, local(t1).y))?;
            let line_feature = feature;
            feature = feature
                .checked_add(1)
                .ok_or_else(|| Error::Invalid("opening symbol feature limit exceeded".into()))?;
            if let Some((start, end)) =
                crate::plan_workspace::clip_plan_segment(start, end, context.crop)
            {
                lines.push(PlanLine {
                    entity: id,
                    feature: line_feature,
                    start,
                    end,
                    role,
                });
            }
        }
    }
    Ok(lines)
}

const LABELS: [&str; 4] = [
    "Offset from wall start (m)",
    "Width (m)",
    "Height (m)",
    "Sill above floor (m)",
];
pub(super) struct OpeningDraft {
    state_supported: bool,
    state_operation: os_model::WindowOperation,
    state_override: bool,
    state_value: String,
    paired: bool,
    inactive_state_value: String,
    context: PlanContext,
    providers: Vec<(String, Id)>,
    clearance_end: Option<os_model::ClearanceEnd>,
    clearance_value: String,
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
    dimension_overrides: [bool; 2],
    override_sill: bool,
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
            .ok_or_else(|| Error::Invalid("Select a native wall".into()))?;
        ensure(
            model.views[&view].parameters.level == Some(wall.parameters.level),
            "Use a floor plan on the host wall level",
        )?;
        let drawing = editor.native_wall_plan(view)?;
        ensure(
            context.show_walls
                && drawing
                    .items(context)?
                    .iter()
                    .any(|item| item.entity == host),
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
                type_id.and_then(|id| model.resolve_opening_type(id).ok().map(|t| t.parameters));
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
                open_state: Default::default(),
                width_override: None,
                height_override: None,
                sill_override: None,
                pane_position_override: None,
                lite_side_override: None,
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
        ensure(
            model.views[&view]
                .parameters
                .plan
                .unwrap()
                .visibility
                .shows_opening(resolved.kind),
            "This opening category is hidden in the plan",
        )?;
        if editing {
            ensure(
                editor.native_drawing(view)?.is_native_line(id),
                "Opening is hidden in this plan",
            )?;
        }
        let values = [
            params.offset,
            resolved.width,
            resolved.height,
            resolved.sill,
        ]
        .map(|n| n.to_string());
        Ok(Self {
            state_supported: resolved.supports_open_state(&wall.parameters),
            paired: matches!(
                resolved.family.door_leaves,
                os_model::DoorLeaves::Paired { .. }
            ),
            inactive_state_value: match params.open_state {
                os_model::OpeningState::DoorPairAngles {
                    inactive_degrees, ..
                } => inactive_degrees,
                os_model::OpeningState::DoorAngle(v) => v,
                _ => 90.,
            }
            .to_string(),
            state_operation: resolved.window_operation,
            state_override: params.open_state != os_model::OpeningState::Default,
            state_value: match params.open_state {
                os_model::OpeningState::DoorPairAngles { active_degrees, .. } => active_degrees,
                os_model::OpeningState::Default => {
                    if resolved.kind == OpeningKind::Door {
                        90.
                    } else {
                        0.
                    }
                }
                os_model::OpeningState::DoorAngle(v)
                | os_model::OpeningState::SlidingFraction(v)
                | os_model::OpeningState::CasementAngle(v) => v,
            }
            .to_string(),
            context,
            providers: crate::plan_workspace::plan_provider_signature(editor),
            session: editor.document.session_id(),
            revision: editor.document.revision(),
            view,
            selection,
            activation: editor.host.activation_id(os_walls::PLUGIN_ID),
            id,
            editing,
            clearance_end: model.opening_clearances.get(&id).map(|c| c.end),
            clearance_value: model
                .opening_clearances
                .get(&id)
                .map_or(params.offset, |c| c.distance)
                .to_string(),
            kind: resolved.kind,
            type_id: resolved.type_id,
            dimension_overrides: [
                params.width_override.is_some(),
                params.height_override.is_some(),
            ],
            override_sill: params.sill_override.is_some(),
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
            && editor.native_plan_context(self.view).ok() == Some(self.context)
            && crate::plan_workspace::plan_provider_signature(editor) == self.providers
    }

    #[cfg(test)]
    pub(super) fn set_open_state_for_test(&mut self, value: &str) {
        self.state_override = true;
        self.state_value = value.into();
    }

    fn parameters(&self) -> Result<OpeningParams> {
        let mut values = [0.0; 4];
        for (i, text) in self.values.iter().enumerate() {
            if self.type_id.is_some()
                && !match i {
                    0 => true,
                    1 | 2 => self.dimension_overrides[i - 1],
                    _ => self.override_sill,
                }
            {
                continue;
            }
            values[i] = text
                .trim()
                .parse()
                .map_err(|_| Error::Invalid(format!("{} needs a number", LABELS[i])))?;
        }
        let mut p = self.params.clone();
        if self.state_supported {
            p.open_state = if self.state_override {
                let value =
                    self.state_value.trim().parse::<f64>().map_err(|_| {
                        Error::Invalid("Opening state needs a finite number".into())
                    })?;
                if self.kind == OpeningKind::Door {
                    if self.paired {
                        os_model::OpeningState::DoorPairAngles {
                            active_degrees: value,
                            inactive_degrees: self.inactive_state_value.trim().parse().map_err(
                                |_| Error::Invalid("Inactive leaf angle needs a number".into()),
                            )?,
                        }
                    } else {
                        os_model::OpeningState::DoorAngle(value)
                    }
                } else if self.state_operation == os_model::WindowOperation::Sliding {
                    os_model::OpeningState::SlidingFraction(value)
                } else {
                    os_model::OpeningState::CasementAngle(value)
                }
            } else {
                os_model::OpeningState::Default
            };
        }
        p.offset = values[0];
        if self.type_id.is_some() {
            p.width_override = self.dimension_overrides[0].then_some(values[1]);
            p.height_override = self.dimension_overrides[1].then_some(values[2]);
            p.sill_override = self.override_sill.then_some(values[3]);
        }
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

    fn clearance(&self) -> Result<Option<os_model::OpeningClearance>> {
        self.clearance_end
            .map(|end| {
                let distance = self.clearance_value.trim().parse::<f64>().map_err(|_| {
                    Error::Invalid("Clearance needs a finite number of metres".into())
                })?;
                ensure(
                    distance.is_finite() && distance >= 0.,
                    "Clearance must be finite and nonnegative",
                )?;
                Ok(os_model::OpeningClearance { end, distance })
            })
            .transpose()
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
        if self.editing {
            model.openings.get_mut(&self.id).unwrap().parameters = params;
        } else {
            let mut opening = Opening::new("core.opening", params);
            opening.header.id = self.id;
            model.openings.insert(self.id, opening);
        }
        if let Some(lock) = self.clearance()? {
            model.opening_clearances.insert(self.id, lock);
        } else {
            model.opening_clearances.remove(&self.id);
        }
        model.settle_opening_clearances(editor.document.model())?;
        model.validate()?;
        let resolved = model.resolve_opening(&model.openings[&self.id].parameters)?;
        host_mesh(&model, resolved.host)?;
        for opening in model
            .openings
            .values()
            .filter(|o| o.parameters.host == resolved.host)
        {
            panel_mesh(&model, opening.id())?;
        }
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
            self.preview(editor, view, selected)?;
            editor.document.execute(
                "Edit opening",
                vec![
                    Command::UpdateOpening {
                        id: self.id,
                        parameters: self.parameters()?,
                    },
                    Command::SetOpeningClearance {
                        id: self.id,
                        clearance: self.clearance()?,
                    },
                ],
            )?;
            editor.regenerate()?;
        } else {
            self.preview(editor, view, selected)?;
            let mut parameters = self.parameters()?;
            let mut commands = Vec::new();
            if self.type_id.is_none() {
                let resolved = editor.document.model().resolve_opening(&parameters)?;
                let opening_type = OpeningType::new(
                    "core.opening_type",
                    OpeningTypeParams {
                        window_operation: Default::default(),
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
                        .add_enabled(
                            self.plan_shows_opening_kind(OpeningKind::Door),
                            egui::Button::new("Door"),
                        )
                        .on_hover_text("Place a door by clicking a visible wall")
                        .clicked()
                    {
                        self.begin_opening_placement(OpeningKind::Door);
                    }
                    if ui
                        .add_enabled(
                            self.plan_shows_opening_kind(OpeningKind::Window),
                            egui::Button::new("Window"),
                        )
                        .on_hover_text("Place a window by clicking a visible wall")
                        .clicked()
                    {
                        self.begin_opening_placement(OpeningKind::Window);
                    }
                    if ui.button("Change opening type…").clicked() {
                        self.begin_opening_type_assignment();
                    }
                    if ui.button("Edit selected openings…").clicked() {
                        self.begin_opening_batch_edit();
                    }
                });
                ui.horizontal(|ui| {
                    ui.menu_button("Exact new…", |ui| {
                        if ui
                            .add_enabled(
                                self.plan_shows_opening_kind(OpeningKind::Door),
                                egui::Button::new("Door dimensions…"),
                            )
                            .clicked()
                        {
                            self.begin_opening(Some(OpeningKind::Door));
                            ui.close();
                        }
                        if ui
                            .add_enabled(
                                self.plan_shows_opening_kind(OpeningKind::Window),
                                egui::Button::new("Window dimensions…"),
                            )
                            .clicked()
                        {
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
                                    self.change_placement_type(kind, *id);
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
                    if ui
                        .add_enabled(
                            self.selected_ids.len() == 1
                                && self.selected.is_some_and(|id| {
                                    self.editor.document.model().openings.contains_key(&id)
                                }),
                            egui::Button::new("Array along wall"),
                        )
                        .clicked()
                    {
                        self.begin_opening_array();
                    }
                    if ui
                        .add_enabled(
                            self.selected.is_some_and(|id| {
                                self.editor.document.model().openings.contains_key(&id)
                            }),
                            egui::Button::new("Rehost"),
                        )
                        .on_hover_text(
                            "Move the selected door or window to another visible native wall",
                        )
                        .clicked()
                    {
                        self.begin_opening_rehost();
                    }
                    if ui
                        .add_enabled(
                            self.selected_ids.len() == 1
                                && self.selected.is_some_and(|id| {
                                    self.editor.document.model().openings.contains_key(&id)
                                }),
                            egui::Button::new("Align opening center"),
                        )
                        .on_hover_text(
                            "Project this opening center onto another native wall centerline",
                        )
                        .clicked()
                    {
                        self.begin_opening_align();
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
            ui.label("Inherit type dimensions or override them for this instance. Windows can also override sill. All dimensions are in metres.");
            egui::ScrollArea::vertical()
                .id_salt(("opening-properties", draft.id))
                .max_height((ctx.content_rect().height()-260.0).max(100.0))
                .show(ui, |ui| {
                ui.group(|ui| {
                    ui.label("Opening state");
                    ui.add_enabled_ui(draft.state_supported, |ui| {
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut draft.state_override, false, "Default pose");
                            ui.radio_value(&mut draft.state_override, true, "Override pose");
                        });
                        let label = if draft.paired { "Active leaf angle (0–90°)" }
                            else if draft.kind == OpeningKind::Door { "Door angle (0–90°)" }
                            else if draft.state_operation == os_model::WindowOperation::Sliding { "Sliding fraction (0–1)" }
                            else { "Casement angle (0–90°)" };
                        ui.label(label);
                        ui.add_enabled(draft.state_override, egui::TextEdit::singleline(&mut draft.state_value)
                            .char_limit(64).desired_width(170.0));
                        if draft.paired {
                            ui.label("Inactive leaf angle (0–90°)");
                            ui.add_enabled(draft.state_override, egui::TextEdit::singleline(&mut draft.inactive_state_value).char_limit(64).desired_width(170.0));
                            if ui.button("Reset paired poses to Default").clicked() {
                                draft.state_override = false;
                            }
                        }
                    });
                    if !draft.state_supported {
                        ui.label("Pose unavailable: requires a straight host and rectangular door or Sliding/Casement window. Stored pose is retained; current geometry is preserved.");
                    } else {
                        ui.label("Default: doors 90°, windows closed. Sliding Start sash travels toward End; Casement hinges at Start toward the left of the host.");
                    }
                });
                egui::Grid::new("opening_fields").num_columns(2).show(ui, |ui| {
                    ui.label("Name"); ui.add(egui::TextEdit::singleline(&mut draft.params.name).char_limit(256).desired_width(170.0)); ui.end_row();
                    ui.label(LABELS[0]);
                    ui.add_enabled(draft.clearance_end.is_none(), egui::TextEdit::singleline(&mut draft.values[0]).char_limit(64).desired_width(170.0));
                    ui.end_row();
                    match self.editor.document.model().resolve_opening(&draft.params) {
                        Ok(resolved) if resolved.type_id.is_some() => {
                            let defaults = self.editor.document.model().resolve_opening_type(resolved.type_id.unwrap()).expect("validated opening type").parameters;
                            ui.label("Assigned type");
                            ui.label(format!("{} · defaults {:.3} × {:.3} m · sill {:.3} m",
                                resolved.type_name.unwrap_or_else(|| "Opening type".into()),
                                defaults.width, defaults.height, defaults.sill));
                            ui.end_row();
                            for (index, label, default) in [(1, "width", defaults.width), (2, "height", defaults.height)] {
                                ui.label(LABELS[index]);
                                ui.vertical(|ui| {
                                    ui.horizontal(|ui| {
                                        ui.radio_value(&mut draft.dimension_overrides[index - 1], false, format!("Inherit {label}"));
                                        ui.radio_value(&mut draft.dimension_overrides[index - 1], true, format!("Override {label}"));
                                    });
                                    ui.add_enabled(draft.dimension_overrides[index - 1],
                                        egui::TextEdit::singleline(&mut draft.values[index]).char_limit(64).desired_width(170.0));
                                    let effective = if draft.dimension_overrides[index - 1] {
                                        draft.values[index].trim().parse::<f64>().ok()
                                    } else { Some(default) };
                                    ui.label(effective.map_or_else(|| "Invalid value".into(), |v| format!("Effective {label}: {v:.3} m · default {default:.3} m")));
                                    if ui.button(format!("Reset {label} to type default")).clicked() {
                                        draft.dimension_overrides[index - 1] = false;
                                        draft.values[index] = default.to_string();
                                    }
                                });
                                ui.end_row();
                            }
                            if let Some(lite) = defaults.family.side_lite.as_ref() {
                                ui.label("Lite side");
                                ui.vertical(|ui| {
                                    for (value, label) in [(None, "Inherit lite side"), (Some(os_model::LiteSide::Start), "Lite at start"), (Some(os_model::LiteSide::End), "Lite at end")] {
                                        ui.radio_value(&mut draft.params.lite_side_override, value, label);
                                    }
                                    let effective = draft.params.lite_side_override.unwrap_or(lite.side);
                                    ui.label(format!("Effective lite: {effective:?} · type {:?}", lite.side));
                                    if ui.button("Reset lite to type default").clicked() {
                                        draft.params.lite_side_override = None;
                                    }
                                });
                                ui.end_row();
                            }
                            if resolved.kind == OpeningKind::Window {
                                ui.label("Pane position");
                                ui.vertical(|ui| {
                                    ui.radio_value(&mut draft.params.pane_position_override, None, "Inherit pane position");
                                    for (position, label) in [
                                        (WindowPanePosition::Center, "Center"),
                                        (WindowPanePosition::LeftFace, "Left face"),
                                        (WindowPanePosition::RightFace, "Right face"),
                                    ] {
                                        ui.radio_value(&mut draft.params.pane_position_override, Some(position), label);
                                    }
                                    let effective = draft.params.pane_position_override.unwrap_or(defaults.pane_position);
                                    ui.label(format!("Effective pane: {effective:?} · default {:?}", defaults.pane_position));
                                    if ui.button("Reset pane to type default").clicked() {
                                        draft.params.pane_position_override = None;
                                    }
                                });
                                ui.end_row();
                                ui.label("Sill mode");
                                ui.vertical(|ui| {
                                    ui.radio_value(&mut draft.override_sill, false, "Use type default");
                                    ui.radio_value(&mut draft.override_sill, true, "Override sill");
                                });
                                ui.end_row();
                                let default_sill = defaults.sill;
                                ui.label(LABELS[3]);
                                ui.add_enabled(draft.override_sill,
                                    egui::TextEdit::singleline(&mut draft.values[3]).char_limit(64).desired_width(170.0));
                                ui.end_row();
                                ui.label("Effective sill");
                                let effective = if draft.override_sill { draft.values[3].parse::<f64>().ok() } else { Some(default_sill) };
                                ui.label(effective.map_or_else(|| "Invalid value".into(), |v| format!("{v:.3} m (type default {default_sill:.3} m)")));
                                ui.end_row();
                                if ui.button("Reset sill to type default").clicked() {
                                    draft.override_sill = false;
                                    draft.values[3] = default_sill.to_string();
                                }
                                ui.end_row();
                            }
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
                if draft.editing {
                    ui.group(|ui| {
                        ui.label("Host-end clearance");
                        let previous = draft.clearance_end;
                        ui.horizontal(|ui| {
                            ui.selectable_value(&mut draft.clearance_end, None, "Unlocked");
                            ui.selectable_value(&mut draft.clearance_end, Some(os_model::ClearanceEnd::Start), "Lock Start");
                            ui.selectable_value(&mut draft.clearance_end, Some(os_model::ClearanceEnd::End), "Lock End");
                        });
                        if previous != draft.clearance_end
                            && let Some(end) = draft.clearance_end
                            && let Ok(parameters) = draft.parameters()
                            && let Ok(resolved) = self.editor.document.model().resolve_opening(&parameters)
                        {
                            draft.clearance_value = match end {
                                os_model::ClearanceEnd::Start => resolved.offset,
                                os_model::ClearanceEnd::End => host.length() - resolved.width - resolved.offset,
                            }.to_string();
                        }
                        if draft.clearance_end.is_some() {
                            ui.label("Centerline clearance (m)");
                            ui.add(egui::TextEdit::singleline(&mut draft.clearance_value).char_limit(64).desired_width(170.0));
                        }
                    });
                }
                ui.label("Keep at least 1 mm at wall ends, above the opening and between openings. Left/right are viewed along wall start → end. Reversing the wall reverses this frame.");
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
    fn paired_doors_plan_mesh_coordinates_preview_history_and_invalid_pose() {
        let mut e = Editor::new().unwrap();
        let level = *e.document.model().levels.keys().next().unwrap();
        let view = e.create_floor_plan("Pair plan", level).unwrap();
        let mut wp = default_wall(level);
        wp.path = os_model::WallPath::Straight {
            start: Point2::new(2., 3.),
            end: Point2::new(8., 11.),
        };
        let wall = os_model::Wall::new(os_walls::WALL_TYPE, wp);
        let host = wall.id();
        e.command("Host", Command::AddWall(wall)).unwrap();
        let new =
            OpeningDraft::begin(&e, Some(view), Some(host), Some(OpeningKind::Door), None).unwrap();
        let id = new.apply(&mut e, Some(view), Some(host), false).unwrap();
        let ty = e.document.model().openings[&id]
            .parameters
            .type_id()
            .unwrap();
        let mut parameters = e.document.model().opening_types[&ty].parameters.clone();
        parameters.family.door_leaves = os_model::DoorLeaves::Paired {
            active_fraction: 0.65,
        };
        parameters.family.frame_width = 0.04;
        parameters.family.side_lite = Some(os_model::SideLite {
            side: os_model::LiteSide::End,
            width_fraction: 0.2,
            mullion_width: 0.04,
            material: None,
        });
        e.command("Pair", Command::UpdateOpeningType { id: ty, parameters })
            .unwrap();
        let before = e.document.model().clone();
        let scene = e.scene.clone();
        let stats = e.document.history_stats();
        let mut draft = OpeningDraft::begin(&e, Some(view), Some(id), None, None).unwrap();
        draft.state_override = true;
        draft.state_value = "30".into();
        draft.inactive_state_value = "75".into();
        let lines = draft.preview(&e, Some(view), Some(id)).unwrap();
        assert_eq!(e.document.model(), &before);
        assert_eq!(e.scene, scene);
        let p = before
            .resolve_opening(&draft.parameters().unwrap())
            .unwrap();
        let wall = before.resolve_wall(host).unwrap().parameters;
        let context = e.native_plan_context(view).unwrap();
        let mesh = os_geometry::openings::component_mesh(&p, &wall, 0.).unwrap();
        for leaf in os_geometry::openings::door_pair(&p).unwrap().unwrap() {
            let ends = [0., 1.].map(|u| {
                let q = component_point(&leaf, &wall, u);
                world(&wall, q.x, q.y)
            });
            let projected = ends.map(|q| context.basis.world_to_plane(q).unwrap());
            assert!(
                lines.iter().any(|l| l.start.distance(projected[0]) < 1e-9
                    && l.end.distance(projected[1]) < 1e-9)
            );
            // Each leaf end's centerline is the midpoint of its two lower mesh corners.
            for end in ends {
                assert!(mesh.vertices.iter().filter(|v| v.z.abs() < 1e-9).any(|a| {
                    mesh.vertices.iter().filter(|v| v.z.abs() < 1e-9).any(|b| {
                        (a.x - b.x).hypot(a.y - b.y) > 1e-6
                            && Point2::new((a.x + b.x) / 2., (a.y + b.y) / 2.).distance(end) < 1e-9
                    })
                }));
            }
        }
        for invalid in ["NaN", "-1", "91"] {
            draft.inactive_state_value = invalid.into();
            assert!(draft.apply(&mut e, Some(view), Some(id), false).is_err());
            assert_eq!(e.document.model(), &before);
            assert_eq!(e.document.history_stats(), stats);
        }
        draft.inactive_state_value = "75".into();
        draft.apply(&mut e, Some(view), Some(id), false).unwrap();
        let after = e.document.model().clone();
        assert_eq!(after.openings[&id].header, before.openings[&id].header);
        assert_eq!(e.scene[&host], scene[&host]);
        assert_eq!(
            e.document.history_stats().undo_entries,
            stats.undo_entries + 1
        );
        e.undo().unwrap();
        assert_eq!(e.document.model(), &before);
        e.redo().unwrap();
        assert_eq!(e.document.model(), &after);
    }
    #[test]
    fn opening_state_disposable_preview_atomic_apply_history_and_stale() {
        for (kind, operation, value) in [
            (OpeningKind::Door, os_model::WindowOperation::Fixed, "45"),
            (
                OpeningKind::Window,
                os_model::WindowOperation::Sliding,
                "0.5",
            ),
            (
                OpeningKind::Window,
                os_model::WindowOperation::Casement,
                "45",
            ),
        ] {
            let mut e = Editor::new().unwrap();
            let level = *e.document.model().levels.keys().next().unwrap();
            let view = e.create_floor_plan("Plan", level).unwrap();
            let wall = os_model::Wall::new(os_walls::WALL_TYPE, default_wall(level));
            let host = wall.id();
            e.command("Host", Command::AddWall(wall)).unwrap();
            let new = OpeningDraft::begin(&e, Some(view), Some(host), Some(kind), None).unwrap();
            let id = new.apply(&mut e, Some(view), Some(host), false).unwrap();
            if kind == OpeningKind::Window {
                let ty = e.document.model().openings[&id]
                    .parameters
                    .type_id()
                    .unwrap();
                let mut parameters = e.document.model().opening_types[&ty].parameters.clone();
                parameters.window_operation = operation;
                e.command(
                    "Operation",
                    Command::UpdateOpeningType { id: ty, parameters },
                )
                .unwrap();
            }
            let before = e.document.model().clone();
            let scene = e.scene.clone();
            let stats = e.document.history_stats();
            let revision = e.document.revision();
            let mut draft = OpeningDraft::begin(&e, Some(view), Some(id), None, None).unwrap();
            let closed = draft.preview(&e, Some(view), Some(id)).unwrap();
            draft.state_override = true;
            for invalid in ["NaN", "inf", "-1", "91"] {
                draft.state_value = invalid.into();
                assert!(draft.apply(&mut e, Some(view), Some(id), false).is_err());
                assert_eq!(e.document.model(), &before);
                assert_eq!(e.scene, scene);
                assert_eq!(e.document.history_stats(), stats);
                assert_eq!(e.document.revision(), revision);
            }
            draft.state_value = value.into();
            let preview = draft.preview(&e, Some(view), Some(id)).unwrap();
            assert_ne!(preview, closed);
            assert_eq!(
                preview.iter().map(|l| l.feature).collect::<Vec<_>>(),
                closed.iter().map(|l| l.feature).collect::<Vec<_>>()
            );
            assert_eq!(e.document.model(), &before);
            assert_eq!(e.scene, scene);
            drop(draft);
            let mut draft = OpeningDraft::begin(&e, Some(view), Some(id), None, None).unwrap();
            draft.state_override = true;
            draft.state_value = value.into();
            draft.apply(&mut e, Some(view), Some(id), false).unwrap();
            let after = e.document.model().clone();
            assert_eq!(after.openings[&id].header, before.openings[&id].header);
            assert_eq!(after.opening_types, before.opening_types);
            assert_eq!(e.scene[&host], scene[&host]);
            assert_ne!(e.scene[&id], scene[&id]);
            assert_eq!(
                e.document.history_stats().undo_entries,
                stats.undo_entries + 1
            );
            let old_host = os_geometry::walls::NativeWall::from_model(&before, host).unwrap();
            let new_host = os_geometry::walls::NativeWall::from_model(&after, host).unwrap();
            assert_eq!(
                old_host.layer_quantities().unwrap(),
                new_host.layer_quantities().unwrap()
            );
            assert!(draft.apply(&mut e, Some(view), Some(id), false).is_err());
            e.undo().unwrap();
            assert_eq!(e.document.model(), &before);
            assert_eq!(e.scene, scene);
            e.redo().unwrap();
            assert_eq!(e.document.model(), &after);
            let stale = OpeningDraft::begin(&e, Some(view), Some(id), None, None).unwrap();
            e.document = Document::from_model(after.clone()).unwrap();
            assert!(!stale.current(&e, Some(view), Some(id)));
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("state.osb");
            e.save(&path).unwrap();
            let mut reopened = Editor::new().unwrap();
            reopened.open(&path).unwrap();
            assert_eq!(reopened.scene, e.scene);
        }
    }

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
