//! Sparse, view-specific stroke overrides for native floor-plan elements.
use crate::{Model, PlanStrokeStyle, PlanViewType, ViewKind};
use os_core::{Id, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MAX_PLAN_ELEMENT_OVERRIDES_PER_VIEW: usize = 4096;
pub const MAX_PLAN_ELEMENT_OVERRIDES_PER_PROJECT: usize = 10_000;

/// A complete cut or projected stroke. An absent role inherits the current
/// phase/category appearance; it is never a cached copy of resolved graphics.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanElementGraphicsOverride {
    pub cut: Option<PlanStrokeStyle>,
    pub projected: Option<PlanStrokeStyle>,
}

impl PlanElementGraphicsOverride {
    pub fn validate(self) -> Result<()> {
        ensure(
            self.cut.is_some() || self.projected.is_some(),
            "omit empty plan element graphics overrides",
        )?;
        if let Some(style) = self.cut {
            style.validate()?;
        }
        if let Some(style) = self.projected {
            style.validate()?;
        }
        Ok(())
    }
}

/// Strictly deserialize both map levels so duplicate IDs and oversized
/// assignments cannot silently overwrite one another or consume unbounded work.
pub(crate) fn deserialize<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<BTreeMap<Id, BTreeMap<Id, PlanElementGraphicsOverride>>, D::Error> {
    use serde::de::{Error, MapAccess, Visitor};

    struct Overrides;
    impl<'de> Visitor<'de> for Overrides {
        type Value = BTreeMap<Id, BTreeMap<Id, PlanElementGraphicsOverride>>;

        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("unique plan view IDs with bounded unique element graphics overrides")
        }

        fn visit_map<A: MapAccess<'de>>(
            self,
            mut input: A,
        ) -> std::result::Result<Self::Value, A::Error> {
            let mut result = BTreeMap::new();
            let mut total = 0usize;
            while let Some(view) = input.next_key::<Id>()? {
                let elements = input.next_value_seed(ElementOverridesSeed)?;
                total = total.saturating_add(elements.len());
                if elements.is_empty()
                    || elements.len() > MAX_PLAN_ELEMENT_OVERRIDES_PER_VIEW
                    || total > MAX_PLAN_ELEMENT_OVERRIDES_PER_PROJECT
                    || result.insert(view, elements).is_some()
                {
                    return Err(A::Error::custom(
                        "invalid, duplicate or excessive plan element graphics overrides",
                    ));
                }
            }
            Ok(result)
        }
    }

    struct ElementOverridesSeed;
    impl<'de> serde::de::DeserializeSeed<'de> for ElementOverridesSeed {
        type Value = BTreeMap<Id, PlanElementGraphicsOverride>;

        fn deserialize<D: serde::Deserializer<'de>>(
            self,
            deserializer: D,
        ) -> std::result::Result<Self::Value, D::Error> {
            struct Elements;
            impl<'de> Visitor<'de> for Elements {
                type Value = BTreeMap<Id, PlanElementGraphicsOverride>;

                fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                    f.write_str("unique native element IDs and graphics override values")
                }

                fn visit_map<A: MapAccess<'de>>(
                    self,
                    mut input: A,
                ) -> std::result::Result<Self::Value, A::Error> {
                    let mut result = BTreeMap::new();
                    while let Some((element, value)) = input.next_entry()? {
                        if result.len() == MAX_PLAN_ELEMENT_OVERRIDES_PER_VIEW
                            || result.insert(element, value).is_some()
                        {
                            return Err(A::Error::custom(
                                "duplicate or excessive plan element graphics overrides",
                            ));
                        }
                    }
                    Ok(result)
                }
            }
            deserializer.deserialize_map(Elements)
        }
    }

    deserializer.deserialize_map(Overrides)
}

impl Model {
    pub fn plan_element_graphics_compatible(&self, view: Id, element: Id) -> bool {
        let Some(view) = self.views.get(&view) else {
            return false;
        };
        if view.parameters.kind != ViewKind::Plan
            || !view
                .parameters
                .plan
                .is_some_and(|p| p.view_type == PlanViewType::FloorPlan)
        {
            return false;
        }
        let Some(level) = view.parameters.level else {
            return false;
        };
        self.walls
            .get(&element)
            .is_some_and(|wall| wall.parameters.level == level)
            || self.openings.get(&element).is_some_and(|opening| {
                self.walls
                    .get(&opening.parameters.host)
                    .is_some_and(|wall| wall.parameters.level == level)
            })
            || self
                .floors
                .get(&element)
                .is_some_and(|floor| floor.parameters.level == level)
    }

    pub fn validate_plan_element_graphics_replacement(
        &self,
        view: Id,
        overrides: &BTreeMap<Id, PlanElementGraphicsOverride>,
    ) -> Result<()> {
        ensure(
            self.views.get(&view).is_some_and(|view| {
                view.parameters.kind == ViewKind::Plan
                    && view
                        .parameters
                        .plan
                        .is_some_and(|p| p.view_type == PlanViewType::FloorPlan)
            }),
            "element graphics overrides require an authored floor plan",
        )?;
        ensure(
            overrides.len() <= MAX_PLAN_ELEMENT_OVERRIDES_PER_VIEW,
            "a plan may have at most 4096 element graphics overrides",
        )?;
        ensure(
            self.plan_element_graphics_overrides
                .iter()
                .filter(|(other, _)| **other != view)
                .map(|(_, values)| values.len())
                .sum::<usize>()
                .saturating_add(overrides.len())
                <= MAX_PLAN_ELEMENT_OVERRIDES_PER_PROJECT,
            "project element graphics overrides exceed 10000 assignments",
        )?;
        for (element, value) in overrides {
            ensure(
                self.plan_element_graphics_compatible(view, *element),
                format!("element {element} is not eligible for this floor plan"),
            )?;
            value.validate()?;
            if self.floors.contains_key(element) {
                ensure(
                    value.cut.is_none(),
                    "floor boundaries support projected, not cut, overrides",
                )?;
            }
        }
        Ok(())
    }

    pub(crate) fn validate_plan_element_graphics(&self) -> Result<()> {
        ensure(
            self.plan_element_graphics_overrides
                .values()
                .map(BTreeMap::len)
                .sum::<usize>()
                <= MAX_PLAN_ELEMENT_OVERRIDES_PER_PROJECT,
            "project element graphics overrides exceed 10000 assignments",
        )?;
        for (view, overrides) in &self.plan_element_graphics_overrides {
            ensure(
                !overrides.is_empty(),
                "omit empty plan element override maps",
            )?;
            ensure(
                overrides.len() <= MAX_PLAN_ELEMENT_OVERRIDES_PER_VIEW,
                "a plan may have at most 4096 element graphics overrides",
            )?;
            self.validate_plan_element_graphics_replacement(*view, overrides)?;
        }
        Ok(())
    }
}
