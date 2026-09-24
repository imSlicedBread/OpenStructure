use os_core::{Id, Point2};
use os_geometry::{plan::*, section::*, walls::*};
use os_model::*;

fn wall(profile: Vec<Point2>, reverse: bool) -> NativeWall {
    let id = Id::new();
    let parameters = WallParams {
        name: "Host".into(),
        start: Point2::new(4., 3.),
        end: Point2::new(4., if reverse { -3. } else { 9. }),
        thickness: 0.3,
        height: 3.,
        level: Id::new(),
        material: None,
    };
    let family = OpeningFamily {
        host_cut: OpeningHostCut::Profile,
        profile: profile.clone(),
        cut_profile: profile,
        ..Default::default()
    };
    let opening = ResolvedOpening {
        family,
        name: "Cut".into(),
        host: id,
        offset: 2.,
        kind: OpeningKind::Window,
        width: 2.,
        height: 2.,
        sill: 0.5,
        pane_position: Default::default(),
        type_id: None,
        type_name: None,
        hinge: Default::default(),
        swing: Default::default(),
    };
    NativeWall {
        entity: id,
        parameters,
        elevation: 10.,
        openings: vec![opening],
        stations: (0., 6.),
        interfaces: vec![],
        layers: vec![
            ResolvedWallLayer {
                id: Some(Id::new()),
                name: "Outer".into(),
                function: LayerFunction::Finish,
                material: Some(Id::new()),
                density_kg_m3: Some(500.),
                min: -0.15,
                max: -0.05,
            },
            ResolvedWallLayer {
                id: Some(Id::new()),
                name: "Core".into(),
                function: LayerFunction::Structure,
                material: Some(Id::new()),
                density_kg_m3: Some(2000.),
                min: -0.05,
                max: 0.15,
            },
        ],
    }
}

