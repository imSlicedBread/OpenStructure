//! Persisted paper placement foundation, distinct from named model views.
//! No drawing, title block, navigation state, or export data is stored here.
use crate::{Entity, Model, ViewKind};
use os_core::{Id, Point2, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const MAX_SHEETS: usize = 1_000;
pub const MAX_SHEET_VIEWPORTS: usize = 64;
pub const MAX_SHEET_SCHEDULES: usize = 16;

/// Millimetres from the top-left paper corner, +X right, +Y down.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SheetPaperRect {
    #[serde(deserialize_with = "strict_point")]
    pub min_mm: Point2,
    #[serde(deserialize_with = "strict_point")]
    pub max_mm: Point2,
}

impl SheetPaperRect {
    fn overlaps(self, other: Self) -> bool {
        self.min_mm.x < other.max_mm.x
            && self.max_mm.x > other.min_mm.x
            && self.min_mm.y < other.max_mm.y
            && self.max_mm.y > other.min_mm.y
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SheetSchedulePlacement {
    pub id: Id,
    pub schedule: Id,
    pub paper_rect_mm: SheetPaperRect,
}

/// Fixed supported page geometry in millimetres.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SheetPaperSize {
    #[default]
    A3Landscape,
}

impl SheetPaperSize {
    pub fn dimensions_mm(self) -> Point2 {
        match self {
            Self::A3Landscape => Point2::new(420.0, 297.0),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SheetViewport {
    pub id: Id,
    pub view: Id,
    /// Center in the source 2D view plane: plan-plane metres for Plans, or
    /// section station/elevation metres for Sections. Independent of camera.
    #[serde(deserialize_with = "strict_point")]
    pub model_center_m: Point2,
    /// Paper origin is top-left, +X right and +Y down, in millimetres.
    #[serde(deserialize_with = "strict_point")]
    pub paper_center_mm: Point2,
    pub width_mm: f64,
    pub height_mm: f64,
    /// Model metres convert to paper mm by multiplying by 1000 / denominator.
    pub scale_denominator: f64,
    /// None inherits the source view name; an override must be nonempty.
    pub title_override: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SheetParams {
    pub number: String,
    pub name: String,
    pub paper_size: SheetPaperSize,
    pub viewports: Vec<SheetViewport>,
    pub schedule_placements: Vec<SheetSchedulePlacement>,
}
pub type Sheet = Entity<SheetParams>;

fn strict_point<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Point2, D::Error> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Point {
        x: f64,
        y: f64,
    }
    let point = Point::deserialize(deserializer)?;
    Ok(Point2::new(point.x, point.y))
}

fn text_valid(value: &str, limit: usize) -> bool {
    !value.trim().is_empty() && value.len() <= limit && !value.chars().any(char::is_control)
}

impl SheetParams {
    pub fn new(number: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            number: number.into(),
            name: name.into(),
            paper_size: SheetPaperSize::default(),
            viewports: Vec::new(),
            schedule_placements: Vec::new(),
        }
    }

    pub fn validate(&self, model: &Model) -> Result<()> {
        ensure(
            text_valid(&self.number, 64),
            "sheet number must be 1-64 bytes without controls",
        )?;
        ensure(
            text_valid(&self.name, 256),
            "sheet name must be 1-256 bytes without controls",
        )?;
        ensure(
            self.viewports.len() <= MAX_SHEET_VIEWPORTS,
            "sheet exceeds 64 viewports",
        )?;
        let page = self.paper_size.dimensions_mm();
        let mut ids = BTreeSet::new();
        for viewport in &self.viewports {
            ensure(
                !viewport.id.0.is_nil() && ids.insert(viewport.id),
                "invalid or duplicate viewport identity",
            )?;
            ensure(
                model
                    .views
                    .get(&viewport.view)
                    .is_some_and(|view| match view.parameters.kind {
                        ViewKind::Plan => {
                            view.parameters.level.is_some() && view.parameters.plan.is_some()
                        }
                        ViewKind::Section => {
                            view.parameters.level.is_some() && view.parameters.section.is_some()
                        }
                        ViewKind::Perspective => false,
                    }),
                "sheet viewport requires an existing configured Plan or Section view",
            )?;
            let center = viewport.paper_center_mm;
            ensure(
                center.is_finite()
                    && viewport.width_mm.is_finite()
                    && viewport.height_mm.is_finite()
                    && viewport.width_mm >= 0.01
                    && viewport.height_mm >= 0.01
                    && viewport.width_mm <= page.x
                    && viewport.height_mm <= page.y
                    && center.x >= viewport.width_mm / 2.0
                    && center.y >= viewport.height_mm / 2.0
                    && center.x <= page.x - viewport.width_mm / 2.0
                    && center.y <= page.y - viewport.height_mm / 2.0,
                "sheet viewport must be finite, at least 0.01 mm wide/high, and inside the page",
            )?;
            ensure(
                viewport.model_center_m.is_finite()
                    && viewport.model_center_m.x.abs() <= 1e6
                    && viewport.model_center_m.y.abs() <= 1e6,
                "viewport model center must be finite and within +/- 1000000 source-view metres",
            )?;
            ensure(
                viewport.scale_denominator.is_finite()
                    && (0.001..=1e6).contains(&viewport.scale_denominator),
                "viewport scale denominator must be finite and within 0.001-1000000",
            )?;
            ensure(
                viewport
                    .title_override
                    .as_deref()
                    .is_none_or(|title| text_valid(title, 256)),
                "viewport title override must be 1-256 bytes without controls",
            )?;
        }
        ensure(
            self.schedule_placements.len() <= MAX_SHEET_SCHEDULES,
            "sheet exceeds 16 schedule tables",
        )?;
        for (index, placement) in self.schedule_placements.iter().enumerate() {
            ensure(
                !placement.id.0.is_nil() && ids.insert(placement.id),
                "invalid or duplicate schedule placement identity",
            )?;
            ensure(
                model.schedules.contains_key(&placement.schedule),
                "sheet schedule reference is missing",
            )?;
            let rect = placement.paper_rect_mm;
            ensure(
                rect.min_mm.is_finite()
                    && rect.max_mm.is_finite()
                    && rect.min_mm.x >= 8.0
                    && rect.min_mm.y >= 8.0
                    && rect.max_mm.x <= page.x - 8.0
                    && rect.max_mm.y <= page.y - 8.0
                    && rect.max_mm.x - rect.min_mm.x >= 0.01
                    && rect.max_mm.y - rect.min_mm.y >= 0.01,
                "schedule table must be finite, positive and inside the paper frame (8 mm margin)",
            )?;
            ensure(
                rect.max_mm.y <= page.y - 48.0,
                "schedule table overlaps the title block",
            )?;
            for viewport in &self.viewports {
                let other = SheetPaperRect {
                    min_mm: Point2::new(
                        viewport.paper_center_mm.x - viewport.width_mm / 2.0,
                        viewport.paper_center_mm.y - viewport.height_mm / 2.0,
                    ),
                    max_mm: Point2::new(
                        viewport.paper_center_mm.x + viewport.width_mm / 2.0,
                        viewport.paper_center_mm.y + viewport.height_mm / 2.0,
                    ),
                };
                ensure(
                    !rect.overlaps(other),
                    "schedule table overlaps a drawing viewport",
                )?;
            }
            ensure(
                !self.schedule_placements[..index]
                    .iter()
                    .any(|p| rect.overlaps(p.paper_rect_mm)),
                "schedule tables overlap",
            )?;
        }
        Ok(())
    }
}

pub(crate) fn validate(model: &Model, ids: &mut BTreeSet<Id>) -> Result<()> {
    ensure(
        model.sheets.len() <= MAX_SHEETS,
        "model exceeds 1000 sheets",
    )?;
    let mut numbers = BTreeSet::new();
    for sheet in model.sheets.values() {
        sheet.parameters.validate(model)?;
        ensure(
            numbers.insert(sheet.parameters.number.trim().to_ascii_lowercase()),
            "sheet number must be unique (trimmed, ASCII case-insensitive)",
        )?;
        for viewport in &sheet.parameters.viewports {
            ensure(
                ids.insert(viewport.id),
                "duplicate global viewport identity",
            )?;
        }
        for placement in &sheet.parameters.schedule_placements {
            ensure(
                ids.insert(placement.id),
                "duplicate global schedule placement identity",
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ExtensionEntity, PluginRequirement, View, ViewParams};
    use std::collections::BTreeMap;

    fn fixture() -> (Model, Id) {
        let mut model = Model::new("Sheets");
        let view = View::new(
            "core.view",
            ViewParams::floor_plan("Ground plan", *model.levels.keys().next().unwrap()),
        );
        let mut params = SheetParams::new("A101", "Ground floor");
        params.viewports.push(SheetViewport {
            id: Id::new(),
            view: view.id(),
            model_center_m: Point2::new(23.0, -17.0),
            paper_center_mm: Point2::new(210.0, 148.5),
            width_mm: 300.0,
            height_mm: 200.0,
            scale_denominator: 100.0,
            title_override: Some("Plan".into()),
        });
        let sheet = Sheet::new("core.sheet", params);
        let id = sheet.id();
        model.views.insert(view.id(), view);
        model.sheets.insert(id, sheet);
        (model, id)
    }

    #[test]
    fn schedule_placements_validate_references_identity_bounds_and_overlap() {
        let (mut model, id) = fixture();
        let schedule = crate::Schedule::new(
            "core.schedule",
            crate::ScheduleParams::new("Doors", crate::ScheduleCategory::Door),
        );
        let schedule_id = schedule.id();
        model.schedules.insert(schedule_id, schedule);
        let placement = SheetSchedulePlacement {
            id: Id::new(),
            schedule: schedule_id,
            paper_rect_mm: SheetPaperRect {
                min_mm: Point2::new(10.0, 10.0),
                max_mm: Point2::new(50.0, 40.0),
            },
        };
        model
            .sheets
            .get_mut(&id)
            .unwrap()
            .parameters
            .schedule_placements
            .push(placement);
        model.validate().unwrap();
        for case in 0..9 {
            let mut invalid = model.clone();
            let params = &mut invalid.sheets.get_mut(&id).unwrap().parameters;
            match case {
                0 => params.schedule_placements[0].schedule = Id::new(),
                1 => params.schedule_placements[0].id = schedule_id,
                2 => params.schedule_placements[0].paper_rect_mm.min_mm.x = f64::NAN,
                3 => params.schedule_placements[0].paper_rect_mm.max_mm.x = 421.0,
                4 => params.schedule_placements[0].paper_rect_mm.max_mm.y = 260.0,
                5 => params.schedule_placements[0].paper_rect_mm.max_mm = Point2::new(100.0, 80.0),
                6 => params
                    .schedule_placements
                    .push(params.schedule_placements[0].clone()),
                7 => {
                    let mut other = params.schedule_placements[0].clone();
                    other.id = Id::new();
                    params.schedule_placements.push(other);
                }
                _ => {
                    params.schedule_placements =
                        vec![params.schedule_placements[0].clone(); MAX_SHEET_SCHEDULES + 1]
                }
            }
            assert!(invalid.validate().is_err(), "case {case}");
        }
    }

    #[test]
    fn sheets_preserve_identity_centers_and_independent_scales() {
        let (mut model, id) = fixture();
        let sheet = model.sheets.get_mut(&id).unwrap();
        let mut other = sheet.parameters.viewports[0].clone();
        other.id = Id::new();
        other.scale_denominator = 50.0;
        other.model_center_m = Point2::new(-8.0, 6.0);
        other.title_override = None;
        sheet.parameters.viewports.push(other);
        model.validate().unwrap();
        let json = serde_json::to_value(&model).unwrap();
        let restored: Model = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(restored, model);
        assert_eq!(
            SheetPaperSize::default().dimensions_mm(),
            Point2::new(420.0, 297.0)
        );
        for field in ["model_center_m", "paper_center_mm", "scale_denominator"] {
            let mut bad = json.clone();
            bad["sheets"][id.to_string()]["parameters"]["viewports"][0]
                .as_object_mut()
                .unwrap()
                .remove(field);
            assert!(
                serde_json::from_value::<Model>(bad).is_err(),
                "missing {field}"
            );
        }
        for point in ["model_center_m", "paper_center_mm"] {
            let mut bad = json.clone();
            bad["sheets"][id.to_string()]["parameters"]["viewports"][0][point]["z"] = 0.into();
            assert!(serde_json::from_value::<Model>(bad).is_err());
        }
        let mut bad = json;
        bad["sheets"][id.to_string()]["parameters"]["paper_size"] = "A4".into();
        assert!(serde_json::from_value::<Model>(bad).is_err());
    }

    #[test]
    fn sheets_reject_invalid_geometry_references_and_text() {
        let (model, id) = fixture();
        for case in 0..24 {
            let mut bad = model.clone();
            let perspective = bad
                .views
                .values()
                .find(|v| v.parameters.kind == ViewKind::Perspective)
                .unwrap()
                .id();
            let params = &mut bad.sheets.get_mut(&id).unwrap().parameters;
            let viewport = &mut params.viewports[0];
            match case {
                0 => viewport.width_mm = 0.0,
                1 => viewport.height_mm = -1.0,
                2 => viewport.width_mm = f64::NAN,
                3 => viewport.height_mm = f64::INFINITY,
                4 => viewport.paper_center_mm.x = f64::NAN,
                5 => viewport.paper_center_mm.y = f64::INFINITY,
                6 => viewport.paper_center_mm.x = 0.0,
                7 => viewport.paper_center_mm.y = 297.0,
                8 => viewport.width_mm = 421.0,
                9 => viewport.height_mm = 298.0,
                10 => viewport.scale_denominator = 0.0,
                11 => viewport.scale_denominator = -1.0,
                12 => viewport.scale_denominator = f64::INFINITY,
                13 => viewport.scale_denominator = 1e6 + 1.0,
                14 => viewport.model_center_m.x = f64::NAN,
                15 => viewport.model_center_m.y = 1e6 + 1.0,
                16 => viewport.view = Id::new(),
                17 => viewport.view = perspective,
                18 => viewport.title_override = Some("\n".into()),
                19 => viewport.title_override = Some("a".repeat(257)),
                20 => params.number = " ".into(),
                21 => params.name = "\tName".into(),
                22 => params.number = "a".repeat(65),
                _ => params.name = "a".repeat(257),
            }
            assert!(bad.validate().is_err(), "case {case}");
        }
        let mut unassigned = model.clone();
        let view = model.sheets[&id].parameters.viewports[0].view;
        unassigned.views.get_mut(&view).unwrap().parameters.level = None;
        assert!(unassigned.validate().is_err());
        let mut edge = model;
        let viewport = &mut edge.sheets.get_mut(&id).unwrap().parameters.viewports[0];
        viewport.width_mm = 420.0;
        viewport.height_mm = 297.0;
        edge.validate().unwrap();
    }

    #[test]
    fn sheet_viewport_accepts_configured_sections_only() {
        let (mut model, sheet_id) = fixture();
        let level = *model.levels.keys().next().unwrap();
        let section = View::new(
            "core.view",
            ViewParams {
                name: "Building section".into(),
                kind: ViewKind::Section,
                level: Some(level),
                settings_revision: 0,
                plan: None,
                section: Some(crate::SectionViewSettings::new(
                    Point2::new(0.0, 0.0),
                    Point2::new(8.0, 0.0),
                    -0.5,
                    4.0,
                )),
            },
        );
        let section_id = section.id();
        model.views.insert(section_id, section);
        model
            .sheets
            .get_mut(&sheet_id)
            .unwrap()
            .parameters
            .viewports[0]
            .view = section_id;
        model.validate().unwrap();

        let mut unconfigured = model;
        unconfigured
            .views
            .get_mut(&section_id)
            .unwrap()
            .parameters
            .section = None;
        assert!(unconfigured.validate().is_err());
    }

    #[test]
    fn sheets_enforce_global_ids_numbers_and_collection_bounds() {
        let (model, id) = fixture();
        for collision in [
            id,
            model.project.id(),
            model.sheets[&id].parameters.viewports[0].view,
            Id(Default::default()),
        ] {
            let mut bad = model.clone();
            bad.sheets.get_mut(&id).unwrap().parameters.viewports[0].id = collision;
            assert!(bad.validate().is_err());
        }
        let mut duplicate_number = model.clone();
        let mut second = Sheet::new("core.sheet", SheetParams::new(" a101 ", "Duplicate"));
        duplicate_number.sheets.insert(second.id(), second.clone());
        assert!(duplicate_number.validate().is_err());
        second.parameters.number = "A102".into();
        second.parameters.viewports = model.sheets[&id].parameters.viewports.clone();
        let mut duplicate_viewport = model.clone();
        duplicate_viewport.sheets.insert(second.id(), second);
        assert!(duplicate_viewport.validate().is_err());

        let mut extension_collision = model.clone();
        let viewport_id = model.sheets[&id].parameters.viewports[0].id;
        extension_collision.plugin_requirements.insert(
            "org.example.fixture".into(),
            PluginRequirement {
                version: "1.0.0".into(),
            },
        );
        extension_collision.extensions.insert(
            viewport_id,
            ExtensionEntity {
                envelope_version: 1,
                id: viewport_id,
                owner: "org.example.fixture".into(),
                type_id: "org.example.fixture.item".into(),
                name: "Collision".into(),
                payload_schema_version: 1,
                relationships: BTreeMap::new(),
                depends_on: BTreeSet::new(),
                payload: serde_json::json!({}),
            },
        );
        assert!(extension_collision.validate().is_err());
        let mut bounded = model.clone();
        let viewport = model.sheets[&id].parameters.viewports[0].clone();
        bounded.sheets.get_mut(&id).unwrap().parameters.viewports = (0..MAX_SHEET_VIEWPORTS)
            .map(|_| SheetViewport {
                id: Id::new(),
                ..viewport.clone()
            })
            .collect();
        bounded.validate().unwrap();
        bounded
            .sheets
            .get_mut(&id)
            .unwrap()
            .parameters
            .viewports
            .push(viewport);
        assert!(bounded.validate().is_err());
        let mut bounded = model;
        for n in 1..MAX_SHEETS {
            let sheet = Sheet::new("core.sheet", SheetParams::new(format!("S{n}"), "Sheet"));
            bounded.sheets.insert(sheet.id(), sheet);
        }
        bounded.validate().unwrap();
        let sheet = Sheet::new("core.sheet", SheetParams::new("Extra", "Sheet"));
        bounded.sheets.insert(sheet.id(), sheet);
        assert!(bounded.validate().is_err());
    }

    #[test]
    fn sheets_account_for_owned_history_allocations() {
        let (mut model, id) = fixture();
        let mut empty = model.clone();
        empty.sheets.clear();
        assert!(
            model.estimated_memory_bytes()
                > empty.estimated_memory_bytes() + std::mem::size_of::<Sheet>()
        );
        let before = model.estimated_memory_bytes();
        let sheet = model.sheets.get_mut(&id).unwrap();
        sheet.parameters.name.reserve(4096);
        sheet.parameters.number.reserve(4096);
        sheet.parameters.viewports[0]
            .title_override
            .as_mut()
            .unwrap()
            .reserve(4096);
        sheet.parameters.viewports.reserve(100);
        sheet
            .header
            .properties
            .insert("metadata".into(), "x".repeat(4096).into());
        assert!(
            model.estimated_memory_bytes()
                >= before + 4 * 4096 + 100 * std::mem::size_of::<SheetViewport>()
        );
    }
}
