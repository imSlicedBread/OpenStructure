//! Native apertures, measured in analytic host-path stations.
use crate::{Entity, Model, WallParams};
use os_core::{Id, Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpeningKind {
    Door,
    Window,
}

/// Type-level plan symbol and static closed sash representation for rectangular
/// windows on straight hosts. Curved/custom-profile windows use the symbol only.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum WindowOperation {
    #[default]
    Fixed,
    Sliding,
    Casement,
}

/// Persisted instance pose. Default preserves historical doors at 90 degrees
/// and windows closed. Unsupported hosts/profiles retain this value dormant.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum OpeningState {
    #[default]
    Default,
    DoorAngle(f64),
    DoorPairAngles {
        active_degrees: f64,
        inactive_degrees: f64,
    },
    SlidingFraction(f64),
    CasementAngle(f64),
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
    pub window_operation: WindowOperation,
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
    pub open_state: OpeningState,
    pub name: String,
    pub host: Id,
    /// Centerline travel to the first jamb from the host start, in metres.
    pub offset: f64,
    pub definition: OpeningDefinition,
    /// Explicit null inherits; Some pins this instance, including equal defaults.
    #[serde(deserialize_with = "required_sill_override")]
    pub width_override: Option<f64>,
    #[serde(deserialize_with = "required_sill_override")]
    pub height_override: Option<f64>,
    /// Typed windows may pin their sill independently of the type default.
    /// Explicit null means inherit; missing fields require schema migration.
    #[serde(deserialize_with = "required_sill_override")]
    pub sill_override: Option<f64>,
    /// Typed windows only. Explicit null inherits; missing requires migration.
    #[serde(deserialize_with = "required_pane_position_override")]
    pub pane_position_override: Option<WindowPanePosition>,
    /// Typed two-bay families only. Null inherits; Some pins even equal defaults.
    #[serde(deserialize_with = "required_lite_side_override")]
    pub lite_side_override: Option<crate::LiteSide>,
    pub hinge: DoorHinge,
    pub swing: DoorSwing,
}
pub type Opening = Entity<OpeningParams>;

fn required_lite_side_override<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<crate::LiteSide>, D::Error> {
    Option::<crate::LiteSide>::deserialize(deserializer)
}

fn required_pane_position_override<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<WindowPanePosition>, D::Error> {
    Option::<WindowPanePosition>::deserialize(deserializer)
}

fn required_sill_override<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<f64>, D::Error> {
    Option::<f64>::deserialize(deserializer)
}

