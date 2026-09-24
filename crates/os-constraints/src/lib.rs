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
    for (wall, assignment) in &model.wall_type_assignments {
        reverse.entry(assignment.type_id).or_default().push(*wall);
    }
    for ty in model.wall_types.values() {
        for layer in &ty.parameters.layers {
            if let Some(material) = layer.material {
                reverse.entry(material).or_default().push(ty.id());
            }
        }
    }
    for join in model.wall_joins.values() {
        for wall in join.parameters.members() {
            reverse.entry(wall).or_default().push(join.id());
            reverse.entry(join.id()).or_default().push(wall);
        }
    }
    for column in model.columns.values() {
        reverse
            .entry(column.parameters.level)
            .or_default()
            .push(column.id());
        if let Some(material) = column.parameters.material {
            reverse.entry(material).or_default().push(column.id());
        }
    }
    for floor in model.floors.values() {
        reverse
            .entry(floor.parameters.level)
            .or_default()
            .push(floor.id());
        if let Some(material) = floor.parameters.material {
            reverse.entry(material).or_default().push(floor.id());
        }
    }
    for entity in model.extensions.values() {
        for target in entity.references() {
            reverse.entry(*target).or_default().push(entity.id);
        }
    }
    for opening in model.openings.values() {
        if let Some(type_id) = opening.parameters.type_id() {
            reverse.entry(type_id).or_default().push(opening.id());
        }
        reverse
            .entry(opening.id())
            .or_default()
            .push(opening.parameters.host);
        reverse
            .entry(opening.parameters.host)
            .or_default()
            .push(opening.id());
    }
    for dimension in model.dimensions.values() {
        let p = &dimension.parameters;
        for target in std::iter::once(p.view).chain(p.references().map(|reference| reference.wall))
        {
            reverse.entry(target).or_default().push(dimension.id());
        }
        // Annotation edits invalidate their owning drawing as well.
        reverse.entry(dimension.id()).or_default().push(p.view);
    }
    for tag in model.room_tags.values() {
        for target in [tag.parameters.room, tag.parameters.view] {
            reverse.entry(target).or_default().push(tag.id());
        }
        reverse
            .entry(tag.id())
            .or_default()
            .push(tag.parameters.view);
    }
    for line in model.room_separation_lines.values() {
        reverse
            .entry(line.parameters.level)
            .or_default()
            .push(line.id());
    }
    // Every wall or separator on a room's level can change enclosure, including walls that
    // did not bound its previous face. The document unions before/after graphs.
    for room in model.rooms.values() {
        reverse
            .entry(room.parameters.level)
            .or_default()
            .push(room.id());
        for wall in model
            .walls
            .values()
            .filter(|w| w.parameters.level == room.parameters.level)
        {
            reverse.entry(wall.id()).or_default().push(room.id());
        }
        for line in model
            .room_separation_lines
            .values()
            .filter(|line| line.parameters.level == room.parameters.level)
        {
            reverse.entry(line.id()).or_default().push(room.id());
        }
    }
    let mut seen = changed.clone();
    seen.extend(affected_walls(model, changed));
    // Plans may show walls from neighboring levels inside their depth/cut range.
    // Conservatively invalidate all plan views for a geometry/datum change;
    // settings edits invalidate their own view and any extension references.
    let geometry_changed = seen.iter().any(|id| {
        model.walls.contains_key(id)
            || model.wall_types.contains_key(id)
            || model.wall_joins.contains_key(id)
            || model.floors.contains_key(id)
            || model.columns.contains_key(id)
            || model.extensions.contains_key(id)
            || model.levels.contains_key(id)
            || model.grids.contains_key(id)
            || model.materials.contains_key(id)
            || model.rooms.contains_key(id)
            || model.room_separation_lines.contains_key(id)
    });
    seen.extend(
        model
            .views
            .values()
            .filter(|view| {
                (view.parameters.kind == os_model::ViewKind::Plan
                    || (view.parameters.kind == os_model::ViewKind::Section
                        && view.parameters.section.is_some()))
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
            || model.wall_joins.contains_key(id)
            || model.floors.contains_key(id)
            || model.columns.contains_key(id)
            || model.walls.contains_key(id)
            || model.openings.contains_key(id)
            || model.opening_types.contains_key(id)
            || model.rooms.contains_key(id)
            || model.room_separation_lines.contains_key(id)
            || model.views.contains_key(id)
            || model.dimensions.contains_key(id)
            || model.room_tags.contains_key(id)
            || model.grids.contains_key(id)
    });
    // Schedule rows derive host data on each revision; include schedule IDs for
    // consumers with dependency caches as well as revision-bound sheet tables.
    if seen
        .iter()
        .any(|id| model.walls.contains_key(id) || model.openings.contains_key(id))
    {
        seen.extend(model.schedules.keys().copied());
    }
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
                || model.openings.values().any(|o| {
                    o.parameters.host == *id
                        && (changed.contains(&o.id())
                            || o.parameters
                                .type_id()
                                .is_some_and(|ty| changed.contains(&ty)))
                })
                || changed.contains(&wall.parameters.level)
                || wall
                    .parameters
                    .material
                    .is_some_and(|id| changed.contains(&id)))
            .then_some(*id)
        })
        .collect()
}
