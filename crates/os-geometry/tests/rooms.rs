use os_core::{Id, Point2};
use os_geometry::rooms::*;

fn segment(a: (f64, f64), b: (f64, f64)) -> BoundarySegment {
    (Id::new(), Point2::new(a.0, a.1), Point2::new(b.0, b.1))
}
fn rectangle() -> Vec<BoundarySegment> {
    vec![
        segment((0., 0.), (10., 0.)),
        segment((10., 0.), (10., 8.)),
        segment((10., 8.), (0., 8.)),
        segment((0., 8.), (0., 0.)),
    ]
}

#[test]
fn closed_rectangle_area_and_key_are_order_and_direction_independent() {
    let walls = rectangle();
    let original = derive_faces(&walls).unwrap();
    assert_eq!(original.faces.len(), 1);
    assert_eq!(original.faces[0].area_m2, 80.0);
    assert!(original.diagnostics.is_empty());
    for rotation in 0..walls.len() {
        let mut shuffled = walls.clone();
        shuffled.rotate_left(rotation);
        for (_, a, b) in &mut shuffled {
            std::mem::swap(a, b);
        }
        assert_eq!(derive_faces(&shuffled).unwrap(), original);
    }
    assert_eq!(
        original.assign_seed(Point2::new(-1., 4.)),
        Err(SeedDiagnostic::NotEnclosed)
    );
    assert_eq!(
        original.assign_seed(Point2::new(0., 4.)),
        Err(SeedDiagnostic::OnBoundary)
    );
    assert_eq!(
        original.assign_seed(Point2::new(f64::NAN, 4.)),
        Err(SeedDiagnostic::InvalidSeed)
    );
}

#[test]
fn mixed_wall_and_room_separator_boundaries_are_stable_and_topology_aware() {
    let walls = rectangle();
    let original = derive_faces(&walls).unwrap();
    let accepted = original.faces[0].key.clone();
    let seed = Point2::new(2.0, 4.0);

    // A separator can close a room without requiring a physical wall on that edge.
    let mut three_walls = walls.clone();
    three_walls.pop();
    let closing_separator = segment((0.0, 8.0), (0.0, 0.0));
    let closing_id = closing_separator.0;
    three_walls.push(closing_separator);
    let closed = derive_faces(&three_walls).unwrap();
    assert_eq!(closed.faces.len(), 1);
    assert_eq!(closed.faces[0].area_m2, 80.0);
    assert!(
        closed.faces[0]
            .key
            .as_signature()
            .iter()
            .any(|(id, _)| *id == closing_id)
    );

    // Adding a separator splits a persisted wall-only face without silently
    // assigning that existing room identity to one of the new faces.
    let divider = segment((5.0, 0.0), (5.0, 8.0));
    let divider_id = divider.0;
    let mut mixed = walls.clone();
    mixed.push(divider);
    let split = derive_faces(&mixed).unwrap();
    assert_eq!(split.faces.len(), 2);
    assert!(split.faces.iter().all(|face| face.area_m2 == 40.0));
    assert!(split.faces.iter().all(|face| {
        face.key
            .as_signature()
            .iter()
            .any(|(id, _)| *id == divider_id)
    }));
    assert_eq!(
        split.resolve_seed(seed, &accepted),
        Err(SeedDiagnostic::FaceChanged)
    );
    assert_eq!(
        derive_faces(&walls)
            .unwrap()
            .resolve_seed(seed, &accepted)
            .unwrap()
            .area_m2,
        80.0
    );
}

#[test]
fn persisted_signature_prevents_remapping_after_partition_move_or_delete() {
    let mut walls = rectangle();
    walls.push(segment((5., 0.), (5., 8.)));
    let seed = Point2::new(2., 4.);
    let initial = derive_faces(&walls).unwrap();
    assert_eq!(initial.faces.len(), 2);
    assert!(initial.faces.iter().all(|face| face.area_m2 == 40.0));
    // Reconstruct only from serialized-compatible UUID/bool data, not old geometry.
    let stored_signature = initial
        .assign_seed(seed)
        .unwrap()
        .key
        .as_signature()
        .to_vec();
    let restored_key = FaceKey::from_signature(&stored_signature).unwrap();
    walls[4].1.x = 6.;
    walls[4].2.x = 6.;
    let moved = derive_faces(&walls).unwrap();
    assert_eq!(
        moved.resolve_seed(seed, &restored_key).unwrap().area_m2,
        48.0
    );
    walls[4].1.x = 1.;
    walls[4].2.x = 1.;
    assert_eq!(
        derive_faces(&walls)
            .unwrap()
            .resolve_seed(seed, &restored_key),
        Err(SeedDiagnostic::FaceChanged)
    );
    walls.pop();
    assert_eq!(
        derive_faces(&walls)
            .unwrap()
            .resolve_seed(seed, &restored_key),
        Err(SeedDiagnostic::FaceChanged)
    );
    assert_eq!(
        initial.resolve_seed(seed, &restored_key).unwrap().area_m2,
        40.0
    );
}

