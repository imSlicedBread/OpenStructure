use super::*;
use crate::{GeometryKernel, PrismKernel};
use os_model::*;

fn fixture() -> (Model, Id, Id, Id) {
    let mut model = Model::new("Elevation");
    let level = *model.levels.keys().next().unwrap();
    model.levels.get_mut(&level).unwrap().parameters.elevation = 10.;
    let wall = Wall::new(
        "wall",
        WallParams {
            name: "Host".into(),
            level,
            height: 4.,
            thickness: 0.3,
            material: None,
            path: WallPath::Straight {
                start: Point2::new(0., 0.),
                end: Point2::new(8., 0.),
            },
        },
    );
    let host = wall.id();
    model.walls.insert(host, wall);
    let ty = OpeningType::new(
        "opening_type",
        OpeningTypeParams {
            name: "Window".into(),
            kind: OpeningKind::Window,
            width: 2.,
            height: 2.,
            sill: 1.,
            window_operation: WindowOperation::Fixed,
            pane_position: WindowPanePosition::Center,
            family: OpeningFamily {
                frame_width: 0.06,
                ..Default::default()
            },
        },
    );
    let type_id = ty.id();
    model.opening_types.insert(type_id, ty);
    let opening = Opening::new(
        "opening",
        OpeningParams {
            name: "Window".into(),
            host,
            offset: 2.,
            definition: OpeningDefinition::Typed { type_id },
            width_override: None,
            height_override: None,
            sill_override: None,
            pane_position_override: None,
            lite_side_override: None,
            hinge: DoorHinge::Start,
            swing: DoorSwing::Left,
            open_state: OpeningState::Default,
        },
    );
    let id = opening.id();
    model.openings.insert(id, opening);
    (model, host, id, type_id)
}

fn has(lines: &[ElevationSegment], entity: Id, a: (f64, f64), b: (f64, f64)) -> bool {
    let a = Point2::new(a.0, a.1);
    let b = Point2::new(b.0, b.1);
    lines.iter().any(|s| {
        s.entity == entity
            && ((s.start.distance(a) < 1e-7 && s.end.distance(b) < 1e-7)
                || (s.end.distance(a) < 1e-7 && s.start.distance(b) < 1e-7))
    })
}

#[test]
fn plain_wall_has_four_edges_without_rear_lines_or_diagonals() {
    let (mut model, host, _, _) = fixture();
    model.openings.clear();
    for left in [false, true] {
        let lines = ElevationSnapshot::capture(&model, host, left)
            .unwrap()
            .derive()
            .unwrap();
        assert_eq!(lines.len(), 4, "{lines:?}");
        assert!(has(&lines, host, (0., 10.), (8., 10.)));
        assert!(has(&lines, host, (0., 14.), (8., 14.)));
        assert!(has(&lines, host, (0., 10.), (0., 14.)));
        assert!(has(&lines, host, (8., 10.), (8., 14.)));
    }
}

#[test]
fn apertures_frames_and_panels_keep_station_direction_after_rotation_and_reversal() {
    let (mut model, host, opening, _) = fixture();
    let expected = ElevationSnapshot::capture(&model, host, true)
        .unwrap()
        .derive()
        .unwrap();
    assert!(expected.iter().any(|s| s.entity == opening));
    assert!(has(&expected, host, (2., 11.), (4., 11.)));
    assert!(has(&expected, host, (2., 13.), (4., 13.)));
    // These used to leak from the rectangular cell decomposition.
    assert!(!has(&expected, host, (2., 10.), (2., 14.)));
    for (a, b) in [((2., 3.), (8.4, 7.8)), ((8.4, 7.8), (2., 3.))] {
        model.walls.get_mut(&host).unwrap().parameters.path = WallPath::Straight {
            start: Point2::new(a.0, a.1),
            end: Point2::new(b.0, b.1),
        };
        for left in [false, true] {
            let lines = ElevationSnapshot::capture(&model, host, left)
                .unwrap()
                .derive()
                .unwrap();
            assert_eq!(lines.len(), expected.len(), "{lines:?}");
            for s in &expected {
                assert!(
                    has(&lines, s.entity, (s.start.x, s.start.y), (s.end.x, s.end.y)),
                    "missing {s:?} in {lines:?}"
                );
            }
        }
    }
}

#[test]
fn compound_faces_keep_only_observed_layer_material() {
    let (mut model, host, _, _) = fixture();
    model.openings.clear();
    let mut snapshot = ElevationSnapshot::capture(&model, host, true).unwrap();
    let mut rear = snapshot.wall.layers[0].clone();
    rear.id = Some(Id::new());
    rear.material = Some(Id::new());
    rear.min = -0.15;
    rear.max = 0.;
    let mut front = rear.clone();
    front.id = Some(Id::new());
    front.material = Some(Id::new());
    front.min = 0.;
    front.max = 0.15;
    snapshot.wall.layers = vec![rear.clone(), front.clone()];
    for left in [false, true] {
        snapshot.left_side = left;
        let lines = snapshot.clone().derive().unwrap();
        let layer = if left { &front } else { &rear };
        assert_eq!(lines.len(), 4);
        assert!(lines.iter().all(|s| s.entity == host
            && s.surface.layer == layer.id
            && s.surface.material == layer.material));
    }
}

