//! Graph-aware native junction editing. Drafts own no live document mutations.
use super::*;
use os_model::{Model, WallAnchor, WallEndpoint, WallJoinParams};
use os_render::snapping::SnapQuery;
use std::collections::BTreeSet;

const UNAVAILABLE: &str = "Junction drag requires a native join at this grip. Unconnected endpoints on joined walls are unavailable.";
const PROVIDER_UNAVAILABLE: &str = "Junction drag is unavailable with an installed Wall provider; it requires an atomic multi-wall command.";
// Constraint solving is dense and runs synchronously during pointer movement.
// Keep its worst-case memory and frame cost bounded for hostile/oversized models.
const MAX_INTERACTIVE_COMPONENT_WALLS: usize = 64;

fn validate_component_size(wall_count: usize) -> Result<()> {
    os_core::ensure(
        wall_count <= MAX_INTERACTIVE_COMPONENT_WALLS,
        "Connected junction is too large for interactive editing (64-wall limit)",
    )
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Grip {
    Endpoint(WallEdit),
    TeeStation(Id),
}

impl From<WallEdit> for Grip {
    fn from(mode: WallEdit) -> Self {
        Self::Endpoint(mode)
    }
}

impl Grip {
    pub(super) fn endpoint(self) -> Option<WallEdit> {
        match self {
            Self::Endpoint(mode) => Some(mode),
            Self::TeeStation(_) => None,
        }
    }
}

fn joined(model: &Model, id: Id, grip: Grip) -> Result<Option<(Id, WallJoinParams)>> {
    let joins: Vec<_> = model
        .wall_joins
        .values()
        .filter(|join| join.parameters.members().contains(&id))
        .collect();
    if joins.is_empty() {
        return Ok(None);
    }
    let anchor = match grip {
        Grip::Endpoint(mode) => {
            let endpoint = match mode {
                WallEdit::ResizeStart => WallEndpoint::Start,
                WallEdit::ResizeEnd => WallEndpoint::End,
                _ => return Err(Error::Unsupported(UNAVAILABLE.into())),
            };
            Some(WallAnchor { wall: id, endpoint })
        }
        Grip::TeeStation(_) => None,
    };
    let join = joins
        .into_iter()
        .find(|join| match grip {
            Grip::Endpoint(_) => join.parameters.anchors().contains(&anchor.unwrap()),
            Grip::TeeStation(join_id) => matches!(join.parameters,
            WallJoinParams::Tee { host, .. } if host == id && join.id() == join_id),
        })
        .ok_or_else(|| Error::Unsupported(UNAVAILABLE.into()))?;
    let (members, _) = component(model, id);
    validate_component_size(members.len())?;
    os_core::ensure(
        members.iter().all(|id| {
            model
                .walls
                .get(id)
                .is_some_and(|wall| wall.header.type_id == os_walls::WALL_TYPE)
        }),
        UNAVAILABLE,
    )?;
    Ok(Some((join.id(), join.parameters.clone())))
}

fn component(model: &Model, seed: Id) -> (BTreeSet<Id>, BTreeSet<Id>) {
    let mut adjacency: BTreeMap<Id, Vec<Id>> = BTreeMap::new();
    for (id, join) in &model.wall_joins {
        for member in join.parameters.members() {
            adjacency.entry(member).or_default().push(*id);
        }
    }
    let mut walls = BTreeSet::from([seed]);
    let mut joins = BTreeSet::new();
    let mut pending = vec![seed];
    while let Some(wall) = pending.pop() {
        for id in adjacency.get(&wall).into_iter().flatten() {
            if joins.insert(*id) {
                for member in model.wall_joins[id].parameters.members() {
                    if walls.insert(member) {
                        pending.push(member);
                    }
                }
            }
        }
    }
    (walls, joins)
}

fn provider_available(editor: &Editor) -> bool {
    #[cfg(feature = "external-plugins")]
    if editor.host.worker_supported(os_plugin_api::wall::OWNER) || editor.plugin_work_pending() {
        return false;
    }
    editor.host.activation_id(os_walls::PLUGIN_ID).is_some()
}

/// False is an ordinary unconnected endpoint. Err is visibly unavailable and
/// must never acquire the single-wall gesture.
pub(super) fn availability(editor: &Editor, id: Id, grip: impl Into<Grip>) -> Result<bool> {
    let joined = joined(editor.document.model(), id, grip.into())?.is_some();
    if joined {
        os_core::ensure(provider_available(editor), PROVIDER_UNAVAILABLE)?;
    }
    Ok(joined)
}

pub(super) fn visible_availability(
    editor: &Editor,
    drawing: &PlanDrawing,
    context: PlanContext,
    id: Id,
    grip: impl Into<Grip>,
) -> Result<bool> {
    let grip = grip.into();
    let available = availability(editor, id, grip)?;
    if let Some((_, parameters)) = joined(editor.document.model(), id, grip)? {
        let items = drawing.items(context)?;
        os_core::ensure(
            parameters
                .members()
                .iter()
                .all(|id| items.iter().any(|item| item.entity == *id)),
            "Both junction walls must be visible",
        )?;
    }
    Ok(available)
}

pub(super) fn station_handles(
    model: &Model,
    selected: Id,
    context: PlanContext,
    camera: PlanCamera,
    rect: egui::Rect,
) -> Vec<(Grip, egui::Pos2)> {
    model
        .wall_joins
        .values()
        .filter_map(|join| {
            let WallJoinParams::Tee { host, station, .. } = join.parameters else {
                return None;
            };
            if host != selected {
                return None;
            }
            let wall = &model.walls.get(&host)?.parameters;
            let point = context
                .basis
                .world_to_plane(os_model::wall_point(wall, station, 0.0))
                .ok()?;
            if !point_in_plan_crop(context, point) {
                return None;
            }
            let screen = camera
                .project(point, [f64::from(rect.width()), f64::from(rect.height())])
                .ok()?;
            let pos = rect.min + egui::vec2(screen.x as f32, screen.y as f32);
            (pos.is_finite() && rect.contains(pos)).then_some((Grip::TeeStation(join.id()), pos))
        })
        .collect()
}

pub(super) fn hit_station(handles: &[(Grip, egui::Pos2)], pointer: egui::Pos2) -> Option<Grip> {
    handles
        .iter()
        .filter_map(|(grip, pos)| {
            let distance = pos.distance_sq(pointer);
            (distance <= ENDPOINT_HIT_RADIUS * ENDPOINT_HIT_RADIUS).then_some((*grip, distance))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(grip, _)| grip)
}

pub(super) struct JunctionDrag {
    context: PlanContext,
    selected: Id,
    drawing: Id,
    providers: Vec<(String, Id)>,
    activation: Option<Id>,
    original: Model,
    members: BTreeSet<Id>,
    joins: BTreeSet<Id>,
    parameters: WallJoinParams,
    node: Point2,
    axis: Point2,
    pub(super) grip: Grip,
    origin: egui::Pos2,
    moved: bool,
}

impl JunctionDrag {
    pub(super) fn commit_messages(&self) -> (&'static str, &'static str) {
        match self.parameters {
            WallJoinParams::Butt { .. } => ("Move Butt junction", "Butt junction moved."),
            WallJoinParams::Corner { .. } => ("Move Corner junction", "Corner junction moved."),
            WallJoinParams::Tee { .. } => ("Move Tee junction", "Tee junction moved."),
        }
    }

    pub(super) fn begin(
        editor: &Editor,
        drawing: &PlanDrawing,
        context: PlanContext,
        selected: Id,
        grip: impl Into<Grip>,
        origin: egui::Pos2,
    ) -> Result<Self> {
        os_core::ensure(provider_available(editor), PROVIDER_UNAVAILABLE)?;
        os_core::ensure(
            editor.document.session_id() == context.session_id
                && editor.document.revision() == context.model_revision,
            "Junction context is stale",
        )?;
        let model = editor.document.model();
        let grip = grip.into();
        let (_, parameters) = joined(model, selected, grip)?
            .ok_or_else(|| Error::Invalid("No junction at this grip".into()))?;
        let items = drawing.items(context)?;
        os_core::ensure(
            parameters
                .members()
                .iter()
                .all(|id| items.iter().any(|item| item.entity == *id)),
            "Both junction walls must be visible",
        )?;
        let (node, axis) = match parameters {
            WallJoinParams::Butt { a, .. } => {
                let wall = &model.walls[&a.wall].parameters;
                (a.endpoint.point(wall), os_model::axis(wall))
            }
            WallJoinParams::Corner { a, b, .. } => {
                let anchor = if a.wall == selected { a } else { b };
                let wall = &model.walls[&selected].parameters;
                (anchor.endpoint.point(wall), os_model::axis(wall))
            }
            WallJoinParams::Tee { host, station, .. } => {
                let wall = &model.walls[&host].parameters;
                (
                    os_model::wall_point(wall, station, 0.0),
                    os_model::axis(wall),
                )
            }
        };
        os_core::ensure(
            point_in_plan_crop(context, context.basis.world_to_plane(node)?),
            "Junction is outside the plan crop",
        )?;
        let (members, joins) = component(model, selected);
        Ok(Self {
            context,
            selected,
            drawing: drawing.identity(),
            providers: plan_provider_signature(editor),
            activation: editor.host.activation_id(os_walls::PLUGIN_ID),
            original: model.clone(),
            members,
            joins,
            parameters,
            node,
            axis,
            grip,
            origin,
            moved: false,
        })
    }

    pub(super) fn current(
        &self,
        editor: &Editor,
        active: Option<Id>,
        selected: Option<Id>,
        drawing: Option<&PlanDrawing>,
    ) -> bool {
        active == Some(self.context.view_id)
            && editor.document.session_id() == self.context.session_id
            && editor.document.revision() == self.context.model_revision
            && selected == Some(self.selected)
            && editor.native_plan_context(self.context.view_id).ok() == Some(self.context)
            && editor.host.activation_id(os_walls::PLUGIN_ID) == self.activation
            && plan_provider_signature(editor) == self.providers
            && provider_available(editor)
            && drawing.is_some_and(|drawing| {
                drawing.identity() == self.drawing && drawing.items(self.context).is_ok()
            })
    }

    fn project(&self, point: Point2) -> Point2 {
        let distance =
            (point.x - self.node.x) * self.axis.x + (point.y - self.node.y) * self.axis.y;
        Point2::new(
            self.node.x + distance * self.axis.x,
            self.node.y + distance * self.axis.y,
        )
    }

    fn target(
        &self,
        drawing: &PlanDrawing,
        camera: PlanCamera,
        rect: egui::Rect,
        pointer: egui::Pos2,
        snaps: SnapOptions,
    ) -> Result<Point2> {
        let size = [f64::from(rect.width()), f64::from(rect.height())];
        let plane = camera.unproject(
            Point2::new(
                f64::from(pointer.x - rect.left()),
                f64::from(pointer.y - rect.top()),
            ),
            size,
        )?;
        let point = self.project(self.context.basis.plane_to_world(plane)?);
        let plane = self.context.basis.world_to_plane(point)?;
        let query = SnapQuery {
            camera,
            viewport: size,
            pointer: camera.project(plane, size)?,
            radius_pixels: 12.0,
            endpoints: snaps.enabled && snaps.endpoints,
            midpoints: snaps.enabled && snaps.midpoints,
            intersections: snaps.enabled && snaps.intersections,
            nearest: snaps.enabled && snaps.nearest,
            perpendicular_from: (snaps.enabled && snaps.perpendicular)
                .then_some(self.context.basis.world_to_plane(self.node)?),
            axis_extensions: snaps.enabled && snaps.axis_extensions,
            exclude_entity: None,
        };
        let excluded = &self.members;
        let axis = os_render::snapping::SnapAxis {
            origin: self.context.basis.world_to_plane(self.node)?,
            direction: {
                let end = self.context.basis.world_to_plane(Point2::new(
                    self.node.x + self.axis.x,
                    self.node.y + self.axis.y,
                ))?;
                let start = self.context.basis.world_to_plane(self.node)?;
                Point2::new(end.x - start.x, end.y - start.y)
            },
        };
        if let Some(hit) = drawing
            .snap_on_axis(self.context, query, excluded, axis)?
            .candidate_on_axis(self.context, query, excluded, axis)?
        {
            let world = self.context.basis.plane_to_world(hit.point)?;
            // Never present an off-axis acquisition as a snap. Projection keeps
            // both endpoints on one exact node despite floating-point noise.
            if self.project(world).distance(world) <= 1e-9 {
                return Ok(self.project(world));
            }
        }
        Ok(point)
    }

    fn commands(&self, point: Point2) -> Result<(Vec<Command>, Vec<WallParams>)> {
        let mut point = self.project(point);
        if let WallJoinParams::Tee { host, .. } = self.parameters {
            let wall = &self.original.walls[&host].parameters;
            let station =
                (point.x - wall.start().x) * self.axis.x + (point.y - wall.start().y) * self.axis.y;
            point = os_model::wall_point(wall, station, 0.0);
        }
        os_core::ensure(
            point.is_finite()
                && point_in_plan_crop(self.context, self.context.basis.world_to_plane(point)?),
            "Junction target is outside the plan crop",
        )?;
        let delta = Point2::new(point.x - self.node.x, point.y - self.node.y);
        let ids: Vec<_> = self.members.iter().copied().collect();
        let indices: BTreeMap<_, _> = ids.iter().enumerate().map(|(i, id)| (*id, i * 4)).collect();
        let index = |a: WallAnchor| {
            indices[&a.wall]
                + if a.endpoint == WallEndpoint::Start {
                    0
                } else {
                    2
                }
        };
        let mut solve = Displacements::new(ids.len() * 4);
        // Cartesian endpoint displacements have equal weights. Projecting zero
        // onto the affine constraint space minimizes their summed squared motion.
        // UUID-sorted walls/joins and fixed row order make rank decisions stable.
        for id in &ids {
            let axis = os_model::axis(&self.original.walls[id].parameters);
            let i = indices[id];
            solve.equation(
                &[
                    (i, -axis.y),
                    (i + 1, axis.x),
                    (i + 2, axis.y),
                    (i + 3, -axis.x),
                ],
                0.0,
            )?;
        }
        for id in &self.joins {
            match self.original.wall_joins[id].parameters {
                WallJoinParams::Butt { a, b } | WallJoinParams::Corner { a, b, .. } => {
                    for coordinate in 0..2 {
                        solve.equation(
                            &[(index(a) + coordinate, 1.0), (index(b) + coordinate, -1.0)],
                            0.0,
                        )?;
                    }
                }
                WallJoinParams::Tee { host, branch, .. } => {
                    let axis = os_model::axis(&self.original.walls[&host].parameters);
                    let (h, b) = (indices[&host], index(branch));
                    // Station is free along the host. Only line membership is a
                    // constraint; derive the new station from the solved contact.
                    solve.equation(
                        &[(b, -axis.y), (b + 1, axis.x), (h, axis.y), (h + 1, -axis.x)],
                        0.0,
                    )?;
                }
            }
        }
        match self.parameters {
            WallJoinParams::Butt { a, b } => {
                for anchor in [a, b] {
                    solve.pin(index(anchor), delta)?;
                    solve.pin(
                        index(WallAnchor {
                            endpoint: anchor.endpoint.opposite(),
                            ..anchor
                        }),
                        Point2::new(0.0, 0.0),
                    )?;
                }
            }
            WallJoinParams::Corner { a, b, .. } => {
                let (selected, peer) = if a.wall == self.selected {
                    (a, b)
                } else {
                    (b, a)
                };
                solve.pin(index(selected), delta)?;
                solve.pin(
                    index(WallAnchor {
                        endpoint: selected.endpoint.opposite(),
                        ..selected
                    }),
                    Point2::new(0.0, 0.0),
                )?;
                // Preserve the isolated gesture: the immediate peer translates
                // rigidly; its downstream neighbours may resize or translate.
                solve.pin(indices[&peer.wall], delta)?;
                solve.pin(indices[&peer.wall] + 2, delta)?;
            }
            WallJoinParams::Tee { host, branch, .. } => {
                solve.pin(indices[&host], Point2::new(0.0, 0.0))?;
                solve.pin(indices[&host] + 2, Point2::new(0.0, 0.0))?;
                solve.pin(indices[&branch.wall], delta)?;
                solve.pin(indices[&branch.wall] + 2, delta)?;
            }
        }
        let displacement = solve.finish()?;
        let mut walls: BTreeMap<_, _> = ids
            .iter()
            .map(|id| {
                let mut wall = self.original.walls[id].parameters.clone();
                let i = indices[id];
                *wall.path.straight_start_mut()? = Point2::new(
                    wall.start().x + displacement[i],
                    wall.start().y + displacement[i + 1],
                );
                *wall.path.straight_end_mut()? = Point2::new(
                    wall.end().x + displacement[i + 2],
                    wall.end().y + displacement[i + 3],
                );
                Ok((*id, wall))
            })
            .collect::<Result<_>>()?;
        // Each anchor belongs to at most one join. Assign shared nodes once to
        // remove roundoff without a sequential geometry propagation walk.
        for id in &self.joins {
            if let WallJoinParams::Butt { a, b } | WallJoinParams::Corner { a, b, .. } =
                self.original.wall_joins[id].parameters
            {
                let node = a.endpoint.point(&walls[&a.wall]);
                set_endpoint(walls.get_mut(&b.wall).unwrap(), b.endpoint, node)?;
            }
        }
        let mut joins = Vec::new();
        // Calculate every Tee from the same solved hosts before assigning any
        // contact. Full model validation rejects numerical inconsistency.
        for id in &self.joins {
            if let WallJoinParams::Tee { host, branch, .. } =
                self.original.wall_joins[id].parameters
            {
                let wall = &walls[&host];
                let axis = os_model::axis(wall);
                let node = branch.endpoint.point(&walls[&branch.wall]);
                let station =
                    (node.x - wall.start().x) * axis.x + (node.y - wall.start().y) * axis.y;
                joins.push((
                    *id,
                    WallJoinParams::Tee {
                        host,
                        station,
                        branch,
                    },
                    os_model::wall_point(wall, station, 0.0),
                ));
            }
        }
        for (_, join, node) in &joins {
            if let WallJoinParams::Tee { branch, .. } = join {
                set_endpoint(walls.get_mut(&branch.wall).unwrap(), branch.endpoint, *node)?;
            }
        }
        let mut commands = Vec::new();
        for (id, wall) in &walls {
            let original = &self.original.walls[id].parameters;
            let axis = os_model::axis(original);
            os_core::ensure(
                (wall.end().x - wall.start().x) * axis.x + (wall.end().y - wall.start().y) * axis.y
                    >= 0.001,
                "Junction drag would collapse or reverse a wall",
            )?;
            wall.validate()?;
            let candidate_axis = os_model::axis(wall);
            os_core::ensure(
                (axis.x * candidate_axis.y - axis.y * candidate_axis.x).abs() <= 1e-12,
                "Junction solve changed a wall direction",
            )?;
            if wall == original {
                continue;
            }
            commands.push(Command::UpdateWall {
                id: *id,
                parameters: wall.clone(),
            });
            let start = Point2::new(
                wall.start().x - original.start().x,
                wall.start().y - original.start().y,
            );
            let end = Point2::new(
                wall.end().x - original.end().x,
                wall.end().y - original.end().y,
            );
            // Rigid motion carries hosted openings. Resizing preserves their
            // original axial position while carrying perpendicular translation.
            if start.distance(end) > 1e-10 {
                let shift = start.x * axis.x + start.y * axis.y;
                if shift.abs() > 1e-12 {
                    for opening in self
                        .original
                        .openings
                        .values()
                        .filter(|o| o.parameters.host == *id)
                    {
                        if self.original.opening_clearances.contains_key(&opening.id()) {
                            continue;
                        }
                        let mut parameters = opening.parameters.clone();
                        parameters.offset -= shift;
                        commands.push(Command::UpdateOpening {
                            id: opening.id(),
                            parameters,
                        });
                    }
                }
            }
        }
        for (id, parameters, _) in joins {
            if parameters != self.original.wall_joins[&id].parameters {
                commands.push(Command::UpdateWallJoin { id, parameters });
            }
        }
        Document::from_model(self.original.clone())?
            .execute("Preview wall junction", commands.clone())?;
        Ok((commands, walls.into_values().collect()))
    }

    pub(super) fn update(&mut self, ctx: &egui::Context, pointer: egui::Pos2) {
        self.moved |=
            pointer.distance(self.origin) > ctx.options(|o| o.input_options.max_click_dist);
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn preview(
        &self,
        drawing: &PlanDrawing,
        camera: PlanCamera,
        rect: egui::Rect,
        pointer: egui::Pos2,
        snaps: SnapOptions,
        painter: &egui::Painter,
    ) -> Result<Vec<Command>> {
        let target = self.target(drawing, camera, rect, pointer, snaps)?;
        let (commands, walls) = self.commands(target)?;
        let screen = |point| -> Result<egui::Pos2> {
            let p = camera.project(
                self.context.basis.world_to_plane(point)?,
                [f64::from(rect.width()), f64::from(rect.height())],
            )?;
            Ok(rect.min + egui::vec2(p.x as f32, p.y as f32))
        };
        for wall in walls {
            painter.line_segment(
                [screen(wall.start())?, screen(wall.end())?],
                egui::Stroke::new(2.0, theme::ACCENT),
            );
        }
        paint_grip(painter, screen(target)?, theme::ACCENT, 2.0);
        Ok(commands)
    }

    pub(super) fn moved(&self) -> bool {
        self.moved
    }
}

fn set_endpoint(wall: &mut WallParams, endpoint: WallEndpoint, node: Point2) -> Result<()> {
    match endpoint {
        WallEndpoint::Start => *wall.path.straight_start_mut()? = node,
        WallEndpoint::End => *wall.path.straight_end_mut()? = node,
    }
    Ok(())
}

/// Reorthogonalized row-space solve. Dependent rows detect incompatible cycles;
/// independent rows define the unique minimum Euclidean norm displacement.
struct Displacements {
    size: usize,
    basis: Vec<(Vec<f64>, f64)>,
    equations: Vec<(Vec<(usize, f64)>, f64)>,
    pins: Vec<(usize, f64)>,
}

impl Displacements {
    fn new(size: usize) -> Self {
        Self {
            size,
            basis: Vec::new(),
            equations: Vec::new(),
            pins: Vec::new(),
        }
    }

    fn pin(&mut self, index: usize, point: Point2) -> Result<()> {
        self.pins.extend([(index, point.x), (index + 1, point.y)]);
        self.equation(&[(index, 1.0)], point.x)?;
        self.equation(&[(index + 1, 1.0)], point.y)
    }

    fn equation(&mut self, coefficients: &[(usize, f64)], value: f64) -> Result<()> {
        self.equations.push((coefficients.to_vec(), value));
        let mut row = vec![0.0; self.size];
        for &(i, coefficient) in coefficients {
            row[i] += coefficient;
        }
        let mut rhs = value;
        for _ in 0..2 {
            for (basis, target) in &self.basis {
                let projection: f64 = row.iter().zip(basis).map(|(a, b)| a * b).sum();
                for (a, b) in row.iter_mut().zip(basis) {
                    *a -= projection * b;
                }
                rhs -= projection * target;
            }
        }
        let norm = row.iter().map(|v| v * v).sum::<f64>().sqrt();
        if norm <= 1e-10 {
            os_core::ensure(
                rhs.abs() <= 1e-9,
                "Inconsistent connected junction constraints",
            )?;
        } else {
            for coefficient in &mut row {
                *coefficient /= norm;
            }
            self.basis.push((row, rhs / norm));
        }
        Ok(())
    }

    fn finish(self) -> Result<Vec<f64>> {
        let mut result = vec![0.0; self.size];
        for (row, value) in self.basis {
            for (x, coefficient) in result.iter_mut().zip(row) {
                *x += value * coefficient;
            }
        }
        // Clean machine epsilon noise, especially fixed coordinates. Validate
        // original equations afterwards rather than trusting elimination alone.
        for value in &mut result {
            if value.abs() < 1e-12 {
                *value = 0.0;
            }
        }
        for (index, value) in self.pins {
            result[index] = value;
        }
        for (row, value) in self.equations {
            let actual: f64 = row.iter().map(|(i, c)| result[*i] * c).sum();
            os_core::ensure(
                actual.is_finite() && (actual - value).abs() <= 1e-9,
                "Unstable connected junction solve",
            )?;
        }
        Ok(result)
    }
}

pub(super) fn paint_grip(
    painter: &egui::Painter,
    pos: egui::Pos2,
    color: egui::Color32,
    width: f32,
) {
    painter.add(egui::Shape::convex_polygon(
        vec![
            pos + egui::vec2(0.0, -ENDPOINT_RADIUS),
            pos + egui::vec2(ENDPOINT_RADIUS, 0.0),
            pos + egui::vec2(0.0, ENDPOINT_RADIUS),
            pos + egui::vec2(-ENDPOINT_RADIUS, 0.0),
        ],
        theme::CANVAS,
        egui::Stroke::new(width, color),
    ));
}

#[cfg(test)]
mod solve_tests {
    use super::*;

    #[test]
    fn junction_graph_equations_reject_conflicting_cycle_and_minimize_free_motion() {
        let mut solve = Displacements::new(4);
        solve.equation(&[(0, 1.0), (1, -1.0)], 0.0).unwrap();
        solve.equation(&[(1, 1.0), (2, -1.0)], 0.0).unwrap();
        solve.equation(&[(2, 1.0), (0, -1.0)], 0.0).unwrap();
        solve.equation(&[(0, 1.0)], 0.25).unwrap();
        let result = solve.finish().unwrap();
        assert!(result[..3].iter().all(|v| (*v - 0.25).abs() < 1e-12));
        assert_eq!(result[3], 0.0);
        let mut solve = Displacements::new(3);
        solve.equation(&[(0, 1.0), (1, -1.0)], 0.0).unwrap();
        solve.equation(&[(1, 1.0), (2, -1.0)], 0.0).unwrap();
        assert!(solve.equation(&[(2, 1.0), (0, -1.0)], 1.0).is_err());
    }

    #[test]
    fn interactive_component_size_is_bounded() {
        assert!(validate_component_size(MAX_INTERACTIVE_COMPONENT_WALLS).is_ok());
        let error = validate_component_size(MAX_INTERACTIVE_COMPONENT_WALLS + 1)
            .unwrap_err()
            .to_string();
        assert!(error.contains("64-wall limit"));
    }
}