#[test]
fn crossings_and_t_junctions_split_faces_with_conserved_area() {
    let mut walls = rectangle();
    walls.push(segment((5., -1.), (5., 9.)));
    walls.push(segment((-1., 4.), (11., 4.)));
    let result = derive_faces(&walls).unwrap();
    assert_eq!(result.faces.len(), 4);
    assert_eq!(result.faces.iter().map(|f| f.area_m2).sum::<f64>(), 80.);
    assert!(result.faces.iter().all(|f| f.area_m2 == 20.));
    assert_eq!(
        result.assign_seed(Point2::new(5., 4.)),
        Err(SeedDiagnostic::OnBoundary)
    );
    assert!(matches!(
        result.diagnostics.as_slice(),
        [BoundaryDiagnostic::OpenChains(_)]
    ));
}

#[test]
fn diagonal_faces_have_independently_known_area() {
    let mut walls = rectangle();
    walls.push(segment((0., 0.), (10., 8.)));
    let result = derive_faces(&walls).unwrap();
    assert_eq!(result.faces.len(), 2);
    assert!(result.faces.iter().all(|f| (f.area_m2 - 40.).abs() < 1e-9));
}

#[test]
fn dangling_walls_keep_enclosed_face_but_a_gap_does_not_close() {
    let mut walls = rectangle();
    let before = derive_faces(&walls).unwrap();
    walls.push(segment((5., 0.), (5., 3.)));
    let with_spur = derive_faces(&walls).unwrap();
    assert_eq!(with_spur.faces.len(), 1);
    assert_eq!(with_spur.faces[0].key, before.faces[0].key);
    assert_eq!(
        with_spur.assign_seed(Point2::new(5., 2.)),
        Err(SeedDiagnostic::OnBoundary)
    );
    walls.pop();
    walls[0].1.x = 2. * ROOM_TOLERANCE;
    let gap = derive_faces(&walls).unwrap();
    assert!(gap.faces.is_empty());
    assert_eq!(
        gap.assign_seed(Point2::new(2., 4.)),
        Err(SeedDiagnostic::NotEnclosed)
    );
    assert!(matches!(
        gap.diagnostics.as_slice(),
        [BoundaryDiagnostic::OpenChains(_)]
    ));
}

#[test]
fn lattice_ties_are_explicit_and_tiny_endpoint_noise_is_stable() {
    let mut walls = rectangle();
    walls[0].1.x = 0.49 * ROOM_TOLERANCE;
    assert_eq!(derive_faces(&walls).unwrap().faces.len(), 1);
    walls[0].1.x = 0.5 * ROOM_TOLERANCE;
    assert!(derive_faces(&walls).unwrap().faces.is_empty());
}

#[test]
fn overlaps_duplicates_and_unbounded_inputs_fail_explicitly() {
    let mut walls = rectangle();
    walls.push(segment((2., 0.), (7., 0.)));
    assert!(matches!(
        derive_faces(&walls),
        Err(BoundaryDiagnostic::OverlappingBoundaries(_, _))
    ));
    walls.pop();
    walls.push(walls[0]);
    assert!(matches!(
        derive_faces(&walls),
        Err(BoundaryDiagnostic::DuplicateBoundary(_))
    ));
    for p in [
        Point2::new(f64::INFINITY, 0.),
        Point2::new(1e7, 0.),
        walls[0].2,
    ] {
        let mut bad = rectangle();
        bad[0].1 = p;
        assert!(matches!(
            derive_faces(&bad),
            Err(BoundaryDiagnostic::InvalidSegment(_))
        ));
    }
    assert_eq!(
        derive_faces(&vec![walls[0]; MAX_ROOM_SEGMENTS + 1]),
        Err(BoundaryDiagnostic::ResourceLimit)
    );
    assert!(FaceKey::from_signature(&[]).is_none());
    assert!(FaceKey::from_signature(&[(walls[0].0, true); 3]).is_none());
}

#[test]
fn nested_loops_are_diagnosed_instead_of_returning_wrong_area() {
    let mut walls = rectangle();
    walls.extend([
        segment((2., 2.), (3., 2.)),
        segment((3., 2.), (3., 3.)),
        segment((3., 3.), (2., 3.)),
        segment((2., 3.), (2., 2.)),
    ]);
    assert_eq!(derive_faces(&walls), Err(BoundaryDiagnostic::NestedLoops));
}

#[test]
fn translated_small_room_area_avoids_large_coordinate_cancellation() {
    let mut walls = rectangle();
    for (_, a, b) in &mut walls {
        for p in [a, b] {
            p.x = p.x * 0.01 + 900_000.;
            p.y = p.y * 0.01 + 900_000.;
        }
    }
    assert!((derive_faces(&walls).unwrap().faces[0].area_m2 - 0.008).abs() < 1e-10);
}
