//! Persisted named-plan commands and read-only native-wall plan derivation.
use crate::Editor;
use os_core::{Id, Result, ensure};
use os_document::Command;
use os_geometry::plan::{HorizontalBasis, PlanCrop, PlanRange};
use os_model::{PlanSettings, View, ViewKind, ViewParams};
use os_render::plan::{MAX_PLAN_ELEMENTS, PlanContext, PlanDrawing};
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
                .saturating_add(model.extensions.len())
                .saturating_add(model.grids.len())
                <= MAX_PLAN_ELEMENTS,
            "plan exceeds 10000 elements",
        )?;
        let mut walls = Vec::new();
        if context.show_walls {
            for (id, wall) in &model.walls {
                let level = &model.levels[&wall.parameters.level];
                // Copy only bounded geometry inputs, not names, metadata or payloads.
                let p = &wall.parameters;
                walls.push((
                    *id,
                    os_model::WallParams {
                        name: "Plan geometry".into(),
                        start: p.start,
                        end: p.end,
                        thickness: p.thickness,
                        height: p.height,
                        level: p.level,
                        material: None,
                    },
                    level.parameters.elevation,
                ));
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
        Ok(PlanSnapshot {
            context,
            grids,
            walls,
            unavailable,
        })
    }
}

pub(crate) struct PlanSnapshot {
    pub context: PlanContext,
    walls: Vec<(Id, os_model::WallParams, f64)>,
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
        let mut solids = BTreeMap::new();
        let mut segments = Vec::new();
        for (id, parameters, elevation) in self.walls {
            solids.insert(id, os_walls::wall_solid(&parameters, elevation)?);
            segments.push(os_render::snapping::SnapSegment {
                entity: id,
                feature: 0,
                start: parameters.start,
                end: parameters.end,
            });
        }
        let drawing = PlanDrawing::from_prisms(self.context, &solids, self.unavailable)?;
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
        drawing
            .with_grids(grids)?
            .with_provider_lines(lines)?
            .with_snap_segments(segments)
    }
}
