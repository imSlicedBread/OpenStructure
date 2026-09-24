//! Native rectangular apertures, measured along their straight host wall.
use crate::{Entity, Model, WallParams};
use os_core::{Id, Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpeningKind {
    Door,
    Window,
}

/// Jamb relative to the host's stored start-to-end direction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DoorHinge {
    #[default]
    Start,
    End,
}

/// Left is the positive host-local normal. Reversing the host reverses this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DoorSwing {
    #[default]
    Left,
    Right,
}

/// Pane alignment in the host's stored start-to-end frame; left is +local y.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum WindowPanePosition {
    #[default]
    Center,
    LeftFace,
    RightFace,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpeningTypeParams {
    pub family: crate::OpeningFamily,
    pub name: String,
    pub kind: OpeningKind,
    pub width: f64,
    pub height: f64,
    /// Above host level; doors always have a zero sill.
    pub sill: f64,
    pub pane_position: WindowPanePosition,
}
pub type OpeningType = Entity<OpeningTypeParams>;

/// Typed instances persist only a reference, never cached type dimensions.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum OpeningDefinition {
    Legacy {
        kind: OpeningKind,
        width: f64,
        height: f64,
        sill: f64,
    },
    Typed {
        type_id: Id,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpeningParams {
    pub name: String,
    pub host: Id,
    /// Distance to the first jamb from the host start, in metres.
    pub offset: f64,
    pub definition: OpeningDefinition,
    pub hinge: DoorHinge,
    pub swing: DoorSwing,
}
pub type Opening = Entity<OpeningParams>;

/// Ephemeral effective dimensions resolved from the current project model.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedOpening {
    pub family: crate::OpeningFamily,
    pub name: String,
    pub host: Id,
    pub offset: f64,
    pub kind: OpeningKind,
    pub width: f64,
    pub height: f64,
    pub sill: f64,
    pub pane_position: WindowPanePosition,
    pub type_id: Option<Id>,
    pub type_name: Option<String>,
    pub hinge: DoorHinge,
    pub swing: DoorSwing,
}

fn validate_name(name: &str) -> Result<()> {
    ensure(
        !name.trim().is_empty() && name.len() <= 256 && !name.chars().any(char::is_control),
        "opening/type name must be 1-256 bytes without control characters",
    )
}

impl OpeningTypeParams {
    pub fn validate(&self) -> Result<()> {
        validate_name(&self.name)?;
        ensure(
            self.kind == OpeningKind::Window || self.pane_position == WindowPanePosition::Center,
            "doors require Center pane position",
        )?;
        validate_dimensions(self.kind, self.width, self.height, self.sill)?;
        self.family.validate_for(self.width, self.height, self.kind)
    }
}

impl Model {
    /// Resolve and validate dimensions without requiring a placed host. Host fit
    /// is checked by `OpeningParams::validate` and whole-model validation.
    pub fn resolve_opening(&self, opening: &OpeningParams) -> Result<ResolvedOpening> {
        let resolved = match opening.definition {
            OpeningDefinition::Legacy {
                kind,
                width,
                height,
                sill,
            } => ResolvedOpening {
                family: Default::default(),
                name: opening.name.clone(),
                host: opening.host,
                offset: opening.offset,
                kind,
                width,
                height,
                sill,
                type_id: None,
                pane_position: WindowPanePosition::Center,
                type_name: None,
                hinge: opening.hinge,
                swing: opening.swing,
            },
            OpeningDefinition::Typed { type_id } => {
                let ty = self
                    .opening_types
                    .get(&type_id)
                    .ok_or_else(|| os_core::Error::Invalid("opening type missing".into()))?;
                ty.parameters.validate()?;
                let p = &ty.parameters;
                ResolvedOpening {
                    family: p.family.clone(),
                    name: opening.name.clone(),
                    host: opening.host,
                    offset: opening.offset,
                    kind: p.kind,
                    width: p.width,
                    height: p.height,
                    sill: p.sill,
                    type_id: Some(type_id),
                    pane_position: p.pane_position,
                    type_name: Some(p.name.clone()),
                    hinge: opening.hinge,
                    swing: opening.swing,
                }
            }
        };
        validate_dimensions(
            resolved.kind,
            resolved.width,
            resolved.height,
            resolved.sill,
        )?;
        ensure(
            resolved.kind == OpeningKind::Door
                || (resolved.hinge == DoorHinge::Start && resolved.swing == DoorSwing::Left),
            "windows require default Start/Left orientation",
        )?;
        Ok(resolved)
    }
}

impl OpeningParams {
    pub fn type_id(&self) -> Option<Id> {
        match self.definition {
            OpeningDefinition::Typed { type_id } => Some(type_id),
            OpeningDefinition::Legacy { .. } => None,
        }
    }

