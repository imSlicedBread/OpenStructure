//! Ordered project phases and element lifetimes.
use crate::{Entity, Model};
use os_core::{Id, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const MAX_PHASES: usize = 64;
pub const MAX_PHASE_NAME_BYTES: usize = 128;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhaseParams {
    pub name: String,
    /// Zero-based display order. Stable IDs, not this ordinal, are references.
    pub order: u32,
}

impl PhaseParams {
    pub fn validate(&self) -> Result<()> {
        ensure(
            !self.name.trim().is_empty()
                && self.name == self.name.trim()
                && self.name.len() <= MAX_PHASE_NAME_BYTES
                && !self.name.chars().any(char::is_control),
            "phase name must be trimmed, nonempty, and at most 128 bytes without control characters",
        )
    }
}

pub type Phase = Entity<PhaseParams>;

pub fn new_phase(name: impl Into<String>, order: u32) -> Phase {
    Entity::new(
        "core.phase",
        PhaseParams {
            name: name.into(),
            order,
        },
    )
}

/// A building element's lifetime relative to the ordered project phases.
/// Temporary is derived when creation and demolition use the same phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ElementLifecycle {
    pub created_in: Id,
    pub demolished_in: Option<Id>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PhaseStatus {
    Existing,
    New,
    Demolished,
    Temporary,
    Future,
    PreviouslyDemolished,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PhaseFilter {
    /// Existing, new, demolished and temporary elements through the target phase.
    #[default]
    ShowAll,
    ShowExisting,
    ShowNew,
    ShowDemolished,
    ShowTemporary,
}

impl PhaseFilter {
    pub fn includes(self, status: PhaseStatus) -> bool {
        match self {
            Self::ShowAll => matches!(
                status,
                PhaseStatus::Existing
                    | PhaseStatus::New
                    | PhaseStatus::Demolished
                    | PhaseStatus::Temporary
            ),
            Self::ShowExisting => status == PhaseStatus::Existing,
            Self::ShowNew => status == PhaseStatus::New,
            Self::ShowDemolished => status == PhaseStatus::Demolished,
            Self::ShowTemporary => status == PhaseStatus::Temporary,
        }
    }
}

impl Model {
    pub fn ordered_phases(&self) -> Vec<&Phase> {
        let mut phases: Vec<_> = self.phases.values().collect();
        phases.sort_by_key(|phase| (phase.parameters.order, phase.id()));
        phases
    }

    pub fn existing_phase(&self) -> Option<Id> {
        self.ordered_phases().first().map(|phase| phase.id())
    }

    pub fn latest_phase(&self) -> Option<Id> {
        self.ordered_phases().last().map(|phase| phase.id())
    }

    pub fn is_phaseable_element(&self, id: Id) -> bool {
        self.walls.contains_key(&id)
            || self.openings.contains_key(&id)
            || self.floors.contains_key(&id)
            || self.stairs.contains_key(&id)
            || self.roofs.contains_key(&id)
            || self.ceilings.contains_key(&id)
            || self.columns.contains_key(&id)
            || self.rooms.contains_key(&id)
            || self.room_separation_lines.contains_key(&id)
    }

    /// Unassigned legacy/native elements are Existing in the earliest phase.
    pub fn element_lifecycle(&self, id: Id) -> Option<ElementLifecycle> {
        if !self.is_phaseable_element(id) {
            return None;
        }
        self.element_lifecycles.get(&id).copied().or_else(|| {
            self.existing_phase().map(|created_in| ElementLifecycle {
                created_in,
                demolished_in: None,
            })
        })
    }

    pub fn phase_status(&self, id: Id, target: Id) -> Result<PhaseStatus> {
        let order = |phase: Id| {
            self.phases
                .get(&phase)
                .map(|phase| phase.parameters.order)
                .ok_or_else(|| os_core::Error::Invalid("phase reference is missing".into()))
        };
        let target_order = order(target)?;
        let lifecycle = self
            .element_lifecycle(id)
            .ok_or_else(|| os_core::Error::Invalid("element phase is unavailable".into()))?;
        let created_order = order(lifecycle.created_in)?;
        let demolished_order = lifecycle.demolished_in.map(order).transpose()?;
        if created_order > target_order {
            return Ok(PhaseStatus::Future);
        }
        if let Some(demolished_order) = demolished_order {
            if demolished_order < target_order {
                return Ok(PhaseStatus::PreviouslyDemolished);
            }
            if demolished_order == target_order {
                return Ok(if created_order == target_order {
                    PhaseStatus::Temporary
                } else {
                    PhaseStatus::Demolished
                });
            }
        }
        Ok(if created_order == target_order && target_order > 0 {
            PhaseStatus::New
        } else {
            PhaseStatus::Existing
        })
    }

    pub(crate) fn validate_phases(&self) -> Result<()> {
        ensure(
            !self.phases.is_empty() && self.phases.len() <= MAX_PHASES,
            "project requires 1 to 64 phases",
        )?;
        let mut orders = BTreeSet::new();
        let mut names = BTreeSet::new();
        for (id, phase) in &self.phases {
            ensure(*id == phase.id(), "phase map key does not match identity")?;
            phase.parameters.validate()?;
            ensure(
                orders.insert(phase.parameters.order),
                "duplicate phase order",
            )?;
            ensure(
                names.insert(phase.parameters.name.to_lowercase()),
                "phase names must be unique without regard to case",
            )?;
        }
        ensure(
            orders.iter().copied().eq(0..self.phases.len() as u32),
            "phase order must be contiguous starting at zero",
        )?;

        ensure(
            self.element_lifecycles.len() <= 1_000_000,
            "element lifecycle limit exceeded",
        )?;
        for (id, lifecycle) in &self.element_lifecycles {
            ensure(
                self.is_phaseable_element(*id),
                "element lifecycle references a missing or unsupported element",
            )?;
            let created = self.phases.get(&lifecycle.created_in).ok_or_else(|| {
                os_core::Error::Invalid("element creation phase is missing".into())
            })?;
            if let Some(demolished_in) = lifecycle.demolished_in {
                let demolished = self.phases.get(&demolished_in).ok_or_else(|| {
                    os_core::Error::Invalid("element demolition phase is missing".into())
                })?;
                ensure(
                    created.parameters.order <= demolished.parameters.order,
                    "element cannot be demolished before it is created",
                )?;
            }
        }
        for opening in self.openings.values() {
            let Some(host) = self.walls.get(&opening.parameters.host) else {
                continue;
            };
            let Some(opening_lifecycle) = self.element_lifecycles.get(&opening.id()).copied()
            else {
                // An omitted lifecycle remains the legacy Existing fallback. Do not
                // infer a contradictory creation phase from a newer host.
                continue;
            };
            let host_lifecycle = self
                .element_lifecycle(host.id())
                .ok_or_else(|| os_core::Error::Invalid("host wall phase is unavailable".into()))?;
            let opening_created = self.phases[&opening_lifecycle.created_in].parameters.order;
            let host_created = self.phases[&host_lifecycle.created_in].parameters.order;
            ensure(
                host_created <= opening_created,
                "hosted opening cannot be created before its wall",
            )?;
            if let Some(host_demolished_in) = host_lifecycle.demolished_in {
                let host_demolished = self.phases[&host_demolished_in].parameters.order;
                ensure(
                    opening_created <= host_demolished,
                    "hosted opening cannot be created after its wall is demolished",
                )?;
                let opening_demolished = opening_lifecycle
                    .demolished_in
                    .map(|phase| self.phases[&phase].parameters.order);
                ensure(
                    opening_demolished.is_some_and(|order| order <= host_demolished),
                    "hosted opening must be demolished no later than its wall",
                )?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Opening, OpeningDefinition, OpeningKind, OpeningParams, Wall, WallParams};
    use os_core::Point2;

    #[test]
    fn lifecycle_status_tracks_existing_new_demolition_temporary_and_future() {
        let mut model = Model::new("Phases");
        let phases = model.ordered_phases();
        let existing = phases[0].id();
        let new_construction = phases[1].id();
        let wall = Wall::new(
            "org.openstructure.walls.wall",
            WallParams {
                name: "Partition".into(),
                path: crate::WallPath::Straight {
                    start: Point2::new(0.0, 0.0),
                    end: Point2::new(4.0, 0.0),
                },
                thickness: 0.2,
                height: 3.0,
                level: model.levels.keys().next().copied().unwrap(),
                material: None,
            },
        );
        let wall_id = wall.id();
        model.walls.insert(wall_id, wall);
        model.element_lifecycles.insert(
            wall_id,
            ElementLifecycle {
                created_in: existing,
                demolished_in: Some(new_construction),
            },
        );
        assert_eq!(
            model.phase_status(wall_id, existing).unwrap(),
            PhaseStatus::Existing
        );
        assert_eq!(
            model.phase_status(wall_id, new_construction).unwrap(),
            PhaseStatus::Demolished
        );
        assert!(
            model.phase_status(wall_id, Id::new()).is_err(),
            "unknown target phases are rejected"
        );
        assert!(
            model.phase_status(Id::new(), existing).is_err(),
            "missing element IDs have no phase status"
        );

        model.element_lifecycles.insert(
            wall_id,
            ElementLifecycle {
                created_in: new_construction,
                demolished_in: Some(new_construction),
            },
        );
        assert_eq!(
            model.phase_status(wall_id, new_construction).unwrap(),
            PhaseStatus::Temporary
        );
        assert_eq!(
            model.phase_status(wall_id, existing).unwrap(),
            PhaseStatus::Future
        );
        assert!(PhaseFilter::ShowTemporary.includes(PhaseStatus::Temporary));
        assert!(!PhaseFilter::ShowAll.includes(PhaseStatus::Future));
    }

    #[test]
    fn project_phases_are_ordered_bounded_and_strictly_validated() {
        let mut model = Model::new("Phases");
        assert_eq!(model.ordered_phases()[0].parameters.name, "Existing");
        assert_eq!(
            model.ordered_phases()[1].parameters.name,
            "New Construction"
        );
        model.validate().unwrap();

        let duplicate_order = new_phase("Duplicate", 1);
        model.phases.insert(duplicate_order.id(), duplicate_order);
        assert!(model.validate().is_err());
    }

    #[test]
    fn hosted_opening_lifetime_must_fit_inside_its_host_wall_lifetime() {
        let mut model = Model::new("Hosted phasing");
        let phases = model.ordered_phases();
        let existing = phases[0].id();
        let new_construction = phases[1].id();
        let wall = Wall::new(
            "org.openstructure.walls.wall",
            WallParams {
                name: "Existing host".into(),
                path: crate::WallPath::Straight {
                    start: Point2::new(0.0, 0.0),
                    end: Point2::new(4.0, 0.0),
                },
                thickness: 0.2,
                height: 3.0,
                level: model.levels.keys().next().copied().unwrap(),
                material: None,
            },
        );
        let wall_id = wall.id();
        model.walls.insert(wall_id, wall);
        model.element_lifecycles.insert(
            wall_id,
            ElementLifecycle {
                created_in: existing,
                demolished_in: Some(new_construction),
            },
        );
        let opening = Opening::new(
            "core.opening",
            OpeningParams {
                width_override: None,
                height_override: None,
                sill_override: None,
                pane_position_override: None,
                lite_side_override: None,
                name: "New door".into(),
                host: wall_id,
                offset: 1.0,
                definition: OpeningDefinition::Legacy {
                    kind: OpeningKind::Door,
                    width: 0.9,
                    height: 2.1,
                    sill: 0.0,
                },
                hinge: Default::default(),
                swing: Default::default(),
            },
        );
        let opening_id = opening.id();
        model.openings.insert(opening_id, opening);
        model.element_lifecycles.insert(
            opening_id,
            ElementLifecycle {
                created_in: new_construction,
                demolished_in: None,
            },
        );
        assert!(model.validate().is_err(), "opening outlives its host");

        model
            .element_lifecycles
            .get_mut(&opening_id)
            .unwrap()
            .demolished_in = Some(new_construction);
        model.validate().unwrap();

        model
            .element_lifecycles
            .get_mut(&wall_id)
            .unwrap()
            .demolished_in = Some(existing);
        assert!(
            model.validate().is_err(),
            "opening cannot be created after the host wall was demolished"
        );
    }
}
