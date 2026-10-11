//! Frozen linked wall elevations share the native drawing worker and output path.
use super::*;
use os_model::{ElevationSide, ElevationViewSettings, Model};
use os_render::plan::PlanLine;

pub(crate) struct Snapshot {
    context: PlanContext,
    geometry: os_geometry::elevation::ElevationSnapshot,
    dimensions: Vec<os_render::plan::PlanDimensionItem>,
}

impl Snapshot {
    pub(crate) fn derive(self) -> Result<PlanDrawing> {
        let mut lines: BTreeMap<Id, Vec<PlanLine>> = BTreeMap::new();
        let mut surfaces = BTreeMap::new();
        for segment in self.geometry.derive()? {
            let output = lines.entry(segment.entity).or_default();
            let feature = output.len() as u32;
            output.push(PlanLine {
                entity: segment.entity,
                feature,
                start: segment.start,
                end: segment.end,
                role: PlanRole::Projected,
            });
            surfaces.insert((segment.entity, feature), segment.surface);
        }
        PlanDrawing::from_prisms(
            self.context,
            &BTreeMap::new(),
            lines.keys().copied().collect(),
        )?
        .with_native_lines(lines)
        .and_then(|drawing| {
            drawing
                .with_line_surfaces(surfaces)
                .with_dimensions(self.dimensions)
        })
    }
}

pub(crate) fn preview(model: &Model, context: PlanContext) -> Result<PlanDrawing> {
    let settings = model.views[&context.view_id]
        .parameters
        .elevation
        .ok_or_else(|| os_core::Error::Invalid("Elevation settings missing".into()))?;
    settings.resolve_host(model)?;
    Snapshot {
        context,
        dimensions: dimension_graphics(model, context.view_id),
        geometry: os_geometry::elevation::ElevationSnapshot::capture(
            model,
            settings.host,
            settings.side == ElevationSide::Left,
        )?,
    }
    .derive()
}

impl Editor {
    pub fn create_wall_elevation(
        &mut self,
        name: &str,
        host: Id,
        side: ElevationSide,
    ) -> Result<Id> {
        let model = self.document.model();
        let wall = model.resolve_wall(host)?.parameters;
        ensure(
            wall.path.is_straight(),
            "Wall elevations do not support curved hosts",
        )?;
        let z = model.levels[&wall.level].parameters.elevation;
        let settings = ElevationViewSettings {
            schema_version: 1,
            host,
            side,
            min_station: -0.25,
            max_station: wall.length() + 0.25,
            bottom_elevation: z - 0.25,
            top_elevation: z + wall.height + 0.25,
        };
        settings.resolve_host(model)?;
        let view = View::new(
            "core.view",
            ViewParams {
                name: name.into(),
                kind: ViewKind::Elevation,
                level: None,
                settings_revision: 0,
                plan: None,
                section: None,
                facade: None,
                elevation: Some(settings),
            },
        );
        let id = view.id();
        self.command("Create wall elevation", Command::AddView(view))?;
        Ok(id)
    }

    pub fn native_elevation_context(&self, view: Id) -> Result<PlanContext> {
        let model = self.document.model();
        let view = model
            .views
            .get(&view)
            .ok_or_else(|| os_core::Error::Invalid("Elevation view missing".into()))?;
        ensure(
            view.parameters.kind == ViewKind::Elevation,
            "View is not an elevation",
        )?;
        let s = view
            .parameters
            .elevation
            .ok_or_else(|| os_core::Error::Invalid("Elevation settings missing".into()))?;
        s.resolve_host(model)?;
        Ok(PlanContext {
            session_id: self.document.session_id(),
            model_revision: self.document.revision(),
            view_id: view.id(),
            settings_revision: view.parameters.settings_revision,
            basis: HorizontalBasis::default(),
            range: PlanRange {
                top: s.top_elevation,
                cut: (s.bottom_elevation + s.top_elevation) * 0.5,
                bottom: s.bottom_elevation,
                depth: s.bottom_elevation,
            },
            crop: Some(PlanCrop {
                min: Point2::new(s.min_station, s.bottom_elevation),
                max: Point2::new(s.max_station, s.top_elevation),
            }),
            scale_denominator: 100.0,
            show_walls: true,
            show_extensions: false,
        })
    }

    pub(crate) fn elevation_snapshot(&self, view: Id) -> Result<Snapshot> {
        let context = self.native_elevation_context(view)?;
        let s = self.document.model().views[&view]
            .parameters
            .elevation
            .unwrap();
        Ok(Snapshot {
            context,
            dimensions: dimension_graphics(self.document.model(), view),
            geometry: os_geometry::elevation::ElevationSnapshot::capture(
                self.document.model(),
                s.host,
                s.side == ElevationSide::Left,
            )?,
        })
    }
}

pub(crate) fn dimension_graphic(
    model: &Model,
    id: Id,
    p: &os_model::DimensionParams,
) -> os_render::plan::PlanDimensionItem {
    let mut item = os_render::plan::PlanDimensionItem {
        entity: id,
        spans: Vec::new(),
        shared_start_witness: false,
        witness_start: p.orphan_hint,
        witness_end: p.orphan_hint,
        line_start: p.orphan_hint,
        line_end: p.orphan_hint,
        value_m: None,
        orphan_hint: p.orphan_hint,
        diagnostic: None,
    };
    match p.resolve(model) {
        Ok(r) => {
            item.witness_start = r.first;
            item.witness_end = r.second;
            (item.line_start, item.line_end) = if p.layout.is_section() {
                p.section_line(r)
            } else {
                p.elevation_line(r)
            };
            item.value_m = Some(r.length_metres);
        }
        Err(reason) => item.diagnostic = Some(format!("{reason:?}")),
    }
    item
}

fn dimension_graphics(model: &Model, view: Id) -> Vec<os_render::plan::PlanDimensionItem> {
    model
        .dimensions
        .values()
        .filter(|d| d.parameters.view == view)
        .map(|d| dimension_graphic(model, d.id(), &d.parameters))
        .collect()
}