#[test]
fn triangle_arch_mesh_plan_section_layer_volume_mass_and_reversed_axes() {
    for (profile, area) in [
        (
            vec![
                Point2::new(0., 0.),
                Point2::new(1., 0.),
                Point2::new(0.5, 1.),
            ],
            0.5,
        ),
        (
            [
                (0., 0.),
                (1., 0.),
                (1., 0.6),
                (0.8, 0.9),
                (0.5, 1.),
                (0.2, 0.9),
                (0., 0.6),
            ]
            .map(|(x, y)| Point2::new(x, y))
            .to_vec(),
            0.87,
        ),
    ] {
        for reverse in [false, true] {
            for clockwise in [false, true] {
                let mut profile = profile.clone();
                if clockwise {
                    profile.reverse();
                }
                let w = wall(profile, reverse);
                let expected_area = 18. - 4. * area;
                let mesh = w.mesh().unwrap();
                assert!((mesh.signed_volume() - expected_area * 0.3).abs() < 1e-9);
                assert!((w.net_volume().unwrap() - mesh.signed_volume()).abs() < 1e-9);
                assert!(matches!(w.cells(), Err(os_core::Error::Unsupported(_))));
                assert!(matches!(
                    wall_prisms(&w.parameters, w.elevation, &w.openings),
                    Err(os_core::Error::Unsupported(_))
                ));
                for (layer, q) in w.layers.iter().zip(w.layer_quantities().unwrap()) {
                    assert_eq!(q.layer, layer.id);
                    assert_eq!(q.material, layer.material);
                    assert!((q.volume_m3 - expected_area * (layer.max - layer.min)).abs() < 1e-10);
                    assert!(
                        (q.mass_kg.unwrap() - q.volume_m3 * layer.density_kg_m3.unwrap()).abs()
                            < 1e-10
                    );
                    assert!(
                        mesh.surfaces
                            .iter()
                            .any(|s| s.layer == layer.id && s.material == layer.material)
                    );
                }
                let direction = Point2::new(0., if reverse { -1. } else { 1. });
                for (_, regions) in w.layer_mesh_regions().unwrap() {
                    let mut section_area = 0.;
                    for region in regions {
                        // Section lies strictly inside both layer regions only
                        // after selecting each layer's midpoint below.
                        let origin = Point2::new(
                            region.vertices.iter().map(|v| v.x).sum::<f64>()
                                / region.vertices.len() as f64,
                            3.,
                        );
                        for contour in
                            vertical_section(&region, VerticalSectionPlane { origin, direction })
                                .unwrap()
                        {
                            section_area += os_geometry::floors::signed_area(&contour.points).abs();
                        }
                    }
                    assert!((section_area - expected_area).abs() < 1e-8);
                }
                for height in [0.75, 1.5, 2.25, 2.75] {
                    let range = PlanRange {
                        top: 13.5,
                        cut: 10. + height,
                        bottom: 10.,
                        depth: 9.,
                    };
                    let footprints = w
                        .layer_plan_footprints(range, HorizontalBasis::default(), None)
                        .unwrap();
                    let spans = os_geometry::openings::cut_plan_spans(
                        &w.openings[0],
                        10.,
                        range.cut,
                        range.depth,
                    )
                    .unwrap();
                    let void_width = if height < 2.5 {
                        spans.iter().map(|(a, b)| (b - a) * 2.).sum::<f64>()
                    } else {
                        0.
                    };
                    for (layer, parts) in footprints {
                        let cut_area: f64 = parts
                            .iter()
                            .filter(|f| f.role == PlanRole::Cut)
                            .map(PlanFootprint::area)
                            .sum();
                        assert!(
                            (cut_area - (6. - void_width) * (layer.max - layer.min)).abs() < 1e-8,
                            "height {height}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn rectangular_defaults_and_equivalent_winding_keep_the_original_mesh_exactly() {
    let mut w = wall(OpeningFamily::rectangle(), false);
    let original = w.mesh().unwrap();
    let cells = w.cells().unwrap();
    w.openings[0].family.host_cut = OpeningHostCut::Rectangular;
    assert_eq!(w.mesh().unwrap(), original);
    w.openings[0].family.cut_profile.reverse();
    assert_eq!(w.mesh().unwrap(), original);
    assert_eq!(w.cells().unwrap(), cells);
}

#[test]
fn profile_cut_plan_footprints_preserve_opening_void_crop_and_pick_geometry() {
    let w = wall(
        vec![
            Point2::new(0., 0.),
            Point2::new(1., 0.),
            Point2::new(0.5, 1.),
        ],
        false,
    );
    let basis = HorizontalBasis {
        origin: Point2::new(2., 1.),
        rotation: 0.37,
    };
    let range = PlanRange {
        top: 13.5,
        cut: 11.5,
        bottom: 10.,
        depth: 9.,
    };
    let view_point = |station, transverse| {
        basis
            .world_to_plane(os_geometry::openings::world(
                &w.parameters,
                station,
                transverse,
            ))
            .unwrap()
    };
    let opening_midpoint = view_point(3., 0.);
    let wall_point = view_point(1., 0.);
    let uncut: Vec<_> = w
        .layer_plan_footprints(range, basis, None)
        .unwrap()
        .into_iter()
        .flat_map(|(_, footprints)| footprints)
        .filter(|footprint| footprint.role == PlanRole::Cut)
        .collect();
    assert!(!uncut.is_empty());
    assert!(
        uncut
            .iter()
            .all(|footprint| !footprint.contains(opening_midpoint)),
        "the midpoint of the triangular cut stays empty in plan and picking"
    );
    assert!(uncut.iter().any(|footprint| footprint.contains(wall_point)));

    let crop = PlanCrop {
        min: Point2::new(wall_point.x - 0.1, wall_point.y - 0.1),
        max: Point2::new(wall_point.x + 0.1, wall_point.y + 0.1),
    };
    let cropped: Vec<_> = w
        .layer_plan_footprints(range, basis, Some(crop))
        .unwrap()
        .into_iter()
        .flat_map(|(_, footprints)| footprints)
        .filter(|footprint| footprint.role == PlanRole::Cut)
        .collect();
    assert!(
        cropped
            .iter()
            .any(|footprint| footprint.contains(wall_point))
    );
    assert!(cropped.iter().all(|footprint| {
        footprint.vertices().iter().all(|point| {
            point.x >= crop.min.x - 1e-9
                && point.x <= crop.max.x + 1e-9
                && point.y >= crop.min.y - 1e-9
                && point.y <= crop.max.y + 1e-9
        })
    }));
}

#[test]
fn profile_cut_mesh_and_sections_support_join_trimmed_wall_members() {
    let mut w = wall(
        vec![
            Point2::new(0., 0.),
            Point2::new(1., 0.),
            Point2::new(0.5, 1.),
        ],
        false,
    );
    w.stations = (0.2, 5.8);
    // The start-end contact is outside the opening clearance. This is the
    // vertical trace of a joined butt face across the wall thickness.
    w.interfaces = vec![(
        os_geometry::openings::world(&w.parameters, w.stations.0, -0.15),
        os_geometry::openings::world(&w.parameters, w.stations.0, 0.15),
    )];

    let expected_area = 5.6 * 3. - 2.;
    let expected_volume = expected_area * 0.3;
    assert!((w.net_volume().unwrap() - expected_volume).abs() < 1e-9);
    w.mesh().unwrap().validate().unwrap();
    for (_, regions) in w.layer_mesh_regions().unwrap() {
        assert!(!regions.is_empty());
        for region in regions {
            region.validate().unwrap();
        }
    }
}

#[test]
fn profile_cut_rejects_invalid_station_limits_instead_of_emitting_geometry() {
    let mut w = wall(
        vec![
            Point2::new(0., 0.),
            Point2::new(1., 0.),
            Point2::new(0.5, 1.),
        ],
        false,
    );
    w.stations = (0., w.parameters.length() + w.parameters.thickness);
    assert!(w.mesh().is_err());
    assert!(w.net_volume().is_err());
    assert!(
        w.layer_plan_footprints(PlanRange::default(), HorizontalBasis::default(), None)
            .is_err()
    );
}
