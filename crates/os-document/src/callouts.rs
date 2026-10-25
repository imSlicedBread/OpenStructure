use super::*;

impl Document {
    /// Copies settings and resolved graphics by value. Annotations remain owned by
    /// their original views; all views read the same authoritative model entities.
    pub fn enlarged_plan_commands(
        &self,
        parent: Id,
        name: String,
        crop: PlanViewCrop,
        scale: f64,
    ) -> Result<(Id, Id, Vec<Command>)> {
        let source = self
            .model
            .views
            .get(&parent)
            .ok_or_else(|| Error::Invalid("parent missing".into()))?;
        let mut parameters = source.parameters.clone();
        ensure(
            parameters.kind == ViewKind::Plan
                && parameters
                    .plan
                    .is_some_and(|p| p.view_type == PlanViewType::FloorPlan),
            "callouts require a native floor plan",
        )?;
        parameters.name = name;
        parameters.settings_revision = 0;
        let settings = parameters.plan.as_mut().expect("checked plan");
        settings.crop = Some(crop);
        settings.scale_denominator = scale;
        parameters.validate_edit()?;
        let target = View::new("core.view", parameters);
        let target_id = target.id();
        let marker = PlanCallout::new(
            "core.plan_callout",
            PlanCalloutParams {
                parent,
                target: target_id,
                owns_target: true,
                boundary: crop,
                destination: None,
            },
        );
        let marker_id = marker.id();
        let mut commands = vec![Command::AddView(target), Command::AddPlanCallout(marker)];
        if let Some(styles) = self.model.plan_graphics_styles(parent) {
            commands.push(Command::SetPlanGraphics {
                view: target_id,
                binding: Some(PlanGraphicsBinding {
                    template: None,
                    local: styles,
                }),
            });
        }
        if let Some(overrides) = self.model.plan_element_graphics_overrides.get(&parent) {
            commands.push(Command::SetPlanElementGraphicsOverrides {
                view: target_id,
                overrides: overrides.clone(),
            });
        }
        let rules = self.model.copied_plan_graphics_rules(parent);
        if !rules.is_empty() {
            commands.push(Command::SetPlanGraphicsRules {
                view: target_id,
                rules,
            });
        }
        self.preview_commands(commands.clone())?;
        Ok((marker_id, target_id, commands))
    }
}

impl Document {
    pub fn enlarged_section_commands(
        &self,
        parent: Id,
        name: String,
        crop: SectionCalloutBounds,
        scale: f64,
    ) -> Result<(Id, Id, Vec<Command>)> {
        let source = self
            .model
            .views
            .get(&parent)
            .ok_or_else(|| Error::Invalid("parent missing".into()))?;
        let mut parameters = source.parameters.clone();
        ensure(
            parameters.kind == ViewKind::Section && parameters.section.is_some(),
            "callouts require a native section",
        )?;
        crop.validate()?;
        parameters.name = name;
        parameters.settings_revision = 0;
        let settings = parameters.section.as_mut().expect("checked section");
        settings.min_station = crop.min.x;
        settings.max_station = crop.max.x;
        settings.bottom_elevation = crop.min.y;
        settings.top_elevation = crop.max.y;
        settings.scale_denominator = scale;
        parameters.validate_edit()?;
        let target = View::new("core.view", parameters);
        let target_id = target.id();
        let marker = SectionCallout::new(
            "core.section_callout",
            SectionCalloutParams {
                parent,
                target: target_id,
                owns_target: true,
                boundary: crop,
                destination: None,
            },
        );
        let marker_id = marker.id();
        let commands = vec![Command::AddView(target), Command::AddSectionCallout(marker)];
        self.preview_commands(commands.clone())?;
        Ok((marker_id, target_id, commands))
    }
}
