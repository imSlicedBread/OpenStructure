use super::*;
use os_model::*;
fn wall(model: &mut Model, level: Id, a: Point2, b: Point2, height: f64) -> Id {
    let w = Wall::new(
        "org.openstructure.walls.wall",
        WallParams {
            name: "Wall".into(),
            path: WallPath::Straight { start: a, end: b },
            height,
            thickness: 0.2,
            level,
            material: None,
        },
    );
    let id = w.id();
    model.walls.insert(id, w);
    id
}
fn opening(model: &mut Model, host: Id, kind: OpeningKind, offset: f64) -> Id {
    let ty = OpeningType::new(
        "core.opening_type",
        OpeningTypeParams {
            name: "Framed".into(),
            kind,
            width: 1.,
            height: if kind == OpeningKind::Door { 2.1 } else { 1. },
            sill: if kind == OpeningKind::Door { 0. } else { 1. },
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
    let o = Opening::new(
        "core.opening",
        OpeningParams {
            name: "Opening".into(),
            host,
            offset,
            definition: OpeningDefinition::Typed { type_id },
            open_state: Default::default(),
            width_override: None,
            height_override: None,
            sill_override: None,
            pane_position_override: None,
            lite_side_override: None,
            hinge: Default::default(),
            swing: Default::default(),
        },
    );
    let id = o.id();
    model.openings.insert(id, o);
    id
}
fn fixture() -> (Model, Id, FacadeViewSettings) {
    let m = Model::new("Facade");
    let level = *m.levels.keys().next().unwrap();
    let s = FacadeViewSettings::new(Point2::new(0., 5.), Point2::new(10., 5.), -1., 10.).unwrap();
    (m, level, s)
}
fn lines(m: &Model, level: Id, s: FacadeViewSettings) -> Vec<ElevationSegment> {
    FacadeSnapshot::capture(m, level, s)
        .unwrap()
        .derive()
        .unwrap()
}

#[test]
fn facade_shared_wall_door_window_occlusion_partial_depth_crop_and_reversal() {
    let (mut m, level, mut s) = fixture();
    let back = wall(&mut m, level, Point2::new(0., 0.), Point2::new(8., 0.), 3.);
    let door = opening(&mut m, back, OpeningKind::Door, 1.);
    let window = opening(&mut m, back, OpeningKind::Window, 5.);
    let initial = lines(&m, level, s);
    assert!(initial.iter().any(|l| l.entity == door));
    assert!(initial.iter().any(|l| l.entity == window));
    let front = wall(&mut m, level, Point2::new(0., 2.), Point2::new(8., 2.), 4.);
    let occluded = lines(&m, level, s);
    assert!(occluded.iter().all(|l| l.entity == front), "{occluded:?}");
    m.walls.get_mut(&front).unwrap().parameters.path = WallPath::Straight {
        start: Point2::new(0., 2.),
        end: Point2::new(4., 2.),
    };
    let partial = lines(&m, level, s);
    assert!(!partial.iter().any(|l| l.entity == door));
    assert!(partial.iter().any(|l| l.entity == window));
    assert!(
        partial
            .iter()
            .filter(|l| l.entity == back)
            .all(|l| l.start.x >= 4. - 1e-7 && l.end.x >= 4. - 1e-7)
    );
    s.near_depth = 4.; // the closer wall is outside the view volume, not an occluder
    assert_eq!(lines(&m, level, s), initial);
    s.far_depth = 4.5;
    assert!(
        lines(&m, level, s).iter().all(|l| l.entity == door),
        "only the open door leaf reaches this depth interval"
    );
    s.near_depth = 6.;
    s.far_depth = 7.;
    assert!(lines(&m, level, s).is_empty());
    s.near_depth = 0.;
    s.far_depth = 30.;
    s.min_station = 4.5;
    s.max_station = 7.;
    s.bottom_elevation = 0.5;
    s.top_elevation = 2.5;
    let cropped = lines(&m, level, s);
    assert!(
        cropped
            .iter()
            .all(|l| l.start.x >= 4.5 && l.end.x <= 7. && l.start.y >= 0.5 && l.end.y <= 2.5)
    );
    assert!(
        !cropped
            .iter()
            .any(|l| (l.start.x - 4.5).abs() < 1e-7 && (l.end.x - 4.5).abs() < 1e-7),
        "crop must not synthesize a boundary"
    );
    // A wall endpoint reversal cannot rotate a saved world basis.
    m.openings.clear();
    s.min_station = 0.;
    s.max_station = 10.;
    s.bottom_elevation = -1.;
    s.top_elevation = 10.;
    let before = lines(&m, level, s);
    for w in m.walls.values_mut() {
        let a = w.parameters.start();
        let b = w.parameters.end();
        w.parameters.path = WallPath::Straight { start: b, end: a };
    }
    let after = lines(&m, level, s);
    assert_eq!(before, after);
}

#[test]
fn facade_multistorey_slab_roof_materials_and_rotated_world_axis() {
    let (mut m, level, s) = fixture();
    let building = m.levels[&level].parameters.building;
    let upper = Level::new(
        "core.level",
        LevelParams {
            name: "Upper".into(),
            elevation: 4.,
            building,
        },
    );
    let upper_id = upper.id();
    m.levels.insert(upper_id, upper);
    let lower = wall(&mut m, level, Point2::new(0., 0.), Point2::new(8., 0.), 3.);
    let upper_wall = wall(
        &mut m,
        upper_id,
        Point2::new(0., 0.),
        Point2::new(8., 0.),
        3.,
    );
    let material = Material::new(
        "core.material",
        MaterialParams {
            name: "Stone".into(),
            density_kg_m3: 2200.,
            color: [120, 130, 140],
        },
    );
    let mat = material.id();
    m.materials.insert(mat, material);
    let ring = vec![
        Point2::new(0., -1.),
        Point2::new(8., -1.),
        Point2::new(8., 1.),
        Point2::new(0., 1.),
    ];
    let floor = Floor::new(
        "core.floor",
        FloorParams {
            name: "Slab".into(),
            level: upper_id,
            boundary: ring.clone(),
            holes: vec![],
            thickness: 0.3,
            top_offset: 0.,
            material: Some(mat),
        },
    );
    let slab = floor.id();
    m.floors.insert(slab, floor);
    // Roof constructor fields are kept explicit so the snapshot covers the native plane.
    let roof = RoofParams {
        name: "Roof".into(),
        level: upper_id,
        boundary: ring,
        holes: vec![],
        material: Some(mat),
        thickness: 0.25,
        top_offset: 3.2,
        slope_start: Point2::new(0., 0.),
        slope_end: Point2::new(8., 0.),
        rise_per_run: 0.05,
    };
    let roof = Roof::new("core.roof", roof);
    let roof_id = roof.id();
    m.roofs.insert(roof_id, roof);
    let result = lines(&m, level, s);
    assert!(result.iter().any(|l| l.entity == lower));
    assert!(
        result
            .iter()
            .any(|l| l.entity == upper_wall && l.start.y >= 4.)
    );
    assert!(
        result
            .iter()
            .any(|l| l.entity == slab && l.surface.material == Some(mat))
    );
    assert!(
        result
            .iter()
            .any(|l| l.entity == roof_id && l.surface.material == Some(mat))
    );
    let angle = 0.63_f64;
    let (sin, cos) = angle.sin_cos();
    let rotate = |p: Point2| Point2::new(20. + cos * p.x - sin * p.y, -12. + sin * p.x + cos * p.y);
    for w in m.walls.values_mut() {
        w.parameters.path = WallPath::Straight {
            start: rotate(w.parameters.start()),
            end: rotate(w.parameters.end()),
        };
    }
    for f in m.floors.values_mut() {
        for p in &mut f.parameters.boundary {
            *p = rotate(*p);
        }
    }
    for r in m.roofs.values_mut() {
        for p in &mut r.parameters.boundary {
            *p = rotate(*p);
        }
        r.parameters.slope_start = rotate(r.parameters.slope_start);
        r.parameters.slope_end = rotate(r.parameters.slope_end);
    }
    let mut rotated = s;
    rotated.origin = rotate(s.origin);
    rotated.axis = Point2::new(cos, sin);
    let actual = lines(&m, level, rotated);
    assert_eq!(actual.len(), result.len());
    for expected in result {
        assert!(
            actual.iter().any(|l| l.entity == expected.entity
                && l.surface == expected.surface
                && l.start.distance(expected.start) < 1e-7
                && l.end.distance(expected.end) < 1e-7),
            "missing {expected:?}"
        );
    }
    let front = wall(
        &mut m,
        level,
        rotate(Point2::new(0., 2.)),
        rotate(Point2::new(8., 2.)),
        8.,
    );
    assert!(
        lines(&m, level, rotated).iter().all(|l| l.entity == front),
        "foreground wall must hide slabs/roof as well as walls"
    );
}

#[test]
fn facade_slab_and_roof_are_shared_occluders_not_independent_overlays() {
    for roof in [false, true] {
        let (mut m, level, s) = fixture();
        let host = wall(&mut m, level, Point2::new(0., 0.), Point2::new(8., 0.), 3.);
        let door = opening(&mut m, host, OpeningKind::Door, 1.);
        let ring = vec![
            Point2::new(0., 2.),
            Point2::new(8., 2.),
            Point2::new(8., 3.),
            Point2::new(0., 3.),
        ];
        let id = if roof {
            let e = Roof::new(
                "core.roof",
                RoofParams {
                    name: "Occluder".into(),
                    level,
                    boundary: ring,
                    holes: vec![],
                    material: None,
                    thickness: 1.,
                    top_offset: 1.6,
                    slope_start: Point2::new(0., 0.),
                    slope_end: Point2::new(1., 0.),
                    rise_per_run: 0.,
                },
            );
            let id = e.id();
            m.roofs.insert(id, e);
            id
        } else {
            let e = Floor::new(
                "core.floor",
                FloorParams {
                    name: "Occluder".into(),
                    level,
                    boundary: ring,
                    holes: vec![],
                    material: None,
                    thickness: 1.,
                    top_offset: 1.6,
                },
            );
            let id = e.id();
            m.floors.insert(id, e);
            id
        };
        let visible = lines(&m, level, s);
        assert!(visible.iter().any(|l| l.entity == id));
        assert!(visible.iter().any(|l| l.entity == door));
        for line in visible
            .iter()
            .filter(|l| l.entity == host || l.entity == door)
        {
            let mid = (line.start.y + line.end.y) * 0.5;
            assert!(
                mid <= 0.6 + 1e-7 || mid >= 1.6 - 1e-7,
                "occluded wall/opening edge leaked: {line:?}"
            );
        }
    }
}

#[test]
fn facade_aggregate_budget_and_unsupported_inputs_fail_without_truncation() {
    let (mut m, level, s) = fixture();
    for i in 0..350 {
        wall(
            &mut m,
            level,
            Point2::new(0., -(i as f64) * 0.01),
            Point2::new(8., -(i as f64) * 0.01),
            3.,
        );
    }
    assert!(
        FacadeSnapshot::capture(&m, level, s)
            .err()
            .unwrap()
            .to_string()
            .contains("aggregate")
    );
    let (mut m, level, s) = fixture();
    let id = wall(&mut m, level, Point2::new(0., 0.), Point2::new(8., 0.), 3.);
    m.walls.get_mut(&id).unwrap().parameters.path = WallPath::CircularArc {
        center: Point2::new(0., 0.),
        radius: 4.,
        start_angle_rad: 0.,
        signed_sweep_rad: 1.,
    };
    assert!(
        FacadeSnapshot::capture(&m, level, s)
            .err()
            .unwrap()
            .to_string()
            .contains("unsupported")
    );
}
