//! Atomic commands, undo/redo and dependency change events.
//!
//! ```
//! use os_document::{Command, Document};
//! let mut document = Document::new("Example")?;
//! document.execute("Rename", vec![Command::RenameProject("Building A".into())])?;
//! assert!(document.undo());
//! assert_eq!(document.model().project.parameters.name, "Example");
//! # Ok::<(), os_core::Error>(())
//! ```
use os_core::{Error, Id, Result, ensure};
use os_model::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
mod callouts;
mod plan_duplication;
pub use plan_duplication::{PlanDuplicate, PlanDuplicateCounts, PlanDuplicateMode};
mod drawing_issues;
mod history;
use history::{History, HistoryStore};
pub use history::{HistoryLimits, HistoryStats};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "command", content = "data")]
pub enum Command {
    /// Capture existing walls and their supported dependents without changing live IDs.
    CreateWallGroup {
        walls: BTreeSet<Id>,
        view: Id,
        name: String,
    },
    /// Translation is relative to the source instance's current placement.
    DuplicateGroupInstance {
        id: Id,
        translation: os_core::Point2,
    },
    /// Explicit identities allocated once for a transient preview and its commit.
    DuplicateGroupInstancePrepared {
        id: Id,
        translation: os_core::Point2,
        instance: Id,
        members: BTreeMap<Id, Id>,
    },
    SetGroupInstanceTransform {
        id: Id,
        transform: GroupTransform,
    },
    UpdateGroupDefinition {
        id: Id,
        parameters: GroupDefinitionParams,
    },
    MakeGroupInstanceUnique {
        id: Id,
        name: String,
    },
    UngroupInstance(Id),
    SetPlanHiddenOpenings {
        view: Id,
        hidden: BTreeSet<Id>,
    },
    AddTextNote(TextNote),
    UpdateTextNote {
        id: Id,
        parameters: TextNoteParams,
    },
    RemoveTextNote(Id),
    AddPlanCallout(PlanCallout),
    UpdatePlanCallout {
        id: Id,
        parameters: PlanCalloutParams,
    },
    RemovePlanCallout(Id),
    AddSectionCallout(SectionCallout),
    UpdateSectionCallout {
        id: Id,
        parameters: SectionCalloutParams,
    },
    RemoveSectionCallout(Id),
    PutSheetSet(SheetSet),
    RemoveSheetSet(Id),
    PutDrawingIssue(DrawingIssue),
    RemoveDrawingIssue(Id),
    AddCurtainSystem(CurtainSystem),
    UpdateCurtainSystem {
        id: Id,
        parameters: CurtainSystemParams,
    },
    RemoveCurtainSystem(Id),
    AddCurtainPanelType(CurtainPanelType),
    UpdateCurtainPanelType {
        id: Id,
        parameters: CurtainPanelTypeParams,
    },
    RemoveCurtainPanelType(Id),
    AddCurtainMullionType(CurtainMullionType),
    UpdateCurtainMullionType {
        id: Id,
        parameters: CurtainMullionTypeParams,
    },
    RemoveCurtainMullionType(Id),
    SetOpeningClearance {
        id: Id,
        clearance: Option<OpeningClearance>,
    },
    AddPhase(Phase),
    UpdatePhase {
        id: Id,
        parameters: PhaseParams,
    },
    RemovePhase(Id),
    SetElementLifecycle {
        element: Id,
        lifecycle: ElementLifecycle,
    },
    AddPlanGraphicsTemplate(PlanGraphicsTemplate),
    UpdatePlanGraphicsTemplate {
        id: Id,
        parameters: PlanGraphicsTemplateParams,
    },
    RemovePlanGraphicsTemplate(Id),
    SetPlanGraphics {
        view: Id,
        binding: Option<PlanGraphicsBinding>,
    },
    SetPlanElementGraphicsOverrides {
        view: Id,
        overrides: BTreeMap<Id, PlanElementGraphicsOverride>,
    },
    SetPlanGraphicsRules {
        view: Id,
        rules: Vec<PlanGraphicsRule>,
    },
    AddMaterial(Material),
    AddMaterialPattern(MaterialPattern),
    UpdateMaterialPattern {
        id: Id,
        parameters: MaterialPatternParams,
    },
    RemoveMaterialPattern(Id),
    UpdateMaterial {
        id: Id,
        parameters: MaterialParams,
    },
    RemoveMaterial(Id),
    AddWallType(WallType),
    UpdateWallType {
        id: Id,
        parameters: WallTypeParams,
    },
    RemoveWallType(Id),
    AssignWallType {
        wall: Id,
        assignment: Option<WallTypeAssignment>,
    },
    AddButtJoin(ButtJoin),
    AddWallJoin(WallJoin),
    UpdateWallJoin {
        id: Id,
        parameters: WallJoinParams,
    },
    RemoveWallJoin(Id),
    UpdateButtJoin {
        id: Id,
        parameters: ButtJoinParams,
    },
    RemoveButtJoin(Id),
    AddSchedule(Schedule),
    UpdateSchedule {
        id: Id,
        parameters: ScheduleParams,
    },
    RemoveSchedule(Id),
    AddSheet(Sheet),
    UpdateSheet {
        id: Id,
        parameters: SheetParams,
    },
    RemoveSheet(Id),
    AddRoomTag(RoomTag),
    AddOpeningTag(OpeningTag),
    UpdateOpeningTag {
        id: Id,
        parameters: OpeningTagParams,
    },
    RemoveOpeningTag(Id),
    AddDetailLine(DetailLine),
    UpdateDetailLine {
        id: Id,
        parameters: DetailLineParams,
    },
    RemoveDetailLine(Id),
    AddRoomSeparationLine(RoomSeparationLine),
    UpdateRoomSeparationLine {
        id: Id,
        parameters: RoomSeparationLineParams,
    },
    RemoveRoomSeparationLine(Id),
    UpdateRoomTag {
        id: Id,
        parameters: RoomTagParams,
    },
    RemoveRoomTag(Id),
    AddFloor(Floor),
    AddColumn(Column),
    AddCaseworkType(CaseworkType),
    AddCasework(Casework),
    AddStair(Stair),
    AddRamp(Ramp),
    UpdateRamp {
        id: Id,
        parameters: RampParams,
    },
    RemoveRamp(Id),
    AddRailing(StairRailing),
    UpdateRailing {
        id: Id,
        parameters: StairRailingParams,
    },
    RemoveRailing(Id),
    AddRailingType(StairRailingType),
    UpdateRailingType {
        id: Id,
        parameters: StairRailingTypeParams,
    },
    RemoveRailingType(Id),
    AddCeiling(Ceiling),
    UpdateCeiling {
        id: Id,
        parameters: CeilingParams,
    },
    RemoveCeiling(Id),
    AddRoof(Roof),
    UpdateRoof {
        id: Id,
        parameters: RoofParams,
    },
    RemoveRoof(Id),
    UpdateStair {
        id: Id,
        parameters: StairParams,
    },
    RemoveStair(Id),
    UpdateColumn {
        id: Id,
        parameters: ColumnParams,
    },
    RemoveColumn(Id),
    UpdateCaseworkType {
        id: Id,
        parameters: CaseworkTypeParams,
    },
    RemoveCaseworkType(Id),
    UpdateCasework {
        id: Id,
        parameters: CaseworkParams,
    },
    RemoveCasework(Id),
    UpdateFloor {
        id: Id,
        parameters: FloorParams,
    },
    RemoveFloor(Id),
    AddOpeningType(OpeningType),
    AddLengthParameter(LengthParameter),
    UpdateLengthParameter {
        id: Id,
        parameters: LengthParameterParams,
    },
    RemoveLengthParameter(Id),
    SetOpeningTypeLengthBindings {
        id: Id,
        bindings: OpeningTypeLengthBindings,
    },
    UpdateOpeningType {
        id: Id,
        parameters: OpeningTypeParams,
    },
    RemoveOpeningType(Id),
    AddDimension(Dimension),
    UpdateDimension {
        id: Id,
        parameters: DimensionParams,
    },
    RemoveDimension(Id),
    AddRoom(Room),
    UpdateRoom {
        id: Id,
        parameters: RoomParams,
    },
    RemoveRoom(Id),
    AddOpening(Opening),
    UpdateOpening {
        id: Id,
        parameters: OpeningParams,
    },
    RemoveOpening(Id),
    AddWall(Wall),
    UpdateWall {
        id: Id,
        parameters: WallParams,
    },
    AddLevel(Level),
    UpdateLevel {
        id: Id,
        parameters: LevelParams,
    },
    RemoveWall(Id),
    RemoveLevel(Id),
    AddView(View),
    UpdateView {
        id: Id,
        parameters: ViewParams,
    },
    RemoveView(Id),
    AddGrid(Grid),
    UpdateGrid {
        id: Id,
        parameters: GridParams,
    },
    RemoveGrid(Id),
    RenameProject(String),
    AddExtension(ExtensionEntity),
    /// Also used for migration proposals. Ownership and identity cannot change.
    ReplaceExtension {
        expected_schema_version: u32,
        entity: ExtensionEntity,
    },
    RemoveExtension(Id),
    SetPluginRequirement {
        plugin_id: String,
        requirement: Option<PluginRequirement>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChangeEvent {
    pub revision: u64,
    pub label: String,
    pub changed: BTreeSet<Id>,
    pub invalidated: BTreeSet<Id>,
}
pub struct Document {
    session_id: Id,
    model: Model,
    auxiliary_files: BTreeMap<String, Vec<u8>>,
    history: HistoryStore,
    events: Vec<ChangeEvent>,
    revision: u64,
}

fn phaseable_ids(model: &Model) -> BTreeSet<Id> {
    let mut ids = BTreeSet::new();
    ids.extend(model.walls.keys().copied());
    ids.extend(model.openings.keys().copied());
    ids.extend(model.floors.keys().copied());
    ids.extend(model.stairs.keys().copied());
    ids.extend(model.ramps.keys().copied());
    ids.extend(model.railings.keys().copied());
    ids.extend(model.curtain_systems.keys().copied());
    ids.extend(model.roofs.keys().copied());
    ids.extend(model.ceilings.keys().copied());
    ids.extend(model.columns.keys().copied());
    ids.extend(model.casework.keys().copied());
    ids.extend(model.rooms.keys().copied());
    ids.extend(model.room_separation_lines.keys().copied());
    ids
}

/// Replace canonical members and optional shared metadata together. Lifecycles
/// belong to live members and are intentionally left untouched.
fn install_group_members(
    model: &mut Model,
    members: GroupMaterialization,
    changed: &mut BTreeSet<Id>,
) {
    for id in members.walls.keys() {
        model.wall_type_assignments.remove(id);
    }
    for id in members.openings.keys() {
        model.opening_clearances.remove(id);
    }
    changed.extend(members.walls.keys().copied());
    changed.extend(members.openings.keys().copied());
    changed.extend(members.joins.keys().copied());
    changed.extend(members.opening_tags.keys().copied());
    model.walls.extend(members.walls);
    model.openings.extend(members.openings);
    model.wall_joins.extend(members.joins);
    model.opening_tags.extend(members.opening_tags);
    model
        .wall_type_assignments
        .extend(members.wall_type_assignments);
    model.opening_clearances.extend(members.opening_clearances);
}

fn ensure_group_materialization_available(
    model: &Model,
    members: &GroupMaterialization,
) -> Result<()> {
    ensure(
        members.walls.keys().all(|id| !model.walls.contains_key(id))
            && members
                .openings
                .keys()
                .all(|id| !model.openings.contains_key(id))
            && members
                .joins
                .keys()
                .all(|id| !model.wall_joins.contains_key(id))
            && members
                .opening_tags
                .keys()
                .all(|id| !model.opening_tags.contains_key(id)),
        "duplicate group member identity already exists",
    )
}

fn group_instance(model: &Model, id: Id) -> Result<GroupInstance> {
    model
        .group_instances
        .get(&id)
        .cloned()
        .ok_or_else(|| Error::Invalid("group instance missing".into()))
}

fn remove_orphan_group_definition(model: &mut Model, definition: Id) {
    if !model
        .group_instances
        .values()
        .any(|instance| instance.parameters.definition == definition)
    {
        model.group_definitions.remove(&definition);
    }
}

impl Document {
    pub fn new(name: &str) -> Result<Self> {
        Self::from_model(Model::new(name))
    }
    pub fn from_model(model: Model) -> Result<Self> {
        Self::from_model_and_files(model, BTreeMap::new())
    }
    /// Construct a loaded document with opaque, inert auxiliary contents.
    /// Storage adapters must validate their format's paths and resource bounds.
    pub fn from_model_and_files(
        model: Model,
        auxiliary_files: BTreeMap<String, Vec<u8>>,
    ) -> Result<Self> {
        os_constraints::validate(&model)?;
        Ok(Self {
            session_id: Id::new(),
            model,
            auxiliary_files,
            history: HistoryStore::default(),
            events: vec![],
            revision: 0,
        })
    }
    pub fn model(&self) -> &Model {
        &self.model
    }
    /// Ephemeral identity of this open document, distinct from its persistent project UUID.
    pub fn session_id(&self) -> Id {
        self.session_id
    }
    /// Preserved bytes, not executable plugins or editable model entities.
    /// No mutable accessor: future asset editing must use transactions.
    pub fn auxiliary_files(&self) -> &BTreeMap<String, Vec<u8>> {
        &self.auxiliary_files
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn can_undo(&self) -> bool {
        !self.history.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.history.redo.is_empty()
    }
    pub fn history_stats(&self) -> HistoryStats {
        self.history.stats()
    }
    /// Adjust session retention without changing the model, revision or events.
    /// Shrinking releases whole oldest entries; impossible limits fail atomically.
    pub fn set_history_limits(&mut self, limits: HistoryLimits) -> Result<()> {
        self.history.set_limits(limits)
    }
    pub fn drain_events(&mut self) -> Vec<ChangeEvent> {
        std::mem::take(&mut self.events)
    }

    /// All commands validate together and either all commit or none do.
    pub fn execute(&mut self, label: &str, commands: Vec<Command>) -> Result<()> {
        let (candidate, changed) = self.prepare_commands(commands)?;
        if candidate == self.model {
            return Ok(());
        }
        let history = History::new(
            label,
            &self.model,
            &candidate,
            changed,
            self.history.stats().limits,
        )?;
        self.emit(&history, label);
        self.model = candidate;
        self.history.record(history);
        Ok(())
    }

    /// Uses exactly the commit evaluator without publishing model/history/events.
    pub fn preview_commands(&self, commands: Vec<Command>) -> Result<Model> {
        self.prepare_commands(commands).map(|(model, _)| model)
    }

    fn prepare_commands(&self, commands: Vec<Command>) -> Result<(Model, BTreeSet<Id>)> {
        ensure(!commands.is_empty(), "empty transaction")?;
        let mut candidate = self.model.clone();
        let original_phaseable = phaseable_ids(&self.model);
        let mut changed = BTreeSet::new();
        let mut edited_views = BTreeSet::new();
        let mut explicit_hidden = BTreeMap::new();
        let mut explicit_plan_element_overrides = BTreeMap::new();
        let mut explicit_plan_graphics_rules = BTreeMap::new();
        for command in commands {
            let command = match command {
                Command::DuplicateGroupInstance { id, translation } => {
                    let source = group_instance(&candidate, id)?;
                    Command::DuplicateGroupInstancePrepared {
                        id,
                        translation,
                        instance: Id::new(),
                        members: source
                            .parameters
                            .members
                            .keys()
                            .map(|slot| (*slot, Id::new()))
                            .collect(),
                    }
                }
                other => other,
            };
            let id = match command {
                Command::CreateWallGroup { walls, view, name } => {
                    let (parameters, members) = GroupDefinitionParams::capture_wall_assembly(
                        &candidate, &walls, view, name,
                    )?;
                    ensure(
                        members
                            .values()
                            .all(|member| candidate.group_instance_for_member(*member).is_none()),
                        "group member already belongs to an instance",
                    )?;
                    let definition = GroupDefinition::create(parameters);
                    ensure(
                        !candidate.group_definitions.contains_key(&definition.id()),
                        "group definition identity collision",
                    )?;
                    let instance = GroupInstance::from_members(definition.id(), members);
                    ensure(
                        !candidate.group_instances.contains_key(&instance.id()),
                        "group instance identity collision",
                    )?;
                    changed.insert(definition.id());
                    changed.extend(instance.parameters.members.values().copied());
                    let id = instance.id();
                    candidate
                        .group_definitions
                        .insert(definition.id(), definition);
                    candidate.group_instances.insert(id, instance);
                    id
                }
                Command::DuplicateGroupInstance { .. } => unreachable!("allocated above"),
                Command::DuplicateGroupInstancePrepared {
                    id,
                    translation,
                    instance: instance_id,
                    members: identities,
                } => {
                    let source = group_instance(&candidate, id)?;
                    let mut parameters = source.parameters.clone();
                    ensure(
                        parameters.members.keys().eq(identities.keys()),
                        "duplicate group requires exactly the source slots",
                    )?;
                    parameters.members = identities;
                    parameters.transform.translation.x += translation.x;
                    parameters.transform.translation.y += translation.y;
                    let definition = candidate
                        .group_definitions
                        .get(&parameters.definition)
                        .ok_or_else(|| Error::Invalid("group definition missing".into()))?;
                    let members = definition.parameters.materialize(&parameters)?;
                    let mut instance = GroupInstance::create(parameters);
                    instance.header.id = instance_id;
                    ensure_group_materialization_available(&candidate, &members)?;
                    ensure(
                        !candidate.group_instances.contains_key(&instance.id()),
                        "group instance identity collision",
                    )?;
                    // Copy the source instance's phase state, independently of its definition.
                    for (slot, member) in &instance.parameters.members {
                        if let Some(lifecycle) = candidate
                            .element_lifecycles
                            .get(&source.parameters.members[slot])
                            .copied()
                        {
                            candidate.element_lifecycles.insert(*member, lifecycle);
                        }
                    }
                    install_group_members(&mut candidate, members, &mut changed);
                    changed.insert(instance.parameters.definition);
                    let id = instance.id();
                    candidate.group_instances.insert(id, instance);
                    id
                }
                Command::SetGroupInstanceTransform { id, transform } => {
                    let mut instance = group_instance(&candidate, id)?;
                    instance.parameters.transform = transform;
                    let definition = candidate
                        .group_definitions
                        .get(&instance.parameters.definition)
                        .ok_or_else(|| Error::Invalid("group definition missing".into()))?;
                    let members = definition.parameters.materialize(&instance.parameters)?;
                    install_group_members(&mut candidate, members, &mut changed);
                    changed.insert(instance.parameters.definition);
                    candidate.group_instances.insert(id, instance);
                    id
                }
                Command::UpdateGroupDefinition { id, parameters } => {
                    let definition = candidate
                        .group_definitions
                        .get(&id)
                        .ok_or_else(|| Error::Invalid("group definition missing".into()))?;
                    let old = &definition.parameters.template;
                    let new = &parameters.template;
                    // Slots retain both identity and entity kind so replacement cannot
                    // leave an obsolete entity in another live map.
                    ensure(
                        old.walls.keys().eq(new.walls.keys())
                            && old.openings.keys().eq(new.openings.keys())
                            && old.joins.keys().eq(new.joins.keys())
                            && old.opening_tags.keys().eq(new.opening_tags.keys()),
                        "group definition update requires stable typed member slots",
                    )?;
                    let instances: Vec<_> = candidate
                        .group_instances
                        .values()
                        .filter(|instance| instance.parameters.definition == id)
                        .cloned()
                        .collect();
                    for instance in instances {
                        let members = parameters.materialize(&instance.parameters)?;
                        install_group_members(&mut candidate, members, &mut changed);
                        changed.insert(instance.id());
                    }
                    candidate.group_definitions.get_mut(&id).unwrap().parameters = parameters;
                    id
                }
                Command::MakeGroupInstanceUnique { id, name } => {
                    let mut instance = group_instance(&candidate, id)?;
                    let old_definition = instance.parameters.definition;
                    let definition = candidate
                        .group_definitions
                        .get(&old_definition)
                        .ok_or_else(|| Error::Invalid("group definition missing".into()))?;
                    // Definitions own distinct slot IDs. Remap the canonical template
                    // through identity placement, preserving this instance's transform.
                    let slots: BTreeMap<_, _> = instance
                        .parameters
                        .members
                        .keys()
                        .map(|slot| (*slot, Id::new()))
                        .collect();
                    let remapped = definition.parameters.materialize(&GroupInstanceParams {
                        definition: old_definition,
                        members: slots.clone(),
                        transform: GroupTransform::default(),
                    })?;
                    let definition = GroupDefinition::create(GroupDefinitionParams {
                        name,
                        template: GroupTemplate {
                            walls: remapped.walls,
                            openings: remapped.openings,
                            joins: remapped.joins,
                            opening_tags: remapped.opening_tags,
                            wall_type_assignments: remapped.wall_type_assignments,
                            opening_clearances: remapped.opening_clearances,
                        },
                    });
                    ensure(
                        !candidate.group_definitions.contains_key(&definition.id()),
                        "group definition identity collision",
                    )?;
                    instance.parameters.members = instance
                        .parameters
                        .members
                        .iter()
                        .map(|(slot, member)| (slots[slot], *member))
                        .collect();
                    instance.parameters.definition = definition.id();
                    changed.extend(instance.parameters.members.values().copied());
                    changed.extend([old_definition, definition.id()]);
                    candidate
                        .group_definitions
                        .insert(definition.id(), definition);
                    candidate.group_instances.insert(id, instance);
                    remove_orphan_group_definition(&mut candidate, old_definition);
                    id
                }
                Command::UngroupInstance(id) => {
                    let instance = group_instance(&candidate, id)?;
                    changed.extend(instance.parameters.members.values().copied());
                    changed.insert(instance.parameters.definition);
                    candidate.group_instances.remove(&id);
                    remove_orphan_group_definition(&mut candidate, instance.parameters.definition);
                    id
                }
                Command::SetPlanHiddenOpenings { view, hidden } => {
                    candidate.validate_plan_hidden_openings_replacement(view, &hidden)?;
                    explicit_hidden.insert(view, hidden.clone());
                    if hidden.is_empty() {
                        candidate.plan_hidden_openings.remove(&view);
                    } else {
                        candidate.plan_hidden_openings.insert(view, hidden);
                    }
                    view
                }
                Command::PutSheetSet(set) => {
                    let id = set.id;
                    candidate.sheet_sets.insert(id, set);
                    id
                }
                Command::RemoveSheetSet(id) => {
                    ensure(
                        candidate.sheet_sets.remove(&id).is_some(),
                        "sheet set missing",
                    )?;
                    id
                }
                Command::PutDrawingIssue(issue) => {
                    let id = issue.id;
                    ensure(
                        issue.state == IssueState::Draft
                            && candidate
                                .drawing_issues
                                .get(&id)
                                .is_none_or(|old| old.state == IssueState::Draft),
                        "reviewed/issued records cannot be rewritten; create a new draft",
                    )?;
                    if !candidate.drawing_issues.contains_key(&id) {
                        ensure(
                            issue.sequence
                                == candidate
                                    .drawing_issues
                                    .values()
                                    .filter(|i| i.set == issue.set)
                                    .map(|i| i.sequence)
                                    .max()
                                    .unwrap_or(0)
                                    .checked_add(1)
                                    .ok_or_else(|| {
                                        Error::Invalid("issue sequence exhausted".into())
                                    })?,
                            "issue sequence must be next in set",
                        )?;
                    }
                    candidate.drawing_issues.insert(id, issue);
                    id
                }
                Command::RemoveDrawingIssue(id) => {
                    ensure(
                        candidate
                            .drawing_issues
                            .get(&id)
                            .is_some_and(|i| i.state != IssueState::Issued),
                        "issued records cannot be removed",
                    )?;
                    candidate.drawing_issues.remove(&id);
                    id
                }
                Command::AddPhase(phase) => {
                    let id = phase.id();
                    ensure(!candidate.phases.contains_key(&id), "phase already exists")?;
                    candidate.phases.insert(id, phase);
                    id
                }
                Command::UpdatePhase { id, parameters } => {
                    candidate
                        .phases
                        .get_mut(&id)
                        .ok_or_else(|| Error::Invalid("phase missing".into()))?
                        .parameters = parameters;
                    id
                }
                Command::RemovePhase(id) => {
                    ensure(candidate.phases.remove(&id).is_some(), "phase missing")?;
                    id
                }
                Command::SetElementLifecycle { element, lifecycle } => {
                    ensure(
                        candidate.is_phaseable_element(element),
                        "element is missing or cannot be phased",
                    )?;
                    candidate.element_lifecycles.insert(element, lifecycle);
                    element
                }
                Command::AddMaterialPattern(pattern) => {
                    let id = pattern.id();
                    ensure(
                        !candidate.material_patterns.contains_key(&id),
                        "pattern already exists",
                    )?;
                    candidate.material_patterns.insert(id, pattern);
                    id
                }
                Command::UpdateMaterialPattern { id, parameters } => {
                    let pattern = candidate
                        .material_patterns
                        .get_mut(&id)
                        .ok_or_else(|| Error::Invalid("pattern missing".into()))?;
                    pattern.parameters = parameters;
                    id
                }
                Command::RemoveMaterialPattern(id) => {
                    ensure(
                        candidate.material_patterns.remove(&id).is_some(),
                        "pattern missing",
                    )?;
                    id
                }
                Command::AddMaterial(material) => {
                    let id = material.id();
                    ensure(
                        !candidate.materials.contains_key(&id),
                        "material already exists",
                    )?;
                    candidate.materials.insert(id, material);
                    id
                }
                Command::UpdateMaterial { id, parameters } => {
                    candidate
                        .materials
                        .get_mut(&id)
                        .ok_or_else(|| Error::Invalid("material missing".into()))?
                        .parameters = parameters;
                    id
                }
                Command::RemoveMaterial(id) => {
                    ensure(
                        candidate.materials.remove(&id).is_some(),
                        "material missing",
                    )?;
                    id
                }
                Command::AddWallType(ty) => {
                    let id = ty.id();
                    ensure(
                        !candidate.wall_types.contains_key(&id),
                        "wall type already exists",
                    )?;
                    candidate.wall_types.insert(id, ty);
                    id
                }
                Command::UpdateWallType { id, parameters } => {
                    candidate
                        .wall_types
                        .get_mut(&id)
                        .ok_or_else(|| Error::Invalid("wall type missing".into()))?
                        .parameters = parameters;
                    id
                }
                Command::RemoveWallType(id) => {
                    ensure(
                        candidate.wall_types.remove(&id).is_some(),
                        "wall type missing",
                    )?;
                    id
                }
                Command::AssignWallType { wall, assignment } => {
                    ensure(candidate.walls.contains_key(&wall), "wall missing")?;
                    if let Some(assignment) = assignment {
                        candidate.wall_type_assignments.insert(wall, assignment);
                    } else {
                        candidate.wall_type_assignments.remove(&wall);
                    }
                    wall
                }
                Command::AddSheet(sheet) => {
                    let id = sheet.id();
                    ensure(!candidate.sheets.contains_key(&id), "sheet already exists")?;
                    candidate.sheets.insert(id, sheet);
                    id
                }
                Command::UpdateSheet { id, parameters } => {
                    candidate
                        .sheets
                        .get_mut(&id)
                        .ok_or_else(|| Error::Invalid("sheet missing".into()))?
                        .parameters = parameters;
                    id
                }
                Command::RemoveSheet(id) => {
                    ensure(candidate.sheets.remove(&id).is_some(), "sheet missing")?;
                    id
                }
                Command::AddFloor(floor) => {
                    let id = floor.id();
                    ensure(!candidate.floors.contains_key(&id), "floor already exists")?;
                    candidate.floors.insert(id, floor);
                    id
                }
                Command::AddColumn(column) => {
                    let id = column.id();
                    ensure(
                        !candidate.columns.contains_key(&id),
                        "column already exists",
                    )?;
                    candidate.columns.insert(id, column);
                    id
                }
                Command::AddCaseworkType(casework_type) => {
                    let id = casework_type.id();
                    ensure(
                        !candidate.casework_types.contains_key(&id),
                        "casework type already exists",
                    )?;
                    candidate.casework_types.insert(id, casework_type);
                    id
                }
                Command::AddCasework(casework) => {
                    let id = casework.id();
                    ensure(
                        !candidate.casework.contains_key(&id),
                        "casework already exists",
                    )?;
                    candidate.casework.insert(id, casework);
                    id
                }
                Command::AddCurtainSystem(entity) => {
                    let id = entity.id();
                    ensure(
                        !candidate.curtain_systems.contains_key(&id),
                        "curtain entity already exists",
                    )?;
                    candidate.curtain_systems.insert(id, entity);
                    id
                }
                Command::UpdateCurtainSystem { id, parameters } => {
                    let assembly = candidate
                        .curtain_systems
                        .get_mut(&id)
                        .ok_or_else(|| Error::Invalid("curtain entity missing".into()))?;
                    parameters.validate_transition(&assembly.parameters)?;
                    assembly.parameters = parameters;
                    id
                }
                Command::RemoveCurtainSystem(id) => {
                    ensure(
                        candidate.curtain_systems.remove(&id).is_some(),
                        "curtain entity missing",
                    )?;
                    id
                }
                Command::AddCurtainPanelType(entity) => {
                    let id = entity.id();
                    ensure(
                        !candidate.curtain_panel_types.contains_key(&id),
                        "curtain entity already exists",
                    )?;
                    candidate.curtain_panel_types.insert(id, entity);
                    id
                }
                Command::UpdateCurtainPanelType { id, parameters } => {
                    candidate
                        .curtain_panel_types
                        .get_mut(&id)
                        .ok_or_else(|| Error::Invalid("curtain entity missing".into()))?
                        .parameters = parameters;
                    id
                }
                Command::RemoveCurtainPanelType(id) => {
                    ensure(
                        candidate.curtain_panel_types.remove(&id).is_some(),
                        "curtain entity missing",
                    )?;
                    id
                }
                Command::AddCurtainMullionType(entity) => {
                    let id = entity.id();
                    ensure(
                        !candidate.curtain_mullion_types.contains_key(&id),
                        "curtain entity already exists",
                    )?;
                    candidate.curtain_mullion_types.insert(id, entity);
                    id
                }
                Command::UpdateCurtainMullionType { id, parameters } => {
                    candidate
                        .curtain_mullion_types
                        .get_mut(&id)
                        .ok_or_else(|| Error::Invalid("curtain entity missing".into()))?
                        .parameters = parameters;
                    id
                }
                Command::RemoveCurtainMullionType(id) => {
                    ensure(
                        candidate.curtain_mullion_types.remove(&id).is_some(),
                        "curtain entity missing",
                    )?;
                    id
                }
                Command::AddStair(stair) => {
                    let id = stair.id();
                    ensure(!candidate.stairs.contains_key(&id), "stair already exists")?;
                    candidate.stairs.insert(id, stair);
                    id
                }
                Command::AddRamp(ramp) => {
                    let id = ramp.id();
                    ensure(!candidate.ramps.contains_key(&id), "ramp already exists")?;
                    candidate.ramps.insert(id, ramp);
                    id
                }
                Command::UpdateRamp { id, parameters } => {
                    candidate
                        .ramps
                        .get_mut(&id)
                        .ok_or_else(|| Error::Invalid("ramp missing".into()))?
                        .parameters = parameters;
                    id
                }
                Command::RemoveRamp(id) => {
                    ensure(candidate.ramps.remove(&id).is_some(), "ramp missing")?;
                    id
                }
                Command::AddRailing(entity) => {
                    let id = entity.id();
                    ensure(
                        !candidate.railings.contains_key(&id),
                        "railing already exists",
                    )?;
                    candidate.railings.insert(id, entity);
                    id
                }
                Command::UpdateRailing { id, parameters } => {
                    candidate
                        .railings
                        .get_mut(&id)
                        .ok_or_else(|| Error::Invalid("railing missing".into()))?
                        .parameters = parameters;
                    id
                }
                Command::RemoveRailing(id) => {
                    ensure(candidate.railings.remove(&id).is_some(), "railing missing")?;
                    id
                }
                Command::UpdateRailingType { id, parameters } => {
                    candidate
                        .railing_types
                        .get_mut(&id)
                        .ok_or_else(|| Error::Invalid("railing type missing".into()))?
                        .parameters = parameters;
                    id
                }
                Command::RemoveRailingType(id) => {
                    ensure(
                        candidate.railing_types.remove(&id).is_some(),
                        "railing type missing",
                    )?;
                    id
                }
                Command::AddRailingType(railing_type) => {
                    let id = railing_type.id();
                    ensure(
                        !candidate.railing_types.contains_key(&id),
                        "railing type already exists",
                    )?;
                    candidate.railing_types.insert(id, railing_type);
                    id
                }

                Command::AddCeiling(ceiling) => {
                    let id = ceiling.id();
                    ensure(
                        !candidate.ceilings.contains_key(&id),
                        "ceiling already exists",
                    )?;
                    candidate.ceilings.insert(id, ceiling);
                    id
                }
                Command::UpdateCeiling { id, parameters } => {
                    candidate
                        .ceilings
                        .get_mut(&id)
                        .ok_or_else(|| Error::Invalid("ceiling missing".into()))?
                        .parameters = parameters;
                    id
                }
                Command::RemoveCeiling(id) => {
                    ensure(candidate.ceilings.remove(&id).is_some(), "ceiling missing")?;
                    id
                }
                Command::AddRoof(roof) => {
                    let id = roof.id();
                    ensure(!candidate.roofs.contains_key(&id), "roof already exists")?;
                    candidate.roofs.insert(id, roof);
                    id
                }
                Command::UpdateRoof { id, parameters } => {
                    candidate
                        .roofs
                        .get_mut(&id)
                        .ok_or_else(|| Error::Invalid("roof missing".into()))?
                        .parameters = parameters;
                    id
                }
                Command::RemoveRoof(id) => {
                    ensure(candidate.roofs.remove(&id).is_some(), "roof missing")?;
                    id
                }
                Command::UpdateStair { id, parameters } => {
                    candidate
                        .stairs
                        .get_mut(&id)
                        .ok_or_else(|| Error::Invalid("stair missing".into()))?
                        .parameters = parameters;
                    id
                }
                Command::RemoveStair(id) => {
                    ensure(candidate.stairs.remove(&id).is_some(), "stair missing")?;
                    id
                }
                Command::UpdateColumn { id, parameters } => {
                    candidate
                        .columns
                        .get_mut(&id)
                        .ok_or_else(|| Error::Invalid("column missing".into()))?
                        .parameters = parameters;
                    id
                }
                Command::RemoveColumn(id) => {
                    ensure(candidate.columns.remove(&id).is_some(), "column missing")?;
                    id
                }
                Command::UpdateCaseworkType { id, parameters } => {
                    candidate
                        .casework_types
                        .get_mut(&id)
                        .ok_or_else(|| Error::Invalid("casework type missing".into()))?
                        .parameters = parameters;
                    id
                }
                Command::RemoveCaseworkType(id) => {
                    ensure(
                        candidate.casework_types.remove(&id).is_some(),
                        "casework type missing",
                    )?;
                    id
                }
                Command::UpdateCasework { id, parameters } => {
                    candidate
                        .casework
                        .get_mut(&id)
                        .ok_or_else(|| Error::Invalid("casework missing".into()))?
                        .parameters = parameters;
                    id
                }
                Command::RemoveCasework(id) => {
                    ensure(candidate.casework.remove(&id).is_some(), "casework missing")?;
                    id
                }
                Command::UpdateFloor { id, parameters } => {
                    candidate
                        .floors
                        .get_mut(&id)
                        .ok_or_else(|| Error::Invalid("floor missing".into()))?
                        .parameters = parameters;
                    id
                }
                Command::RemoveFloor(id) => {
                    ensure(candidate.floors.remove(&id).is_some(), "floor missing")?;
                    id
                }
                Command::SetOpeningClearance { id, clearance } => {
                    ensure(
                        candidate.openings.contains_key(&id),
                        "Clearance opening missing",
                    )?;
                    if let Some(lock) = clearance {
                        candidate.opening_clearances.insert(id, lock);
                    } else {
                        candidate.opening_clearances.remove(&id);
                    }
                    id
                }
                Command::AddLengthParameter(parameter) => {
                    let id = parameter.id();
                    ensure(
                        !candidate.length_parameters.contains_key(&id),
                        "Length parameter already exists",
                    )?;
                    candidate.length_parameters.insert(id, parameter);
                    id
                }
                Command::UpdateLengthParameter { id, parameters } => {
                    candidate
                        .length_parameters
                        .get_mut(&id)
                        .ok_or_else(|| Error::Invalid("Length parameter missing".into()))?
                        .parameters = parameters;
                    id
                }
                Command::RemoveLengthParameter(id) => {
                    let uses = candidate.length_parameter_uses(id);
                    ensure(
                        uses.is_empty() && candidate.formula_parameter_dependents(id).is_empty(),
                        format!(
                            "Length parameter is referenced by {} dimension(s): {uses:?}",
                            uses.len()
                        ),
                    )?;
                    ensure(
                        candidate.length_parameters.remove(&id).is_some(),
                        "Length parameter missing",
                    )?;
                    id
                }
                Command::SetOpeningTypeLengthBindings { id, bindings } => {
                    candidate.set_opening_type_length_bindings(id, bindings)?;
                    id
                }
                Command::AddOpeningType(ty) => {
                    let id = ty.id();
                    ensure(
                        !candidate.opening_types.contains_key(&id),
                        "opening type already exists",
                    )?;
                    candidate.opening_types.insert(id, ty);
                    id
                }
                Command::UpdateOpeningType { id, mut parameters } => {
                    candidate.freeze_unbound_extrusions(id, &mut parameters)?;
                    let ty = candidate
                        .opening_types
                        .get_mut(&id)
                        .ok_or_else(|| Error::Invalid("opening type missing".into()))?;
                    ensure(
                        ty.parameters.kind == parameters.kind,
                        "opening type kind is immutable",
                    )?;
                    ty.parameters = parameters;
                    id
                }
                Command::RemoveOpeningType(id) => {
                    candidate.opening_type_length_bindings.remove(&id);
                    ensure(
                        candidate.opening_types.remove(&id).is_some(),
                        "opening type missing",
                    )?;
                    // Whole-candidate validation below rejects remaining references,
                    // allowing reassignment/removal in the same atomic transaction.
                    id
                }
                Command::AddDimension(dimension) => {
                    dimension.parameters.validate_creation(&candidate)?;
                    let id = dimension.id();
                    ensure(
                        !candidate.dimensions.contains_key(&id),
                        "dimension already exists",
                    )?;
                    candidate.dimensions.insert(id, dimension);
                    id
                }
                Command::AddSchedule(schedule) => {
                    let id = schedule.id();
                    ensure(
                        !candidate.schedules.contains_key(&id),
                        "schedule already exists",
                    )?;
                    candidate.schedules.insert(id, schedule);
                    id
                }
                Command::UpdateSchedule { id, parameters } => {
                    candidate
                        .schedules
                        .get_mut(&id)
                        .ok_or_else(|| Error::Invalid("schedule missing".into()))?
                        .parameters = parameters;
                    id
                }
                Command::RemoveSchedule(id) => {
                    ensure(
                        candidate.schedules.remove(&id).is_some(),
                        "schedule missing",
                    )?;
                    id
                }
                Command::AddOpeningTag(tag) => {
                    tag.parameters.validate_creation(&candidate)?;
                    let id = tag.id();
                    ensure(
                        !candidate.opening_tags.contains_key(&id),
                        "opening tag already exists",
                    )?;
                    candidate.opening_tags.insert(id, tag);
                    id
                }
                Command::AddRoomTag(tag) => {
                    tag.parameters.validate_creation(&candidate)?;
                    let id = tag.id();
                    ensure(
                        !candidate.room_tags.contains_key(&id),
                        "room tag already exists",
                    )?;
                    candidate.room_tags.insert(id, tag);
                    id
                }
                Command::AddTextNote(note) => {
                    let id = note.id();
                    ensure(
                        !candidate.text_notes.contains_key(&id),
                        "note already exists",
                    )?;
                    candidate.text_notes.insert(id, note);
                    id
                }
                Command::UpdateTextNote { id, parameters } => {
                    candidate
                        .text_notes
                        .get_mut(&id)
                        .ok_or_else(|| Error::Invalid("note missing".into()))?
                        .parameters = parameters;
                    id
                }
                Command::RemoveTextNote(id) => {
                    ensure(candidate.text_notes.remove(&id).is_some(), "note missing")?;
                    id
                }
                Command::AddDetailLine(line) => {
                    let id = line.id();
                    ensure(
                        !candidate.detail_lines.contains_key(&id),
                        "detail line already exists",
                    )?;
                    candidate.detail_lines.insert(id, line);
                    id
                }
                Command::UpdateDetailLine { id, parameters } => {
                    candidate
                        .detail_lines
                        .get_mut(&id)
                        .ok_or_else(|| Error::Invalid("detail line missing".into()))?
                        .parameters = parameters;
                    id
                }
                Command::RemoveDetailLine(id) => {
                    ensure(
                        candidate.detail_lines.remove(&id).is_some(),
                        "detail line missing",
                    )?;
                    id
                }
                Command::AddRoomSeparationLine(line) => {
                    let id = line.id();
                    ensure(
                        !candidate.room_separation_lines.contains_key(&id),
                        "room separation line already exists",
                    )?;
                    candidate.room_separation_lines.insert(id, line);
                    id
                }
                Command::UpdateRoomSeparationLine { id, parameters } => {
                    candidate
                        .room_separation_lines
                        .get_mut(&id)
                        .ok_or_else(|| Error::Invalid("room separation line missing".into()))?
                        .parameters = parameters;
                    id
                }
                Command::RemoveRoomSeparationLine(id) => {
                    ensure(
                        candidate.room_separation_lines.remove(&id).is_some(),
                        "room separation line missing",
                    )?;
                    id
                }
                Command::UpdateOpeningTag { id, parameters } => {
                    let tag = candidate
                        .opening_tags
                        .get_mut(&id)
                        .ok_or_else(|| Error::Invalid("opening tag missing".into()))?;
                    ensure(
                        tag.parameters.view == parameters.view
                            && tag.parameters.opening == parameters.opening,
                        "opening tag view and target cannot change",
                    )?;
                    tag.parameters = parameters;
                    id
                }
                Command::RemoveOpeningTag(id) => {
                    ensure(
                        candidate.opening_tags.remove(&id).is_some(),
                        "opening tag missing",
                    )?;
                    id
                }
                Command::UpdateRoomTag { id, parameters } => {
                    let tag = candidate
                        .room_tags
                        .get_mut(&id)
                        .ok_or_else(|| Error::Invalid("room tag missing".into()))?;
                    ensure(
                        tag.parameters.view == parameters.view
                            && tag.parameters.room == parameters.room,
                        "room tag view and target cannot change",
                    )?;
                    tag.parameters = parameters;
                    id
                }
                Command::RemoveRoomTag(id) => {
                    ensure(
                        candidate.room_tags.remove(&id).is_some(),
                        "room tag missing",
                    )?;
                    id
                }
                Command::UpdateDimension { id, parameters } => {
                    if parameters.layout.is_section() {
                        let old = &candidate
                            .dimensions
                            .get(&id)
                            .ok_or_else(|| Error::Invalid("dimension missing".into()))?
                            .parameters;
                        if old.view != parameters.view || old.layout != parameters.layout {
                            parameters.validate_creation(&candidate)?;
                        } else if old.first != parameters.first || old.second != parameters.second {
                            parameters.validate()?;
                            for (before, after) in [
                                (old.first, parameters.first),
                                (old.second, parameters.second),
                            ] {
                                if before != after {
                                    after.resolve_section(&candidate, parameters.view).map_err(
                                        |e| {
                                            Error::Invalid(format!(
                                                "Replacement section reference: {e:?}"
                                            ))
                                        },
                                    )?;
                                }
                            }
                            ensure(
                                parameters.resolve_section(&candidate)
                                    != Err(os_model::DimensionDiagnostic::CoincidentAnchors),
                                "coincident section references",
                            )?;
                        }
                    }
                    if parameters.layout.is_elevation() {
                        let old = &candidate
                            .dimensions
                            .get(&id)
                            .ok_or_else(|| Error::Invalid("dimension missing".into()))?
                            .parameters;
                        if old.view != parameters.view || old.layout != parameters.layout {
                            parameters.validate_creation(&candidate)?;
                        } else if old.first != parameters.first || old.second != parameters.second {
                            parameters.validate()?;
                            let host = candidate
                                .views
                                .get(&parameters.view)
                                .and_then(|v| v.parameters.elevation)
                                .ok_or_else(|| Error::Invalid("elevation view missing".into()))?
                                .host;
                            for (before, after) in [
                                (old.first, parameters.first),
                                (old.second, parameters.second),
                            ] {
                                if before != after {
                                    after.resolve_elevation(&candidate, host).map_err(|e| {
                                        Error::Invalid(format!(
                                            "replacement reference cannot resolve: {e:?}"
                                        ))
                                    })?;
                                }
                            }
                            ensure(
                                parameters.resolve_elevation(&candidate)
                                    != Err(os_model::DimensionDiagnostic::CoincidentAnchors),
                                "coincident elevation references",
                            )?;
                        }
                    }
                    if parameters.layout == os_model::DimensionLayout::ArcLength
                        && candidate
                            .walls
                            .get(&parameters.first.entity())
                            .is_some_and(|wall| {
                                !wall.parameters.path.is_straight()
                                    && candidate
                                        .views
                                        .get(&parameters.view)
                                        .and_then(|v| v.parameters.level)
                                        == Some(wall.parameters.level)
                            })
                    {
                        parameters.validate_creation(&candidate)?;
                    }
                    candidate
                        .dimensions
                        .get_mut(&id)
                        .ok_or_else(|| Error::Invalid("dimension missing".into()))?
                        .parameters = parameters;
                    id
                }
                Command::RemoveDimension(id) => {
                    ensure(
                        candidate.dimensions.remove(&id).is_some(),
                        "dimension missing",
                    )?;
                    id
                }
                Command::AddRoom(room) => {
                    let id = room.id();
                    ensure(!candidate.rooms.contains_key(&id), "room already exists")?;
                    candidate.rooms.insert(id, room);
                    id
                }
                Command::UpdateRoom { id, parameters } => {
                    candidate
                        .rooms
                        .get_mut(&id)
                        .ok_or_else(|| Error::Invalid("room missing".into()))?
                        .parameters = parameters;
                    id
                }
                Command::RemoveRoom(id) => {
                    ensure(candidate.rooms.remove(&id).is_some(), "room missing")?;
                    id
                }
                Command::AddOpening(opening) => {
                    let id = opening.id();
                    ensure(
                        !candidate.openings.contains_key(&id),
                        "opening already exists",
                    )?;
                    candidate.openings.insert(id, opening);
                    id
                }
                Command::UpdateOpening { id, parameters } => {
                    candidate
                        .openings
                        .get_mut(&id)
                        .ok_or_else(|| Error::Invalid("opening missing".into()))?
                        .parameters = parameters;
                    id
                }
                Command::RemoveOpening(id) => {
                    ensure(candidate.openings.remove(&id).is_some(), "opening missing")?;
                    id
                }
                Command::AddWall(wall) => {
                    let id = wall.id();
                    ensure(!candidate.walls.contains_key(&id), "wall already exists")?;
                    candidate.walls.insert(id, wall);
                    id
                }
                Command::AddButtJoin(join) => {
                    ensure(
                        join.header.type_id == "core.wall_butt_join",
                        "invalid legacy butt join type",
                    )?;
                    let id = join.id();
                    ensure(
                        !candidate.wall_joins.contains_key(&id),
                        "butt join already exists",
                    )?;
                    let mut header = join.header;
                    header.type_id = "core.wall_join".into();
                    candidate.wall_joins.insert(
                        id,
                        WallJoin {
                            header,
                            parameters: join.parameters.into(),
                        },
                    );
                    id
                }
                Command::AddWallJoin(join) => {
                    let id = join.id();
                    ensure(
                        !candidate.wall_joins.contains_key(&id),
                        "wall join already exists",
                    )?;
                    candidate.wall_joins.insert(id, join);
                    id
                }
                Command::UpdateWallJoin { id, parameters } => {
                    candidate
                        .wall_joins
                        .get_mut(&id)
                        .ok_or_else(|| Error::Invalid("wall join missing".into()))?
                        .parameters = parameters;
                    id
                }
                Command::UpdateButtJoin { id, parameters } => {
                    candidate
                        .wall_joins
                        .get_mut(&id)
                        .ok_or_else(|| Error::Invalid("butt join missing".into()))?
                        .parameters = parameters.into();
                    id
                }
                Command::RemoveButtJoin(id) | Command::RemoveWallJoin(id) => {
                    ensure(
                        candidate.wall_joins.remove(&id).is_some(),
                        "butt join missing",
                    )?;
                    id
                }
                Command::UpdateWall { id, parameters } => {
                    candidate
                        .walls
                        .get_mut(&id)
                        .ok_or_else(|| Error::Invalid("wall missing".into()))?
                        .parameters = parameters;
                    id
                }
                Command::AddLevel(level) => {
                    let id = level.id();
                    ensure(!candidate.levels.contains_key(&id), "level already exists")?;
                    candidate.levels.insert(id, level);
                    id
                }
                Command::UpdateLevel { id, parameters } => {
                    candidate
                        .levels
                        .get_mut(&id)
                        .ok_or_else(|| Error::Invalid("level missing".into()))?
                        .parameters = parameters;
                    id
                }
                Command::RemoveWall(id) => {
                    candidate.wall_type_assignments.remove(&id);
                    ensure(candidate.walls.remove(&id).is_some(), "wall missing")?;
                    id
                }
                Command::RemoveLevel(id) => {
                    ensure(candidate.levels.remove(&id).is_some(), "level missing")?;
                    id
                }
                Command::RenameProject(name) => {
                    candidate.project.parameters.name = name;
                    candidate.project.id()
                }
                Command::AddGrid(grid) => {
                    let id = grid.id();
                    ensure(!candidate.grids.contains_key(&id), "grid already exists")?;
                    candidate.grids.insert(id, grid);
                    id
                }
                Command::UpdateGrid { id, parameters } => {
                    candidate
                        .grids
                        .get_mut(&id)
                        .ok_or_else(|| Error::Invalid("grid missing".into()))?
                        .parameters = parameters;
                    id
                }
                Command::RemoveGrid(id) => {
                    ensure(candidate.grids.remove(&id).is_some(), "grid missing")?;
                    id
                }
                Command::AddView(view) => {
                    view.parameters.validate_edit()?;
                    edited_views.insert(view.id());
                    ensure(
                        view.parameters.settings_revision == 0,
                        "new view revision must be zero",
                    )?;
                    let id = view.id();
                    ensure(!candidate.views.contains_key(&id), "view already exists")?;
                    candidate.views.insert(id, view);
                    id
                }
                Command::AddPlanCallout(callout) => {
                    let id = callout.id();
                    ensure(
                        !candidate.plan_callouts.contains_key(&id),
                        "callout already exists",
                    )?;
                    candidate.plan_callouts.insert(id, callout);
                    id
                }
                Command::UpdatePlanCallout { id, parameters } => {
                    candidate
                        .plan_callouts
                        .get_mut(&id)
                        .ok_or_else(|| Error::Invalid("callout missing".into()))?
                        .parameters = parameters;
                    id
                }
                Command::RemovePlanCallout(id) => {
                    ensure(
                        candidate.plan_callouts.remove(&id).is_some(),
                        "callout missing",
                    )?;
                    id
                }
                Command::AddSectionCallout(callout) => {
                    ensure(
                        candidate.compatible_callout_sections(
                            callout.parameters.parent,
                            callout.parameters.target,
                        ),
                        "new section callout requires matching authored plane, side, mode and depth",
                    )?;
                    let id = callout.id();
                    ensure(
                        !candidate.section_callouts.contains_key(&id),
                        "callout already exists",
                    )?;
                    candidate.section_callouts.insert(id, callout);
                    id
                }
                Command::UpdateSectionCallout { id, parameters } => {
                    let old = candidate
                        .section_callouts
                        .get(&id)
                        .ok_or_else(|| Error::Invalid("callout missing".into()))?;
                    if old.parameters.parent != parameters.parent
                        || old.parameters.target != parameters.target
                    {
                        ensure(
                            candidate
                                .compatible_callout_sections(parameters.parent, parameters.target),
                            "retargeted section callout requires matching authored plane, side, mode and depth",
                        )?;
                    }
                    candidate
                        .section_callouts
                        .get_mut(&id)
                        .ok_or_else(|| Error::Invalid("callout missing".into()))?
                        .parameters = parameters;
                    id
                }
                Command::RemoveSectionCallout(id) => {
                    ensure(
                        candidate.section_callouts.remove(&id).is_some(),
                        "callout missing",
                    )?;
                    id
                }
                Command::UpdateView { id, mut parameters } => {
                    parameters.validate_edit()?;
                    edited_views.insert(id);
                    let view = candidate
                        .views
                        .get_mut(&id)
                        .ok_or_else(|| Error::Invalid("view missing".into()))?;
                    ensure(
                        parameters.kind == view.parameters.kind,
                        "view kind cannot change; create a separate view",
                    )?;
                    parameters.settings_revision = view.parameters.settings_revision;
                    if parameters != view.parameters {
                        parameters.settings_revision =
                            parameters.settings_revision.checked_add(1).ok_or_else(|| {
                                Error::Invalid("view settings revision exhausted".into())
                            })?;
                        view.parameters = parameters;
                    }
                    id
                }
                Command::RemoveView(id) => {
                    ensure(
                        !candidate
                            .detail_lines
                            .values()
                            .any(|l| l.parameters.view == id)
                            && !candidate
                                .text_notes
                                .values()
                                .any(|n| n.parameters.view == id),
                        "remove owned annotations before removing their view",
                    )?;
                    ensure(
                        !candidate.drawing_issues.values().any(|issue| {
                            issue.manifest.as_ref().is_some_and(|m| {
                                m.sheets
                                    .iter()
                                    .any(|s| s.views.iter().any(|(view, _)| *view == id))
                            })
                        }),
                        "drawing issue references this view",
                    )?;
                    candidate.plan_graphics.remove(&id);
                    candidate.plan_hidden_openings.remove(&id);
                    ensure(
                        !candidate.sheets.values().any(|sheet| {
                            sheet
                                .parameters
                                .viewports
                                .iter()
                                .any(|viewport| viewport.view == id)
                        }),
                        "remove sheet placements before removing their view",
                    )?;
                    ensure(
                        !candidate
                            .dimensions
                            .values()
                            .any(|d| d.parameters.view == id),
                        "remove owned dimensions before removing their view",
                    )?;
                    ensure(candidate.views.remove(&id).is_some(), "view missing")?;
                    id
                }
                Command::AddExtension(entity) => {
                    let id = entity.id;
                    ensure(
                        !candidate.extensions.contains_key(&id),
                        "extension already exists",
                    )?;
                    candidate.extensions.insert(id, entity);
                    id
                }
                Command::AddPlanGraphicsTemplate(template) => {
                    let id = template.id();
                    ensure(
                        !candidate.plan_graphics_templates.contains_key(&id),
                        "graphics template already exists",
                    )?;
                    candidate.plan_graphics_templates.insert(id, template);
                    id
                }
                Command::UpdatePlanGraphicsTemplate { id, parameters } => {
                    candidate
                        .plan_graphics_templates
                        .get_mut(&id)
                        .ok_or_else(|| Error::Invalid("graphics template missing".into()))?
                        .parameters = parameters;
                    id
                }
                Command::RemovePlanGraphicsTemplate(id) => {
                    ensure(
                        !candidate
                            .plan_graphics
                            .values()
                            .any(|b| b.template == Some(id)),
                        "unlink all views before removing this graphics template",
                    )?;
                    ensure(
                        candidate.plan_graphics_templates.remove(&id).is_some(),
                        "graphics template missing",
                    )?;
                    id
                }
                Command::SetPlanGraphics { view, binding } => {
                    ensure(
                        candidate
                            .views
                            .get(&view)
                            .is_some_and(|v| v.parameters.kind == ViewKind::Plan),
                        "graphics settings require a native plan view",
                    )?;
                    if let Some(binding) = binding {
                        candidate.plan_graphics.insert(view, binding);
                    } else {
                        candidate.plan_graphics.remove(&view);
                    }
                    view
                }
                Command::SetPlanGraphicsRules { view, rules } => {
                    if rules.is_empty() {
                        candidate.plan_graphics_rules.remove(&view);
                    } else {
                        candidate.plan_graphics_rules.insert(view, rules.clone());
                    }
                    explicit_plan_graphics_rules.insert(view, rules);
                    view
                }
                Command::SetPlanElementGraphicsOverrides { view, overrides } => {
                    if overrides.is_empty() {
                        candidate.plan_element_graphics_overrides.remove(&view);
                    } else {
                        candidate
                            .plan_element_graphics_overrides
                            .insert(view, overrides.clone());
                    }
                    explicit_plan_element_overrides.insert(view, overrides);
                    view
                }
                Command::ReplaceExtension {
                    expected_schema_version,
                    entity,
                } => {
                    let id = entity.id;
                    let old = candidate
                        .extensions
                        .get(&id)
                        .ok_or_else(|| Error::Invalid("extension missing".into()))?;
                    ensure(
                        old.payload_schema_version == expected_schema_version,
                        "extension payload schema changed",
                    )?;
                    ensure(
                        old.owner == entity.owner && old.type_id == entity.type_id,
                        "extension ownership/type cannot change",
                    )?;
                    candidate.extensions.insert(id, entity);
                    id
                }
                Command::RemoveExtension(id) => {
                    ensure(
                        candidate.extensions.remove(&id).is_some(),
                        "extension missing",
                    )?;
                    id
                }
                Command::SetPluginRequirement {
                    plugin_id,
                    requirement,
                } => {
                    ensure(valid_plugin_id(&plugin_id), "invalid plugin requirement ID")?;
                    changed.extend(
                        candidate
                            .extensions
                            .values()
                            .filter(|e| e.owner == plugin_id)
                            .map(|e| e.id),
                    );
                    if let Some(requirement) = requirement {
                        candidate.plugin_requirements.insert(plugin_id, requirement);
                    } else {
                        ensure(
                            candidate.plugin_requirements.remove(&plugin_id).is_some(),
                            "plugin requirement missing",
                        )?;
                    }
                    candidate.project.id()
                }
            };
            changed.insert(id);
        }
        let current_phaseable = phaseable_ids(&candidate);
        // Validate requested replacements against the complete graph before
        // lifecycle cleanup. Bad user input must never become a partial success.
        for (view, hidden) in &explicit_hidden {
            candidate.validate_plan_hidden_openings_replacement(*view, hidden)?;
        }
        for (view, overrides) in &explicit_plan_element_overrides {
            candidate.validate_plan_element_graphics_replacement(*view, overrides)?;
        }
        for (view, rules) in &explicit_plan_graphics_rules {
            candidate.validate_plan_graphics_rules_replacement(*view, rules)?;
        }
        let old_rules = std::mem::take(&mut candidate.plan_graphics_rules);
        for (view, rules) in old_rules {
            if candidate.plan_graphics_rules_compatible(view) {
                candidate.plan_graphics_rules.insert(view, rules);
            } else {
                changed.insert(view);
            }
        }
        let old_hidden = std::mem::take(&mut candidate.plan_hidden_openings);
        for (view, mut hidden) in old_hidden {
            let old_count = hidden.len();
            hidden.retain(|id| candidate.plan_hidden_opening_compatible(view, *id));
            if hidden.len() != old_count {
                changed.insert(view);
            }
            if !hidden.is_empty() {
                candidate.plan_hidden_openings.insert(view, hidden);
            }
        }
        let old_element_overrides = std::mem::take(&mut candidate.plan_element_graphics_overrides);
        for (view, mut overrides) in old_element_overrides {
            let old_count = overrides.len();
            overrides
                .retain(|element, _| candidate.plan_element_graphics_compatible(view, *element));
            if overrides.len() != old_count {
                changed.insert(view);
            }
            if !overrides.is_empty() {
                candidate
                    .plan_element_graphics_overrides
                    .insert(view, overrides);
            }
        }
        candidate
            .element_lifecycles
            .retain(|id, _| current_phaseable.contains(id));
        let latest_phase = candidate
            .latest_phase()
            .ok_or_else(|| Error::Invalid("project has no phases".into()))?;
        for id in current_phaseable.difference(&original_phaseable) {
            candidate
                .element_lifecycles
                .entry(*id)
                .or_insert(ElementLifecycle {
                    created_in: latest_phase,
                    demolished_in: None,
                });
        }
        ensure(
            !candidate.views.is_empty() || self.model.views.is_empty(),
            "cannot remove the last view",
        )?;
        // Authored elevations must resolve against the final transaction graph.
        // Unedited orphans remain preservable after host deletion/path changes.
        for id in edited_views {
            if let Some(settings) = candidate
                .views
                .get(&id)
                .and_then(|view| view.parameters.elevation)
            {
                settings.resolve_host(&candidate)?;
            }
        }
        changed.extend(candidate.settle_opening_clearances(&self.model)?);
        for old in self.model.plan_callouts.values() {
            if let CalloutReference::Placed { viewport, .. } =
                self.model.callout_reference(&old.parameters)
                && candidate
                    .plan_callouts
                    .get(&old.id())
                    .is_some_and(|c| c.parameters == old.parameters)
            {
                ensure(
                    candidate.sheets.values().any(|s| {
                        s.parameters
                            .viewports
                            .iter()
                            .any(|v| v.id == viewport && v.view == old.parameters.target)
                    }),
                    "remove or explicitly repair the callout before deleting its referenced placement",
                )?;
            }
        }
        for old in self.model.section_callouts.values() {
            if let CalloutReference::Placed { viewport, .. } =
                self.model.section_callout_reference(&old.parameters)
                && candidate
                    .section_callouts
                    .get(&old.id())
                    .is_some_and(|c| c.parameters == old.parameters)
            {
                ensure(
                    candidate.sheets.values().any(|s| {
                        s.parameters
                            .viewports
                            .iter()
                            .any(|v| v.id == viewport && v.view == old.parameters.target)
                    }),
                    "remove or explicitly repair the callout before deleting its referenced placement",
                )?;
            }
        }
        os_constraints::validate(&candidate)?;
        // Embedded viewport UUIDs participate in change events as well as identity
        // validation. Retaining a UUID during an edit preserves its identity.
        let sheet_changes: Vec<_> = changed.iter().copied().collect();
        for id in sheet_changes {
            let before = self.model.sheets.get(&id);
            let after = candidate.sheets.get(&id);
            for (source, other) in [(before, after), (after, before)] {
                if let Some(sheet) = source {
                    for viewport in &sheet.parameters.viewports {
                        if other.and_then(|s| {
                            s.parameters.viewports.iter().find(|v| v.id == viewport.id)
                        }) != Some(viewport)
                        {
                            changed.insert(viewport.id);
                        }
                    }
                }
            }
        }
        Ok((candidate, changed))
    }
    fn emit(&mut self, h: &History, label: &str) {
        let mut invalidated = os_constraints::affected_entities(&h.before, &h.changed);
        invalidated.extend(os_constraints::affected_entities(&h.after, &h.changed));
        // Sheet placement edits invalidate both old and new referenced plans.
        // Conversely, changed plan contents/settings invalidate placed viewports
        // and sheets. Resolve to a fixed point so extension dependents also see
        // sheet/viewport UUIDs, including removal and undo/redo transitions.
        let mut seeds = h.changed.clone();
        loop {
            seeds.extend(invalidated.iter().copied());
            let previous = seeds.len();
            for model in [&h.before, &h.after] {
                for callout in model.plan_callouts.values() {
                    let p = &callout.parameters;
                    let reference_changed = model.sheets.values().any(|s| {
                        seeds.contains(&s.id())
                            && s.parameters.viewports.iter().any(|v| v.view == p.target)
                    });
                    if reference_changed
                        || seeds.contains(&callout.id())
                        || seeds.contains(&p.target)
                        || seeds.contains(&p.parent)
                    {
                        invalidated.extend([callout.id(), p.parent, p.target]);
                    }
                }
                for callout in model.section_callouts.values() {
                    let p = &callout.parameters;
                    let reference_changed = model.sheets.values().any(|s| {
                        seeds.contains(&s.id())
                            && s.parameters.viewports.iter().any(|v| v.view == p.target)
                    });
                    if reference_changed
                        || seeds.contains(&callout.id())
                        || seeds.contains(&p.target)
                        || seeds.contains(&p.parent)
                    {
                        invalidated.extend([callout.id(), p.parent, p.target]);
                    }
                }
                for note in model.text_notes.values() {
                    if seeds.contains(&note.id()) || seeds.contains(&note.parameters.view) {
                        invalidated.extend([note.id(), note.parameters.view]);
                    }
                }
                for line in model.detail_lines.values() {
                    if seeds.contains(&line.id()) || seeds.contains(&line.parameters.view) {
                        invalidated.extend([line.id(), line.parameters.view]);
                    }
                }
                for (view, binding) in &model.plan_graphics {
                    if binding.template.is_some_and(|id| seeds.contains(&id)) {
                        invalidated.insert(*view);
                    }
                }
                for sheet in model.sheets.values() {
                    let edited = h.changed.contains(&sheet.id());
                    for viewport in &sheet.parameters.viewports {
                        if edited || h.changed.contains(&viewport.id) {
                            invalidated.extend([sheet.id(), viewport.id, viewport.view]);
                        } else if seeds.contains(&viewport.view) {
                            invalidated.extend([sheet.id(), viewport.id]);
                        }
                    }
                    if edited {
                        invalidated.insert(sheet.id());
                    }
                }
                invalidated.extend(os_constraints::affected_entities(model, &seeds));
            }
            seeds.extend(invalidated.iter().copied());
            if seeds.len() == previous {
                break;
            }
        }
        self.revision += 1;
        self.events.push(ChangeEvent {
            revision: self.revision,
            label: label.into(),
            changed: h.changed.clone(),
            invalidated,
        });
    }
    pub fn undo(&mut self) -> bool {
        let Some(h) = self.history.undo.pop_back() else {
            return false;
        };
        self.emit(&h, &format!("Undo {}", h.label));
        self.model = h.before.clone();
        self.history.redo.push_back(h);
        true
    }
    pub fn redo(&mut self) -> bool {
        let Some(h) = self.history.redo.pop_back() else {
            return false;
        };
        self.emit(&h, &format!("Redo {}", h.label));
        self.model = h.after.clone();
        self.history.undo.push_back(h);
        true
    }
}

#[cfg(test)]
#[path = "tests/openings.rs"]
mod opening_tests;

#[cfg(test)]
#[path = "tests/floors.rs"]
mod floor_tests;

#[cfg(test)]
#[path = "tests/columns.rs"]
mod column_tests;
#[cfg(test)]
#[path = "tests/ramps.rs"]
mod ramp_tests;
#[cfg(test)]
#[path = "tests/stairs.rs"]
mod stair_tests;

#[cfg(test)]
#[path = "tests/casework.rs"]
mod casework_tests;
#[cfg(test)]
#[path = "tests/railings.rs"]
mod railing_tests;

#[cfg(test)]
#[path = "tests/ceilings.rs"]
mod ceiling_tests;

#[cfg(test)]
#[path = "tests/sheets.rs"]
mod sheet_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use os_core::Point2;
    fn wall(doc: &Document) -> Wall {
        Wall::new(
            "org.openstructure.walls.wall",
            WallParams {
                name: "Wall".into(),
                path: os_model::WallPath::Straight {
                    start: Point2::new(0.0, 0.0),
                    end: Point2::new(5.0, 0.0),
                },
                thickness: 0.2,
                height: 3.0,
                level: *doc.model().levels.keys().next().unwrap(),
                material: None,
            },
        )
    }
    #[test]
    fn atomic_history_and_branching() {
        let mut doc = Document::new("Test").unwrap();
        let w = wall(&doc);
        let id = w.id();
        let before = doc.model().clone();
        assert!(
            doc.execute(
                "bad",
                vec![Command::AddWall(w.clone()), Command::RemoveWall(Id::new())]
            )
            .is_err()
        );
        assert_eq!(doc.model(), &before);
        assert_eq!(doc.revision(), 0);
        assert!(!doc.can_undo());
        doc.execute("wall", vec![Command::AddWall(w)]).unwrap();
        assert!(doc.undo());
        assert_eq!(doc.model(), &before);
        assert!(doc.redo());
        assert!(doc.model().walls.contains_key(&id));
        doc.undo();
        doc.execute("rename", vec![Command::RenameProject("Branch".into())])
            .unwrap();
        assert!(!doc.can_redo());
    }
    #[test]
    fn level_changes_invalidate_walls_and_reject_dangling_references() {
        let mut doc = Document::new("Test").unwrap();
        let w = wall(&doc);
        let id = w.id();
        let level = w.parameters.level;
        doc.execute("add", vec![Command::AddWall(w)]).unwrap();
        doc.drain_events();
        let mut parameters = doc.model().levels[&level].parameters.clone();
        parameters.elevation = 4.0;
        doc.execute(
            "raise",
            vec![Command::UpdateLevel {
                id: level,
                parameters,
            }],
        )
        .unwrap();
        assert!(doc.drain_events()[0].invalidated.contains(&id));
        assert!(
            doc.execute("remove level", vec![Command::RemoveLevel(level)])
                .is_err()
        );
    }
}
