//! Saved schedule definitions. Rows are resolved from the current model by consumers.
use crate::{Entity, Model};
use os_core::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const MAX_SCHEDULES: usize = 256;
pub const MAX_SCHEDULE_NAME_BYTES: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScheduleCategory {
    Door,
    Window,
    All,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ScheduleColumn {
    Name,
    Type,
    Level,
    Host,
    Width,
    Height,
    Sill,
    Id,
}

impl ScheduleColumn {
    pub const ALL: [Self; 8] = [
        Self::Name,
        Self::Type,
        Self::Level,
        Self::Host,
        Self::Width,
        Self::Height,
        Self::Sill,
        Self::Id,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Name => "Instance / name",
            Self::Type => "Type",
            Self::Level => "Level",
            Self::Host => "Host wall",
            Self::Width => "Width (m)",
            Self::Height => "Height (m)",
            Self::Sill => "Sill (m)",
            Self::Id => "Stable ID",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScheduleSort {
    LevelKindName,
    Name,
    Type,
    Width,
    Height,
    Sill,
}
impl ScheduleSort {
    pub const ALL: [Self; 6] = [
        Self::LevelKindName,
        Self::Name,
        Self::Type,
        Self::Width,
        Self::Height,
        Self::Sill,
    ];
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScheduleParams {
    pub name: String,
    pub category: ScheduleCategory,
    pub columns: Vec<ScheduleColumn>,
    pub sort: ScheduleSort,
}
pub type Schedule = Entity<ScheduleParams>;

impl ScheduleParams {
    pub fn new(name: impl Into<String>, category: ScheduleCategory) -> Self {
        Self {
            name: name.into(),
            category,
            columns: ScheduleColumn::ALL.to_vec(),
            sort: ScheduleSort::LevelKindName,
        }
    }
    pub fn validate(&self) -> Result<()> {
        ensure(
            !self.name.trim().is_empty()
                && self.name.len() <= MAX_SCHEDULE_NAME_BYTES
                && self.name == self.name.trim()
                && !self.name.chars().any(char::is_control),
            "schedule name must be trimmed, nonempty, and at most 128 bytes without control characters",
        )?;
        ensure(
            !self.columns.is_empty() && self.columns.len() <= ScheduleColumn::ALL.len(),
            "schedule requires 1 to 8 columns",
        )?;
        ensure(
            self.columns.iter().copied().collect::<BTreeSet<_>>().len() == self.columns.len(),
            "duplicate schedule columns",
        )
    }
}

pub(crate) fn validate(model: &Model) -> Result<()> {
    ensure(
        model.schedules.len() <= MAX_SCHEDULES,
        "too many schedules (maximum 256)",
    )?;
    let mut names = BTreeSet::new();
    for schedule in model.schedules.values() {
        schedule.parameters.validate()?;
        ensure(
            names.insert(schedule.parameters.name.to_lowercase()),
            "schedule names must be unique ignoring case",
        )?;
    }
    Ok(())
}
