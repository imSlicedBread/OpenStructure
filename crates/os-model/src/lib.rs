//! Serializable semantic model. Geometry is deliberately absent.
use os_core::{Id, Point2, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const SCHEMA_VERSION: u32 = 54;
mod wall_path;
pub use wall_path::*;
mod phases;
pub use phases::*;
mod opening_tags;
pub use opening_tags::*;
mod plan_graphics;
pub use plan_graphics::*;
mod columns;
pub use columns::*;
mod wall_types;
pub use wall_types::*;
mod wall_joins;
pub use wall_joins::*;
mod detail_lines;
pub use detail_lines::*;
mod schedules;
pub use schedules::*;
mod sheets;
pub use sheets::*;
mod sections;
pub use sections::*;
mod floors;
pub use floors::*;
mod stairs;
pub use stairs::*;
mod ceilings;
pub use ceilings::*;
mod roofs;
pub use roofs::*;
mod dimensions;
pub use dimensions::*;
mod room_separation_lines;
pub use room_separation_lines::*;
mod rooms;
pub use rooms::*;
mod room_tags;
pub use room_tags::*;
mod openings;
pub use openings::*;
mod opening_family;
pub use opening_family::*;
mod extensions;
mod grids;
mod memory;
mod plans;
pub use extensions::*;
pub use grids::*;
pub use plans::*;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Header {
    pub id: Id,
    pub type_id: String,
    pub schema_version: u32,
    pub properties: BTreeMap<String, serde_json::Value>,
    /// Additional typed relationships beyond the parameter parent references.
    pub relationships: BTreeMap<String, Vec<Id>>,
}
impl Header {
    pub fn new(type_id: &str) -> Self {
        Self {
            id: Id::new(),
            type_id: type_id.into(),
            schema_version: SCHEMA_VERSION,
            properties: BTreeMap::new(),
            relationships: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Entity<T> {
    pub header: Header,
    pub parameters: T,
}
impl<T> Entity<T> {
    pub fn new(type_id: &str, parameters: T) -> Self {
        Self {
            header: Header::new(type_id),
            parameters,
        }
    }
    pub fn id(&self) -> Id {
        self.header.id
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectParams {
    pub name: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SiteParams {
    pub name: String,
    pub project: Id,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BuildingParams {
    pub name: String,
    pub site: Id,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LevelParams {
    pub name: String,
    pub elevation: f64,
    pub building: Id,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MaterialParams {
    pub name: String,
    pub density_kg_m3: f64,
    /// Opaque display RGB bytes, independent of density and material identity.
    pub color: [u8; 3],
}
/// Frozen pre-schema-34 display swatch, also used for unassigned layer fallback.
pub fn legacy_surface_color(id: Id) -> [u8; 3] {
    let b = id.0.as_bytes();
    [140 + b[0] % 80, 140 + b[1] % 80, 140 + b[2] % 80]
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewParams {
    pub name: String,
    pub kind: ViewKind,
    pub level: Option<Id>,
    /// Host-managed state version; document revision also guards undo/redo.
    pub settings_revision: u64,
    pub plan: Option<PlanSettings>,
    /// Persisted section plane and vertical range for native Section views.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub section: Option<SectionViewSettings>,
}
impl ViewParams {
    pub fn reflected_ceiling_plan(name: impl Into<String>, level: Id) -> Self {
        Self {
            plan: Some(PlanSettings::reflected_ceiling()),
            ..Self::floor_plan(name, level)
        }
    }

    pub fn floor_plan(name: impl Into<String>, level: Id) -> Self {
        Self {
            name: name.into(),
            kind: ViewKind::Plan,
            level: Some(level),
            settings_revision: 0,
            plan: Some(PlanSettings::default()),
            section: None,
        }
    }
    pub fn validate(&self) -> Result<()> {
        match (self.kind, self.plan, self.section) {
            (ViewKind::Plan, Some(plan), None) => plan.validate(),
            (ViewKind::Plan, None, _) => Err(os_core::Error::Invalid(
                "floor plan settings missing".into(),
            )),
            (ViewKind::Plan, Some(_), Some(_)) => Err(os_core::Error::Invalid(
                "section settings cannot be attached to a Plan view".into(),
            )),
            (ViewKind::Section, None, Some(section)) => {
                ensure(self.level.is_some(), "section view needs a marker level")?;
                section.validate()
            }
            (ViewKind::Section, None, None) => Ok(()),
            (ViewKind::Section, Some(_), _) => Err(os_core::Error::Invalid(
                "plan settings cannot be attached to a Section view".into(),
            )),
            (ViewKind::Perspective, None, None) => Ok(()),
            (ViewKind::Perspective, _, _) => Err(os_core::Error::Invalid(
                "model-view settings cannot be attached to a Perspective view".into(),
            )),
        }
    }
    /// New/edited views must be usable. Loaded legacy unassigned/unnamed views
    /// remain preservable and can be repaired explicitly, never guessed on open.
    pub fn validate_edit(&self) -> Result<()> {
        self.validate()?;
        ensure(
            !self.name.trim().is_empty()
                && self.name.len() <= 256
                && !self.name.chars().any(char::is_control),
            "view name must be 1-256 bytes without control characters",
        )?;
        ensure(
            self.kind != ViewKind::Plan || self.level.is_some(),
            "floor plan needs an associated level",
        )?;
        ensure(
            self.kind != ViewKind::Section || (self.level.is_some() && self.section.is_some()),
            "section view needs a marker level and section definition",
        )
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ViewKind {
    Perspective,
    Plan,
    Section,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WallParams {
    pub name: String,
    pub path: WallPath,
    pub thickness: f64,
    pub height: f64,
    pub level: Id,
    pub material: Option<Id>,
}
impl WallParams {
    pub fn start(&self) -> Point2 {
        self.path.start()
    }
    pub fn end(&self) -> Point2 {
        self.path.end()
    }
    pub fn length(&self) -> f64 {
        self.path.length()
    }
    pub fn validate(&self) -> Result<()> {
        self.path.validate(self.thickness)?;
        ensure(
            self.thickness.is_finite() && self.thickness > 1e-6,
            "wall thickness must be positive",
        )?;
        ensure(
            self.height.is_finite() && self.height > 1e-6,
            "wall height must be positive",
        )?;
        ensure(!self.name.trim().is_empty(), "wall name is empty")
    }
    /// Change length while retaining the start point and direction.
    pub fn with_length(&self, length: f64) -> Result<Self> {
        self.validate()?;
        ensure(length.is_finite() && length > 1e-6, "invalid wall length")?;
        let mut result = self.clone();
        let scale = length / self.length();
        result.path = match self.path {
            WallPath::Straight { start, end } => WallPath::Straight {
                start,
                end: Point2::new(
                    start.x + (end.x - start.x) * scale,
                    start.y + (end.y - start.y) * scale,
                ),
            },
            WallPath::CircularArc {
                center,
                radius,
                start_angle_rad,
                signed_sweep_rad,
            } => WallPath::CircularArc {
                center,
                radius,
                start_angle_rad,
                signed_sweep_rad: signed_sweep_rad * scale,
            },
        };
        result.validate()?;
        Ok(result)
    }
}
pub type Project = Entity<ProjectParams>;
pub type Site = Entity<SiteParams>;
pub type Building = Entity<BuildingParams>;
pub type Level = Entity<LevelParams>;
pub type Wall = Entity<WallParams>;
pub type Material = Entity<MaterialParams>;
pub type View = Entity<ViewParams>;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Model {
    pub schema_version: u32,
    pub project: Project,
    pub sites: BTreeMap<Id, Site>,
    pub buildings: BTreeMap<Id, Building>,
    pub levels: BTreeMap<Id, Level>,
    pub walls: BTreeMap<Id, Wall>,
    pub wall_types: BTreeMap<Id, WallType>,
    pub wall_type_assignments: BTreeMap<Id, WallTypeAssignment>,
    pub wall_joins: BTreeMap<Id, WallJoin>,
    pub floors: BTreeMap<Id, Floor>,
    pub stairs: BTreeMap<Id, Stair>,
    pub roofs: BTreeMap<Id, Roof>,
    pub ceilings: BTreeMap<Id, Ceiling>,
    pub columns: BTreeMap<Id, Column>,
    pub openings: BTreeMap<Id, Opening>,
    pub opening_types: BTreeMap<Id, OpeningType>,
    pub rooms: BTreeMap<Id, Room>,
    pub room_separation_lines: BTreeMap<Id, RoomSeparationLine>,
    pub phases: BTreeMap<Id, Phase>,
    pub element_lifecycles: BTreeMap<Id, ElementLifecycle>,
    pub room_tags: BTreeMap<Id, RoomTag>,
    pub opening_tags: BTreeMap<Id, OpeningTag>,
    pub detail_lines: BTreeMap<Id, DetailLine>,
    pub dimensions: BTreeMap<Id, Dimension>,
    pub grids: BTreeMap<Id, Grid>,
    pub materials: BTreeMap<Id, Material>,
    pub views: BTreeMap<Id, View>,
    pub plan_graphics_templates: BTreeMap<Id, PlanGraphicsTemplate>,
    pub plan_graphics: BTreeMap<Id, PlanGraphicsBinding>,
    pub sheets: BTreeMap<Id, Sheet>,
    pub schedules: BTreeMap<Id, Schedule>,
    pub extensions: BTreeMap<Id, ExtensionEntity>,
    pub plugin_requirements: BTreeMap<String, PluginRequirement>,
}
impl Model {
    /// Bootstrap is only exposed through Document::new in application workflows.
    pub fn new(name: &str) -> Self {
        let project = Project::new("core.project", ProjectParams { name: name.into() });
        let site = Site::new(
            "core.site",
            SiteParams {
                name: "Site".into(),
                project: project.id(),
            },
        );
        let building = Building::new(
            "core.building",
            BuildingParams {
                name: "Building".into(),
                site: site.id(),
            },
        );
        let level = Level::new(
            "core.level",
            LevelParams {
                name: "Ground".into(),
                elevation: 0.0,
                building: building.id(),
            },
        );
        let view = View::new(
            "core.view",
            ViewParams {
                name: "3D".into(),
                kind: ViewKind::Perspective,
                level: None,
                settings_revision: 0,
                plan: None,
                section: None,
            },
        );
        let existing_phase = new_phase("Existing", 0);
        let new_construction_phase = new_phase("New Construction", 1);
        Self {
            schema_version: SCHEMA_VERSION,
            project,
            sites: BTreeMap::from([(site.id(), site)]),
            buildings: BTreeMap::from([(building.id(), building)]),
            levels: BTreeMap::from([(level.id(), level)]),
            walls: BTreeMap::new(),
            wall_types: BTreeMap::new(),
            wall_type_assignments: BTreeMap::new(),
            wall_joins: BTreeMap::new(),
            floors: BTreeMap::new(),
            stairs: BTreeMap::new(),
            roofs: BTreeMap::new(),
            ceilings: BTreeMap::new(),
            columns: BTreeMap::new(),
            openings: BTreeMap::new(),
            opening_types: BTreeMap::new(),
            rooms: BTreeMap::new(),
            room_separation_lines: BTreeMap::new(),
            phases: BTreeMap::from([
                (existing_phase.id(), existing_phase),
                (new_construction_phase.id(), new_construction_phase),
            ]),
            element_lifecycles: BTreeMap::new(),
            room_tags: BTreeMap::new(),
            opening_tags: BTreeMap::new(),
            detail_lines: BTreeMap::new(),
            dimensions: BTreeMap::new(),
            grids: BTreeMap::new(),
            materials: BTreeMap::new(),
            views: BTreeMap::from([(view.id(), view)]),
            plan_graphics_templates: BTreeMap::new(),
            plan_graphics: BTreeMap::new(),
            sheets: BTreeMap::new(),
            schedules: BTreeMap::new(),
            extensions: BTreeMap::new(),
            plugin_requirements: BTreeMap::new(),
        }
    }

    pub fn validate(&self) -> Result<()> {
        ensure(
            self.schema_version == SCHEMA_VERSION,
            "unsupported model schema",
        )?;
        let mut ids = BTreeSet::new();
        let mut headers = Vec::new();
        let mut check = |key: Id, h: &Header, expected: &str| -> Result<()> {
            ensure(key == h.id && !h.id.0.is_nil(), "invalid entity identity")?;
            ensure(ids.insert(h.id), "duplicate entity identity")?;
            ensure(
                h.type_id == expected,
                format!("expected entity type {expected}"),
            )?;
            ensure(
                h.schema_version == SCHEMA_VERSION,
                "unsupported entity schema",
            )
        };
        check(self.project.id(), &self.project.header, "core.project")?;
        headers.push(&self.project.header);
        macro_rules! check_map {
            ($field:ident, $type:literal) => {
                for (id, e) in &self.$field {
                    check(*id, &e.header, $type)?;
                    headers.push(&e.header);
                }
            };
        }
        check_map!(sites, "core.site");
        check_map!(buildings, "core.building");
        check_map!(levels, "core.level");
        check_map!(columns, "core.column");
        check_map!(grids, "core.grid");
        check_map!(walls, "org.openstructure.walls.wall");
        check_map!(wall_types, "core.wall_type");
        ensure(
            self.wall_types.len() <= MAX_WALL_TYPES,
            "wall type limit exceeded",
        )?;
        for ty in self.wall_types.values() {
            ty.parameters.validate(self)?;
        }
        for id in self.wall_type_assignments.keys() {
            ensure(
                self.walls.contains_key(id),
                "wall type assignment wall missing",
            )?;
            self.resolve_wall(*id)?;
        }
        check_map!(wall_joins, "core.wall_join");
        validate_wall_joins(self)?;
        check_map!(openings, "core.opening");
        check_map!(opening_types, "core.opening_type");
        check_map!(rooms, "core.room");
        check_map!(room_separation_lines, "core.room_separation_line");
        check_map!(phases, "core.phase");
        self.validate_phases()?;
        room_separation_lines::validate(self)?;
        check_map!(room_tags, "core.room_tag");
        check_map!(opening_tags, "core.opening_tag");
        check_map!(detail_lines, "core.detail_line");
        detail_lines::validate(self)?;
        check_map!(floors, "core.floor");
        check_map!(stairs, "core.stair");
        check_map!(roofs, "core.roof");
        check_map!(ceilings, "core.ceiling");
        check_map!(dimensions, "core.dimension");
        check_map!(materials, "core.material");
        check_map!(views, "core.view");
        check_map!(plan_graphics_templates, "core.plan_graphics_template");
        self.validate_plan_graphics()?;
        check_map!(sheets, "core.sheet");
        check_map!(schedules, "core.schedule");
        schedules::validate(self)?;
        sheets::validate(self, &mut ids)?;
        extensions::validate(self, &mut ids)?;
        for ty in self.wall_types.values() {
            for layer in &ty.parameters.layers {
                ensure(ids.insert(layer.id), "wall layer identity already in use")?;
            }
        }
        for h in headers {
            for targets in h.relationships.values() {
                for target in targets {
                    ensure(ids.contains(target), "dangling relationship")?;
                }
            }
        }
        ensure(
            !self.project.parameters.name.trim().is_empty(),
            "project name is empty",
        )?;
        for e in self.sites.values() {
            ensure(
                e.parameters.project == self.project.id(),
                "site project missing",
            )?;
        }
        for e in self.buildings.values() {
            ensure(
                self.sites.contains_key(&e.parameters.site),
                "building site missing",
            )?;
        }
        for e in self.levels.values() {
            ensure(
                self.buildings.contains_key(&e.parameters.building),
                "level building missing",
            )?;
            ensure(
                e.parameters.elevation.is_finite(),
                "level elevation must be finite",
            )?;
            ensure(!e.parameters.name.trim().is_empty(), "level name is empty")?;
        }
        let mut grid_names = BTreeSet::new();
        for grid in self.grids.values() {
            grid.parameters.validate()?;
            ensure(
                self.buildings.contains_key(&grid.parameters.building),
                "grid building missing",
            )?;
            ensure(
                grid_names.insert((grid.parameters.building, grid.parameters.name.as_str())),
                "duplicate grid name in building",
            )?;
        }
        for e in self.walls.values() {
            e.parameters.validate()?;
            ensure(
                self.levels.contains_key(&e.parameters.level),
                "wall level missing",
            )?;
            if let Some(id) = e.parameters.material {
                ensure(self.materials.contains_key(&id), "wall material missing")?;
            }
        }
        openings::validate(self)?;
        for column in self.columns.values() {
            column.parameters.validate_in(self)?;
        }
        for stair in self.stairs.values() {
            stair.parameters.validate_in(self)?;
        }
        for ceiling in self.ceilings.values() {
            ceiling.parameters.validate_in(self)?;
        }
        for roof in self.roofs.values() {
            roof.parameters.validate_in(self)?;
        }
        for floor in self.floors.values() {
            let p = &floor.parameters;
            p.validate()?;
            let level = self
                .levels
                .get(&p.level)
                .ok_or_else(|| os_core::Error::Invalid("floor level missing".into()))?;
            let top = level.parameters.elevation + p.top_offset;
            ensure(
                top.is_finite() && (top - p.thickness).is_finite() && top - p.thickness < top,
                "floor elevation overflow",
            )?;
            if let Some(id) = p.material {
                ensure(self.materials.contains_key(&id), "floor material missing")?;
            }
        }
        dimensions::validate(self)?;
        room_tags::validate(self)?;
        opening_tags::validate(self)?;
        let mut room_numbers = BTreeSet::new();
        for room in self.rooms.values() {
            room.parameters.validate()?;
            for material in [
                room.parameters.floor_material,
                room.parameters.wall_material,
                room.parameters.ceiling_material,
            ]
            .into_iter()
            .flatten()
            {
                ensure(
                    self.materials.contains_key(&material),
                    "room material missing",
                )?;
            }
            ensure(
                self.levels.contains_key(&room.parameters.level),
                "room level missing",
            )?;
            ensure(
                room_numbers.insert((
                    room.parameters.level,
                    room.parameters.number.trim().to_ascii_lowercase(),
                )),
                "room number must be unique on its level",
            )?;
        }
        for e in self.materials.values() {
            ensure(
                e.parameters.density_kg_m3.is_finite() && e.parameters.density_kg_m3 > 0.0,
                "invalid material density",
            )?;
        }
        for e in self.views.values() {
            e.parameters.validate()?;
            if let Some(phase) = e.parameters.plan.and_then(|plan| plan.target_phase) {
                ensure(
                    self.phases.contains_key(&phase),
                    "plan target phase missing",
                )?;
            }
            if let Some(id) = e.parameters.level {
                ensure(self.levels.contains_key(&id), "view level missing")?;
                if let Some(settings) = e.parameters.plan {
                    settings.range.at_level_for_view(
                        settings.view_type,
                        self.levels[&id].parameters.elevation,
                    )?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_corrupt_graphs_and_retains_identity() {
        let mut model = Model::new("Test");
        model.validate().unwrap();
        let encoded = serde_json::to_string(&model).unwrap();
        assert_eq!(model, serde_json::from_str::<Model>(&encoded).unwrap());
        model
            .levels
            .values_mut()
            .next()
            .unwrap()
            .parameters
            .building = Id::new();
        assert!(model.validate().is_err());
    }
    #[test]
    fn validates_wall_and_preserves_direction() {
        let wall = WallParams {
            name: "W".into(),
            path: crate::WallPath::Straight {
                start: Point2::new(1.0, 2.0),
                end: Point2::new(4.0, 6.0),
            },
            thickness: 0.2,
            height: 3.0,
            level: Id::new(),
            material: None,
        };
        let resized = wall.with_length(10.0).unwrap();
        assert_eq!(resized.end(), Point2::new(7.0, 10.0));
        for value in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(wall.with_length(value).is_err());
        }
        let mut bad = wall.clone();
        *bad.path.straight_end_mut().unwrap() = bad.start();
        assert!(bad.validate().is_err());
        bad = wall;
        bad.thickness = -0.2;
        assert!(bad.validate().is_err());
    }
}
