//! Dependency invalidation for derived geometry; no general constraint solver yet.
use os_core::{Id, Result};
use os_model::Model;
use std::collections::BTreeMap;
use std::collections::BTreeSet;

/// Conservatively invalidate extension dependents, including ordinary named references.
/// Cyclic reference groups are allowed and visited once; explicit prerequisites
/// are separately required to be acyclic by model validation.
pub fn affected_entities(model: &Model, changed: &BTreeSet<Id>) -> BTreeSet<Id> {
    let mut reverse: BTreeMap<Id, Vec<Id>> = BTreeMap::new();
    for entity in model.extensions.values() {
        for target in entity.references() {
            reverse.entry(*target).or_default().push(entity.id);
        }
    }
    let mut seen = changed.clone();
    seen.extend(affected_walls(model, changed));
    // Plans may show walls from neighboring levels inside their depth/cut range.
    // Conservatively invalidate all plan views for a geometry/datum change;
    // settings edits invalidate their own view and any extension references.
    let geometry_changed = seen.iter().any(|id| {
        model.walls.contains_key(id)
            || model.extensions.contains_key(id)
            || model.levels.contains_key(id)
            || model.grids.contains_key(id)
            || model.materials.contains_key(id)
    });
    seen.extend(
        model
            .views
            .values()
            .filter(|view| {
                view.parameters.kind == os_model::ViewKind::Plan
                    && (geometry_changed || changed.contains(&view.id()))
            })
            .map(|view| view.id()),
    );
    let mut pending: Vec<_> = seen.iter().copied().collect();
    while let Some(id) = pending.pop() {
        if let Some(children) = reverse.get(&id) {
            for child in children {
                if seen.insert(*child) {
                    pending.push(*child);
                }
            }
        }
    }
    seen.retain(|id| {
        model.extensions.contains_key(id)
            || model.walls.contains_key(id)
            || model.views.contains_key(id)
            || model.grids.contains_key(id)
    });
    seen
}

pub fn validate(model: &Model) -> Result<()> {
    model.validate()
}

/// Level elevation and material changes invalidate all attached walls.
pub fn affected_walls(model: &Model, changed: &BTreeSet<Id>) -> BTreeSet<Id> {
    model
        .walls
        .iter()
        .filter_map(|(id, wall)| {
            (changed.contains(id)
                || changed.contains(&wall.parameters.level)
                || wall
                    .parameters
                    .material
                    .is_some_and(|id| changed.contains(&id)))
            .then_some(*id)
        })
        .collect()
}
