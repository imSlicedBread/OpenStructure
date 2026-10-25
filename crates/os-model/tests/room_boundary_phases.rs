use os_core::{Id, Point2};
use os_model::{
    ElementLifecycle, Model, RoomSeparationLine, RoomSeparationLineParams, Wall, WallParams,
    WallPath, new_phase,
};

#[test]
fn physical_boundaries_include_only_existing_new_and_temporary_for_both_source_kinds() {
    let mut model = Model::new("Boundary lifetimes");
    let level = *model.levels.keys().next().unwrap();
    let existing = model.existing_phase().unwrap();
    let target = model.latest_phase().unwrap();
    let later = new_phase("Later", 2);
    let future = later.id();
    model.phases.insert(future, later);
    let mut expected = Vec::new();
    for (index, (created_in, demolished_in, included)) in [
        (existing, None, true),
        (target, None, true),
        (target, Some(target), true),
        (existing, Some(target), false),
        (future, None, false),
        (existing, Some(existing), false),
    ]
    .into_iter()
    .enumerate()
    {
        let start = Point2::new(index as f64, 0.0);
        let end = Point2::new(index as f64, 4.0);
        let wall = Wall::new(
            "core.wall",
            WallParams {
                name: "Boundary".into(),
                path: WallPath::Straight { start, end },
                thickness: 0.2,
                height: 3.0,
                level,
                material: None,
            },
        );
        let line = RoomSeparationLine::new(
            "core.room_separation_line",
            RoomSeparationLineParams { level, start, end },
        );
        for id in [wall.id(), line.id()] {
            model.element_lifecycles.insert(
                id,
                ElementLifecycle {
                    created_in,
                    demolished_in,
                },
            );
            if included {
                expected.push((id, start, end));
            }
        }
        model.walls.insert(wall.id(), wall);
        model.room_separation_lines.insert(line.id(), line);
    }
    expected.sort_by_key(|segment| segment.0);
    assert_eq!(
        model
            .room_boundary_segments_at_phase(level, target)
            .unwrap(),
        expected
    );
    assert_eq!(model.room_boundary_segments(level).unwrap().len(), 12);

    // Legacy unassigned elements still participate as Existing.
    model.element_lifecycles.clear();
    assert_eq!(
        model
            .room_boundary_segments_at_phase(level, target)
            .unwrap(),
        model.room_boundary_segments(level).unwrap()
    );
}

#[test]
fn phase_boundary_queries_reject_missing_context_and_invalid_lifecycles() {
    let mut model = Model::new("Checked boundaries");
    let level = *model.levels.keys().next().unwrap();
    let target = model.latest_phase().unwrap();
    assert!(
        model
            .room_boundary_segments_at_phase(level, Id::new())
            .is_err()
    );
    assert!(
        model
            .room_boundary_segments_at_phase(Id::new(), target)
            .is_err()
    );
    assert!(
        model
            .room_boundary_segments_at_phase(level, target)
            .unwrap()
            .is_empty()
    );
    let line = RoomSeparationLine::new(
        "core.room_separation_line",
        RoomSeparationLineParams {
            level,
            start: Point2::new(0.0, 0.0),
            end: Point2::new(0.0, 4.0),
        },
    );
    let id = line.id();
    model.room_separation_lines.insert(id, line);
    for (created_in, demolished_in) in [
        (Id::new(), None),
        (target, Some(Id::new())),
        (target, model.existing_phase()),
    ] {
        model.element_lifecycles.insert(
            id,
            ElementLifecycle {
                created_in,
                demolished_in,
            },
        );
        assert!(
            model
                .room_boundary_segments_at_phase(level, target)
                .is_err()
        );
        assert_eq!(model.room_boundary_segments(level).unwrap().len(), 1);
    }
}
