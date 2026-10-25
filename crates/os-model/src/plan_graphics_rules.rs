//! Ordered, live predicates for native floor-plan strokes. Match IDs are never saved.
use crate::{
    Model, OpeningKind, PhaseStatus, PlanElementGraphicsOverride, PlanStrokeStyle, PlanViewType,
    ViewKind,
};
use os_core::{Id, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_PLAN_GRAPHICS_RULES_PER_VIEW: usize = 32;
pub const MAX_PLAN_GRAPHICS_RULES_PER_PROJECT: usize = 256;
pub const MAX_PLAN_GRAPHICS_PREDICATES: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanRuleCategory {
    Walls,
    Doors,
    Windows,
    Slabs,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanRuleTextField {
    InstanceName,
    TypeName,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanRuleNumberField {
    WallLength,
    WallHeight,
    WallThickness,
    OpeningWidth,
    OpeningHeight,
    OpeningSill,
    SlabThickness,
    SlabTopOffset,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanRuleTextOp {
    Equals,
    NotEquals,
    Contains,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanRuleNumberOp {
    Equals,
    NotEquals,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanRulePhaseOp {
    Equals,
    NotEquals,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum PlanGraphicsPredicate {
    Text {
        field: PlanRuleTextField,
        op: PlanRuleTextOp,
        value: String,
    },
    Number {
        field: PlanRuleNumberField,
        op: PlanRuleNumberOp,
        value: f64,
    },
    Phase {
        op: PlanRulePhaseOp,
        value: PhaseStatus,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanGraphicsRule {
    pub id: Id,
    pub name: String,
    pub enabled: bool,
    pub category: PlanRuleCategory,
    #[serde(deserialize_with = "deserialize_predicates")]
    pub predicates: Vec<PlanGraphicsPredicate>,
    pub cut: Option<PlanStrokeStyle>,
    pub projected: Option<PlanStrokeStyle>,
}

fn valid_text(text: &str) -> bool {
    text.len() <= 256 && !text.chars().any(char::is_control)
}

impl PlanRuleNumberField {
    pub fn category_compatible(self, category: PlanRuleCategory) -> bool {
        use PlanRuleCategory::*;
        use PlanRuleNumberField::*;
        matches!(
            (self, category),
            (WallLength | WallHeight | WallThickness, Walls)
                | (OpeningWidth | OpeningHeight | OpeningSill, Doors | Windows)
                | (SlabThickness | SlabTopOffset, Slabs)
        )
    }
}

impl PlanGraphicsRule {
    pub fn validate(&self) -> Result<()> {
        ensure(!self.id.0.is_nil(), "nil graphics rule ID")?;
        ensure(
            !self.name.trim().is_empty() && valid_text(&self.name),
            "rule name must be 1-256 bytes without controls",
        )?;
        ensure(
            (1..=MAX_PLAN_GRAPHICS_PREDICATES).contains(&self.predicates.len()),
            "rule needs 1-8 AND predicates",
        )?;
        PlanElementGraphicsOverride {
            cut: self.cut,
            projected: self.projected,
        }
        .validate()?;
        ensure(
            self.category != PlanRuleCategory::Slabs || self.cut.is_none(),
            "slab rules only support projected strokes",
        )?;
        for predicate in &self.predicates {
            match predicate {
                PlanGraphicsPredicate::Text { field, value, .. } => {
                    ensure(
                        valid_text(value),
                        "rule text must be at most 256 bytes without controls",
                    )?;
                    ensure(
                        *field != PlanRuleTextField::TypeName
                            || self.category != PlanRuleCategory::Slabs,
                        "slabs do not have effective type names",
                    )?;
                }
                PlanGraphicsPredicate::Number { field, value, .. } => {
                    ensure(
                        value.is_finite(),
                        "rule numeric operand must be finite metres",
                    )?;
                    ensure(
                        field.category_compatible(self.category),
                        "rule numeric field is incompatible with category",
                    )?;
                }
                PlanGraphicsPredicate::Phase { .. } => {}
            }
        }
        Ok(())
    }
}

fn bounded_sequence<'de, D, T, const N: usize>(
    deserializer: D,
) -> std::result::Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct Bounded<T, const N: usize>(std::marker::PhantomData<T>);
    impl<'de, T: Deserialize<'de>, const N: usize> serde::de::Visitor<'de> for Bounded<T, N> {
        type Value = Vec<T>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(f, "1-{N} entries")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut seq: A,
        ) -> std::result::Result<Self::Value, A::Error> {
            use serde::de::Error;
            let mut values = Vec::new();
            while let Some(value) = seq.next_element()? {
                if values.len() == N {
                    return Err(A::Error::custom("graphics rule sequence limit exceeded"));
                }
                values.push(value);
            }
            if values.is_empty() {
                return Err(A::Error::custom("empty graphics rule sequence"));
            }
            Ok(values)
        }
    }
    deserializer.deserialize_seq(Bounded::<T, N>(std::marker::PhantomData))
}

fn deserialize_predicates<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<Vec<PlanGraphicsPredicate>, D::Error> {
    bounded_sequence::<D, PlanGraphicsPredicate, MAX_PLAN_GRAPHICS_PREDICATES>(d)
}

pub(crate) fn deserialize<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<BTreeMap<Id, Vec<PlanGraphicsRule>>, D::Error> {
    use serde::de::{Error, MapAccess, Visitor};
    #[derive(Deserialize)]
    struct Rules(#[serde(deserialize_with = "rule_list")] Vec<PlanGraphicsRule>);
    fn rule_list<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<Vec<PlanGraphicsRule>, D::Error> {
        bounded_sequence::<D, PlanGraphicsRule, MAX_PLAN_GRAPHICS_RULES_PER_VIEW>(d)
    }
    struct Maps;
    impl<'de> Visitor<'de> for Maps {
        type Value = BTreeMap<Id, Vec<PlanGraphicsRule>>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("unique floor plan IDs and bounded ordered rules")
        }
        fn visit_map<A: MapAccess<'de>>(
            self,
            mut map: A,
        ) -> std::result::Result<Self::Value, A::Error> {
            let mut result = BTreeMap::new();
            let mut ids = BTreeSet::new();
            while let Some((view, Rules(rules))) = map.next_entry::<Id, Rules>()? {
                for rule in &rules {
                    rule.validate().map_err(A::Error::custom)?;
                    if !ids.insert(rule.id) || ids.len() > MAX_PLAN_GRAPHICS_RULES_PER_PROJECT {
                        return Err(A::Error::custom("duplicate or excessive graphics rule IDs"));
                    }
                }
                if view.0.is_nil() || result.insert(view, rules).is_some() {
                    return Err(A::Error::custom("nil or duplicate graphics rule view"));
                }
            }
            Ok(result)
        }
    }
    d.deserialize_map(Maps)
}

struct Facts {
    category: PlanRuleCategory,
    name: String,
    type_name: Option<String>,
    numbers: Vec<(PlanRuleNumberField, f64)>,
    phase: PhaseStatus,
}

impl PlanGraphicsPredicate {
    fn matches(&self, facts: &Facts) -> bool {
        match self {
            Self::Text { field, op, value } => {
                let actual = match field {
                    PlanRuleTextField::InstanceName => Some(facts.name.as_str()),
                    PlanRuleTextField::TypeName => facts.type_name.as_deref(),
                };
                actual.is_some_and(|actual| match op {
                    PlanRuleTextOp::Equals => actual == value,
                    PlanRuleTextOp::NotEquals => actual != value,
                    PlanRuleTextOp::Contains => actual.contains(value.as_str()),
                })
            }
            Self::Number { field, op, value } => facts
                .numbers
                .iter()
                .find(|(f, _)| f == field)
                .is_some_and(|(_, actual)| match op {
                    PlanRuleNumberOp::Equals => actual == value,
                    PlanRuleNumberOp::NotEquals => actual != value,
                    PlanRuleNumberOp::Less => actual < value,
                    PlanRuleNumberOp::LessOrEqual => actual <= value,
                    PlanRuleNumberOp::Greater => actual > value,
                    PlanRuleNumberOp::GreaterOrEqual => actual >= value,
                }),
            Self::Phase { op, value } => match op {
                PlanRulePhaseOp::Equals => facts.phase == *value,
                PlanRulePhaseOp::NotEquals => facts.phase != *value,
            },
        }
    }
}

impl Model {
    pub fn plan_graphics_rules_compatible(&self, view: Id) -> bool {
        self.views.get(&view).is_some_and(|v| {
            v.parameters.kind == ViewKind::Plan
                && v.parameters
                    .plan
                    .is_some_and(|p| p.view_type == PlanViewType::FloorPlan)
        })
    }

    pub fn validate_plan_graphics_rules_replacement(
        &self,
        view: Id,
        rules: &[PlanGraphicsRule],
    ) -> Result<()> {
        ensure(
            self.plan_graphics_rules_compatible(view),
            "graphics rules require an authored floor plan",
        )?;
        ensure(
            rules.len() <= MAX_PLAN_GRAPHICS_RULES_PER_VIEW,
            "at most 32 graphics rules per view",
        )?;
        let mut ids = BTreeSet::new();
        let mut count = 0;
        for other in self
            .plan_graphics_rules
            .iter()
            // Owner cleanup runs after explicit replacements are validated.
            // Count only owners that survive in the final transaction graph.
            .filter(|(id, _)| **id != view && self.plan_graphics_rules_compatible(**id))
            .flat_map(|(_, rules)| rules)
        {
            ensure(ids.insert(other.id), "duplicate graphics rule ID")?;
            count += 1;
        }
        ensure(
            count + rules.len() <= MAX_PLAN_GRAPHICS_RULES_PER_PROJECT,
            "at most 256 graphics rules per project",
        )?;
        for rule in rules {
            rule.validate()?;
            ensure(ids.insert(rule.id), "duplicate graphics rule ID")?;
        }
        Ok(())
    }

    pub(crate) fn validate_plan_graphics_rules(&self) -> Result<()> {
        for (view, rules) in &self.plan_graphics_rules {
            ensure(!rules.is_empty(), "omit empty rule entries")?;
            self.validate_plan_graphics_rules_replacement(*view, rules)?;
        }
        Ok(())
    }

    pub fn copied_plan_graphics_rules(&self, view: Id) -> Vec<PlanGraphicsRule> {
        self.plan_graphics_rules
            .get(&view)
            .into_iter()
            .flatten()
            .map(|rule| {
                let mut copy = rule.clone();
                copy.id = Id::new();
                copy
            })
            .collect()
    }

    /// At most 10000 candidates × 32 rules × 8 predicates. Resolve dimensions
    /// once per candidate, not once per predicate. Geometry visibility is applied
    /// later by the shared drawing builder; this map cannot create geometry.
    pub fn resolve_plan_graphics_rules(
        &self,
        view: Id,
    ) -> Result<BTreeMap<Id, PlanElementGraphicsOverride>> {
        let mut resolved = BTreeMap::new();
        let Some(rules) = self.plan_graphics_rules.get(&view) else {
            return Ok(resolved);
        };
        self.validate_plan_graphics_rules_replacement(view, rules)?;
        ensure(
            self.walls
                .len()
                .saturating_add(self.openings.len())
                .saturating_add(self.floors.len())
                <= 10_000,
            "rule evaluation exceeds 10000 native elements",
        )?;
        let settings = self.views[&view]
            .parameters
            .plan
            .expect("validated floor plan");
        let target = settings
            .target_phase
            .or_else(|| self.latest_phase())
            .ok_or_else(|| os_core::Error::Invalid("rule phase missing".into()))?;
        let active: Vec<_> = rules.iter().filter(|r| r.enabled).collect();
        for id in self
            .walls
            .keys()
            .chain(self.openings.keys())
            .chain(self.floors.keys())
            .copied()
        {
            if !self.plan_element_graphics_compatible(view, id) {
                continue;
            }
            use PlanRuleNumberField::*;
            let phase = self.phase_status(id, target)?;
            let facts = if self.walls.contains_key(&id) {
                if !active.iter().any(|r| r.category == PlanRuleCategory::Walls) {
                    continue;
                }
                let p = self.resolve_wall(id)?.parameters;
                Facts {
                    category: PlanRuleCategory::Walls,
                    numbers: vec![
                        (WallLength, p.length()),
                        (WallHeight, p.height),
                        (WallThickness, p.thickness),
                    ],
                    name: p.name,
                    type_name: self
                        .wall_type_assignments
                        .get(&id)
                        .and_then(|a| self.wall_types.get(&a.type_id))
                        .map(|t| t.parameters.name.clone()),
                    phase,
                }
            } else if let Some(opening) = self.openings.get(&id) {
                let p = self.resolve_opening(&opening.parameters)?;
                Facts {
                    category: match p.kind {
                        OpeningKind::Door => PlanRuleCategory::Doors,
                        OpeningKind::Window => PlanRuleCategory::Windows,
                    },
                    name: p.name,
                    type_name: p.type_name,
                    numbers: vec![
                        (OpeningWidth, p.width),
                        (OpeningHeight, p.height),
                        (OpeningSill, p.sill),
                    ],
                    phase,
                }
            } else {
                let p = &self.floors[&id].parameters;
                Facts {
                    category: PlanRuleCategory::Slabs,
                    name: p.name.clone(),
                    type_name: None,
                    numbers: vec![(SlabThickness, p.thickness), (SlabTopOffset, p.top_offset)],
                    phase,
                }
            };
            let mut strokes = PlanElementGraphicsOverride {
                cut: None,
                projected: None,
            };
            for rule in &active {
                if rule.category == facts.category
                    && rule.predicates.iter().all(|p| p.matches(&facts))
                {
                    strokes.cut = strokes.cut.or(rule.cut);
                    strokes.projected = strokes.projected.or(rule.projected);
                    if strokes.cut.is_some() && strokes.projected.is_some() {
                        break;
                    }
                }
            }
            if strokes.cut.is_some() || strokes.projected.is_some() {
                resolved.insert(id, strokes);
            }
        }
        Ok(resolved)
    }
}