/// Ephemeral effective dimensions resolved from the current project model.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedOpening {
    pub open_state: OpeningState,
    pub window_operation: WindowOperation,
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
            self.kind == OpeningKind::Window || self.window_operation == WindowOperation::Fixed,
            "doors require Fixed window operation",
        )?;
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
        ensure(
            (opening.width_override.is_none() && opening.height_override.is_none())
                || matches!(opening.definition, OpeningDefinition::Typed { .. }),
            "legacy openings cannot override dimensions",
        )?;
        ensure(
            opening.sill_override.is_none()
                || matches!(opening.definition, OpeningDefinition::Typed { .. }),
            "legacy openings cannot override sill",
        )?;
        let mut resolved = match opening.definition {
            OpeningDefinition::Legacy {
                kind,
                width,
                height,
                sill,
            } => ResolvedOpening {
                open_state: opening.open_state,
                window_operation: WindowOperation::Fixed,
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
                let ty = self.resolve_opening_type(type_id)?;
                let p = &ty.parameters;
                ResolvedOpening {
                    open_state: opening.open_state,
                    window_operation: p.window_operation,
                    family: p.family.clone(),
                    name: opening.name.clone(),
                    host: opening.host,
                    offset: opening.offset,
                    kind: p.kind,
                    width: opening.width_override.unwrap_or(p.width),
                    height: opening.height_override.unwrap_or(p.height),
                    sill: opening.sill_override.unwrap_or(p.sill),
                    type_id: Some(type_id),
                    pane_position: opening.pane_position_override.unwrap_or(p.pane_position),
                    type_name: Some(p.name.clone()),
                    hinge: opening.hinge,
                    swing: opening.swing,
                }
            }
        };
        if let Some(side) = opening.lite_side_override {
            ensure(
                resolved.type_id.is_some(),
                "legacy openings cannot override lite side",
            )?;
            let lite = resolved.family.side_lite.as_mut().ok_or_else(|| {
                os_core::Error::Invalid("only typed two-bay openings can override lite side".into())
            })?;
            lite.side = side;
        }
        ensure(
            opening.pane_position_override.is_none()
                || (resolved.kind == OpeningKind::Window && resolved.type_id.is_some()),
            "only typed windows can override pane position",
        )?;
        ensure(
            resolved.kind == OpeningKind::Window || opening.sill_override.is_none(),
            "doors cannot override sill",
        )?;
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
        resolved
            .family
            .validate_for(resolved.width, resolved.height, resolved.kind)?;
        resolved.validate_open_state()?;
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
    pub fn validate_open_state(&self) -> Result<()> {
        let valid = match self.open_state {
            OpeningState::Default => true,
            OpeningState::DoorPairAngles {
                active_degrees,
                inactive_degrees,
            } => {
                self.kind == OpeningKind::Door
                    && matches!(self.family.door_leaves, crate::DoorLeaves::Paired { .. })
                    && [active_degrees, inactive_degrees]
                        .iter()
                        .all(|v| v.is_finite() && (0. ..=90.).contains(v))
            }
            OpeningState::DoorAngle(v) => {
                self.kind == OpeningKind::Door && v.is_finite() && (0. ..=90.).contains(&v)
            }
            OpeningState::SlidingFraction(v) => {
                self.kind == OpeningKind::Window
                    && self.window_operation == WindowOperation::Sliding
                    && v.is_finite()
                    && (0. ..=1.).contains(&v)
            }
            OpeningState::CasementAngle(v) => {
                self.kind == OpeningKind::Window
                    && self.window_operation == WindowOperation::Casement
                    && v.is_finite()
                    && (0. ..=90.).contains(&v)
            }
        };
        ensure(
            valid,
            "opening state is out of range or incompatible with its kind/operation; reset paired poses to Default before switching to Single",
        )
    }

    /// Matches the rectangular sash evaluator, including equivalent windings.
    pub fn rectangular_component(&self) -> bool {
        self.family.rectangular_cut()
            && self
                .family
                .profile
                .iter()
                .all(|v| v.x == 0. || v.x == 1. || v.y == 0. || v.y == 1.)
            && crate::OpeningFamily::rectangle()
                .iter()
                .all(|v| self.family.profile.contains(v))
    }

    pub fn supports_open_state(&self, host: &WallParams) -> bool {
        host.path.is_straight()
            && self.rectangular_component()
            && (self.kind == OpeningKind::Door || self.window_operation != WindowOperation::Fixed)
    }

    pub fn door_angle(&self, host: &WallParams) -> f64 {
        if self.supports_open_state(host)
            && let OpeningState::DoorAngle(angle) = self.open_state
        {
            angle
        } else {
            90.
        }
    }

    pub fn validate_host(&self, host: &WallParams) -> Result<()> {
        self.validate_open_state()?;
        host.validate()?;
        ensure(
            self.family.door_leaves == crate::DoorLeaves::Single || host.path.is_straight(),
            "paired doors require a straight host; choose a straight wall or a Single door type",
        )?;
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
        )?;
        if let crate::WallPath::CircularArc { radius, .. } = host.path {
            // Arc components use the same authored depths/caps as straight hosts.
            // Check the *physical* inner-face clear widths too: centerline fit
            // alone can accept a vanishing pane on a very tight bend.
            let scale = (radius - host.thickness / 2.) / radius;
            let bays = self.family.bays(self.width)?;
            let clear = bays.map_or(self.width - 2. * self.family.frame_width, |b| {
                (b.primary.1 - b.primary.0).min(b.lite.1 - b.lite.0)
            });
            ensure(
                clear * scale >= 0.001,
                "arc leaves less than 1 mm physical clear width",
            )?;
            ensure(
                (self.family.frame_width == 0. || self.family.frame_width * scale >= 0.001)
                    && self
                        .family
                        .side_lite
                        .as_ref()
                        .is_none_or(|lite| lite.mullion_width * scale >= 0.001),
                "arc frame or mullion is narrower than 1 mm at the inner face",
            )?;
            if self.kind == OpeningKind::Door {
                // The displayed leaf is rigid in the tangent-at-hinge frame.
                // An inward leaf must not cross the circle centre or wrap onto
                // another part of its own host. Conservatively bound its reach
                // for both swing directions; never shorten an infeasible leaf.
                let leaf = bays.map_or(self.width, |b| b.primary.1 - b.primary.0);
                ensure(
                    leaf < radius - host.thickness / 2.,
                    "rigid door leaf is too long for the circular host radius",
                )?;
            }
        }
        Ok(())
    }
}

