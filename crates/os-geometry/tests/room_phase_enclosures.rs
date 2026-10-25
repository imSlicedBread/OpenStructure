use os_core::{Id, Point2};
use os_geometry::{ceilings::CeilingResolver, rooms::derive_faces};
use os_model::{
    CeilingParams, ElementLifecycle, Model, Room, RoomParams, RoomSeparationLine,
    RoomSeparationLineParams, Wall, WallParams, WallPath, new_phase,
};

fn wall(model: &mut Model, level: Id, start: (f64, f64), end: (f64, f64)) -> Id {
    let wall = Wall::new(
        "org.openstructure.walls.wall",
        WallParams {
            name: "Boundary".into(),
            path: WallPath::Straight {
                start: Point2::new(start.0, start.1),
                end: Point2::new(end.0, end.1),
            },
            thickness: 0.2,
            height: 3.0,
            level,
            material: None,
        },
    );
    let id = wall.id();
    model.walls.insert(id, wall);
    id
}

fn renovation() -> (Model, Id, Id, Id) {
    let mut model = Model::new("Renovation");
    let level = *model.levels.keys().next().unwrap();
    let existing = model.existing_phase().unwrap();
    let new = model.latest_phase().unwrap();
    for (start, end) in [
        ((0.0, 0.0), (8.0, 0.0)),
        ((8.0, 0.0), (8.0, 6.0)),
        ((8.0, 6.0), (0.0, 6.0)),
        ((0.0, 6.0), (0.0, 0.0)),
    ] {
        wall(&mut model, level, start, end);
    }
    let old_partition = wall(&mut model, level, (4.0, 0.0), (4.0, 6.0));
    let replacement = wall(&mut model, level, (6.0, 0.0), (6.0, 6.0));
    model.element_lifecycles.insert(
        old_partition,
        ElementLifecycle {
            created_in: existing,
            demolished_in: Some(new),
        },
    );
    model.element_lifecycles.insert(
        replacement,
        ElementLifecycle {
            created_in: new,
            demolished_in: None,
        },
    );
    model.validate().unwrap();
    (model, level, existing, new)
}

fn areas(model: &Model, level: Id, phase: Option<Id>) -> Vec<f64> {
    let segments = match phase {
        Some(phase) => model.room_boundary_segments_at_phase(level, phase),
        None => model.room_boundary_segments(level),
    }
    .unwrap();
    let mut areas: Vec<_> = derive_faces(&segments)
        .unwrap()
        .faces
        .iter()
        .map(|face| face.area_m2)
        .collect();
    areas.sort_by(f64::total_cmp);
    areas
}

#[test]
fn demolition_replacement_and_temporary_separator_change_physical_topology() {
    let (mut model, level, existing, new) = renovation();
    assert_eq!(areas(&model, level, Some(existing)), vec![24.0, 24.0]);
    assert_eq!(areas(&model, level, Some(new)), vec![12.0, 36.0]);
    assert_eq!(areas(&model, level, None), vec![12.0, 12.0, 24.0]);
    let later = new_phase("After temporary works", 2);
    let later_id = later.id();
    model.phases.insert(later_id, later);
    let temporary = RoomSeparationLine::new(
        "core.room_separation_line",
        RoomSeparationLineParams {
            level,
            start: Point2::new(2.0, 0.0),
            end: Point2::new(2.0, 6.0),
        },
    );
    model.element_lifecycles.insert(
        temporary.id(),
        ElementLifecycle {
            created_in: new,
            demolished_in: Some(new),
        },
    );
    model
        .room_separation_lines
        .insert(temporary.id(), temporary);
    model.validate().unwrap();
    assert_eq!(areas(&model, level, Some(existing)), vec![24.0, 24.0]);
    assert_eq!(areas(&model, level, Some(new)), vec![12.0, 12.0, 24.0]);
    assert_eq!(areas(&model, level, Some(later_id)), vec![12.0, 36.0]);
}

