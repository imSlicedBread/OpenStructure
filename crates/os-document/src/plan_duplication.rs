//! Independent native floor-plan copies, prepared and committed through normal commands.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanDuplicateMode {
    Plain,
    WithDetailing,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlanDuplicateCounts {
    pub dimensions: usize,
    pub opening_tags: usize,
    pub room_tags: usize,
    pub detail_lines: usize,
    pub text_notes: usize,
    pub callouts: usize,
}
impl PlanDuplicateCounts {
    pub fn total(self) -> usize {
        self.dimensions
            + self.opening_tags
            + self.room_tags
            + self.detail_lines
            + self.text_notes
            + self.callouts
    }
}

/// Disposable, session-bound proposal. IDs survive its one-step undo/redo.
pub struct PlanDuplicate {
    session: Id,
    revision: u64,
    source: Id,
    target: Id,
    name: String,
    commands: Vec<Command>,
}
impl PlanDuplicate {
    pub fn target(&self) -> Id {
        self.target
    }
}

fn remap_header(header: &mut Header, ids: &BTreeMap<Id, Id>) {
    header.id = ids[&header.id];
    for id in header.relationships.values_mut().flatten() {
        if let Some(new) = ids.get(id) {
            *id = *new;
        }
    }
    // Opaque properties are deliberately copied verbatim, never searched for UUID strings.
}

impl Document {
    fn duplicate_source(&self, source: Id) -> Result<&View> {
        let view = self
            .model
            .views
            .get(&source)
            .ok_or_else(|| Error::Invalid("source view missing".into()))?;
        ensure(
            view.parameters.kind == ViewKind::Plan
                && view.parameters.level.is_some()
                && view
                    .parameters
                    .plan
                    .is_some_and(|p| p.view_type == PlanViewType::FloorPlan),
            "duplication requires an authored native floor plan",
        )?;
        Ok(view)
    }

    pub fn validate_plan_duplicate_name(&self, name: &str) -> Result<()> {
        ensure(
            !name.trim().is_empty() && name.len() <= 256 && !name.chars().any(char::is_control),
            "duplicate name must be nonblank, at most 256 bytes, without controls",
        )?;
        let normalized = name.trim().to_lowercase();
        ensure(
            !self
                .model
                .views
                .values()
                .any(|v| v.parameters.name.trim().to_lowercase() == normalized),
            "a view with this name already exists",
        )
    }

    pub fn plan_duplicate_name(&self, source: Id) -> Result<String> {
        let source = &self.duplicate_source(source)?.parameters.name;
        let clean: String = source.trim().chars().filter(|c| !c.is_control()).collect();
        for index in 1.. {
            let suffix = if index == 1 {
                " Copy".into()
            } else {
                format!(" Copy {index}")
            };
            let mut end = clean.len().min(256 - suffix.len());
            while !clean.is_char_boundary(end) {
                end -= 1;
            }
            let name = format!("{}{suffix}", &clean[..end]);
            if self.validate_plan_duplicate_name(&name).is_ok() {
                return Ok(name);
            }
        }
        unreachable!()
    }

    pub fn plan_duplicate_counts(&self, source: Id) -> Result<PlanDuplicateCounts> {
        self.duplicate_source(source)?;
        let m = &self.model;
        Ok(PlanDuplicateCounts {
            dimensions: m
                .dimensions
                .values()
                .filter(|e| e.parameters.view == source)
                .count(),
            opening_tags: m
                .opening_tags
                .values()
                .filter(|e| e.parameters.view == source)
                .count(),
            room_tags: m
                .room_tags
                .values()
                .filter(|e| e.parameters.view == source)
                .count(),
            detail_lines: m
                .detail_lines
                .values()
                .filter(|e| e.parameters.view == source)
                .count(),
            text_notes: m
                .text_notes
                .values()
                .filter(|e| e.parameters.view == source)
                .count(),
            callouts: m
                .plan_callouts
                .values()
                .filter(|e| e.parameters.parent == source)
                .count(),
        })
    }

