//! Persisted named-plan commands and read-only native-wall plan derivation.
use crate::Editor;
use os_core::{Id, Point2, Result, ensure};
use os_document::Command;
use os_geometry::plan::{HorizontalBasis, PlanCrop, PlanRange, PlanRole};
use os_model::{DimensionEndpoint, DimensionParams, PlanSettings, View, ViewKind, ViewParams};
use os_render::plan::{
    MAX_PLAN_ELEMENTS, PlanContext, PlanDimensionItem, PlanDrawing, PlanRoomFace, PlanRoomItem,
};
use std::collections::BTreeMap;

/// Derived drawing retains provider validity authority. Do not cache a bare
/// copied provider drawing after dropping its checked results.
pub struct ProviderPlanDrawing {
    drawing: PlanDrawing,
    providers: Vec<os_plugin_host::plan_graphics::PlanGraphics>,
}

/// Frozen derivation inputs: no Editor, document or runtime handle crosses into
/// the drawing worker. Completed output retains its original provider authority.
pub struct PreparedProviderPlan {
    snapshot: PlanSnapshot,
    lines: BTreeMap<Id, Vec<os_render::plan::PlanLine>>,
    providers: Vec<os_plugin_host::plan_graphics::PlanGraphics>,
}

pub(crate) enum NativeViewSnapshot {
    Plan(Box<PlanSnapshot>),
    Section(Box<SectionSnapshot>),
}
impl NativeViewSnapshot {
    pub(crate) fn derive(self) -> Result<PlanDrawing> {
        match self {
            Self::Plan(snapshot) => (*snapshot).derive(),
            Self::Section(snapshot) => (*snapshot).derive(),
        }
    }
}

impl PreparedProviderPlan {
    pub fn derive(self) -> Result<ProviderPlanDrawing> {
        let drawing = self.snapshot.derive_with_provider_lines(self.lines)?;
        Ok(ProviderPlanDrawing {
            drawing,
            providers: self.providers,
        })
    }
}
impl ProviderPlanDrawing {
    pub fn drawing(&self, editor: &Editor, active_view: Id) -> Result<&PlanDrawing> {
        let context = editor.native_plan_context(active_view)?;
        self.drawing.items(context)?;
        for provider in &self.providers {
            provider.segments(&editor.host, &editor.document, active_view)?;
        }
        Ok(&self.drawing)
    }
}

impl Editor {
    pub fn create_floor_plan(&mut self, name: &str, level: Id) -> Result<Id> {
        let view = View::new("core.view", ViewParams::floor_plan(name, level));
        let id = view.id();
        self.command("Create floor plan", Command::AddView(view))?;
        Ok(id)
    }

    pub fn create_section_view(
        &mut self,
        name: &str,
        marker_level: Id,
        settings: os_model::SectionViewSettings,
    ) -> Result<Id> {
        let view = View::new(
            "core.view",
            ViewParams {
                name: name.into(),
                kind: ViewKind::Section,
                level: Some(marker_level),
                settings_revision: 0,
                plan: None,
                section: Some(settings),
            },
        );
        let id = view.id();
        self.command("Create section view", Command::AddView(view))?;
        Ok(id)
    }

    pub fn update_floor_plan(
        &mut self,
        id: Id,
        name: &str,
        level: Id,
        settings: PlanSettings,
    ) -> Result<()> {
        let view = self
            .document
            .model()
            .views
            .get(&id)
            .ok_or_else(|| os_core::Error::Invalid("plan view missing".into()))?;
        ensure(
            view.parameters.kind == ViewKind::Plan,
            "view is not a floor plan",
        )?;
        let mut parameters = view.parameters.clone();
        parameters.name = name.into();
        parameters.level = Some(level);
        parameters.plan = Some(settings);
        self.command("Update floor plan", Command::UpdateView { id, parameters })
    }

    pub fn native_plan_context(&self, view: Id) -> Result<PlanContext> {
        let model = self.document.model();
        let view = model
            .views
            .get(&view)
            .ok_or_else(|| os_core::Error::Invalid("plan view missing".into()))?;
        ensure(
            view.parameters.kind == ViewKind::Plan,
            "view is not a floor plan",
        )?;
        let level = view
            .parameters
            .level
            .and_then(|id| model.levels.get(&id))
            .ok_or_else(|| {
                os_core::Error::Invalid("floor plan needs a valid associated level".into())
            })?;
        let settings = view
            .parameters
            .plan
            .ok_or_else(|| os_core::Error::Invalid("plan settings missing".into()))?;
        settings.validate()?;
        let range = settings.range;
        Ok(PlanContext {
            session_id: self.document.session_id(),
            model_revision: self.document.revision(),
            view_id: view.id(),
            settings_revision: view.parameters.settings_revision,
            basis: HorizontalBasis {
                origin: settings.basis.origin,
                rotation: settings.basis.rotation,
            },
            range: PlanRange {
                top: range.top,
                cut: range.cut,
                bottom: range.bottom,
                depth: range.depth,
            }
            .at_level(level.parameters.elevation)?,
            crop: settings.crop.map(|crop| PlanCrop {
                min: crop.min,
                max: crop.max,
            }),
            scale_denominator: settings.scale_denominator,
            show_walls: settings.visibility.walls,
            show_extensions: settings.visibility.extensions,
        })
    }

    /// Context for a native drawing view. Sections use metres along their
    /// horizontal cut axis and absolute project elevation as the drawing plane.
    pub fn native_view_context(&self, view: Id) -> Result<PlanContext> {
        let parameters = &self
            .document
            .model()
            .views
            .get(&view)
            .ok_or_else(|| os_core::Error::Invalid("drawing view missing".into()))?
            .parameters;
        match parameters.kind {
            ViewKind::Plan => self.native_plan_context(view),
            ViewKind::Section => self.native_section_context(view),
            ViewKind::Perspective => Err(os_core::Error::Invalid(
                "perspective view has no 2D drawing context".into(),
            )),
        }
    }