pub(crate) fn validate(model: &Model) -> Result<()> {
    for ty in model.opening_types.values() {
        model.resolve_opening_type(ty.id())?;
        for material in [
            ty.parameters.family.panel_material,
            ty.parameters.family.frame_material,
            ty.parameters
                .family
                .side_lite
                .as_ref()
                .and_then(|lite| lite.material),
        ]
        .into_iter()
        .flatten()
        {
            ensure(
                model.materials.contains_key(&material),
                "opening family material missing",
            )?;
        }
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
                path: crate::WallPath::Straight {
                    start: Point2::new(0.0, 0.0),
                    end: Point2::new(10.0, 0.0),
                },
                thickness: 0.2,
                height: 3.0,
                level: *model.levels.keys().next().unwrap(),
                material: None,
            },
        );
        let p = OpeningParams {
            open_state: Default::default(),
            width_override: None,
            height_override: None,
            sill_override: None,
            pane_position_override: None,
            lite_side_override: None,
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
                window_operation: Default::default(),
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
    fn opening_state_validation_and_required_wire() {
        let (mut model, mut p, ty) = fixture();
        for value in [f64::NAN, f64::INFINITY, -1., 91.] {
            p.open_state = OpeningState::DoorAngle(value);
            assert!(model.resolve_opening(&p).is_err());
        }
        for value in [0., 45., 90.] {
            p.open_state = OpeningState::DoorAngle(value);
            model.resolve_opening(&p).unwrap();
        }
        p.definition = OpeningDefinition::Typed { type_id: ty };
        let params = &mut model.opening_types.get_mut(&ty).unwrap().parameters;
        params.kind = OpeningKind::Window;
        params.sill = 0.7;
        params.window_operation = WindowOperation::Sliding;
        assert!(model.resolve_opening(&p).is_err());
        for value in [-0.1, 1.1, f64::NAN, f64::INFINITY] {
            p.open_state = OpeningState::SlidingFraction(value);
            assert!(model.resolve_opening(&p).is_err());
        }
        p.open_state = OpeningState::SlidingFraction(0.5);
        model.resolve_opening(&p).unwrap();
        model
            .opening_types
            .get_mut(&ty)
            .unwrap()
            .parameters
            .window_operation = WindowOperation::Fixed;
        assert!(model.resolve_opening(&p).is_err());
        p.open_state = OpeningState::Default;
        model.resolve_opening(&p).unwrap();
        let mut wire = serde_json::to_value(&p).unwrap();
        wire.as_object_mut().unwrap().remove("open_state");
        assert!(serde_json::from_value::<OpeningParams>(wire).is_err());
        assert!(serde_json::from_str::<OpeningState>(r#"{"DoorAngle":45,"extra":0}"#).is_err());
        assert!(serde_json::from_str::<OpeningState>(r#"{"Unknown":0}"#).is_err());
    }

    #[test]
    fn window_operation_is_required_type_metadata_and_legacy_is_fixed() {
        let (mut model, mut opening, ty) = fixture();
        assert_eq!(
            model.resolve_opening(&opening).unwrap().window_operation,
            WindowOperation::Fixed
        );
        opening.definition = OpeningDefinition::Typed { type_id: ty };
        for operation in [
            WindowOperation::Fixed,
            WindowOperation::Sliding,
            WindowOperation::Casement,
        ] {
            let params = &mut model.opening_types.get_mut(&ty).unwrap().parameters;
            params.window_operation = operation;
            assert_eq!(
                params.validate().is_ok(),
                operation == WindowOperation::Fixed
            );
            params.kind = OpeningKind::Window;
            params.validate().unwrap();
            assert_eq!(
                model.resolve_opening(&opening).unwrap().window_operation,
                operation
            );
            model.opening_types.get_mut(&ty).unwrap().parameters.kind = OpeningKind::Door;
        }
        let params = &mut model.opening_types.get_mut(&ty).unwrap().parameters;
        params.window_operation = WindowOperation::Fixed;
        for bad in [
            None,
            Some(serde_json::json!(null)),
            Some(serde_json::json!("Tilt")),
            Some(serde_json::json!(3)),
        ] {
            let mut value = serde_json::to_value(&params).unwrap();
            value.as_object_mut().unwrap().remove("window_operation");
            if let Some(bad) = bad {
                value["window_operation"] = bad;
            }
            assert!(serde_json::from_value::<OpeningTypeParams>(value).is_err());
        }
    }

    #[test]
    fn lite_side_override_is_required_typed_two_bay_and_pins_equal_defaults() {
        use crate::{
            LiteSide::{End, Start},
            SideLite,
        };
        let (mut model, mut p, ty) = fixture();
        let mut missing = serde_json::to_value(&p).unwrap();
        missing
            .as_object_mut()
            .unwrap()
            .remove("lite_side_override");
        assert!(serde_json::from_value::<OpeningParams>(missing).is_err());
        for bad in [
            serde_json::json!("Left"),
            serde_json::json!(0),
            serde_json::json!({}),
        ] {
            let mut value = serde_json::to_value(&p).unwrap();
            value["lite_side_override"] = bad;
            assert!(serde_json::from_value::<OpeningParams>(value).is_err());
        }
        for kind in [OpeningKind::Door, OpeningKind::Window] {
            p.definition = OpeningDefinition::Legacy {
                kind,
                width: 0.9,
                height: 2.1,
                sill: 0.0,
            };
            for side in [Start, End] {
                p.lite_side_override = Some(side);
                assert!(model.resolve_opening(&p).is_err());
            }
            p.definition = OpeningDefinition::Typed { type_id: ty };
            let defaults = &mut model.opening_types.get_mut(&ty).unwrap().parameters;
            defaults.kind = kind;
            defaults.family.side_lite = None;
            assert!(model.resolve_opening(&p).is_err());
            for default in [Start, End] {
                model
                    .opening_types
                    .get_mut(&ty)
                    .unwrap()
                    .parameters
                    .family
                    .side_lite = Some(SideLite {
                    side: default,
                    width_fraction: 0.25,
                    mullion_width: 0.04,
                    material: None,
                });
                p.lite_side_override = None;
                let inherited = model.resolve_opening(&p).unwrap();
                let mut pinned = p.clone();
                pinned.lite_side_override = Some(default);
                assert_eq!(model.resolve_opening(&pinned).unwrap(), inherited);
                let changed = if default == Start { End } else { Start };
                model
                    .opening_types
                    .get_mut(&ty)
                    .unwrap()
                    .parameters
                    .family
                    .side_lite
                    .as_mut()
                    .unwrap()
                    .side = changed;
                assert_eq!(
                    model
                        .resolve_opening(&p)
                        .unwrap()
                        .family
                        .side_lite
                        .unwrap()
                        .side,
                    changed
                );
                assert_eq!(
                    model
                        .resolve_opening(&pinned)
                        .unwrap()
                        .family
                        .side_lite
                        .unwrap()
                        .side,
                    default
                );
                assert_eq!(
                    serde_json::from_value::<OpeningParams>(serde_json::to_value(&pinned).unwrap())
                        .unwrap(),
                    pinned
                );
                pinned.width_override = Some(0.02);
                assert!(model.resolve_opening(&pinned).is_err());
            }
        }
    }

    #[test]
    fn pane_override_requires_explicit_field_and_distinguishes_inherited_from_pinned_equal() {
        let (mut model, mut p, ty) = fixture();
        let mut missing = serde_json::to_value(&p).unwrap();
        missing
            .as_object_mut()
            .unwrap()
            .remove("pane_position_override");
        assert!(serde_json::from_value::<OpeningParams>(missing).is_err());
        for bad in [
            serde_json::json!("Left"),
            serde_json::json!(0),
            serde_json::json!({}),
        ] {
            let mut value = serde_json::to_value(&p).unwrap();
            value["pane_position_override"] = bad;
            assert!(serde_json::from_value::<OpeningParams>(value).is_err());
        }
        for position in [
            WindowPanePosition::Center,
            WindowPanePosition::LeftFace,
            WindowPanePosition::RightFace,
        ] {
            p.pane_position_override = Some(position);
            assert!(model.resolve_opening(&p).is_err());
            let legacy = p.definition.clone();
            p.definition = OpeningDefinition::Typed { type_id: ty };
            assert!(model.resolve_opening(&p).is_err()); // typed door, including Center
            p.definition = legacy;
        }
        p.definition = OpeningDefinition::Typed { type_id: ty };
        model.opening_types.get_mut(&ty).unwrap().parameters.kind = OpeningKind::Window;
        for default in [
            WindowPanePosition::Center,
            WindowPanePosition::LeftFace,
            WindowPanePosition::RightFace,
        ] {
            model
                .opening_types
                .get_mut(&ty)
                .unwrap()
                .parameters
                .pane_position = default;
            p.pane_position_override = None;
            assert_eq!(model.resolve_opening(&p).unwrap().pane_position, default);
            let mut pinned = p.clone();
            pinned.pane_position_override = Some(default);
            assert_eq!(
                model.resolve_opening(&pinned).unwrap().pane_position,
                default
            );
            let changed = if default == WindowPanePosition::LeftFace {
                WindowPanePosition::RightFace
            } else {
                WindowPanePosition::LeftFace
            };
            model
                .opening_types
                .get_mut(&ty)
                .unwrap()
                .parameters
                .pane_position = changed;
            assert_eq!(model.resolve_opening(&p).unwrap().pane_position, changed);
            assert_eq!(
                model.resolve_opening(&pinned).unwrap().pane_position,
                default
            );
            pinned.validate(&model).unwrap();
            assert_eq!(
                serde_json::from_value::<OpeningParams>(serde_json::to_value(&pinned).unwrap())
                    .unwrap(),
                pinned
            );
        }
        p.definition = OpeningDefinition::Legacy {
            kind: OpeningKind::Window,
            width: 1.,
            height: 1.,
            sill: 0.5,
        };
        p.pane_position_override = Some(WindowPanePosition::Center);
        assert!(model.resolve_opening(&p).is_err());
    }

    #[test]
    fn opening_materials_require_existing_project_materials_even_on_unused_types() {
        let (mut model, _, ty) = fixture();
        let material = crate::Material::new(
            "core.material",
            crate::MaterialParams {
                name: "Timber".into(),
                density_kg_m3: 500.,
                color: [180, 180, 180],
            },
        );
        let id = material.id();
        model.materials.insert(id, material);
        for panel in [true, false] {
            let family = &mut model.opening_types.get_mut(&ty).unwrap().parameters.family;
            if panel {
                family.panel_material = Some(id);
            } else {
                family.frame_material = Some(id);
            }
            model.validate().unwrap();
            let mut missing = model.clone();
            missing.materials.clear();
            assert!(missing.validate().is_err());
        }
    }

    #[test]
    fn dimension_overrides_are_required_pin_defaults_and_validate_effective_family_and_host() {
        let (mut model, mut p, ty) = fixture();
        for field in ["width_override", "height_override"] {
            let mut value = serde_json::to_value(&p).unwrap();
            assert_eq!(value[field], serde_json::Value::Null);
            value.as_object_mut().unwrap().remove(field);
            assert!(serde_json::from_value::<OpeningParams>(value).is_err());
        }
        p.width_override = Some(0.9);
        assert!(model.resolve_opening(&p).is_err()); // legacy owns dimensions
        p.width_override = None;
        p.height_override = Some(2.1);
        assert!(model.resolve_opening(&p).is_err());
        p.definition = OpeningDefinition::Typed { type_id: ty };
        p.width_override = Some(0.9);
        model.opening_types.get_mut(&ty).unwrap().parameters.width = 1.2;
        model.opening_types.get_mut(&ty).unwrap().parameters.height = 2.3;
        let r = model.resolve_opening(&p).unwrap();
        assert_eq!((r.width, r.height, r.sill), (0.9, 2.1, 0.0));
        for value in [f64::NAN, f64::INFINITY, -1., 0., 0.0005] {
            let mut bad = p.clone();
            bad.width_override = Some(value);
            assert!(model.resolve_opening(&bad).is_err());
            bad = p.clone();
            bad.height_override = Some(value);
            assert!(model.resolve_opening(&bad).is_err());
        }
        for (width, height) in [(9.5, 2.1), (0.9, 3.0)] {
            let mut bad = p.clone();
            bad.width_override = Some(width);
            bad.height_override = Some(height);
            assert!(bad.validate(&model).is_err());
        }
        model
            .opening_types
            .get_mut(&ty)
            .unwrap()
            .parameters
            .family
            .frame_width = 0.2;
        p.width_override = Some(0.3);
        assert!(model.resolve_opening(&p).is_err());
        p.width_override = None;
        p.height_override = None;
        let r = model.resolve_opening(&p).unwrap();
        assert_eq!((r.width, r.height), (1.2, 2.3));
        model.opening_types.get_mut(&ty).unwrap().parameters.kind = OpeningKind::Window;
        p.sill_override = Some(0.5);
        p.width_override = Some(1.);
        p.height_override = Some(1.3);
        p.validate(&model).unwrap();
        let r = model.resolve_opening(&p).unwrap();
        assert_eq!((r.width, r.height, r.sill), (1., 1.3, 0.5));
    }

    #[test]
    fn sill_override_is_required_and_only_typed_windows_can_pin_it() {
        let (mut model, mut p, ty) = fixture();
        let serialized = serde_json::to_value(&p).unwrap();
        assert!(serialized["sill_override"].is_null());
        let mut missing = serialized.clone();
        missing.as_object_mut().unwrap().remove("sill_override");
        assert!(serde_json::from_value::<OpeningParams>(missing).is_err());
        assert_eq!(
            serde_json::from_value::<OpeningParams>(serialized).unwrap(),
            p
        );
        for definition in [
            p.definition.clone(),
            OpeningDefinition::Typed { type_id: ty },
        ] {
            p.definition = definition;
            for value in [0.0, 0.9] {
                p.sill_override = Some(value);
                assert!(model.resolve_opening(&p).is_err());
            }
        }
        let t = &mut model.opening_types.get_mut(&ty).unwrap().parameters;
        t.kind = OpeningKind::Window;
        t.height = 1.2;
        t.sill = 0.8;
        p.sill_override = None;
        assert_eq!(model.resolve_opening(&p).unwrap().sill, 0.8);
        for value in [0.0, 0.001, 0.8, 1.3] {
            p.sill_override = Some(value);
            p.validate(&model).unwrap();
            assert_eq!(model.resolve_opening(&p).unwrap().sill, value);
            model.opening_types.get_mut(&ty).unwrap().parameters.sill = 0.9;
            assert_eq!(model.resolve_opening(&p).unwrap().sill, value);
        }
        for value in [
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            -0.1,
            0.0005,
            1.8,
        ] {
            p.sill_override = Some(value);
            assert!(p.validate(&model).is_err());
        }
        p.sill_override = None;
        assert_eq!(model.resolve_opening(&p).unwrap().sill, 0.9);
        p.definition = OpeningDefinition::Legacy {
            kind: OpeningKind::Window,
            width: 0.9,
            height: 1.2,
            sill: 0.7,
        };
        assert_eq!(model.resolve_opening(&p).unwrap().sill, 0.7);
        p.sill_override = Some(0.7);
        assert!(model.resolve_opening(&p).is_err());
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
