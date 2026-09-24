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
mod history;
use history::{History, HistoryStore};
pub use history::{HistoryLimits, HistoryStats};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "command", content = "data")]
pub enum Command {
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
    AddMaterial(Material),
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
    UpdateColumn {
        id: Id,
        parameters: ColumnParams,
    },
    RemoveColumn(Id),
    UpdateFloor {
        id: Id,
        parameters: FloorParams,
    },
    RemoveFloor(Id),
    AddOpeningType(OpeningType),
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
        ensure(!commands.is_empty(), "empty transaction")?;
        let mut candidate = self.model.clone();
        let mut changed = BTreeSet::new();
        for command in commands {
            let id = match command {
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
                Command::AddOpeningType(ty) => {
                    let id = ty.id();
                    ensure(
                        !candidate.opening_types.contains_key(&id),
                        "opening type already exists",
                    )?;
                    candidate.opening_types.insert(id, ty);
                    id
                }
                Command::UpdateOpeningType { id, parameters } => {
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
                    ensure(
                        view.parameters.settings_revision == 0,
                        "new view revision must be zero",
                    )?;
                    let id = view.id();
                    ensure(!candidate.views.contains_key(&id), "view already exists")?;
                    candidate.views.insert(id, view);
                    id
                }
                Command::UpdateView { id, mut parameters } => {
                    parameters.validate_edit()?;
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
                    candidate.plan_graphics.remove(&id);
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
        ensure(
            !candidate.views.is_empty() || self.model.views.is_empty(),
            "cannot remove the last view",
        )?;
        os_constraints::validate(&candidate)?;
        if candidate == self.model {
            return Ok(());
        }
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
                start: Point2::new(0.0, 0.0),
                end: Point2::new(5.0, 0.0),
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