    pub fn validate(&self, model: &Model) -> Result<()> {
        validate_name(&self.name)?;
        let host = model.resolve_wall(self.host)?;
        model.resolve_opening(self)?.validate_host(&host.parameters)
    }
}

fn validate_dimensions(kind: OpeningKind, width: f64, height: f64, sill: f64) -> Result<()> {
    ensure(
        [width, height, sill].iter().all(|v| v.is_finite()),
        "opening dimensions must be finite",
    )?;
    // A millimetre minimum also keeps segmented kernel geometry nondegenerate.
    ensure(
        width >= 0.001 && height >= 0.001 && sill >= 0.0,
        "opening needs dimensions of at least 1 mm and a nonnegative sill",
    )?;
    ensure(
        sill == 0.0 || sill >= 0.001,
        "sill must be zero or at least 1 mm",
    )?;
    ensure(
        kind != OpeningKind::Door || sill == 0.0,
        "doors must start at the host floor",
    )
}

impl ResolvedOpening {
    pub fn validate_host(&self, host: &WallParams) -> Result<()> {
        host.validate()?;
        self.family
            .validate_for(self.width, self.height, self.kind)?;
        validate_dimensions(self.kind, self.width, self.height, self.sill)?;
        let offset = self.offset;
        ensure(
            offset.is_finite() && offset >= 0.001,
            "opening needs finite offset and at least 1 mm end clearance",
        )?;
        ensure(
            host.thickness >= 0.00001,
            "host wall is too thin for a door or window panel",
        )?;
        ensure(
            offset + self.width <= host.length() - 0.001
                && self.sill + self.height <= host.height - 0.001,
            "opening must fit inside host with at least 1 mm end/head clearance",
        )
    }
}