#[test]
fn rear_object_is_fully_hidden_and_partial_occlusion_splits_edges() {
    let (model, host, _, _) = fixture();
    let mut wall = model.walls[&host].parameters.clone();
    let front_id = Id::new();
    let rear_id = Id::new();
    let make = |w: &WallParams| {
        PrismKernel
            .tessellate(&crate::walls::wall_solid(w, 10.).unwrap())
            .unwrap()
    };
    let rear = make(&wall);
    wall.path = WallPath::Straight {
        start: Point2::new(0., 1.),
        end: Point2::new(8., 1.),
    };
    let front = make(&wall);
    let lines = project(
        &[(rear_id, rear.clone()), (front_id, front)],
        Point2::new(0., 0.),
        Point2::new(1., 0.),
        true,
    )
    .unwrap();
    assert_eq!(lines.len(), 4);
    assert!(lines.iter().all(|s| s.entity == front_id));
    wall.path = WallPath::Straight {
        start: Point2::new(2., 1.),
        end: Point2::new(6., 1.),
    };
    let lines = project(
        &[(rear_id, rear), (front_id, make(&wall))],
        Point2::new(0., 0.),
        Point2::new(1., 0.),
        true,
    )
    .unwrap();
    assert!(has(&lines, rear_id, (0., 14.), (2., 14.)));
    assert!(has(&lines, rear_id, (6., 14.), (8., 14.)));
    assert!(!has(&lines, rear_id, (0., 14.), (8., 14.)));
}

#[test]
fn profiled_aperture_uses_true_sloped_boundary() {
    let (mut model, host, _, ty) = fixture();
    let family = &mut model.opening_types.get_mut(&ty).unwrap().parameters.family;
    family.frame_width = 0.;
    family.host_cut = OpeningHostCut::Profile;
    family.cut_profile = vec![
        Point2::new(0., 0.),
        Point2::new(1., 0.),
        Point2::new(0.5, 1.),
    ];
    family.profile = family.cut_profile.clone();
    for left in [false, true] {
        let lines = ElevationSnapshot::capture(&model, host, left)
            .unwrap()
            .derive()
            .unwrap();
        assert!(has(&lines, host, (2., 11.), (3., 13.)), "{lines:?}");
        assert!(has(&lines, host, (3., 13.), (4., 11.)));
        assert!(!has(&lines, host, (2., 13.), (4., 13.)));
        assert!(has(&lines, host, (0., 14.), (8., 14.)));
    }
}

#[test]
fn materialed_lites_pairs_and_eligible_sash_poses_project_on_both_sides() {
    let (mut model, host, opening, ty) = fixture();
    // Native family validation requires referenced project materials.
    let mut materials = Vec::new();
    for name in ["Panel", "Frame", "Lite"] {
        let material = Material::new(
            "material",
            MaterialParams {
                name: name.into(),
                density_kg_m3: 1000.,
                color: [120, 150, 180],
            },
        );
        materials.push(material.id());
        model.materials.insert(material.id(), material);
    }
    {
        let p = &mut model.opening_types.get_mut(&ty).unwrap().parameters;
        p.family.panel_material = Some(materials[0]);
        p.family.frame_material = Some(materials[1]);
        p.family.side_lite = Some(SideLite {
            side: LiteSide::End,
            width_fraction: 0.2,
            mullion_width: 0.05,
            material: Some(materials[2]),
        });
    }
    for (operation, state) in [
        (WindowOperation::Fixed, OpeningState::Default),
        (WindowOperation::Sliding, OpeningState::SlidingFraction(0.6)),
        (WindowOperation::Casement, OpeningState::CasementAngle(37.)),
        (WindowOperation::Casement, OpeningState::CasementAngle(90.)),
    ] {
        model
            .opening_types
            .get_mut(&ty)
            .unwrap()
            .parameters
            .window_operation = operation;
        model
            .openings
            .get_mut(&opening)
            .unwrap()
            .parameters
            .open_state = state;
        for left in [false, true] {
            let lines = ElevationSnapshot::capture(&model, host, left)
                .unwrap()
                .derive()
                .unwrap();
            assert!(
                lines
                    .iter()
                    .any(|s| s.entity == opening && s.surface.material == Some(materials[1]))
            );
            assert!(
                lines
                    .iter()
                    .all(|s| s.start.is_finite() && s.end.is_finite())
            );
            assert!(lines.len() < 100);
        }
    }
    let p = &mut model.opening_types.get_mut(&ty).unwrap().parameters;
    p.kind = OpeningKind::Door;
    p.sill = 0.;
    p.window_operation = WindowOperation::Fixed;
    p.family.door_leaves = DoorLeaves::Paired {
        active_fraction: 0.4,
    };
    model
        .openings
        .get_mut(&opening)
        .unwrap()
        .parameters
        .open_state = OpeningState::DoorPairAngles {
        active_degrees: 23.,
        inactive_degrees: 67.,
    };
    for left in [false, true] {
        let lines = ElevationSnapshot::capture(&model, host, left)
            .unwrap()
            .derive()
            .unwrap();
        assert!(
            lines
                .iter()
                .any(|s| s.entity == opening && s.surface.material == Some(materials[0]))
        );
    }
}

