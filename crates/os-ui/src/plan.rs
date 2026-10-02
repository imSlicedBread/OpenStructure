//! Persisted named-plan commands and read-only native-wall plan derivation.
use crate::Editor;
use os_core::{Id, Point2, Result, ensure};
use os_document::Command;
use os_geometry::plan::{HorizontalBasis, PlanCrop, PlanRange, PlanRole};
use os_model::{DimensionParams, PlanSettings, View, ViewKind, ViewParams};
use os_render::plan::{
    MAX_PLAN_ELEMENTS, PlanContext, PlanDimensionItem, PlanDrawing, PlanRoomFace, PlanRoomItem,
};
use std::collections::BTreeMap;

/// Shared by frozen drawing snapshots and placement previews. Never persisted.
pub(crate) fn opening_tag_graphic(
    model: &os_model::Model,
    entity: Id,
    parameters: &os_model::OpeningTagParams,
    context: PlanContext,
) -> Result<os_render::plan::PlanOpeningTag> {
    let (label, diagnostic) = parameters.label(model);
    let leader_source = parameters.resolve(model).ok().and_then(|opening| {
        let host = &model.walls.get(&opening.host)?.parameters;
        opening.validate_host(host).ok()?;
        let distance = opening.offset + opening.width * 0.5;
        context.basis.world_to_plane(host.path.point(distance)).ok()
    });
    Ok(os_render::plan::PlanOpeningTag {
        entity,
        view: parameters.view,
        opening: parameters.opening,
        anchor: context.basis.world_to_plane(parameters.position)?,
        leader_source,
        label,
        diagnostic,
    })
}

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
    Section(Box<PreparedSectionSnapshot>),
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

/// Build the same checked stair plan graphic used after commit. A placement
/// draft may supply its own UUID without adding an entity to the document.
pub fn project_stair_for_plan(
    entity: Id,
    parameters: &os_model::StairParams,
    lower_z: f64,
    upper_z: f64,
    context: PlanContext,
) -> Result<os_render::plan::PlanStairItem> {
    parameters.dimensions(lower_z, upper_z)?;
    os_render::plan::PlanStairItem::derive(
        entity,
        parameters.start,
        parameters.end,
        parameters.width,
        parameters.riser_count as usize,
        parameters.structural_thickness,
        parameters.material,
        lower_z,
        upper_z,
        context,
    )
}

impl Editor {
    pub fn create_floor_plan(&mut self, name: &str, level: Id) -> Result<Id> {
        let mut view = View::new("core.view", ViewParams::floor_plan(name, level));
        view.parameters.plan.as_mut().unwrap().target_phase = self.document.model().latest_phase();
        let id = view.id();
        self.command("Create floor plan", Command::AddView(view))?;
        Ok(id)
    }