    pub fn native_section_context(&self, view: Id) -> Result<PlanContext> {
        let model = self.document.model();
        let view = model
            .views
            .get(&view)
            .ok_or_else(|| os_core::Error::Invalid("section view missing".into()))?;
        ensure(
            view.parameters.kind == ViewKind::Section,
            "view is not a section",
        )?;
        view.parameters
            .level
            .and_then(|id| model.levels.get(&id))
            .ok_or_else(|| os_core::Error::Invalid("section needs a valid marker level".into()))?;
        let section = view
            .parameters
            .section
            .ok_or_else(|| os_core::Error::Invalid("section definition missing".into()))?;
        section.validate()?;
        let length = section.length()?;
        let middle =
            section.bottom_elevation + (section.top_elevation - section.bottom_elevation) * 0.5;
        Ok(PlanContext {
            session_id: self.document.session_id(),
            model_revision: self.document.revision(),
            view_id: view.id(),
            settings_revision: view.parameters.settings_revision,
            basis: HorizontalBasis::default(),
            range: PlanRange {
                top: section.top_elevation,
                cut: middle,
                bottom: section.bottom_elevation,
                depth: section.bottom_elevation,
            },
            crop: Some(PlanCrop {
                min: Point2::new(0.0, section.bottom_elevation),
                max: Point2::new(length, section.top_elevation),
            }),
            scale_denominator: 100.0,
            show_walls: true,
            show_extensions: false,
        })
    }

    pub fn native_drawing(&self, view: Id) -> Result<PlanDrawing> {
        self.native_view_snapshot(view)?.derive()
    }

    pub(crate) fn native_view_snapshot(&self, view: Id) -> Result<NativeViewSnapshot> {
        let parameters = &self
            .document
            .model()
            .views
            .get(&view)
            .ok_or_else(|| os_core::Error::Invalid("drawing view missing".into()))?
            .parameters;
        match parameters.kind {
            ViewKind::Plan => self
                .plan_snapshot(view)
                .map(Box::new)
                .map(NativeViewSnapshot::Plan),
            ViewKind::Section => self
                .section_snapshot(view)
                .map(Box::new)
                .map(NativeViewSnapshot::Section),
            ViewKind::Perspective => Err(os_core::Error::Invalid(
                "perspective view has no 2D drawing".into(),
            )),
        }
    }