#[test]
fn reversed_host_flips_physical_side_for_asymmetric_posed_door() {
    let (mut model, host, opening, ty) = fixture();
    let p = &mut model.opening_types.get_mut(&ty).unwrap().parameters;
    p.kind = OpeningKind::Door;
    p.sill = 0.;
    model
        .openings
        .get_mut(&opening)
        .unwrap()
        .parameters
        .open_state = OpeningState::DoorAngle(37.);
    let left = ElevationSnapshot::capture(&model, host, true)
        .unwrap()
        .derive()
        .unwrap();
    let right = ElevationSnapshot::capture(&model, host, false)
        .unwrap()
        .derive()
        .unwrap();
    assert!(left.iter().any(|s| !has(
        &right,
        s.entity,
        (s.start.x, s.start.y),
        (s.end.x, s.end.y)
    )));
    // Keep the physical door/void in place while reversing the directed host:
    // its station, hinge and swing must be expressed in the new local frame.
    model.walls.get_mut(&host).unwrap().parameters.path = WallPath::Straight {
        start: Point2::new(8., 0.),
        end: Point2::new(0., 0.),
    };
    let p = &mut model.openings.get_mut(&opening).unwrap().parameters;
    p.offset = 4.;
    p.hinge = DoorHinge::End;
    p.swing = DoorSwing::Right;
    for (new_left, physical_before) in [(true, &right), (false, &left)] {
        let after = ElevationSnapshot::capture(&model, host, new_left)
            .unwrap()
            .derive()
            .unwrap();
        assert_eq!(after.len(), physical_before.len(), "{after:?}");
        for s in physical_before {
            assert!(
                has(
                    &after,
                    s.entity,
                    (8. - s.start.x, s.start.y),
                    (8. - s.end.x, s.end.y)
                ),
                "missing flipped {s:?} in {after:?}"
            );
        }
    }
    // Keeping the same authoring instead makes the asymmetric assembly follow
    // the reversed frame; rotation preserves those local station projections.
    let expected = ElevationSnapshot::capture(&model, host, true)
        .unwrap()
        .derive()
        .unwrap();
    model.walls.get_mut(&host).unwrap().parameters.path = WallPath::Straight {
        start: Point2::new(8.4, 7.8),
        end: Point2::new(2., 3.),
    };
    let rotated = ElevationSnapshot::capture(&model, host, true)
        .unwrap()
        .derive()
        .unwrap();
    assert_eq!(rotated.len(), expected.len());
    for s in &expected {
        assert!(has(
            &rotated,
            s.entity,
            (s.start.x, s.start.y),
            (s.end.x, s.end.y)
        ));
    }
}

#[test]
fn invalid_bounds_coordinates_hosts_and_budgets_fail_explicitly() {
    let (mut model, host, _, ty) = fixture();
    assert!(ElevationSnapshot::capture(&model, Id::new(), true).is_err());
    let snapshot = ElevationSnapshot::capture(&model, host, true).unwrap();
    let bounds = ElevationBounds {
        min_station: 1.,
        max_station: 5.,
        bottom_elevation: 10.5,
        top_elevation: 13.5,
    };
    let lines = snapshot.derive_clipped(bounds).unwrap();
    assert!(
        lines
            .iter()
            .flat_map(|s| [s.start, s.end])
            .all(|p| p.x >= 1. && p.x <= 5. && p.y >= 10.5 && p.y <= 13.5)
    );
    for bad in [
        ElevationBounds {
            min_station: f64::NAN,
            ..bounds
        },
        ElevationBounds {
            max_station: 0.,
            ..bounds
        },
        ElevationBounds {
            top_elevation: 1e7,
            ..bounds
        },
    ] {
        assert!(snapshot.derive_clipped(bad).is_err());
    }
    model
        .opening_types
        .get_mut(&ty)
        .unwrap()
        .parameters
        .family
        .panel_material = Some(Id::new());
    assert!(ElevationSnapshot::capture(&model, host, true).is_err());
    model
        .opening_types
        .get_mut(&ty)
        .unwrap()
        .parameters
        .family
        .panel_material = None;
    model.walls.get_mut(&host).unwrap().parameters.path = WallPath::CircularArc {
        center: Point2::new(0., 0.),
        radius: 8.,
        start_angle_rad: 0.,
        signed_sweep_rad: 1.,
    };
    assert!(matches!(
        ElevationSnapshot::capture(&model, host, true),
        Err(Error::Unsupported(_))
    ));
    let mut invalid = snapshot.clone();
    invalid.wall.elevation = f64::INFINITY;
    assert!(invalid.derive().is_err());
    let mesh = snapshot.wall.mesh().unwrap();
    let meshes = vec![(host, mesh.clone()); MAX_ELEVATION_TRIANGLES / mesh.triangles.len() + 1];
    assert!(project(&meshes, Point2::new(0., 0.), Point2::new(1., 0.), true).is_err());
}