    pub fn create_reflected_ceiling_plan(&mut self, name: &str, level: Id) -> Result<Id> {
        let mut view = View::new("core.view", ViewParams::reflected_ceiling_plan(name, level));
        view.parameters.plan.as_mut().unwrap().target_phase = self.document.model().latest_phase();
        let id = view.id();
        self.command("Create reflected ceiling plan", Command::AddView(view))?;
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
        let range = settings
            .range
            .at_level_for_view(settings.view_type, level.parameters.elevation)?;
        let range = PlanRange {
            top: range.top,
            cut: range.cut,
            bottom: range.bottom,
            depth: range.depth,
        };
        Ok(PlanContext {
            session_id: self.document.session_id(),
            model_revision: self.document.revision(),
            view_id: view.id(),
            settings_revision: view.parameters.settings_revision,
            basis: HorizontalBasis {
                origin: settings.basis.origin,
                rotation: settings.basis.rotation,
            },
            range,
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

    pub(crate) fn section_snapshot(&self, view: Id) -> Result<PreparedSectionSnapshot> {
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
                .saturating_add(model.ceilings.len())
                .saturating_add(model.columns.len())
                .saturating_add(model.casework.len())
                .saturating_add(model.stairs.len())
                .saturating_add(model.ramps.len())
                .saturating_add(model.railings.len())
                .saturating_add(model.openings.len())
                .saturating_add(model.curtain_systems.len())
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
                    holes: floor.parameters.holes.clone(),
                    top,
                    thickness: floor.parameters.thickness,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let mut ceiling_resolver = os_geometry::ceilings::CeilingResolver::default();
        let ceilings = model
            .ceilings
            .values()
            .filter(|ceiling| {
                model.levels[&ceiling.parameters.level].parameters.building == building
            })
            .map(|ceiling| {
                let parameters =
                    ceiling_resolver.effective_parameters(model, &ceiling.parameters)?;
                let Some(parameters) = parameters else {
                    return Ok(None);
                };
                let elevation = model.levels[&parameters.level].parameters.elevation;
                ensure(elevation.is_finite(), "section ceiling level overflow")?;
                Ok(Some((ceiling.id(), parameters, elevation)))
            })
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect();
        let stairs = model
            .stairs
            .values()
            .filter(|stair| {
                model.levels[&stair.parameters.lower_level]
                    .parameters
                    .building
                    == building
            })
            .map(|stair| {
                let lower = model.levels[&stair.parameters.lower_level]
                    .parameters
                    .elevation;
                let upper = model.levels[&stair.parameters.upper_level]
                    .parameters
                    .elevation;
                (stair.id(), stair.parameters.clone(), lower, upper)
            })
            .collect();
        let ramps = model
            .ramps
            .values()
            .filter(|ramp| {
                model.levels[&ramp.parameters.lower_level]
                    .parameters
                    .building
                    == building
            })
            .map(|ramp| {
                let lower = model.levels[&ramp.parameters.lower_level]
                    .parameters
                    .elevation;
                let upper = model.levels[&ramp.parameters.upper_level]
                    .parameters
                    .elevation;
                (ramp.id(), ramp.parameters.clone(), lower, upper)
            })
            .collect();
        let curtain_component_count =
            model
                .curtain_systems
                .values()
                .fold(0usize, |count, curtain| {
                    count
                        .saturating_add(curtain.parameters.panels.len())
                        .saturating_add(curtain.parameters.mullions.len())
                });
        ensure(
            curtain_component_count <= MAX_PLAN_ELEMENTS,
            "section exceeds 10000 curtain components",
        )?;
        let curtains = model
            .curtain_systems
            .values()
            .filter(|curtain| {
                model.levels[&curtain.parameters.level].parameters.building == building
            })
            .map(|curtain| {
                Ok((
                    curtain.id(),
                    os_geometry::curtain_systems::curtain_geometry(&curtain.parameters, model)?
                        .components,
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        let mut railing_members = BTreeMap::new();
        let mut member_count = 0usize;
        for railing in model.railings.values().filter(|railing| {
            let stair = &model.stairs[&railing.parameters.stair].parameters;
            model.levels[&stair.lower_level].parameters.building == building
        }) {
            let geometry = os_geometry::railings::railing_geometry(&railing.parameters, model)?;
            member_count = member_count.saturating_add(geometry.members.len());
            ensure(
                member_count <= MAX_PLAN_ELEMENTS,
                "section exceeds 10000 railing members",
            )?;
            railing_members.insert(railing.id(), geometry.members);
        }
        Ok(PreparedSectionSnapshot {
            railing_members,
            source: SectionSnapshot {
                roofs: model
                    .roofs
                    .values()
                    .filter(|roof| {
                        model.levels[&roof.parameters.level].parameters.building == building
                    })
                    .map(|roof| {
                        (
                            roof.id(),
                            roof.parameters.clone(),
                            model.levels[&roof.parameters.level].parameters.elevation,
                        )
                    })
                    .collect(),
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
                ceilings,
                stairs,
                ramps,
                curtains,
            },
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
        let settings = model.views[&view].parameters.plan.expect("validated plan");
        ensure(
            model
                .walls
                .len()
                .saturating_add(model.openings.len())
                .saturating_add(model.floors.len())
                .saturating_add(model.stairs.len())
                .saturating_add(model.ramps.len())
                .saturating_add(model.railings.len())
                .saturating_add(model.roofs.len())
                .saturating_add(model.ceilings.len())
                .saturating_add(model.columns.len())
                .saturating_add(model.casework.len())
                .saturating_add(model.rooms.len())
                .saturating_add(model.room_separation_lines.len())
                .saturating_add(model.curtain_systems.len())
                <= MAX_PLAN_ELEMENTS,
            "plan phase inputs exceed 10000 elements",
        )?;
        let target = settings
            .target_phase
            .or_else(|| model.latest_phase())
            .ok_or_else(|| os_core::Error::Invalid("plan phase missing".into()))?;
        let phase_statuses = model
            .walls
            .keys()
            .chain(model.openings.keys())
            .chain(model.floors.keys())
            .chain(model.stairs.keys())
            .chain(model.ramps.keys())
            .chain(model.railings.keys())
            .chain(model.roofs.keys())
            .chain(model.ceilings.keys())
            .chain(model.columns.keys())
            .chain(model.casework.keys())
            .chain(model.rooms.keys())
            .chain(model.room_separation_lines.keys())
            .chain(model.curtain_systems.keys())
            .map(|id| Ok((*id, model.phase_status(*id, target)?)))
            .collect::<Result<BTreeMap<_, _>>>()?;
        let mut excluded: std::collections::BTreeSet<_> = phase_statuses
            .iter()
            .filter(|(_, status)| !settings.phase_filter.includes(**status))
            .map(|(id, _)| *id)
            .collect();
        for opening in model.openings.values() {
            if !context.show_walls || excluded.contains(&opening.parameters.host) {
                excluded.insert(opening.id());
            }
        }
        for railing in model.railings.values() {
            if excluded.contains(&railing.parameters.stair) {
                excluded.insert(railing.id());
            }
        }
        // Resolve openings and joins against a view-only model copy. The document,
        // room enclosure solver and committed 3D model retain their full topology.
        let mut wall_source = std::borrow::Cow::Borrowed(model);
        if model
            .walls
            .keys()
            .chain(model.openings.keys())
            .any(|id| excluded.contains(id))
        {
            let source = wall_source.to_mut();
            source.walls.retain(|id, _| !excluded.contains(id));
            source.openings.retain(|id, _| !excluded.contains(id));
            source
                .opening_clearances
                .retain(|id, _| !excluded.contains(id));
            source.wall_joins.retain(|_, join| {
                join.parameters
                    .members()
                    .iter()
                    .all(|id| !excluded.contains(id))
            });
        }
        ensure(
            model
                .walls
                .len()
                .saturating_add(model.floors.len())
                .saturating_add(model.stairs.len().saturating_mul(4))
                .saturating_add(model.ramps.len())
                .saturating_add(model.railings.len())
                .saturating_add(model.columns.len())
                .saturating_add(model.casework.len())
                .saturating_add(model.extensions.len())
                .saturating_add(model.grids.len())
                // Count sources here. Native line attachment below enforces the
                // actual 10000-segment budget; a fixed 64x reservation rejects
                // otherwise valid 256-instance opening selections.
                .saturating_add(model.openings.len())
                .saturating_add(model.dimensions.len())
                .saturating_add(model.room_tags.len())
                .saturating_add(model.opening_tags.len())
                .saturating_add(model.detail_lines.len())
                .saturating_add(model.room_separation_lines.len())
                <= MAX_PLAN_ELEMENTS,
            "plan exceeds 10000 elements",
        )?;
        let mut walls = Vec::new();
        if context.show_walls {
            for id in wall_source.walls.keys() {
                walls.push(os_geometry::walls::NativeWall::from_model(
                    &wall_source,
                    *id,
                )?);
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
        let mut ceiling_resolver = os_geometry::ceilings::CeilingResolver::default();
        // Category visibility affects symbols and references, not the physical
        // apertures already resolved from the phase-filtered wall source above.
        for (id, kind) in &opening_kinds {
            if !settings.visibility.shows_opening(*kind) {
                excluded.insert(*id);
            }
        }
        let mut curtain_lines = BTreeMap::new();
        let mut curtain_line_count = 0usize;
        for curtain in model.curtain_systems.values().filter(|curtain| {
            !excluded.contains(&curtain.id())
                && model.levels[&curtain.parameters.level].parameters.building == building
        }) {
            let components = curtain.parameters.resolve(model)?;
            curtain_line_count = curtain_line_count.saturating_add(components.len() * 4);
            ensure(
                curtain_line_count <= MAX_PLAN_ELEMENTS,
                "curtain plan lines exceed 10000 segments",
            )?;
            curtain_lines.insert(
                curtain.id(),
                curtain_plan_lines(
                    curtain.id(),
                    &curtain.parameters,
                    &components,
                    model.levels[&curtain.parameters.level].parameters.elevation,
                    context,
                )?,
            );
        }
        let mut railing_lines = BTreeMap::new();
        let mut railing_surfaces = BTreeMap::new();
        let mut railing_line_count = 0usize;
        for railing in model.railings.values().filter(|railing| {
            let stair = &model.stairs[&railing.parameters.stair].parameters;
            !excluded.contains(&railing.id())
                && model.levels[&stair.lower_level].parameters.building == building
        }) {
            let geometry = os_geometry::railings::railing_geometry(&railing.parameters, model)?;
            let features = railing_plan_lines(railing.id(), &geometry.members, context)?;
            railing_line_count = railing_line_count.saturating_add(features.len());
            ensure(
                railing_line_count <= MAX_PLAN_ELEMENTS,
                "railing plan exceeds 10000 segments",
            )?;
            let material = model.railing_types[&railing.parameters.railing_type]
                .parameters
                .material;
            for line in &features {
                railing_surfaces.insert(
                    (railing.id(), line.feature),
                    os_geometry::SurfaceIdentity {
                        layer: None,
                        material,
                    },
                );
            }
            railing_lines.insert(railing.id(), features);
        }
        Ok(PlanSnapshot {
            railing_lines,
            railing_surfaces,
            curtain_lines,
            phase_statuses,
            excluded: excluded.clone(),
            material_colors: model
                .materials
                .iter()
                .map(|(id, m)| (*id, m.parameters.color))
                .collect(),
            context,
            graphics,
            opening_kinds,
            wall_ids: model.walls.keys().copied().collect(),
            room_separation_lines: model
                .room_separation_lines
                .values()
                .filter(|line| !excluded.contains(&line.id()))
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
            opening_tags: model
                .opening_tags
                .values()
                .filter(|tag| !excluded.contains(&tag.parameters.opening))
                .filter(|tag| tag.parameters.view == view)
                .map(|tag| opening_tag_graphic(model, tag.id(), &tag.parameters, context))
                .collect::<Result<Vec<_>>>()?,
            room_tags: model
                .room_tags
                .values()
                .filter(|tag| !excluded.contains(&tag.parameters.room))
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
                .filter(|opening| !excluded.contains(&opening.id()))
                .map(|o| Ok((o.id(), model.resolve_opening(&o.parameters)?)))
                .collect::<Result<Vec<_>>>()?,
            room_segments: model.room_boundary_segments(plan_level)?,
            rooms: model
                .rooms
                .values()
                .filter(|room| !excluded.contains(&room.id()))
                .filter(|room| room.parameters.level == plan_level)
                .map(|room| (room.id(), room.parameters.clone()))
                .collect(),
            dimensions: model
                .dimensions
                .values()
                .filter(|dimension| {
                    dimension
                        .parameters
                        .references()
                        .all(|reference| !excluded.contains(&reference.entity()))
                })
                .filter(|dimension| dimension.parameters.view == view)
                .map(|dimension| (dimension.id(), dimension.parameters.clone()))
                .collect(),
            dimension_walls: model
                .walls
                .values()
                .filter(|wall| !excluded.contains(&wall.id()))
                .map(|wall| {
                    (
                        wall.id(),
                        wall.parameters.level,
                        wall.parameters.path,
                        model
                            .resolve_wall(wall.id())
                            .map_or(f64::NAN, |wall| wall.parameters.thickness),
                    )
                })
                .collect(),
            plan_level,
            columns: model
                .columns
                .values()
                .filter(|column| !excluded.contains(&column.id()))
                .map(|column| {
                    let elevation = model.levels[&column.parameters.level].parameters.elevation;
                    (column.id(), column.parameters.clone(), elevation)
                })
                .collect(),
            casework: model
                .casework
                .values()
                .filter(|casework| !excluded.contains(&casework.id()))
                .map(|casework| {
                    let casework_type =
                        &model.casework_types[&casework.parameters.type_id].parameters;
                    let elevation = model.levels[&casework.parameters.level]
                        .parameters
                        .elevation;
                    (
                        casework.id(),
                        casework.parameters.clone(),
                        casework_type.clone(),
                        elevation,
                    )
                })
                .collect(),
            floors: model
                .floors
                .values()
                .filter(|floor| !excluded.contains(&floor.id()))
                .map(|floor| {
                    let level = &model.levels[&floor.parameters.level];
                    (
                        floor.id(),
                        floor.parameters.clone(),
                        level.parameters.elevation,
                    )
                })
                .collect(),
            stairs: model
                .stairs
                .values()
                .filter(|stair| !excluded.contains(&stair.id()))
                .filter(|stair| {
                    model.levels[&stair.parameters.lower_level]
                        .parameters
                        .building
                        == building
                })
                .map(|stair| {
                    let lower = model.levels[&stair.parameters.lower_level]
                        .parameters
                        .elevation;
                    let upper = model.levels[&stair.parameters.upper_level]
                        .parameters
                        .elevation;
                    (stair.id(), stair.parameters.clone(), lower, upper)
                })
                .collect(),
            ramps: model
                .ramps
                .values()
                .filter(|ramp| !excluded.contains(&ramp.id()))
                .filter(|ramp| {
                    model.levels[&ramp.parameters.lower_level]
                        .parameters
                        .building
                        == building
                })
                .map(|ramp| {
                    let lower = model.levels[&ramp.parameters.lower_level]
                        .parameters
                        .elevation;
                    let upper = model.levels[&ramp.parameters.upper_level]
                        .parameters
                        .elevation;
                    (ramp.id(), ramp.parameters.clone(), lower, upper)
                })
                .collect(),
            section_markers,
            roofs: model
                .roofs
                .values()
                .filter(|roof| !excluded.contains(&roof.id()))
                .filter(|roof| model.levels[&roof.parameters.level].parameters.building == building)
                .map(|roof| {
                    (
                        roof.id(),
                        roof.parameters.clone(),
                        model.levels[&roof.parameters.level].parameters.elevation,
                    )
                })
                .collect(),
            ceilings: {
                let settings = model.views[&view]
                    .parameters
                    .plan
                    .expect("validated plan settings");
                if settings.view_type == os_model::PlanViewType::ReflectedCeilingPlan
                    && settings.visibility.ceilings
                {
                    model
                        .ceilings
                        .values()
                        .filter(|ceiling| !excluded.contains(&ceiling.id()))
                        .filter(|ceiling| {
                            model.levels[&ceiling.parameters.level].parameters.building == building
                        })
                        .map(|ceiling| {
                            let parameters = ceiling_resolver
                                .effective_parameters(model, &ceiling.parameters)?;
                            Ok(parameters.map(|parameters| {
                                let elevation =
                                    model.levels[&parameters.level].parameters.elevation;
                                (ceiling.id(), parameters, elevation)
                            }))
                        })
                        .collect::<Result<Vec<_>>>()?
                        .into_iter()
                        .flatten()
                        .collect()
                } else {
                    Vec::new()
                }
            },
            unavailable,
        })
    }
}

#[cfg(test)]
mod opening_visibility_tests;
#[cfg(test)]
pub(crate) mod phase_tests;

/// Component edges keep assembly ownership; feature numbers include hidden
/// components so range/crop changes do not renumber visible features.
fn curtain_plan_lines(
    entity: Id,
    parameters: &os_model::CurtainSystemParams,
    components: &[os_model::CurtainComponent],
    elevation: f64,
    context: PlanContext,
) -> Result<Vec<os_render::plan::PlanLine>> {
    ensure(
        components.len() * 4 <= 4096,
        "curtain exceeds native plan line limit",
    )?;
    let mut lines = Vec::new();
    let mut components = components.iter().collect::<Vec<_>>();
    components.sort_by_key(|component| component.id);
    for (index, component) in components.into_iter().enumerate() {
        let base = elevation + parameters.base_offset + component.min[2];
        let top = elevation + parameters.base_offset + component.max[2];
        let range = context.range;
        let tolerance = os_geometry::plan::PLAN_TOLERANCE;
        if top <= range.depth + tolerance || base >= range.top - tolerance {
            continue;
        }
        let role = if base <= range.cut + tolerance && top > range.cut + tolerance {
            PlanRole::Cut
        } else if top <= range.cut + tolerance && top > range.bottom + tolerance {
            PlanRole::Projected
        } else if top <= range.bottom + tolerance && top > range.depth + tolerance {
            PlanRole::Depth
        } else {
            continue;
        };
        let [x0, y0, z] = component.min;
        let [x1, y1, _] = component.max;
        let points = [[x0, y0, z], [x1, y0, z], [x1, y1, z], [x0, y1, z]]
            .into_iter()
            .map(|local| {
                let world = parameters.world_point(local, elevation);
                context
                    .basis
                    .world_to_plane(Point2::new(world[0], world[1]))
            })
            .collect::<Result<Vec<_>>>()?;
        for edge in 0..4 {
            lines.push(os_render::plan::PlanLine {
                entity,
                feature: (index * 4 + edge) as u32,
                start: points[edge],
                end: points[(edge + 1) % 4],
                role,
            });
        }
    }
    Ok(lines)
}

/// Plan marks come from the checked host-derived member geometry. The rail is
/// split at range transitions; post footprints retain their original feature
/// slots when the range or crop hides other members.
fn railing_plan_lines(
    entity: Id,
    members: &[os_geometry::Mesh],
    context: PlanContext,
) -> Result<Vec<os_render::plan::PlanLine>> {
    use os_geometry::plan::PLAN_TOLERANCE;
    ensure(
        !members.is_empty() && members.len() <= os_model::MAX_STAIR_RAILING_POSTS as usize + 1,
        "railing exceeds plan member budget",
    )?;
    let role = |base: f64, top: f64| {
        let range = context.range;
        if top <= range.depth + PLAN_TOLERANCE || base >= range.top - PLAN_TOLERANCE {
            None
        } else if base <= range.cut + PLAN_TOLERANCE && top > range.cut + PLAN_TOLERANCE {
            Some(PlanRole::Cut)
        } else if top <= range.cut + PLAN_TOLERANCE && top > range.bottom + PLAN_TOLERANCE {
            Some(PlanRole::Projected)
        } else if top <= range.bottom + PLAN_TOLERANCE && top > range.depth + PLAN_TOLERANCE {
            Some(PlanRole::Depth)
        } else {
            None
        }
    };
    let mut lines = Vec::new();
    for (index, member) in members.iter().enumerate() {
        member.validate()?;
        ensure(member.vertices.len() == 8, "invalid railing box member")?;
        ensure(
            member.vertices.iter().all(|v| {
                [v.x, v.y, v.z].into_iter().all(|n| {
                    n.is_finite() && n.abs() <= os_geometry::section::MAX_SECTION_COORDINATE
                })
            }),
            "railing plan member outside coordinate envelope",
        )?;
        let vertices = &member.vertices;
        if index == 0 {
            let a = Point2::new(
                (vertices[0].x + vertices[3].x) * 0.5,
                (vertices[0].y + vertices[3].y) * 0.5,
            );
            let b = Point2::new(
                (vertices[1].x + vertices[2].x) * 0.5,
                (vertices[1].y + vertices[2].y) * 0.5,
            );
            let base = vertices[0].z;
            let rise = vertices[1].z - base;
            let depth = vertices[4].z - base;
            ensure(rise > 0.0 && depth > 0.0, "invalid railing rail slope")?;
            let mut stations = vec![0.0, 1.0];
            for elevation in [
                context.range.depth - depth,
                context.range.bottom - depth,
                context.range.cut - depth,
                context.range.cut,
                context.range.top,
            ] {
                let t = (elevation - base) / rise;
                if t > 0.0 && t < 1.0 {
                    stations.push(t);
                }
            }
            stations.sort_by(f64::total_cmp);
            stations.dedup();
            for (feature, pair) in stations.windows(2).enumerate() {
                let z = base + rise * (pair[0] + pair[1]) * 0.5;
                let Some(role) = role(z, z + depth) else {
                    continue;
                };
                let at = |t: f64| {
                    context
                        .basis
                        .world_to_plane(Point2::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t))
                };
                let start = at(pair[0])?;
                let end = at(pair[1])?;
                if start.distance(end) > PLAN_TOLERANCE {
                    lines.push(os_render::plan::PlanLine {
                        entity,
                        feature: feature as u32,
                        start,
                        end,
                        role,
                    });
                }
            }
        } else {
            let Some(role) = role(vertices[0].z, vertices[4].z) else {
                continue;
            };
            for edge in 0..4 {
                let a = vertices[edge];
                let b = vertices[(edge + 1) % 4];
                lines.push(os_render::plan::PlanLine {
                    entity,
                    feature: (8 + (index - 1) * 4 + edge) as u32,
                    start: context.basis.world_to_plane(Point2::new(a.x, a.y))?,
                    end: context.basis.world_to_plane(Point2::new(b.x, b.y))?,
                    role,
                });
            }
        }
    }
    ensure(
        lines.len() <= 4096,
        "railing exceeds native plan line budget",
    )?;
    Ok(lines)
}

pub(crate) struct PlanSnapshot {
    railing_lines: BTreeMap<Id, Vec<os_render::plan::PlanLine>>,
    railing_surfaces: BTreeMap<(Id, u32), os_geometry::SurfaceIdentity>,
    curtain_lines: BTreeMap<Id, Vec<os_render::plan::PlanLine>>,
    phase_statuses: BTreeMap<Id, os_model::PhaseStatus>,
    excluded: std::collections::BTreeSet<Id>,
    roofs: Vec<(Id, os_model::RoofParams, f64)>,
    material_colors: BTreeMap<Id, [u8; 3]>,
    pub context: PlanContext,
    graphics: Option<os_model::PlanGraphicsStyles>,
    opening_kinds: BTreeMap<Id, os_model::OpeningKind>,
    wall_ids: std::collections::BTreeSet<Id>,
    detail_lines: Vec<os_render::plan::PlanDetailLine>,
    room_separation_lines: Vec<os_render::plan::PlanRoomSeparationLine>,
    room_tags: Vec<os_render::plan::PlanRoomTag>,
    opening_tags: Vec<os_render::plan::PlanOpeningTag>,
    walls: Vec<os_geometry::walls::NativeWall>,
    openings: Vec<(Id, os_model::ResolvedOpening)>,
    room_segments: Vec<os_geometry::rooms::BoundarySegment>,
    rooms: Vec<(Id, os_model::RoomParams)>,
    dimensions: Vec<(Id, DimensionParams)>,
    dimension_walls: Vec<(Id, Id, os_model::WallPath, f64)>,
    plan_level: Id,
    floors: Vec<(Id, os_model::FloorParams, f64)>,
    ceilings: Vec<(Id, os_model::CeilingParams, f64)>,
    stairs: Vec<(Id, os_model::StairParams, f64, f64)>,
    ramps: Vec<(Id, os_model::RampParams, f64, f64)>,
    columns: Vec<(Id, os_model::ColumnParams, f64)>,
    casework: Vec<(
        Id,
        os_model::CaseworkParams,
        os_model::CaseworkTypeParams,
        f64,
    )>,
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
        mut lines: BTreeMap<Id, Vec<os_render::plan::PlanLine>>,
    ) -> Result<PlanDrawing> {
        lines.retain(|id, _| !self.excluded.contains(id));
        let (room_faces, room_items, room_diagnostic) =
            derive_room_graphics(self.context, &self.room_segments, &self.rooms)?;
        let dimension_items = derive_dimension_graphics(
            self.context,
            self.plan_level,
            &self.dimension_walls,
            &self.openings,
            &self.dimensions,
        )?;
        let angular_items = self
            .dimensions
            .iter()
            .filter(|(_, p)| p.layout == os_model::DimensionLayout::Angular)
            .map(|(id, p)| {
                let resolved = p.resolve_angular_with(|wall| {
                    let (_, level, path, _) = self
                        .dimension_walls
                        .iter()
                        .find(|(id, _, _, _)| *id == wall)
                        .ok_or(os_model::DimensionDiagnostic::MissingWall)?;
                    if *level != self.plan_level {
                        return Err(os_model::DimensionDiagnostic::WrongLevel);
                    }
                    if !path.is_straight() {
                        return Err(os_model::DimensionDiagnostic::InvalidGeometry);
                    }
                    Ok((path.start(), path.end()))
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
        let casework_items = self
            .casework
            .iter()
            .map(|(id, parameters, casework_type, elevation)| {
                Ok((
                    *id,
                    os_geometry::casework::casework_solid(parameters, casework_type, *elevation)?,
                    os_geometry::SurfaceIdentity {
                        layer: None,
                        material: casework_type.material,
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
                        total.saturating_add(
                            parameters
                                .boundary
                                .len()
                                .saturating_add(
                                    parameters.holes.iter().map(Vec::len).sum::<usize>(),
                                )
                                .saturating_add(parameters.holes.len().saturating_mul(2))
                                .saturating_sub(2),
                        )
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
                let holes = parameters
                    .holes
                    .iter()
                    .map(|ring| {
                        ring.iter()
                            .map(|point| self.context.basis.world_to_plane(*point))
                            .collect::<Result<Vec<_>>>()
                    })
                    .collect::<Result<Vec<_>>>()?;
                let triangulation =
                    os_geometry::floor_holes::triangulate_floor_rings(&boundary, &holes)?;
                Ok(os_render::plan::PlanFloorItem {
                    entity,
                    area_m2: triangulation.net_area,
                    boundary,
                    holes,
                    vertices: triangulation.vertices,
                    triangles: triangulation.triangles,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let ceiling_items = self
            .ceilings
            .iter()
            .filter_map(|(entity, parameters, level_elevation)| {
                match os_geometry::ceilings::ceiling_plan(
                    parameters,
                    *level_elevation,
                    self.context.range,
                    self.context.basis,
                    self.context.crop,
                ) {
                    Ok(Some(plan)) => Some(Ok(os_render::plan::PlanCeilingItem {
                        entity: *entity,
                        boundary: plan.boundary,
                        holes: plan.holes,
                        vertices: plan.vertices,
                        triangles: plan.triangles,
                        area_m2: plan.area_m2,
                    })),
                    Ok(None) => None,
                    Err(error) => Some(Err(error)),
                }
            })
            .collect::<Result<Vec<_>>>()?;
        let stair_items = self
            .stairs
            .iter()
            .map(|(entity, parameters, lower, upper)| {
                project_stair_for_plan(*entity, parameters, *lower, *upper, self.context)
            })
            .collect::<Result<Vec<_>>>()?;
        let ramp_items = self
            .ramps
            .iter()
            .filter_map(|(entity, parameters, lower, upper)| {
                match os_render::plan::PlanRampItem::from_plan_data(
                    *entity,
                    parameters,
                    *lower,
                    *upper,
                    self.context,
                ) {
                    Ok(Some(item)) => Some(Ok(item)),
                    Ok(None) => None,
                    Err(error) => Some(Err(error)),
                }
            })
            .collect::<Result<Vec<_>>>()?;
        let mut solids = BTreeMap::new();
        let mut segments = Vec::new();
        let mut native_lines = self.curtain_lines;
        native_lines.extend(self.railing_lines);
        let mut seams = BTreeMap::new();
        for (id, roof, elevation) in &self.roofs {
            let plan = os_geometry::roofs::roof_plan(
                roof,
                *elevation,
                self.context.range,
                self.context.basis,
                self.context.crop,
            )?;
            seams.insert(*id, plan.seams);
            solids.insert(
                *id,
                plan.footprints
                    .into_iter()
                    .map(|footprint| {
                        (
                            os_geometry::SurfaceIdentity {
                                layer: None,
                                material: roof.material,
                            },
                            footprint,
                        )
                    })
                    .collect(),
            );
        }
        let mut arcs = Vec::new();
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
            if let os_model::WallPath::CircularArc {
                center,
                radius,
                start_angle_rad,
                signed_sweep_rad,
            } = parameters.path
            {
                arcs.push(os_render::snapping::SnapArc {
                    entity: id,
                    center: self.context.basis.world_to_plane(center)?,
                    radius,
                    start_angle_rad: start_angle_rad - self.context.basis.rotation,
                    signed_sweep_rad,
                });
            } else {
                segments.push(os_render::snapping::SnapSegment {
                    entity: id,
                    feature: 0,
                    start: parameters.start(),
                    end: parameters.end(),
                });
            }
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
        arcs.retain(|arc| visible.contains(&arc.entity));
        for segment in &mut segments {
            segment.start = self.context.basis.world_to_plane(segment.start)?;
            segment.end = self.context.basis.world_to_plane(segment.end)?;
        }
        let mut grids = self.grids;
        let roof_ids: std::collections::BTreeSet<_> =
            self.roofs.iter().map(|(id, _, _)| *id).collect();
        for item in drawing
            .items(self.context)?
            .iter()
            .filter(|item| roof_ids.contains(&item.entity))
        {
            for (start, end) in item.outline() {
                segments.push(os_render::snapping::SnapSegment {
                    entity: item.entity,
                    feature: segments.len() as u32,
                    start,
                    end,
                });
            }
        }
        for grid in &mut grids {
            grid.start = self.context.basis.world_to_plane(grid.start)?;
            grid.end = self.context.basis.world_to_plane(grid.end)?;
        }
        let mut drawing = drawing
            .with_line_surfaces(self.railing_surfaces)
            .with_grids(grids)?
            .with_stairs(stair_items)?
            .with_ramps(ramp_items)?
            .with_native_lines(native_lines)?
            .with_provider_lines(lines)?
            .with_detail_lines(self.detail_lines)?
            .with_room_separation_lines(self.plan_level, self.room_separation_lines)?
            .with_snap_segments(segments)?
            .with_snap_arcs(arcs)?
            .with_room_faces(room_faces)?
            .with_rooms(room_items)?
            .with_dimensions(dimension_items)?
            .with_floors(floor_items)?
            .with_ceilings(ceiling_items)?
            .with_columns(&column_items)?
            .with_casework(&casework_items)?
            .with_angular_dimensions(angular_items)?
            .with_room_tags(self.room_tags)?
            .with_opening_tags(self.opening_tags)?
            .with_room_boundary_diagnostic(room_diagnostic)?
            .with_material_colors(self.material_colors);
        let mut appearances = BTreeMap::new();
        if let Some(styles) = self.graphics {
            use os_geometry::plan::PlanRole;
            use os_render::plan::{PlanElementAppearance, PlanStroke};
            let convert = |style: os_model::PlanStrokeStyle| PlanStroke {
                color: style.color,
                weight_mm: style.weight_mm,
                dashed: style.pattern == os_model::PlanLinePattern::Dashed,
            };
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
        }
        // Resolve once for all consumers. Preserve category weights while phase
        // color and dash pattern take precedence over local/linked styles.
        let mut apply_phase = |id, role, weight| {
            let Some(status) = self.phase_statuses.get(&id) else {
                return;
            };
            use os_model::PhaseStatus::*;
            let (color, dashed) = match status {
                Existing => ([112, 118, 124], false),
                New => ([35, 65, 88], false),
                Demolished => ([155, 82, 68], true),
                Temporary => ([126, 92, 145], true),
                Future | PreviouslyDemolished => return,
            };
            let entry = appearances
                .entry(id)
                .or_insert_with(os_render::plan::PlanElementAppearance::default);
            let stroke = if role == os_geometry::plan::PlanRole::Cut {
                &mut entry.cut
            } else {
                &mut entry.projected
            };
            *stroke = Some(os_render::plan::PlanStroke {
                color,
                dashed,
                weight_mm: stroke.map_or(weight, |value| value.weight_mm),
            });
        };
        for item in drawing
            .items(self.context)?
            .iter()
            .chain(drawing.columns(self.context)?)
            .chain(drawing.casework(self.context)?)
        {
            apply_phase(
                item.entity,
                item.footprint.role,
                if item.footprint.role == os_geometry::plan::PlanRole::Cut {
                    0.35
                } else {
                    0.18
                },
            );
        }
        for line in drawing.provider_lines(self.context)? {
            if drawing.is_native_line(line.entity) {
                apply_phase(line.entity, line.role, 0.18);
            }
        }
        for floor in drawing
            .floors(self.context)?
            .iter()
            .chain(drawing.ceilings(self.context)?)
        {
            apply_phase(floor.entity, os_geometry::plan::PlanRole::Projected, 0.18);
        }
        for room in drawing.rooms(self.context)? {
            apply_phase(room.entity, os_geometry::plan::PlanRole::Projected, 0.15);
        }
        for line in drawing.room_separation_lines(self.context)? {
            apply_phase(
                line.entity,
                os_geometry::plan::PlanRole::Projected,
                os_render::plan::ROOM_SEPARATOR_WEIGHT_MM,
            );
        }
        drawing = drawing.with_appearances(appearances)?;
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
    holes: Vec<Vec<Point2>>,
    top: f64,
    thickness: f64,
}

/// Additional independent solids travel with the frozen section source, without
/// changing the existing curtain/stair source constructors.
pub(crate) struct PreparedSectionSnapshot {
    source: SectionSnapshot,
    railing_members: BTreeMap<Id, Vec<os_geometry::Mesh>>,
}

impl std::ops::Deref for PreparedSectionSnapshot {
    type Target = SectionSnapshot;
    fn deref(&self) -> &Self::Target {
        &self.source
    }
}

impl PreparedSectionSnapshot {
    pub(crate) fn derive(self) -> Result<PlanDrawing> {
        self.source.derive_with_railings(self.railing_members)
    }
}

pub(crate) struct SectionSnapshot {
    roofs: Vec<(Id, os_model::RoofParams, f64)>,
    context: PlanContext,
    interfaces: Vec<os_geometry::walls::ButtInterface>,
    plane: os_geometry::section::VerticalSectionPlane,
    walls: Vec<os_geometry::walls::NativeWall>,
    floors: Vec<SectionFloor>,
    ceilings: Vec<(Id, os_model::CeilingParams, f64)>,
    stairs: Vec<(Id, os_model::StairParams, f64, f64)>,
    ramps: Vec<(Id, os_model::RampParams, f64, f64)>,
    curtains: Vec<(Id, Vec<os_geometry::curtain_systems::CurtainComponentMesh>)>,
}

impl SectionSnapshot {
    #[cfg(test)]
    pub(crate) fn derive(self) -> Result<PlanDrawing> {
        self.derive_with_railings(BTreeMap::new())
    }

    fn derive_with_railings(
        self,
        railing_members: BTreeMap<Id, Vec<os_geometry::Mesh>>,
    ) -> Result<PlanDrawing> {
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
            let mesh = os_geometry::floor_holes::extrude_floor_rings(
                &floor.boundary,
                &floor.holes,
                floor.top,
                floor.thickness,
            )?;
            add_section_contours(
                segments.entry(floor.entity).or_default(),
                vertical_section(&mesh, self.plane)?,
                &mut raw_segment_count,
            )?;
        }
        for (entity, parameters, elevation) in self.ceilings {
            let mesh = os_geometry::ceilings::ceiling_mesh(&parameters, elevation)?;
            add_section_contours(
                segments.entry(entity).or_default(),
                vertical_section(&mesh, self.plane)?,
                &mut raw_segment_count,
            )?;
        }
        for (entity, parameters, elevation) in &self.roofs {
            let mesh = os_geometry::roofs::roof_mesh(parameters, *elevation)?;
            add_section_contours(
                segments.entry(*entity).or_default(),
                vertical_section(&mesh, self.plane)?,
                &mut raw_segment_count,
            )?;
        }
        let stair_materials = self
            .stairs
            .iter()
            .map(|(id, parameters, _, _)| (*id, parameters.material))
            .collect::<BTreeMap<_, _>>();
        let ramp_materials = self
            .ramps
            .iter()
            .map(|(id, parameters, _, _)| (*id, parameters.material))
            .collect::<BTreeMap<_, _>>();
        for (entity, parameters, lower, upper) in self.stairs {
            let mesh = os_geometry::stairs::stair_mesh(&parameters, lower, upper)?;
            add_section_contours(
                segments.entry(entity).or_default(),
                vertical_section(&mesh, self.plane)?,
                &mut raw_segment_count,
            )?;
        }
        for (entity, parameters, lower, upper) in self.ramps {
            let mesh = os_geometry::ramps::ramp_mesh(&parameters, lower, upper)?;
            add_section_contours(
                segments.entry(entity).or_default(),
                vertical_section(&mesh, self.plane)?,
                &mut raw_segment_count,
            )?;
        }

        let mut curtain_lines: BTreeMap<Id, Vec<PlanLine>> = BTreeMap::new();
        for (assembly, components) in self.curtains {
            for component in components {
                // Components are sectioned independently; their touching shells
                // cannot be passed to the section kernel as a single union.
                if !section_plane_crosses_component(&component.mesh, self.plane)? {
                    continue;
                }
                let contours = vertical_section(&component.mesh, self.plane)?;
                let mut component_edges = Vec::new();
                add_section_contours(&mut component_edges, contours, &mut raw_segment_count)?;
                let retained = reduce_section_edges(component_edges)?;
                ensure(
                    retained.len() <= 256,
                    "curtain component exceeds cut segment limit",
                )?;
                let output = curtain_lines.entry(assembly).or_default();
                ensure(
                    output.len().saturating_add(retained.len()) <= MAX_PLAN_ELEMENTS,
                    "curtain assembly exceeds section line limit",
                )?;
                for segment in retained {
                    ensure(
                        [
                            segment.start.x,
                            segment.start.y,
                            segment.end.x,
                            segment.end.y,
                        ]
                        .into_iter()
                        .all(|v| {
                            v.is_finite() && v.abs() <= os_geometry::section::MAX_SECTION_COORDINATE
                        }),
                        "curtain section line outside finite coordinate envelope",
                    )?;
                    let feature = u32::try_from(output.len()).map_err(|_| {
                        os_core::Error::Invalid("curtain section feature overflow".into())
                    })?;
                    output.push(PlanLine {
                        entity: assembly,
                        feature,
                        start: segment.start,
                        end: segment.end,
                        role: PlanRole::Cut,
                    });
                    line_surfaces.insert(
                        (assembly, feature),
                        os_geometry::SurfaceIdentity {
                            layer: None,
                            material: component.component.material,
                        },
                    );
                }
            }
        }

        for (entity, members) in railing_members {
            ensure(
                members.len() <= os_model::MAX_STAIR_RAILING_POSTS as usize + 1,
                "railing exceeds section member budget",
            )?;
            let output = curtain_lines.entry(entity).or_default();
            for (member_index, member) in members.into_iter().enumerate() {
                // The rail and posts touch, but are not a sectionable mesh union.
                if !section_plane_crosses_component(&member, self.plane)? {
                    continue;
                }
                let mut edges = Vec::new();
                add_section_contours(
                    &mut edges,
                    vertical_section(&member, self.plane)?,
                    &mut raw_segment_count,
                )?;
                let retained = reduce_section_edges(edges)?;
                ensure(
                    retained.len() <= 4,
                    "railing box exceeds section edge budget",
                )?;
                ensure(
                    output.len().saturating_add(retained.len()) <= 4096,
                    "railing exceeds native section line budget",
                )?;
                let surface = member.surfaces.first().copied().unwrap_or_default();
                for (edge, segment) in retained.into_iter().enumerate() {
                    let feature = (member_index * 4 + edge) as u32;
                    output.push(PlanLine {
                        entity,
                        feature,
                        start: segment.start,
                        end: segment.end,
                        role: PlanRole::Cut,
                    });
                    line_surfaces.insert((entity, feature), surface);
                }
            }
        }

        os_geometry::walls::remove_section_interfaces(&mut segments, &self.interfaces, self.plane)?;
        let mut lines = layer_lines;
        for (assembly, features) in curtain_lines {
            if !features.is_empty() {
                ensure(
                    !lines.contains_key(&assembly),
                    "duplicate section source identity",
                )?;
                lines.insert(assembly, features);
            }
        }
        let mut stair_lines = Vec::new();
        let mut ramp_lines = Vec::new();
        let mut line_count = lines.values().map(Vec::len).sum::<usize>();
        ensure(
            line_count <= MAX_PLAN_ELEMENTS,
            "section exceeds 10000 layer cut segments",
        )?;
        for (entity, raw_edges) in segments {
            let retained = reduce_section_edges(raw_edges)?;
            ensure(
                retained.len()
                    <= if stair_materials.contains_key(&entity) {
                        772
                    } else {
                        256
                    },
                "section element exceeds cut segment limit",
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
                if let Some(material) = stair_materials
                    .get(&entity)
                    .or_else(|| ramp_materials.get(&entity))
                {
                    line_surfaces.insert(
                        (entity, feature as u32),
                        os_geometry::SurfaceIdentity {
                            layer: None,
                            material: *material,
                        },
                    );
                }
                features.push(PlanLine {
                    entity,
                    feature: feature as u32,
                    start: segment.start,
                    end: segment.end,
                    role: PlanRole::Cut,
                });
            }
            if !features.is_empty() {
                if stair_materials.contains_key(&entity) {
                    stair_lines.push(os_render::plan::PlanStairItem::from_section_lines(
                        entity,
                        stair_materials[&entity],
                        features,
                    ));
                } else if ramp_materials.contains_key(&entity) {
                    ramp_lines.push(os_render::plan::PlanRampItem::from_section_lines(
                        entity,
                        ramp_materials[&entity],
                        features,
                    ));
                } else {
                    lines.insert(entity, features);
                }
            }
        }
        let unavailable = lines.keys().copied().collect();
        PlanDrawing::from_prisms(self.context, &BTreeMap::new(), unavailable)?
            .with_native_lines(lines)
            .and_then(|drawing| drawing.with_stairs(stair_lines))
            .and_then(|drawing| drawing.with_ramps(ramp_lines))
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

/// The section kernel reports the boundary of a coplanar face. Curtain cut
/// drawings omit that boundary when the plane only touches a component's
/// exterior and does not cross its interior.
fn section_plane_crosses_component(
    mesh: &os_geometry::Mesh,
    plane: os_geometry::section::VerticalSectionPlane,
) -> Result<bool> {
    use os_geometry::section::{MAX_SECTION_COORDINATE, SECTION_TOLERANCE};

    let scale = plane.direction.x.abs().max(plane.direction.y.abs());
    ensure(
        scale.is_finite() && scale > 0.0,
        "invalid curtain section direction",
    )?;
    let dx = plane.direction.x / scale;
    let dy = plane.direction.y / scale;
    let length = dx.hypot(dy);
    let normal = Point2::new(-dy / length, dx / length);
    ensure(
        plane.origin.x.is_finite()
            && plane.origin.y.is_finite()
            && plane.origin.x.abs() <= MAX_SECTION_COORDINATE
            && plane.origin.y.abs() <= MAX_SECTION_COORDINATE,
        "invalid curtain section origin",
    )?;
    let mut positive = false;
    let mut negative = false;
    for vertex in &mesh.vertices {
        ensure(
            vertex.x.is_finite()
                && vertex.y.is_finite()
                && vertex.z.is_finite()
                && vertex.x.abs() <= MAX_SECTION_COORDINATE
                && vertex.y.abs() <= MAX_SECTION_COORDINATE
                && vertex.z.abs() <= MAX_SECTION_COORDINATE,
            "curtain section mesh outside coordinate envelope",
        )?;
        let distance =
            normal.x * (vertex.x - plane.origin.x) + normal.y * (vertex.y - plane.origin.y);
        ensure(distance.is_finite(), "curtain section distance overflow")?;
        positive |= distance > SECTION_TOLERANCE;
        negative |= distance < -SECTION_TOLERANCE;
    }
    Ok(positive && negative)
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod stair_section_tests {
    use super::*;

    fn context() -> PlanContext {
        PlanContext {
            session_id: Id::new(),
            model_revision: 1,
            view_id: Id::new(),
            settings_revision: 1,
            basis: HorizontalBasis::default(),
            range: PlanRange {
                top: 4.0,
                cut: 1.5,
                bottom: -1.0,
                depth: -1.0,
            },
            crop: Some(PlanCrop {
                min: Point2::new(0.0, -1.0),
                max: Point2::new(5.0, 4.0),
            }),
            scale_denominator: 100.0,
            show_walls: true,
            show_extensions: false,
        }
    }

    fn snapshot(
        context: PlanContext,
        plane: os_geometry::section::VerticalSectionPlane,
        stair: Id,
        material: Id,
    ) -> SectionSnapshot {
        SectionSnapshot {
            context,
            interfaces: Vec::new(),
            plane,
            walls: Vec::new(),
            floors: Vec::new(),
            roofs: Vec::new(),
            ceilings: Vec::new(),
            stairs: vec![(
                stair,
                os_model::StairParams {
                    name: "Flight".into(),
                    lower_level: Id::new(),
                    upper_level: Id::new(),
                    start: Point2::new(0.0, 0.0),
                    end: Point2::new(4.0, 0.0),
                    width: 1.0,
                    riser_count: 8,
                    structural_thickness: 0.2,
                    material: Some(material),
                },
                0.0,
                2.0,
            )],
            ramps: Vec::new(),
            curtains: Vec::new(),
        }
    }

    #[test]
    fn longitudinal_and_transverse_stair_sections_use_mesh_contours() {
        let stair = Id::new();
        let material = Id::new();
        let longitudinal_context = context();
        let longitudinal = snapshot(
            longitudinal_context,
            os_geometry::section::VerticalSectionPlane {
                origin: Point2::new(0.0, 0.0),
                direction: Point2::new(1.0, 0.0),
            },
            stair,
            material,
        )
        .derive()
        .unwrap();
        let longitudinal_lines = longitudinal.provider_lines(longitudinal_context).unwrap();
        assert!(longitudinal_lines.len() > 8);
        assert!(longitudinal_lines.iter().all(|line| {
            line.entity == stair
                && line.role == PlanRole::Cut
                && longitudinal.line_surface(stair, line.feature).material == Some(material)
        }));

        let transverse_context = context();
        let transverse = snapshot(
            transverse_context,
            os_geometry::section::VerticalSectionPlane {
                origin: Point2::new(2.125, -1.0),
                direction: Point2::new(0.0, 1.0),
            },
            stair,
            material,
        )
        .derive()
        .unwrap();
        let transverse_lines = transverse.provider_lines(transverse_context).unwrap();
        assert_eq!(transverse_lines.len(), 4);
        assert!(transverse_lines.iter().all(|line| line.entity == stair));
    }

    #[test]
    fn maximum_riser_section_uses_bounded_native_stair_line_path() {
        let context = context();
        let stair = Id::new();
        let mut source = snapshot(
            context,
            os_geometry::section::VerticalSectionPlane {
                origin: Point2::new(0.0, 0.0),
                direction: Point2::new(1.0, 0.0),
            },
            stair,
            Id::new(),
        );
        source.stairs[0].1.riser_count = 256;
        let drawing = source.derive().unwrap();
        let lines = drawing.provider_lines(context).unwrap();
        assert!(lines.len() > 256);
        assert!(lines.len() <= 772);
        assert!(lines.iter().all(|line| line.entity == stair));
    }

    #[test]
    fn editor_plan_snapshot_projects_level_connected_stair() {
        use os_model::{Level, LevelParams, Stair};

        let mut editor = Editor::new().unwrap();
        let lower = *editor.document.model().levels.keys().next().unwrap();
        let building = editor.document.model().levels[&lower].parameters.building;
        let upper = Level::new(
            "core.level",
            LevelParams {
                name: "Upper".into(),
                elevation: 2.0,
                building,
            },
        );
        let upper_id = upper.id();
        editor
            .command("Add level", Command::AddLevel(upper))
            .unwrap();
        let stair = Stair::new(
            "core.stair",
            os_model::StairParams {
                name: "Flight".into(),
                lower_level: lower,
                upper_level: upper_id,
                start: Point2::new(0.0, 0.0),
                end: Point2::new(4.0, 0.0),
                width: 1.0,
                riser_count: 8,
                structural_thickness: 0.2,
                material: None,
            },
        );
        let stair_id = stair.id();
        editor
            .command("Add stair", Command::AddStair(stair))
            .unwrap();
        let view = editor.create_floor_plan("Ground", lower).unwrap();
        let context = editor.native_plan_context(view).unwrap();
        let drawing = editor.native_wall_plan(view).unwrap();
        assert_eq!(drawing.stairs(context).unwrap().len(), 1);
        let parameters = &editor.document.model().stairs[&stair_id].parameters;
        let lower_z = editor.document.model().levels[&lower].parameters.elevation;
        let preview = project_stair_for_plan(stair_id, parameters, lower_z, 2.0, context).unwrap();
        assert_eq!(drawing.stairs(context).unwrap()[0], preview);
        assert_eq!(
            drawing.pick(context, Point2::new(0.75, 0.0)).unwrap(),
            Some(stair_id)
        );
        assert_eq!(
            drawing.pick_stair(context, Point2::new(0.75, 0.0)).unwrap(),
            Some(stair_id)
        );
        assert!(
            drawing
                .items(context)
                .unwrap()
                .iter()
                .any(|item| item.entity == stair_id && item.footprint.role == PlanRole::Cut)
        );
    }

    #[test]
    fn placement_preview_helper_is_deterministic_without_document_mutation() {
        use os_model::{Level, LevelParams};

        let mut editor = Editor::new().unwrap();
        let lower = *editor.document.model().levels.keys().next().unwrap();
        let building = editor.document.model().levels[&lower].parameters.building;
        let upper = Level::new(
            "core.level",
            LevelParams {
                name: "Upper".into(),
                elevation: 2.0,
                building,
            },
        );
        let upper_id = upper.id();
        editor
            .command("Add level", Command::AddLevel(upper))
            .unwrap();
        let view = editor.create_floor_plan("Draft", lower).unwrap();
        let context = editor.native_plan_context(view).unwrap();
        let params = os_model::StairParams {
            name: "Uncommitted flight".into(),
            lower_level: lower,
            upper_level: upper_id,
            start: Point2::new(0.0, 0.0),
            end: Point2::new(4.0, 0.0),
            width: 1.0,
            riser_count: 8,
            structural_thickness: 0.2,
            material: None,
        };
        let draft_id = Id::new();
        let revision = editor.document.revision();
        let before = editor.document.model().stairs.len();
        let first = project_stair_for_plan(draft_id, &params, 0.0, 2.0, context).unwrap();
        let second = project_stair_for_plan(draft_id, &params, 0.0, 2.0, context).unwrap();
        assert_eq!(first, second);
        assert!(first.contains(Point2::new(0.75, 0.0)));
        assert!(first.visible_strokes().count() > 8);
        assert_eq!(editor.document.model().stairs.len(), before);
        assert_eq!(editor.document.revision(), revision);
    }
}

#[cfg(test)]
#[path = "plan/curtain_section_tests.rs"]
mod curtain_section_tests;

#[cfg(test)]
#[path = "plan/railing_drawing_tests.rs"]
mod railing_drawing_tests;

fn derive_dimension_graphics(
    context: PlanContext,
    plan_level: Id,
    walls: &[(Id, Id, os_model::WallPath, f64)],
    openings: &[(Id, os_model::ResolvedOpening)],
    dimensions: &[(Id, DimensionParams)],
) -> Result<Vec<PlanDimensionItem>> {
    let walls: BTreeMap<_, _> = walls
        .iter()
        .map(|(id, level, path, thickness)| (*id, (*level, *path, *thickness)))
        .collect();
    let openings: BTreeMap<_, _> = openings.iter().map(|(id, p)| (*id, p)).collect();
    let mut output = Vec::new();
    let mut count = 0usize;
    for (entity, parameters) in dimensions {
        if parameters.layout == os_model::DimensionLayout::Angular {
            continue;
        }
        let orphan_hint = context.basis.world_to_plane(parameters.orphan_hint)?;
        let resolved = parameters.resolve_anchors_with(|reference| {
            reference.resolve_with(
                plan_level,
                |id| walls.get(&id).copied(),
                |id| {
                    let p = openings
                        .get(&id)
                        .ok_or(os_model::DimensionDiagnostic::MissingOpening)?;
                    Ok((p.host, p.offset, p.width))
                },
            )
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