    pub(crate) fn section_snapshot(&self, view: Id) -> Result<SectionSnapshot> {
        let context = self.native_section_context(view)?;
        let model = self.document.model();
        let view_parameters = &model.views[&view].parameters;
        let marker_level = view_parameters
            .level
            .expect("validated section marker level");
        let section = view_parameters
            .section
            .expect("validated section definition");
        let building = model.levels[&marker_level].parameters.building;
        ensure(
            model
                .walls
                .len()
                .saturating_add(model.floors.len())
                .saturating_add(model.columns.len())
                .saturating_add(model.openings.len())
                <= MAX_PLAN_ELEMENTS,
            "section exceeds 10000 model elements",
        )?;
        let walls = model
            .walls
            .values()
            .filter(|wall| model.levels[&wall.parameters.level].parameters.building == building)
            .map(|wall| os_geometry::walls::NativeWall::from_model(model, wall.id()))
            .collect::<Result<Vec<_>>>()?;
        let floors = model
            .floors
            .values()
            .filter(|floor| model.levels[&floor.parameters.level].parameters.building == building)
            .map(|floor| {
                let level = &model.levels[&floor.parameters.level];
                let top = level.parameters.elevation + floor.parameters.top_offset;
                ensure(top.is_finite(), "section floor elevation overflow")?;
                Ok(SectionFloor {
                    entity: floor.id(),
                    boundary: floor.parameters.boundary.clone(),
                    top,
                    thickness: floor.parameters.thickness,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(SectionSnapshot {
            context,
            interfaces: os_geometry::walls::butt_interfaces(model)?,
            plane: os_geometry::section::VerticalSectionPlane {
                origin: section.start,
                direction: Point2::new(
                    section.end.x - section.start.x,
                    section.end.y - section.start.y,
                ),
            },
            walls,
            floors,
        })
    }

    /// Known native straight walls use the documented prism fallback, including
    /// walls authored by the external Wall guest. Unknown payloads are never guessed.
    pub fn native_wall_plan(&self, view: Id) -> Result<PlanDrawing> {
        self.plan_snapshot(view)?.derive()
    }

    /// Read-only composition of already completed/checked service results. No
    /// guest invocation occurs here; missing providers stay explicitly unavailable.
    pub fn plan_with_provider_graphics(
        &self,
        view: Id,
        providers: Vec<os_plugin_host::plan_graphics::PlanGraphics>,
    ) -> Result<ProviderPlanDrawing> {
        self.prepare_provider_plan(view, providers)?.derive()
    }

    /// Validate and copy bounded inputs on the controller, then move the returned
    /// value to the existing drawing worker. Revalidate the completed drawing at
    /// every consumption; edits/unload during derivation never rebase the result.
    pub fn prepare_provider_plan(
        &self,
        view: Id,
        providers: Vec<os_plugin_host::plan_graphics::PlanGraphics>,
    ) -> Result<PreparedProviderPlan> {
        ensure(
            providers.len() <= MAX_PLAN_ELEMENTS,
            "too many plan providers",
        )?;
        let mut lines = BTreeMap::new();
        let mut segment_count = 0usize;
        for provider in &providers {
            let entity = provider.element();
            ensure(
                !lines.contains_key(&entity),
                "duplicate plan provider result for entity",
            )?;
            let source = provider.segments(&self.host, &self.document, view)?;
            segment_count = segment_count.saturating_add(source.len());
            ensure(
                segment_count <= MAX_PLAN_ELEMENTS,
                "plan provider lines exceed 10000 segments",
            )?;
            let mapped = source
                .iter()
                .map(|line| os_render::plan::PlanLine {
                    entity,
                    feature: line.feature,
                    // Service coordinates are already in this checked view plane.
                    // Do not apply the native world-to-plane transform a second time.
                    start: os_core::Point2::new(line.start[0], line.start[1]),
                    end: os_core::Point2::new(line.end[0], line.end[1]),
                    role: match line.role {
                        os_plugin_api::plan::Role::Cut => os_geometry::plan::PlanRole::Cut,
                        os_plugin_api::plan::Role::Projected => {
                            os_geometry::plan::PlanRole::Projected
                        }
                        os_plugin_api::plan::Role::Depth => os_geometry::plan::PlanRole::Depth,
                    },
                })
                .collect();
            lines.insert(entity, mapped);
        }
        Ok(PreparedProviderPlan {
            snapshot: self.plan_snapshot(view)?,
            lines,
            providers,
        })
    }

    pub(crate) fn plan_snapshot(&self, view: Id) -> Result<PlanSnapshot> {
        let context = self.native_plan_context(view)?;
        let model = self.document.model();
        ensure(
            model
                .walls
                .len()
                .saturating_add(model.floors.len())
                .saturating_add(model.columns.len())
                .saturating_add(model.extensions.len())
                .saturating_add(model.grids.len())
                .saturating_add(model.openings.len().saturating_mul(64))
                .saturating_add(model.dimensions.len())
                .saturating_add(model.room_tags.len())
                .saturating_add(model.detail_lines.len())
                .saturating_add(model.room_separation_lines.len())
                <= MAX_PLAN_ELEMENTS,
            "plan exceeds 10000 elements",
        )?;
        let mut walls = Vec::new();
        if context.show_walls {
            for id in model.walls.keys() {
                walls.push(os_geometry::walls::NativeWall::from_model(model, *id)?);
            }
        }
        let unavailable = if context.show_extensions {
            model.extensions.keys().copied().collect()
        } else {
            Vec::new()
        };
        let building = model.levels[&model.views[&view].parameters.level.unwrap()]
            .parameters
            .building;
        let grids = model
            .grids
            .values()
            .filter(|grid| grid.parameters.building == building)
            .map(|grid| os_render::plan::PlanGrid {
                entity: grid.id(),
                name: grid.parameters.name.clone(),
                start: grid.parameters.start,
                end: grid.parameters.end,
            })
            .collect();
        let plan_level = model.views[&view]
            .parameters
            .level
            .ok_or_else(|| os_core::Error::Invalid("floor plan level missing".into()))?;
        let graphics = model.plan_graphics_styles(view);
        let opening_kinds = model
            .openings
            .iter()
            .map(|(id, opening)| Ok((*id, model.resolve_opening(&opening.parameters)?.kind)))
            .collect::<Result<BTreeMap<_, _>>>()?;
        let section_markers = model
            .views
            .values()
            .filter(|candidate| {
                candidate.parameters.kind == ViewKind::Section
                    && candidate.parameters.level == Some(plan_level)
                    && candidate.parameters.section.is_some()
            })
            .map(|candidate| {
                (
                    candidate.id(),
                    candidate
                        .parameters
                        .section
                        .expect("filtered section settings"),
                )
            })
            .collect();
        Ok(PlanSnapshot {
            context,
            graphics,
            opening_kinds,
            wall_ids: model.walls.keys().copied().collect(),
            room_separation_lines: model
                .room_separation_lines
                .values()
                .filter(|line| line.parameters.level == plan_level)
                .map(|line| {
                    Ok(os_render::plan::PlanRoomSeparationLine {
                        entity: line.id(),
                        view,
                        level: plan_level,
                        start: context.basis.world_to_plane(line.parameters.start)?,
                        end: context.basis.world_to_plane(line.parameters.end)?,
                    })
                })
                .collect::<Result<Vec<_>>>()?,
            detail_lines: model
                .detail_lines
                .values()
                .filter(|line| line.parameters.view == view)
                .map(|line| {
                    Ok(os_render::plan::PlanDetailLine {
                        entity: line.id(),
                        view,
                        start: context.basis.world_to_plane(line.parameters.start)?,
                        end: context.basis.world_to_plane(line.parameters.end)?,
                    })
                })
                .collect::<Result<Vec<_>>>()?,
            room_tags: model
                .room_tags
                .values()
                .filter(|tag| tag.parameters.view == view)
                .map(|tag| {
                    let resolved = tag.parameters.resolve(model);
                    Ok(os_render::plan::PlanRoomTag {
                        entity: tag.id(),
                        view,
                        room: tag.parameters.room,
                        anchor: context.basis.world_to_plane(tag.parameters.position)?,
                        label: resolved.as_ref().map_or_else(
                            |_| "Room tag · missing reference".into(),
                            |room| format!("{} · {}", room.parameters.number, room.parameters.name),
                        ),
                        diagnostic: resolved.err().map(str::to_owned),
                    })
                })
                .collect::<Result<Vec<_>>>()?,
            grids,
            walls,
            openings: model
                .openings
                .values()
                .map(|o| Ok((o.id(), model.resolve_opening(&o.parameters)?)))
                .collect::<Result<Vec<_>>>()?,
            room_segments: model.room_boundary_segments(plan_level)?,
            rooms: model
                .rooms
                .values()
                .filter(|room| room.parameters.level == plan_level)
                .map(|room| (room.id(), room.parameters.clone()))
                .collect(),
            dimensions: model
                .dimensions
                .values()
                .filter(|dimension| dimension.parameters.view == view)
                .map(|dimension| (dimension.id(), dimension.parameters.clone()))
                .collect(),
            dimension_walls: model
                .walls
                .values()
                .map(|wall| {
                    (
                        wall.id(),
                        wall.parameters.level,
                        wall.parameters.start,
                        wall.parameters.end,
                    )
                })
                .collect(),
            plan_level,
            columns: model
                .columns
                .values()
                .map(|column| {
                    let elevation = model.levels[&column.parameters.level].parameters.elevation;
                    (column.id(), column.parameters.clone(), elevation)
                })
                .collect(),
            floors: model
                .floors
                .values()
                .map(|floor| {
                    let level = &model.levels[&floor.parameters.level];
                    (
                        floor.id(),
                        floor.parameters.clone(),
                        level.parameters.elevation,
                    )
                })
                .collect(),
            section_markers,
            unavailable,
        })
    }
}

pub(crate) struct PlanSnapshot {
    pub context: PlanContext,
    graphics: Option<os_model::PlanGraphicsStyles>,
    opening_kinds: BTreeMap<Id, os_model::OpeningKind>,
    wall_ids: std::collections::BTreeSet<Id>,
    detail_lines: Vec<os_render::plan::PlanDetailLine>,
    room_separation_lines: Vec<os_render::plan::PlanRoomSeparationLine>,
    room_tags: Vec<os_render::plan::PlanRoomTag>,
    walls: Vec<os_geometry::walls::NativeWall>,
    openings: Vec<(Id, os_model::ResolvedOpening)>,
    room_segments: Vec<os_geometry::rooms::BoundarySegment>,
    rooms: Vec<(Id, os_model::RoomParams)>,
    dimensions: Vec<(Id, DimensionParams)>,
    dimension_walls: Vec<(Id, Id, os_core::Point2, os_core::Point2)>,
    plan_level: Id,
    floors: Vec<(Id, os_model::FloorParams, f64)>,
    columns: Vec<(Id, os_model::ColumnParams, f64)>,
    section_markers: Vec<(Id, os_model::SectionViewSettings)>,
    unavailable: Vec<Id>,
    grids: Vec<os_render::plan::PlanGrid>,
}
impl PlanSnapshot {
    pub fn derive(self) -> Result<PlanDrawing> {
        self.derive_with_provider_lines(BTreeMap::new())
    }
    fn derive_with_provider_lines(
        self,
        lines: BTreeMap<Id, Vec<os_render::plan::PlanLine>>,
    ) -> Result<PlanDrawing> {
        let (room_faces, room_items, room_diagnostic) =
            derive_room_graphics(self.context, &self.room_segments, &self.rooms)?;
        let dimension_items = derive_dimension_graphics(
            self.context,
            self.plan_level,
            &self.dimension_walls,
            &self.dimensions,
        )?;
        let angular_items = self
            .dimensions
            .iter()
            .filter(|(_, p)| p.layout == os_model::DimensionLayout::Angular)
            .map(|(id, p)| {
                let resolved = p.resolve_angular_with(|wall| {
                    let (_, level, start, end) = self
                        .dimension_walls
                        .iter()
                        .find(|(id, _, _, _)| *id == wall)
                        .ok_or(os_model::DimensionDiagnostic::MissingWall)?;
                    if *level != self.plan_level {
                        return Err(os_model::DimensionDiagnostic::WrongLevel);
                    }
                    Ok((*start, *end))
                });
                angular_graphic(self.context, *id, p.orphan_hint, resolved)
            })
            .collect::<Result<Vec<_>>>()?;
        let column_items = self
            .columns
            .iter()
            .map(|(id, parameters, elevation)| {
                Ok((
                    *id,
                    os_geometry::columns::column_solid(parameters, *elevation)?,
                    os_geometry::SurfaceIdentity {
                        layer: None,
                        material: parameters.material,
                    },
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        let floor_triangle_budget =
            self.floors
                .iter()
                .fold(0usize, |total, (_, parameters, level)| {
                    let top = level + parameters.top_offset;
                    let bottom = top - parameters.thickness;
                    if top > self.context.range.depth && bottom < self.context.range.top {
                        total.saturating_add(parameters.boundary.len().saturating_sub(2))
                    } else {
                        total
                    }
                });
        os_core::ensure(
            floor_triangle_budget <= os_render::plan::MAX_PLAN_FLOOR_TRIANGLES,
            "floor plan fills exceed the 50000 triangle limit",
        )?;
        let floor_items = self
            .floors
            .iter()
            .filter_map(|(entity, parameters, level_elevation)| {
                let top = level_elevation + parameters.top_offset;
                let bottom = top - parameters.thickness;
                (top > self.context.range.depth && bottom < self.context.range.top)
                    .then_some((*entity, parameters, top))
            })
            .map(|(entity, parameters, _)| {
                let boundary = parameters
                    .boundary
                    .iter()
                    .map(|point| self.context.basis.world_to_plane(*point))
                    .collect::<Result<Vec<_>>>()?;
                let triangles = os_geometry::floors::triangulate_floor(&boundary)?;
                Ok(os_render::plan::PlanFloorItem {
                    entity,
                    area_m2: os_geometry::floors::signed_area(&boundary).abs(),
                    boundary,
                    triangles,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let mut solids = BTreeMap::new();
        let mut segments = Vec::new();
        let mut native_lines = BTreeMap::new();
        let mut seams = BTreeMap::new();
        for wall in self.walls {
            let id = wall.entity;
            let parameters = &wall.parameters;
            let elevation = wall.elevation;
            seams.insert(
                id,
                wall.seams()
                    .into_iter()
                    .map(|(a, b)| {
                        Ok((
                            self.context.basis.world_to_plane(a)?,
                            self.context.basis.world_to_plane(b)?,
                        ))
                    })
                    .collect::<Result<Vec<_>>>()?,
            );
            let mut cells = Vec::new();
            for (layer, part) in wall.layer_plan_footprints(
                self.context.range,
                self.context.basis,
                self.context.crop,
            )? {
                let surface = os_geometry::SurfaceIdentity {
                    layer: layer.id,
                    material: layer.material,
                };
                cells.extend(part.into_iter().map(|footprint| (surface, footprint)));
            }
            solids.insert(id, cells);
            for (opening_id, opening) in self.openings.iter().filter(|(_, p)| p.host == id) {
                native_lines.insert(
                    *opening_id,
                    crate::opening_tools::plan_symbol(
                        *opening_id,
                        opening,
                        parameters,
                        elevation,
                        self.context,
                    )?,
                );
            }
            segments.push(os_render::snapping::SnapSegment {
                entity: id,
                feature: 0,
                start: parameters.start,
                end: parameters.end,
            });
        }
        for (entity, settings) in self.section_markers {
            native_lines.insert(
                entity,
                section_marker_lines(entity, settings, self.context.basis)?,
            );
        }
        let mut unavailable = self.unavailable;
        unavailable.extend(native_lines.keys().copied());
        let drawing = PlanDrawing::from_layered_footprints(self.context, &solids, unavailable)?
            .without_wall_seams(&seams)?;
        let visible: std::collections::BTreeSet<_> = drawing
            .items(self.context)?
            .iter()
            .map(|item| item.entity)
            .collect();
        segments.retain(|segment| visible.contains(&segment.entity));
        for segment in &mut segments {
            segment.start = self.context.basis.world_to_plane(segment.start)?;
            segment.end = self.context.basis.world_to_plane(segment.end)?;
        }
        let mut grids = self.grids;
        for grid in &mut grids {
            grid.start = self.context.basis.world_to_plane(grid.start)?;
            grid.end = self.context.basis.world_to_plane(grid.end)?;
        }
        let mut drawing = drawing
            .with_grids(grids)?
            .with_native_lines(native_lines)?
            .with_provider_lines(lines)?
            .with_detail_lines(self.detail_lines)?
            .with_room_separation_lines(self.plan_level, self.room_separation_lines)?
            .with_snap_segments(segments)?
            .with_room_faces(room_faces)?
            .with_rooms(room_items)?
            .with_dimensions(dimension_items)?
            .with_floors(floor_items)?
            .with_columns(&column_items)?
            .with_angular_dimensions(angular_items)?
            .with_room_tags(self.room_tags)?
            .with_room_boundary_diagnostic(room_diagnostic)?;
        if let Some(styles) = self.graphics {
            use os_geometry::plan::PlanRole;
            use os_render::plan::{PlanElementAppearance, PlanStroke};
            let convert = |style: os_model::PlanStrokeStyle| PlanStroke {
                color: style.color,
                weight_mm: style.weight_mm,
                dashed: style.pattern == os_model::PlanLinePattern::Dashed,
            };
            let mut appearances = BTreeMap::new();
            for item in drawing.items(self.context)? {
                if !self.wall_ids.contains(&item.entity) {
                    continue;
                }
                let entry = appearances
                    .entry(item.entity)
                    .or_insert_with(PlanElementAppearance::default);
                match item.footprint.role {
                    PlanRole::Cut => entry.cut = Some(convert(styles.walls.cut)),
                    PlanRole::Projected => entry.projected = Some(convert(styles.walls.projected)),
                    PlanRole::Depth => {}
                }
            }
            for line in drawing.provider_lines(self.context)? {
                if !drawing.is_native_line(line.entity) {
                    continue;
                }
                let Some(kind) = self.opening_kinds.get(&line.entity) else {
                    continue;
                };
                let category = match kind {
                    os_model::OpeningKind::Door => styles.doors,
                    os_model::OpeningKind::Window => styles.windows,
                };
                let entry = appearances
                    .entry(line.entity)
                    .or_insert_with(PlanElementAppearance::default);
                match line.role {
                    PlanRole::Cut => entry.cut = Some(convert(category.cut)),
                    PlanRole::Projected => entry.projected = Some(convert(category.projected)),
                    PlanRole::Depth => {}
                }
            }
            for floor in drawing.floors(self.context)? {
                appearances.insert(
                    floor.entity,
                    PlanElementAppearance {
                        cut: None,
                        projected: Some(convert(styles.slabs.projected)),
                    },
                );
            }
            drawing = drawing.with_appearances(appearances)?;
        }
        Ok(drawing)
    }
}

fn section_marker_lines(
    entity: Id,
    settings: os_model::SectionViewSettings,
    basis: HorizontalBasis,
) -> Result<Vec<os_render::plan::PlanLine>> {
    settings.validate()?;
    let length = settings.length()?;
    let dx = (settings.end.x - settings.start.x) / length;
    let dy = (settings.end.y - settings.start.y) / length;
    let arrow_length = length.min(0.35);
    let arrow_half_width = arrow_length * 0.45;
    let root = Point2::new(
        settings.end.x - dx * arrow_length,
        settings.end.y - dy * arrow_length,
    );
    let normal = Point2::new(-dy * arrow_half_width, dx * arrow_half_width);
    let left = Point2::new(root.x + normal.x, root.y + normal.y);
    let right = Point2::new(root.x - normal.x, root.y - normal.y);
    let [start, end, left, right] = [settings.start, settings.end, left, right]
        .map(|point| basis.world_to_plane(point))
        .into_iter()
        .collect::<Result<Vec<_>>>()?
        .try_into()
        .map_err(|_| os_core::Error::Invalid("invalid section marker points".into()))?;
    Ok(vec![
        os_render::plan::PlanLine {
            entity,
            feature: 0,
            start,
            end,
            role: PlanRole::Projected,
        },
        os_render::plan::PlanLine {
            entity,
            feature: 1,
            start: end,
            end: left,
            role: PlanRole::Projected,
        },
        os_render::plan::PlanLine {
            entity,
            feature: 2,
            start: end,
            end: right,
            role: PlanRole::Projected,
        },
    ])
}

struct SectionFloor {
    entity: Id,
    boundary: Vec<Point2>,
    top: f64,
    thickness: f64,
}

pub(crate) struct SectionSnapshot {
    context: PlanContext,
    interfaces: Vec<os_geometry::walls::ButtInterface>,
    plane: os_geometry::section::VerticalSectionPlane,
    walls: Vec<os_geometry::walls::NativeWall>,
    floors: Vec<SectionFloor>,
}

impl SectionSnapshot {
    pub(crate) fn derive(self) -> Result<PlanDrawing> {
        use os_geometry::section::SECTION_TOLERANCE;
        use os_geometry::{plan::PlanRole, section::vertical_section};
        use os_render::plan::PlanLine;

        let mut segments: BTreeMap<Id, Vec<(Point2, Point2)>> = BTreeMap::new();
        let mut layer_lines: BTreeMap<Id, Vec<PlanLine>> = BTreeMap::new();
        let mut line_surfaces = BTreeMap::new();
        let mut raw_segment_count = 0usize;
        for wall in self.walls {
            if wall.layers.iter().any(|layer| layer.id.is_some()) {
                segments.entry(wall.entity).or_default();
                for (layer, cells) in wall.layer_mesh_regions()? {
                    let mut by_wall: BTreeMap<Id, Vec<(Point2, Point2)>> = self
                        .interfaces
                        .iter()
                        .flat_map(|i| i.members)
                        .map(|id| (id, Vec::new()))
                        .collect();
                    for cell in cells {
                        add_section_contours(
                            by_wall.entry(wall.entity).or_default(),
                            vertical_section(&cell, self.plane)?,
                            &mut raw_segment_count,
                        )?;
                    }
                    os_geometry::walls::remove_section_interfaces(
                        &mut by_wall,
                        &self.interfaces,
                        self.plane,
                    )?;
                    let edges =
                        reduce_section_edges(by_wall.remove(&wall.entity).unwrap_or_default())?;
                    let lines = layer_lines.entry(wall.entity).or_default();
                    for edge in edges {
                        let feature = lines.len() as u32;
                        line_surfaces.insert(
                            (wall.entity, feature),
                            os_geometry::SurfaceIdentity {
                                layer: layer.id,
                                material: layer.material,
                            },
                        );
                        lines.push(PlanLine {
                            entity: wall.entity,
                            feature,
                            start: edge.start,
                            end: edge.end,
                            role: PlanRole::Cut,
                        });
                    }
                    ensure(
                        lines.len() <= 256,
                        "section wall exceeds 256 layer cut segments",
                    )?;
                }
                continue;
            }
            for (_, cells) in wall.layer_mesh_regions()? {
                for mesh in &cells {
                    add_section_contours(
                        segments.entry(wall.entity).or_default(),
                        vertical_section(mesh, self.plane)?,
                        &mut raw_segment_count,
                    )?;
                }
            }
        }
        for floor in self.floors {
            let mesh =
                os_geometry::floors::extrude_floor(&floor.boundary, floor.top, floor.thickness)?;
            add_section_contours(
                segments.entry(floor.entity).or_default(),
                vertical_section(&mesh, self.plane)?,
                &mut raw_segment_count,
            )?;
        }

        os_geometry::walls::remove_section_interfaces(&mut segments, &self.interfaces, self.plane)?;
        let mut lines = layer_lines;
        let mut line_count = lines.values().map(Vec::len).sum::<usize>();
        ensure(
            line_count <= MAX_PLAN_ELEMENTS,
            "section exceeds 10000 layer cut segments",
        )?;
        for (entity, raw_edges) in segments {
            let retained = reduce_section_edges(raw_edges)?;
            ensure(
                retained.len() <= 256,
                "section element exceeds 256 cut segments",
            )?;
            let mut features = Vec::with_capacity(retained.len());
            for (feature, segment) in retained.into_iter().enumerate() {
                line_count = line_count.saturating_add(1);
                ensure(
                    line_count <= MAX_PLAN_ELEMENTS,
                    "section exceeds 10000 cut segments",
                )?;
                ensure(
                    segment.start.distance(segment.end) > SECTION_TOLERANCE,
                    "section contains a sub-tolerance edge",
                )?;
                features.push(PlanLine {
                    entity,
                    feature: feature as u32,
                    start: segment.start,
                    end: segment.end,
                    role: PlanRole::Cut,
                });
            }
            if !features.is_empty() {
                lines.insert(entity, features);
            }
        }
        let unavailable = lines.keys().copied().collect();
        PlanDrawing::from_prisms(self.context, &BTreeMap::new(), unavailable)?
            .with_native_lines(lines)
            .map(|drawing| drawing.with_line_surfaces(line_surfaces))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct SectionLineKey {
    direction_x: i64,
    direction_y: i64,
    offset: i64,
}

struct SectionSegment {
    start: Point2,
    end: Point2,
}

struct SectionLineGroup {
    direction: Point2,
    normal: Point2,
    offset: f64,
    intervals: Vec<(f64, f64)>,
}

fn quantized(value: f64, quantum: f64) -> Result<i64> {
    let value = (value / quantum).round();
    ensure(
        value.is_finite() && value.abs() < i64::MAX as f64,
        "section line key overflow",
    )?;
    Ok(value as i64)
}

/// Toggle coincident boundary intervals after splitting T-junctions implicitly
/// at every interval endpoint. This removes internal seams between adjacent
/// wall cells around doors/windows without quadratic point-on-segment scans.
fn reduce_section_edges(edges: Vec<(Point2, Point2)>) -> Result<Vec<SectionSegment>> {
    use os_geometry::section::SECTION_TOLERANCE;

    let mut groups: BTreeMap<SectionLineKey, SectionLineGroup> = BTreeMap::new();
    for (start, end) in edges {
        ensure(
            start.is_finite() && end.is_finite(),
            "section edge is not finite",
        )?;
        let delta = Point2::new(end.x - start.x, end.y - start.y);
        let length = delta.x.hypot(delta.y);
        ensure(
            length.is_finite() && length > SECTION_TOLERANCE,
            "section edge is below topology tolerance",
        )?;
        let mut direction = Point2::new(delta.x / length, delta.y / length);
        if direction.x < 0.0 || (direction.x == 0.0 && direction.y < 0.0) {
            direction = Point2::new(-direction.x, -direction.y);
        }
        let normal = Point2::new(-direction.y, direction.x);
        let offset = normal.x * start.x + normal.y * start.y;
        let key = SectionLineKey {
            direction_x: quantized(direction.x, 1e-14)?,
            direction_y: quantized(direction.y, 1e-14)?,
            offset: quantized(offset, SECTION_TOLERANCE)?,
        };
        let group = groups.entry(key).or_insert_with(|| SectionLineGroup {
            direction,
            normal,
            offset,
            intervals: Vec::new(),
        });
        ensure(
            (group.normal.x * start.x + group.normal.y * start.y - group.offset).abs()
                <= SECTION_TOLERANCE * 2.0
                && (group.normal.x * end.x + group.normal.y * end.y - group.offset).abs()
                    <= SECTION_TOLERANCE * 2.0,
            "section edges have ambiguous near-collinear topology",
        )?;
        let first = direction.x * start.x + direction.y * start.y;
        let second = direction.x * end.x + direction.y * end.y;
        group.intervals.push((first.min(second), first.max(second)));
    }

    let mut result = Vec::new();
    for group in groups.into_values() {
        let mut events = Vec::with_capacity(group.intervals.len() * 2);
        for (start, end) in group.intervals {
            events.push((start, 1i32));
            events.push((end, -1i32));
        }
        events.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        let mut coverage = 0i32;
        let mut previous = None;
        let mut index = 0;
        while index < events.len() {
            let anchor = events[index].0;
            if let Some(start) = previous
                && anchor - start > SECTION_TOLERANCE
                && coverage % 2 != 0
            {
                let point = |distance: f64| {
                    Point2::new(
                        group.direction.x * distance + group.normal.x * group.offset,
                        group.direction.y * distance + group.normal.y * group.offset,
                    )
                };
                result.push(SectionSegment {
                    start: point(start),
                    end: point(anchor),
                });
                ensure(
                    result.len() <= MAX_PLAN_ELEMENTS,
                    "section exceeds 10000 retained cut segments",
                )?;
            }
            let mut delta = 0;
            while index < events.len() && (events[index].0 - anchor).abs() <= SECTION_TOLERANCE {
                delta += events[index].1;
                index += 1;
            }
            coverage += delta;
            previous = Some(anchor);
        }
        ensure(coverage == 0, "unbalanced section boundary intervals")?;
    }
    Ok(result)
}

fn add_section_contours(
    output: &mut Vec<(Point2, Point2)>,
    contours: Vec<os_geometry::section::CutContour>,
    raw_segment_count: &mut usize,
) -> Result<()> {
    use os_geometry::section::SECTION_TOLERANCE;

    for contour in contours {
        let points = contour.points;
        for (&start, &end) in points
            .iter()
            .zip(points.iter().cycle().skip(1))
            .take(points.len())
        {
            *raw_segment_count = raw_segment_count
                .checked_add(1)
                .ok_or_else(|| os_core::Error::Invalid("section segment count overflow".into()))?;
            ensure(
                *raw_segment_count <= MAX_PLAN_ELEMENTS,
                "section exceeds 10000 raw cut segments",
            )?;
            ensure(
                start.distance(end) > SECTION_TOLERANCE,
                "section edge is below topology tolerance",
            )?;
            output.push((start, end));
        }
    }
    Ok(())
}

fn derive_dimension_graphics(
    context: PlanContext,
    plan_level: Id,
    walls: &[(Id, Id, os_core::Point2, os_core::Point2)],
    dimensions: &[(Id, DimensionParams)],
) -> Result<Vec<PlanDimensionItem>> {
    let walls: BTreeMap<_, _> = walls
        .iter()
        .map(|(id, level, start, end)| (*id, (*level, *start, *end)))
        .collect();
    let endpoint = |reference: &os_model::DimensionReference| {
        walls.get(&reference.wall).and_then(|(level, start, end)| {
            (*level == plan_level).then_some(match reference.endpoint {
                DimensionEndpoint::Start => *start,
                DimensionEndpoint::End => *end,
            })
        })
    };
    let mut output = Vec::new();
    let mut count = 0usize;
    for (entity, parameters) in dimensions {
        if parameters.layout == os_model::DimensionLayout::Angular {
            continue;
        }
        let orphan_hint = context.basis.world_to_plane(parameters.orphan_hint)?;
        let resolved = parameters.resolve_anchors_with(|reference| {
            endpoint(&reference).ok_or(if walls.contains_key(&reference.wall) {
                os_model::DimensionDiagnostic::WrongLevel
            } else {
                os_model::DimensionDiagnostic::MissingWall
            })
        });
        let mut item = PlanDimensionItem {
            spans: Vec::new(),
            shared_start_witness: false,
            entity: *entity,
            witness_start: orphan_hint,
            witness_end: orphan_hint,
            line_start: orphan_hint,
            line_end: orphan_hint,
            value_m: None,
            orphan_hint,
            diagnostic: None,
        };
        match resolved {
            Err(reason) => item.diagnostic = Some(format!("{reason:?}")),
            Ok(points) => {
                let length = points[0].distance(points[1]);
                let normal = Point2::new(
                    (points[0].y - points[1].y) / length,
                    (points[1].x - points[0].x) / length,
                );
                let mut spans = Vec::new();
                for i in 1..points.len() {
                    let first = if parameters.layout == os_model::DimensionLayout::Chain {
                        points[i - 1]
                    } else {
                        points[0]
                    };
                    let second = points[i];
                    let offset = parameters.offset_m
                        + if parameters.layout == os_model::DimensionLayout::Baseline {
                            (if parameters.offset_m < 0.0 { -1.0 } else { 1.0 })
                                * (i - 1) as f64
                                * parameters.baseline_spacing_m
                        } else {
                            0.0
                        };
                    spans.push(PlanDimensionItem {
                        spans: Vec::new(),
                        shared_start_witness: parameters.layout == os_model::DimensionLayout::Chain
                            && i > 1,
                        entity: *entity,
                        witness_start: context.basis.world_to_plane(first)?,
                        witness_end: context.basis.world_to_plane(second)?,
                        line_start: context.basis.world_to_plane(Point2::new(
                            first.x + normal.x * offset,
                            first.y + normal.y * offset,
                        ))?,
                        line_end: context.basis.world_to_plane(Point2::new(
                            second.x + normal.x * offset,
                            second.y + normal.y * offset,
                        ))?,
                        value_m: Some(first.distance(second)),
                        orphan_hint,
                        diagnostic: None,
                    });
                }
                item = spans.remove(0);
                item.spans = spans;
            }
        }
        count = count.saturating_add(item.graphic_count());
        os_core::ensure(
            count <= MAX_PLAN_ELEMENTS,
            "dimension graphics exceed plan budget",
        )?;
        output.push(item);
    }
    Ok(output)
}

pub(super) fn angular_graphic(
    context: PlanContext,
    entity: Id,
    hint: Point2,
    resolved: std::result::Result<
        os_model::ResolvedAngularDimension,
        os_model::DimensionDiagnostic,
    >,
) -> Result<os_render::plan::PlanAngularDimension> {
    let hint = context.basis.world_to_plane(hint)?;
    let mut graphic = os_render::plan::PlanAngularDimension {
        entity,
        center: hint,
        radius: 1.0,
        start_radians: 0.0,
        sweep_radians: std::f64::consts::FRAC_PI_2,
        orphan_hint: hint,
        diagnostic: None,
    };
    match resolved {
        Err(reason) => graphic.diagnostic = Some(format!("{reason:?}")),
        Ok(angle) => {
            graphic.center = context.basis.world_to_plane(angle.center)?;
            let start = context.basis.world_to_plane(angle.point(0.0))?;
            let middle = context.basis.world_to_plane(angle.point(0.5))?;
            graphic.radius = angle.radius;
            graphic.start_radians = (start.y - graphic.center.y).atan2(start.x - graphic.center.x);
            let midpoint_angle = (middle.y - graphic.center.y).atan2(middle.x - graphic.center.x);
            graphic.sweep_radians = 2.0
                * ((midpoint_angle - graphic.start_radians + std::f64::consts::PI)
                    .rem_euclid(std::f64::consts::TAU)
                    - std::f64::consts::PI);
        }
    }
    graphic.validate()?;
    Ok(graphic)
}

fn derive_room_graphics(
    context: PlanContext,
    segments: &[os_geometry::rooms::BoundarySegment],
    rooms: &[(Id, os_model::RoomParams)],
) -> Result<(Vec<PlanRoomFace>, Vec<PlanRoomItem>, Option<String>)> {
    use os_geometry::rooms::{FaceKey, SeedDiagnostic};
    let faces = match os_geometry::rooms::derive_faces(segments) {
        Ok(faces) => faces,
        Err(error) => {
            let message = format!(
                "Room boundary derivation failed: {}",
                match error {
                    os_geometry::rooms::BoundaryDiagnostic::InvalidSegment(_) => {
                        "a boundary segment is invalid"
                    }
                    os_geometry::rooms::BoundaryDiagnostic::DuplicateBoundary(_) => {
                        "a boundary identity is duplicated"
                    }
                    os_geometry::rooms::BoundaryDiagnostic::OverlappingBoundaries(_, _) => {
                        "boundary segments overlap"
                    }
                    os_geometry::rooms::BoundaryDiagnostic::AmbiguousTopology => {
                        "the boundary graph is ambiguous"
                    }
                    os_geometry::rooms::BoundaryDiagnostic::NestedLoops => {
                        "nested loops are unsupported"
                    }
                    os_geometry::rooms::BoundaryDiagnostic::ResourceLimit => {
                        "the boundary graph exceeds the safe processing limit"
                    }
                    os_geometry::rooms::BoundaryDiagnostic::OpenChains(_) => {
                        "the boundary graph has open chains"
                    }
                }
            );
            return Ok((
                Vec::new(),
                rooms
                    .iter()
                    .map(|(id, room)| {
                        Ok(PlanRoomItem {
                            entity: *id,
                            number: room.number.clone(),
                            name: room.name.clone(),
                            seed: context.basis.world_to_plane(room.seed)?,
                            boundary: Vec::new(),
                            area_m2: 0.0,
                            boundary_signature: room.boundary_signature.clone(),
                            diagnostic: Some(message.clone()),
                        })
                    })
                    .collect::<Result<Vec<_>>>()?,
                Some(message),
            ));
        }
    };
    let plan_faces = faces
        .faces
        .iter()
        .map(|face| {
            Ok(PlanRoomFace {
                boundary: face
                    .boundary
                    .iter()
                    .map(|point| context.basis.world_to_plane(*point))
                    .collect::<Result<Vec<_>>>()?,
                area_m2: face.area_m2,
                boundary_signature: face.key.as_signature().to_vec(),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let diagnostics = faces
        .diagnostics
        .first()
        .map(|_| "Some boundary chains are open; enclosed rooms remain available".to_owned());
    let items = rooms
        .iter()
        .map(|(id, room)| {
            let seed = context.basis.world_to_plane(room.seed)?;
            let resolved = FaceKey::from_signature(&room.boundary_signature)
                .ok_or(SeedDiagnostic::FaceChanged)
                .and_then(|key| faces.resolve_seed(room.seed, &key));
            match resolved {
                Ok(face) => {
                    let boundary = face
                        .boundary
                        .iter()
                        .map(|point| context.basis.world_to_plane(*point))
                        .collect::<Result<Vec<_>>>()?;
                    Ok(PlanRoomItem {
                        entity: *id,
                        number: room.number.clone(),
                        name: room.name.clone(),
                        seed,
                        boundary,
                        area_m2: face.area_m2,
                        boundary_signature: room.boundary_signature.clone(),
                        diagnostic: None,
                    })
                }
                Err(diagnostic) => Ok(PlanRoomItem {
                    entity: *id,
                    number: room.number.clone(),
                    name: room.name.clone(),
                    seed,
                    boundary: Vec::new(),
                    area_m2: 0.0,
                    boundary_signature: room.boundary_signature.clone(),
                    diagnostic: Some(match diagnostic {
                        SeedDiagnostic::InvalidSeed => "Room seed is invalid".into(),
                        SeedDiagnostic::OnBoundary => "Room seed lies on a room boundary".into(),
                        SeedDiagnostic::NotEnclosed => "Room is not enclosed".into(),
                        SeedDiagnostic::AmbiguousBoundary => {
                            "Room lies in overlapping boundaries".into()
                        }
                        SeedDiagnostic::FaceChanged => {
                            "Room boundary changed; repair or replace this room".into()
                        }
                    }),
                }),
            }
        })
        .collect::<Result<Vec<_>>>()?;
    Ok((plan_faces, items, diagnostics))
}
