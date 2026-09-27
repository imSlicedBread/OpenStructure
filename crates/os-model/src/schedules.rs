//! Saved schedule definitions. Rows are resolved from the current model by consumers.
use crate::{Entity, Model};
use os_core::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const MAX_SCHEDULES: usize = 256;
pub const MAX_SCHEDULE_NAME_BYTES: usize = 128;
pub const MAX_SCHEDULE_FILTERS: usize = 32;
pub const MAX_SCHEDULE_FILTER_TEXT_BYTES: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScheduleTextField {
    Name,
    Type,
    Level,
    Host,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScheduleNumericField {
    Width,
    Height,
    Sill,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScheduleTextOperator {
    Equals,
    NotEquals,
    Contains,
    StartsWith,
}

impl ScheduleTextOperator {
    /// Rust lowercase mapping only; no accent or canonical Unicode normalization.
    pub fn matches(self, actual: &str, expected: &str) -> bool {
        let actual = actual.to_lowercase();
        let expected = expected.to_lowercase();
        match self {
            Self::Equals => actual == expected,
            Self::NotEquals => actual != expected,
            Self::Contains => actual.contains(&expected),
            Self::StartsWith => actual.starts_with(&expected),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScheduleNumericOperator {
    Equals,
    NotEquals,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
}

impl ScheduleNumericOperator {
    /// Compare unrounded metre values without a display tolerance.
    pub fn matches(self, actual: f64, expected: f64) -> bool {
        match self {
            Self::Equals => actual == expected,
            Self::NotEquals => actual != expected,
            Self::Less => actual < expected,
            Self::LessOrEqual => actual <= expected,
            Self::Greater => actual > expected,
            Self::GreaterOrEqual => actual >= expected,
        }
    }
}

/// Tagged wire format keeps field/operator compatibility in the type system.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ScheduleFilter {
    Text {
        field: ScheduleTextField,
        operator: ScheduleTextOperator,
        value: String,
    },
    Numeric {
        field: ScheduleNumericField,
        operator: ScheduleNumericOperator,
        value: f64,
    },
}

impl ScheduleFilter {
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Text { value, .. } => ensure(
                !value.is_empty()
                    && value == value.trim()
                    && value.len() <= MAX_SCHEDULE_FILTER_TEXT_BYTES
                    && !value.chars().any(char::is_control),
                "schedule filter text must be trimmed, nonempty, and at most 256 bytes without control characters",
            ),
            Self::Numeric { value, .. } => ensure(
                value.is_finite(),
                "schedule filter threshold must be finite metres",
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScheduleCategory {
    Door,
    Window,
    All,
    RoomFinish,
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
    Number,
    FloorFinish,
    WallFinish,
    CeilingFinish,
    Area,
    EnclosureStatus,
}

impl ScheduleColumn {
    pub const ROOM: [Self; 8] = [
        Self::Level,
        Self::Number,
        Self::Name,
        Self::FloorFinish,
        Self::WallFinish,
        Self::CeilingFinish,
        Self::Area,
        Self::EnclosureStatus,
    ];
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
            Self::Number => "Number",
            Self::FloorFinish => "Floor Finish",
            Self::WallFinish => "Wall Finish",
            Self::CeilingFinish => "Ceiling Finish",
            Self::Area => "Area (m²)",
            Self::EnclosureStatus => "Enclosure Status",
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
    LevelNumber,
    Number,
    Area,
}
impl ScheduleSort {
    pub const ROOM: [Self; 4] = [Self::LevelNumber, Self::Number, Self::Name, Self::Area];
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
    pub filters: Vec<ScheduleFilter>,
}
pub type Schedule = Entity<ScheduleParams>;

impl ScheduleParams {
    pub fn available_columns(&self) -> &'static [ScheduleColumn] {
        if self.category == ScheduleCategory::RoomFinish {
            &ScheduleColumn::ROOM
        } else {
            &ScheduleColumn::ALL
        }
    }
    pub fn available_sorts(&self) -> &'static [ScheduleSort] {
        if self.category == ScheduleCategory::RoomFinish {
            &ScheduleSort::ROOM
        } else {
            &ScheduleSort::ALL
        }
    }
    pub fn new(name: impl Into<String>, category: ScheduleCategory) -> Self {
        Self {
            name: name.into(),
            category,
            filters: Vec::new(),
            columns: if category == ScheduleCategory::RoomFinish {
                ScheduleColumn::ROOM.to_vec()
            } else {
                ScheduleColumn::ALL.to_vec()
            },
            sort: if category == ScheduleCategory::RoomFinish {
                ScheduleSort::LevelNumber
            } else {
                ScheduleSort::LevelKindName
            },
        }
    }
    pub fn validate(&self) -> Result<()> {
        ensure(
            self.filters.len() <= MAX_SCHEDULE_FILTERS,
            "too many schedule filters (maximum 32)",
        )?;
        ensure(
            self.category != ScheduleCategory::RoomFinish || self.filters.is_empty(),
            "RoomFinish schedule filters are not supported",
        )?;
        for filter in &self.filters {
            filter.validate()?;
        }
        ensure(
            self.columns
                .iter()
                .all(|c| self.available_columns().contains(c))
                && self.available_sorts().contains(&self.sort),
            "schedule columns and sort must match category",
        )?;
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