pub(crate) fn validate(model: &Model) -> Result<()> {
    for ty in model.opening_types.values() {
        ty.parameters.validate()?;
    }
    let mut hosts = std::collections::BTreeMap::<Id, Vec<(f64, f64)>>::new();
    for opening in model.openings.values() {
        let p = &opening.parameters;
        p.validate(model)?;
        hosts
            .entry(p.host)
            .or_default()
            .push((p.offset, model.resolve_opening(p)?.width));
    }
    for openings in hosts.values_mut() {
        openings.sort_by(|a, b| a.0.total_cmp(&b.0));
        for pair in openings.windows(2) {
            ensure(
                pair[0].0 + pair[0].1 + 0.001 <= pair[1].0,
                "openings require at least 1 mm separation along host; stacked openings are not supported",
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Wall;
    use os_core::Point2;

    fn fixture() -> (Model, OpeningParams, Id) {
        let mut model = Model::new("Openings");
        let wall = Wall::new(
            "org.openstructure.walls.wall",
            WallParams {
                name: "Host".into(),
                start: Point2::new(0.0, 0.0),
                end: Point2::new(10.0, 0.0),
                thickness: 0.2,
                height: 3.0,
                level: *model.levels.keys().next().unwrap(),
                material: None,
            },
        );
        let p = OpeningParams {
            hinge: Default::default(),
            swing: Default::default(),
            name: "Door 1".into(),
            host: wall.id(),
            offset: 1.0,
            definition: OpeningDefinition::Legacy {
                kind: OpeningKind::Door,
                width: 0.9,
                height: 2.1,
                sill: 0.0,
            },
        };
        let ty = OpeningType::new(
            "core.opening_type",
            OpeningTypeParams {
                family: Default::default(),
                name: "Standard door".into(),
                pane_position: Default::default(),
                kind: OpeningKind::Door,
                width: 0.9,
                height: 2.1,
                sill: 0.0,
            },
        );
        let id = ty.id();
        model.walls.insert(wall.id(), wall);
        model.opening_types.insert(id, ty);
        (model, p, id)
    }

    #[test]
    fn orientation_is_per_instance_for_typed_and_legacy_doors_and_invalid_for_windows() {
        let (mut model, mut p, ty) = fixture();
        let original_type = model.opening_types[&ty].clone();
        for typed in [false, true] {
            if typed {
                p.definition = OpeningDefinition::Typed { type_id: ty };
            }
            for hinge in [DoorHinge::Start, DoorHinge::End] {
                for swing in [DoorSwing::Left, DoorSwing::Right] {
                    p.hinge = hinge;
                    p.swing = swing;
                    p.validate(&model).unwrap();
                    let resolved = model.resolve_opening(&p).unwrap();
                    assert_eq!((resolved.hinge, resolved.swing), (hinge, swing));
                    assert_eq!(model.opening_types[&ty], original_type);
                    assert_eq!(
                        serde_json::from_value::<OpeningParams>(serde_json::to_value(&p).unwrap())
                            .unwrap(),
                        p
                    );
                }
            }
        }
        for typed in [false, true] {
            model.opening_types.get_mut(&ty).unwrap().parameters.kind = OpeningKind::Window;
            p.definition = if typed {
                OpeningDefinition::Typed { type_id: ty }
            } else {
                OpeningDefinition::Legacy {
                    kind: OpeningKind::Window,
                    width: 0.9,
                    height: 2.1,
                    sill: 0.0,
                }
            };
            for hinge in [DoorHinge::Start, DoorHinge::End] {
                for swing in [DoorSwing::Left, DoorSwing::Right] {
                    p.hinge = hinge;
                    p.swing = swing;
                    assert_eq!(
                        p.validate(&model).is_ok(),
                        hinge == DoorHinge::Start && swing == DoorSwing::Left
                    );
                }
            }
        }
    }

    #[test]
    fn legacy_and_typed_resolution_have_same_geometry_without_persistent_copies() {
        let (model, mut p, ty) = fixture();
        p.validate(&model).unwrap();
        let legacy = model.resolve_opening(&p).unwrap();
        assert_eq!(legacy.type_id, None);
        p.definition = OpeningDefinition::Typed { type_id: ty };
        p.validate(&model).unwrap();
        let typed = model.resolve_opening(&p).unwrap();
        assert_eq!(
            (
                typed.kind,
                typed.width,
                typed.height,
                typed.sill,
                typed.offset,
                typed.host
            ),
            (
                legacy.kind,
                legacy.width,
                legacy.height,
                legacy.sill,
                legacy.offset,
                legacy.host
            )
        );
        assert_eq!(typed.name, "Door 1");
        assert_eq!(typed.type_id, Some(ty));
        assert_eq!(typed.type_name.as_deref(), Some("Standard door"));
        let value = serde_json::to_value(&p).unwrap();
        assert_eq!(
            value["definition"],
            serde_json::json!({"Typed": {"type_id": ty}})
        );
        for field in ["kind", "width", "height", "sill"] {
            assert!(value.get(field).is_none());
            let mut bad = value.clone();
            bad["definition"]["Typed"][field] = 1.into();
            assert!(serde_json::from_value::<OpeningParams>(bad).is_err());
        }
        p.definition = OpeningDefinition::Typed { type_id: Id::new() };
        assert!(model.resolve_opening(&p).is_err());
    }

    #[test]
    fn types_and_legacy_share_dimension_rules_and_host_validation() {
        let (mut model, mut p, ty) = fixture();
        let valid = model.opening_types[&ty].parameters.clone();
        for case in 0..10 {
            let mut bad = valid.clone();
            match case {
                0 => bad.width = f64::NAN,
                1 => bad.height = f64::INFINITY,
                2 => bad.sill = f64::NEG_INFINITY,
                3 => bad.width = 0.0009,
                4 => bad.height = -1.0,
                5 => bad.sill = 0.0005,
                6 => bad.sill = -1.0,
                7 => bad.sill = 0.1,
                8 => bad.name = "\n".into(),
                _ => bad.name = "x".repeat(257),
            }
            assert!(bad.validate().is_err());
            model.opening_types.get_mut(&ty).unwrap().parameters = bad.clone();
            assert!(model.validate().is_err()); // unused types are validated too
            if case < 8 {
                p.definition = OpeningDefinition::Legacy {
                    kind: bad.kind,
                    width: bad.width,
                    height: bad.height,
                    sill: bad.sill,
                };
                assert!(p.validate(&model).is_err());
            }
        }
        model.opening_types.get_mut(&ty).unwrap().parameters = valid;
        p.definition = OpeningDefinition::Typed { type_id: ty };
        for offset in [f64::NAN, -1.0, 0.0, 9.5] {
            p.offset = offset;
            assert!(p.validate(&model).is_err());
        }
        p.offset = 1.0;
        let window = &mut model.opening_types.get_mut(&ty).unwrap().parameters;
        window.kind = OpeningKind::Window;
        window.height = 1.2;
        window.sill = 0.9;
        p.validate(&model).unwrap();
        model.opening_types.get_mut(&ty).unwrap().parameters.sill = 2.0;
        assert!(p.validate(&model).is_err());
    }

    #[test]
    fn type_identity_is_global_non_nil_and_correctly_typed() {
        let (model, _, ty) = fixture();
        for case in 0..4 {
            let mut bad = model.clone();
            let mut entity = bad.opening_types.remove(&ty).unwrap();
            match case {
                0 => entity.header.id = model.project.id(),
                1 => entity.header.id = Id("00000000-0000-0000-0000-000000000000".parse().unwrap()),
                2 => entity.header.type_id = "core.opening".into(),
                _ => entity.header.schema_version = 7,
            }
            bad.opening_types.insert(entity.id(), entity);
            assert!(bad.validate().is_err());
        }
    }
}
