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
                Command::AddWall(wall) => {
                    let id = wall.id();
                    ensure(!candidate.walls.contains_key(&id), "wall already exists")?;
                    candidate.walls.insert(id, wall);
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