fn accepted_ceiling(
    model: &mut Model,
    level: Id,
    phase: Option<Id>,
    number: &str,
) -> CeilingParams {
    // The legacy-only strip between both partitions is absent in either phase.
    let seed = Point2::new(if phase.is_none() { 5.0 } else { 1.0 }, 1.0);
    let segments = match phase {
        Some(phase) => model.room_boundary_segments_at_phase(level, phase),
        None => model.room_boundary_segments(level),
    }
    .unwrap();
    let faces = derive_faces(&segments).unwrap();
    let face = faces.assign_seed(seed).unwrap();
    let room = Room::new(
        "core.room",
        RoomParams {
            number: number.into(),
            name: "Office".into(),
            floor_finish: None,
            wall_finish: None,
            ceiling_finish: None,
            floor_material: None,
            wall_material: None,
            ceiling_material: None,
            level,
            seed,
            boundary_signature: face.key.as_signature().to_vec(),
        },
    );
    let room_id = room.id();
    model.rooms.insert(room_id, room);
    CeilingParams {
        name: "Room ceiling".into(),
        level,
        material: None,
        boundary_room: Some(room_id),
        boundary: face.boundary.clone(),
        holes: Vec::new(),
        thickness: 0.12,
        elevation_offset: 2.55,
    }
}

#[test]
fn ceiling_cache_isolates_each_phase_and_legacy_in_both_query_orders() {
    let (mut model, level, existing, new) = renovation();
    let old_ceiling = accepted_ceiling(&mut model, level, Some(existing), "101");
    let new_ceiling = accepted_ceiling(&mut model, level, Some(new), "102");
    let legacy_ceiling = accepted_ceiling(&mut model, level, None, "103");
    model.validate().unwrap();
    for reverse in [false, true] {
        let mut resolver = CeilingResolver::default();
        let mut cases = [
            (&old_ceiling, Some(existing), true),
            (&old_ceiling, Some(new), false),
            (&new_ceiling, Some(existing), false),
            (&new_ceiling, Some(new), true),
            (&new_ceiling, None, false),
            (&legacy_ceiling, None, true),
            (&legacy_ceiling, Some(existing), false),
            (&legacy_ceiling, Some(new), false),
        ];
        if reverse {
            cases.reverse();
        }
        for _ in 0..2 {
            for (parameters, phase, resolves) in cases {
                let result = match phase {
                    Some(phase) => {
                        resolver.effective_parameters_at_phase(&model, parameters, phase)
                    }
                    None => resolver.effective_parameters(&model, parameters),
                }
                .unwrap();
                assert_eq!(result.is_some(), resolves);
                if let Some(result) = result {
                    assert_eq!(result, *parameters);
                }
                let boundary = match phase {
                    Some(phase) => {
                        resolver.resolved_room_boundary_at_phase(&model, parameters, phase)
                    }
                    None => resolver.resolved_room_boundary(&model, parameters),
                }
                .unwrap();
                assert_eq!(boundary.is_some(), resolves);
            }
        }
    }
}

#[test]
fn ceiling_phase_validation_and_explicit_outlines_remain_distinct() {
    let (mut model, level, existing, new) = renovation();
    let mut ceiling = accepted_ceiling(&mut model, level, Some(existing), "101");
    let mut resolver = CeilingResolver::default();
    assert!(
        resolver
            .resolved_room_boundary_at_phase(&model, &ceiling, Id::new())
            .is_err()
    );
    assert!(
        resolver
            .effective_parameters_at_phase(&model, &ceiling, Id::new())
            .is_err()
    );
    ceiling.boundary_room = None;
    assert_eq!(
        resolver
            .effective_parameters_at_phase(&model, &ceiling, new)
            .unwrap(),
        Some(ceiling.clone())
    );
    assert_eq!(
        resolver.effective_parameters(&model, &ceiling).unwrap(),
        Some(ceiling.clone())
    );
    assert!(
        resolver
            .resolved_room_boundary_at_phase(&model, &ceiling, new)
            .unwrap()
            .is_none()
    );
    assert!(
        resolver
            .effective_parameters_at_phase(&model, &ceiling, Id::new())
            .is_err()
    );
}