    pub fn prepare_plan_duplicate(
        &self,
        source: Id,
        name: String,
        mode: PlanDuplicateMode,
    ) -> Result<PlanDuplicate> {
        let mut view = self.duplicate_source(source)?.clone();
        self.validate_plan_duplicate_name(&name)?;
        let target = Id::new();
        let mut ids = BTreeMap::from([(source, target)]);
        let mut annotations = Vec::new();
        if mode == PlanDuplicateMode::WithDetailing {
            ensure(
                self.plan_duplicate_counts(source)?.total() <= 4096,
                "duplication exceeds 4096 owned annotations/markers",
            )?;
            macro_rules! collect {
                ($field:ident, $owner:ident, $command:ident) => {
                    for entity in self
                        .model
                        .$field
                        .values()
                        .filter(|e| e.parameters.$owner == source)
                    {
                        ids.insert(entity.id(), Id::new());
                        let mut copy = entity.clone();
                        copy.parameters.$owner = target;
                        annotations.push(Command::$command(copy));
                    }
                };
            }
            collect!(dimensions, view, AddDimension);
            collect!(opening_tags, view, AddOpeningTag);
            collect!(room_tags, view, AddRoomTag);
            collect!(detail_lines, view, AddDetailLine);
            collect!(text_notes, view, AddTextNote);
            collect!(plan_callouts, parent, AddPlanCallout);
            for extension in self.model.extensions.values() {
                ensure(
                    !extension.references().any(|id| ids.contains_key(id)),
                    format!("unsupported extension payload cloning: {}", extension.id),
                )?;
            }
            // Existing creation validation is authoritative, including retained orphans.
            for entity in self
                .model
                .dimensions
                .values()
                .filter(|e| e.parameters.view == source)
            {
                entity
                    .parameters
                    .validate_creation(&self.model)
                    .map_err(|e| Error::Invalid(format!("annotation {}: {e}", entity.id())))?;
            }
            for entity in self
                .model
                .opening_tags
                .values()
                .filter(|e| e.parameters.view == source)
            {
                entity
                    .parameters
                    .validate_creation(&self.model)
                    .map_err(|e| Error::Invalid(format!("annotation {}: {e}", entity.id())))?;
            }
            for entity in self
                .model
                .room_tags
                .values()
                .filter(|e| e.parameters.view == source)
            {
                entity
                    .parameters
                    .validate_creation(&self.model)
                    .map_err(|e| Error::Invalid(format!("annotation {}: {e}", entity.id())))?;
            }
        }
        remap_header(&mut view.header, &ids);
        view.parameters.name = name.clone();
        view.parameters.settings_revision = 0;
        let mut commands = vec![Command::AddView(view)];
        if let Some(hidden) = self.model.plan_hidden_openings.get(&source) {
            commands.push(Command::SetPlanHiddenOpenings {
                view: target,
                hidden: hidden.clone(),
            });
        }
        if let Some(binding) = self.model.plan_graphics.get(&source) {
            commands.push(Command::SetPlanGraphics {
                view: target,
                binding: Some(*binding),
            });
        }
        if let Some(overrides) = self.model.plan_element_graphics_overrides.get(&source) {
            commands.push(Command::SetPlanElementGraphicsOverrides {
                view: target,
                overrides: overrides.clone(),
            });
        }
        let rules = self.model.copied_plan_graphics_rules(source);
        if !rules.is_empty() {
            commands.push(Command::SetPlanGraphicsRules {
                view: target,
                rules,
            });
        }
        for command in &mut annotations {
            let header = match command {
                Command::AddDimension(e) => &mut e.header,
                Command::AddOpeningTag(e) => &mut e.header,
                Command::AddRoomTag(e) => &mut e.header,
                Command::AddDetailLine(e) => &mut e.header,
                Command::AddTextNote(e) => &mut e.header,
                Command::AddPlanCallout(e) => {
                    e.parameters.owns_target = false;
                    &mut e.header
                }
                _ => unreachable!(),
            };
            remap_header(header, &ids);
        }
        commands.extend(annotations);
        self.preview_commands(commands.clone())?;
        Ok(PlanDuplicate {
            session: self.session_id,
            revision: self.revision,
            source,
            target,
            name,
            commands,
        })
    }

    pub fn apply_plan_duplicate(&mut self, draft: PlanDuplicate) -> Result<Id> {
        ensure(
            draft.session == self.session_id && draft.revision == self.revision,
            "stale plan duplication; reopen the dialog",
        )?;
        self.duplicate_source(draft.source)?;
        self.validate_plan_duplicate_name(&draft.name)?;
        self.execute("Duplicate plan", draft.commands)?;
        Ok(draft.target)
    }

    /// Removes owned detailing, never shared targets. Normal reference/issue guards apply.
    pub fn delete_plan(&mut self, view: Id) -> Result<()> {
        self.duplicate_source(view)?;
        let mut commands = Vec::new();
        macro_rules! remove {
            ($field:ident, $owner:ident, $command:ident) => {
                commands.extend(
                    self.model
                        .$field
                        .values()
                        .filter(|e| e.parameters.$owner == view)
                        .map(|e| Command::$command(e.id())),
                );
            };
        }
        remove!(dimensions, view, RemoveDimension);
        remove!(opening_tags, view, RemoveOpeningTag);
        remove!(room_tags, view, RemoveRoomTag);
        remove!(detail_lines, view, RemoveDetailLine);
        remove!(text_notes, view, RemoveTextNote);
        remove!(plan_callouts, parent, RemovePlanCallout);
        commands.push(Command::RemoveView(view));
        self.execute("Delete plan and owned detailing", commands)
    }
}
