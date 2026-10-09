//! Facades use frozen geometry and the same checked bounded drawing worker.
use super::*;
use os_model::FacadeViewSettings;
pub(crate) struct Snapshot {
    context: PlanContext,
    geometry: os_geometry::facade::FacadeSnapshot,
}
impl Snapshot {
    pub(crate) fn derive(self) -> Result<PlanDrawing> {
        let mut lines: BTreeMap<Id, Vec<os_render::plan::PlanLine>> = BTreeMap::new();
        let mut surfaces = BTreeMap::new();
        for s in self.geometry.derive()? {
            let output = lines.entry(s.entity).or_default();
            let feature = output.len() as u32;
            output.push(os_render::plan::PlanLine {
                entity: s.entity,
                feature,
                start: s.start,
                end: s.end,
                role: PlanRole::Projected,
            });
            surfaces.insert((s.entity, feature), s.surface);
        }
        PlanDrawing::from_prisms(
            self.context,
            &BTreeMap::new(),
            lines.keys().copied().collect(),
        )?
        .with_native_lines(lines)
        .map(|d| d.with_line_surfaces(surfaces))
    }
}
impl Editor {
    pub fn create_facade_view(
        &mut self,
        name: &str,
        level: Id,
        settings: FacadeViewSettings,
    ) -> Result<Id> {
        os_geometry::facade::FacadeSnapshot::capture(self.document.model(), level, settings)?;
        let view = View::new(
            "core.view",
            ViewParams {
                name: name.into(),
                kind: ViewKind::Facade,
                level: Some(level),
                settings_revision: 0,
                plan: None,
                section: None,
                elevation: None,
                facade: Some(settings),
            },
        );
        let id = view.id();
        self.command("Create facade and plan marker", Command::AddView(view))?;
        Ok(id)
    }
    pub fn native_facade_context(&self, id: Id) -> Result<PlanContext> {
        let view = self
            .document
            .model()
            .views
            .get(&id)
            .ok_or_else(|| os_core::Error::Invalid("facade view missing".into()))?;
        ensure(
            view.parameters.kind == ViewKind::Facade,
            "view is not a facade",
        )?;
        view.parameters.validate_edit()?;
        let s = view.parameters.facade.unwrap();
        ensure(
            view.parameters
                .level
                .is_some_and(|id| self.document.model().levels.contains_key(&id)),
            "facade marker level missing",
        )?;
        Ok(PlanContext {
            session_id: self.document.session_id(),
            model_revision: self.document.revision(),
            view_id: id,
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
            scale_denominator: 100.,
            show_walls: true,
            show_extensions: false,
        })
    }
    pub(crate) fn facade_snapshot(&self, id: Id) -> Result<Snapshot> {
        let context = self.native_facade_context(id)?;
        let p = &self.document.model().views[&id].parameters;
        Ok(Snapshot {
            context,
            geometry: os_geometry::facade::FacadeSnapshot::capture(
                self.document.model(),
                p.level.unwrap(),
                p.facade.unwrap(),
            )?,
        })
    }
}
