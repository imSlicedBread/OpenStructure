//! Persistent per-view symbol exclusions. Host apertures retain their geometry.
use crate::*;
use os_core::{Result, ensure};
use serde::{
    Deserializer,
    de::{Error, MapAccess, Visitor},
};

pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
    d: D,
) -> std::result::Result<BTreeMap<Id, BTreeSet<Id>>, D::Error> {
    struct Map;
    impl<'de> Visitor<'de> for Map {
        type Value = BTreeMap<Id, BTreeSet<Id>>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("unique view IDs with nonempty unique opening ID arrays")
        }
        fn visit_map<A: MapAccess<'de>>(
            self,
            mut input: A,
        ) -> std::result::Result<Self::Value, A::Error> {
            let mut result = BTreeMap::new();
            let mut total = 0;
            while let Some((view, ids)) = input.next_entry::<Id, Vec<Id>>()? {
                let count = ids.len();
                total += count;
                let ids: BTreeSet<_> = ids.into_iter().collect();
                if count == 0
                    || count > 4096
                    || total > 10000
                    || ids.len() != count
                    || result.insert(view, ids).is_some()
                {
                    return Err(A::Error::custom(
                        "invalid, duplicate or excessive hidden opening assignments",
                    ));
                }
            }
            Ok(result)
        }
    }
    d.deserialize_map(Map)
}

impl Model {
    pub fn is_native_floor_plan(&self, view: Id) -> bool {
        self.views.get(&view).is_some_and(|v| {
            v.parameters.kind == ViewKind::Plan
                && v.parameters.level.is_some()
                && v.parameters
                    .plan
                    .is_some_and(|p| p.view_type == PlanViewType::FloorPlan)
        })
    }

    pub fn plan_hidden_opening_compatible(&self, view: Id, opening: Id) -> bool {
        self.is_native_floor_plan(view)
            && self
                .openings
                .get(&opening)
                .and_then(|o| self.walls.get(&o.parameters.host))
                .is_some_and(|w| self.views[&view].parameters.level == Some(w.parameters.level))
    }

    /// Empty replacement is legal; persisted empty entries are not.
    pub fn validate_plan_hidden_openings_replacement(
        &self,
        view: Id,
        hidden: &BTreeSet<Id>,
    ) -> Result<()> {
        ensure(
            self.is_native_floor_plan(view),
            "hidden openings require an authored FloorPlan",
        )?;
        ensure(
            hidden.len() <= 4096,
            "a plan may hide at most 4096 openings",
        )?;
        for id in hidden {
            ensure(
                self.plan_hidden_opening_compatible(view, *id),
                format!("hidden opening {id} must be native and hosted on the plan level"),
            )?;
        }
        let total = self
            .plan_hidden_openings
            .iter()
            .filter(|(id, _)| **id != view)
            .map(|(_, ids)| ids.len())
            .sum::<usize>()
            + hidden.len();
        ensure(
            total <= 10000,
            "project hidden opening assignments exceed 10000",
        )
    }

    pub fn validate_plan_hidden_openings(&self) -> Result<()> {
        ensure(
            self.plan_hidden_openings
                .values()
                .map(BTreeSet::len)
                .sum::<usize>()
                <= 10000,
            "project hidden opening assignments exceed 10000",
        )?;
        for (view, hidden) in &self.plan_hidden_openings {
            ensure(!hidden.is_empty(), "omit empty hidden opening sets")?;
            self.validate_plan_hidden_openings_replacement(*view, hidden)?;
        }
        Ok(())
    }
}
