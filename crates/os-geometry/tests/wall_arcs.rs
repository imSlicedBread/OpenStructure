use os_core::Point2;
use os_model::*;
use std::f64::consts::PI;

fn aperture(sweep: f64, kind: OpeningKind) -> os_geometry::walls::NativeWall {
    let mut model = Model::new("Arc apertures");
    let level = *model.levels.keys().next().unwrap();
    model.levels.get_mut(&level).unwrap().parameters.elevation = 7.;
    let wall = Wall::new(
        "org.openstructure.walls.wall",
        WallParams {
            name: "Arc".into(),
            path: WallPath::CircularArc {
                center: Point2::new(3., -2.),
                radius: 4.,
                start_angle_rad: 0.3,
                signed_sweep_rad: sweep,
            },
            thickness: 0.3,
            height: 3.,
            level,
            material: None,
        },
    );
    let host = wall.id();
    model.walls.insert(host, wall);
    let mut layers = Vec::new();
    for (name, thickness, density) in [("Finish", 0.1, 500.), ("Core", 0.2, 2000.)] {
        let material = Material::new(
            "core.material",
            MaterialParams {
                name: name.into(),
                density_kg_m3: density,
                color: [100, 120, 140],
            },
        );
        let id = material.id();
        model.materials.insert(id, material);
        layers.push(WallLayer {
            id: os_core::Id::new(),
            name: name.into(),
            thickness,
            function: LayerFunction::Structure,
            material: Some(id),
        });
    }
    let ty = WallType::new(
        "core.wall_type",
        WallTypeParams {
            name: "Layered".into(),
            layers,
        },
    );
    let type_id = ty.id();
    model.wall_types.insert(type_id, ty);
    model.wall_type_assignments.insert(
        host,
        WallTypeAssignment {
            type_id,
            flipped: false,
        },
    );
    let opening = Opening::new(
        "core.opening",
        OpeningParams {
            open_state: Default::default(),
            name: "Aperture".into(),
            host,
            offset: 1.,
            definition: OpeningDefinition::Legacy {
                kind,
                width: 1.2,
                height: 1.8,
                sill: if kind == OpeningKind::Door { 0. } else { 0.6 },
            },
            width_override: None,
            height_override: None,
            sill_override: None,
            pane_position_override: None,
            lite_side_override: None,
            hinge: DoorHinge::Start,
            swing: DoorSwing::Left,
        },
    );
    model.openings.insert(opening.id(), opening);
    model.validate().unwrap();
    os_geometry::walls::NativeWall::from_model(&model, host).unwrap()
}

fn assert_closed(mesh: &os_geometry::Mesh) {
    let mut edges = std::collections::BTreeMap::new();
    for t in &mesh.triangles {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            *edges.entry((a, b)).or_insert(0) += 1;
        }
    }
    for (&(a, b), &n) in &edges {
        assert_eq!(n, 1, "duplicate edge {a} {b}");
        assert_eq!(edges.get(&(b, a)), Some(&1), "open edge {a} {b}");
    }
    assert!(mesh.signed_volume() > 0.);
}

#[test]
fn multiple_and_concave_arc_cuts_keep_closed_shells_and_reject_overlaps_and_joins() {
    use os_geometry::section::*;
    for sweep in [1.5 * PI, -1.5 * PI] {
        let mut w = aperture(sweep, OpeningKind::Window);
        let mut second = w.openings[0].clone();
        second.offset = 3.;
        second.family.host_cut = OpeningHostCut::Profile;
        second.family.cut_profile = [
            (0., 0.),
            (1., 0.),
            (1., 1.),
            (0.6, 1.),
            (0.6, 0.4),
            (0.4, 0.4),
            (0.4, 1.),
            (0., 1.),
        ]
        .map(|(x, y)| Point2::new(x, y))
        .to_vec();
        second.family.profile = second.family.cut_profile.clone();
        w.openings.push(second);
        let mesh = w.mesh().unwrap();
        assert_closed(&mesh);
        let exact = (w.parameters.length() * 3. - 1.2 * 1.8 * (1. + 0.88)) * 0.3;
        assert!((w.net_volume().unwrap() - exact).abs() < 1e-11);
        let component =
            os_geometry::openings::component_mesh(&w.openings[1], &w.parameters, w.elevation)
                .unwrap();
        assert_closed(&component);
        for s in [1., 1.6, 2.2, 3.24, 3.6, 3.96] {
            let t = w.parameters.path.tangent(s);
            let plane = VerticalSectionPlane {
                origin: w.parameters.path.point(s),
                direction: Point2::new(-t.y, t.x),
            };
            for (layer, meshes) in w.layer_mesh_regions().unwrap() {
                for mesh in meshes {
                    vertical_section(&mesh, plane).unwrap_or_else(|e| {
                        panic!("sweep {sweep} station {s} layer {}: {e}", layer.name)
                    });
                }
            }
        }
        w.openings[1].offset = 2.2005;
        assert!(w.mesh().is_err());
        assert!(w.net_volume().is_err());
        assert!(
            w.layer_plan_footprints(Default::default(), Default::default(), None)
                .is_err()
        );
        w.openings[1].offset = 3.;
        w.interfaces
            .push((w.parameters.path.start(), w.parameters.path.end()));
        assert!(w.mesh().is_err());
        assert!(w.net_volume().is_err());
        w.interfaces.clear();
        w.stations.0 = 0.01;
        assert!(w.mesh().is_err());
    }
}

#[test]
fn arc_window_alignment_and_rotated_cropped_plan_keep_radial_bounds() {
    use os_geometry::plan::*;
    for sweep in [PI / 2., -PI / 2.] {
        let w = aperture(sweep, OpeningKind::Window);
        for position in [
            WindowPanePosition::Center,
            WindowPanePosition::LeftFace,
            WindowPanePosition::RightFace,
        ] {
            let mut p = w.openings[0].clone();
            p.pane_position = position;
            p.family.frame_width = 0.04;
            let material = os_core::Id::new();
            p.family.panel_material = Some(material);
            let mesh =
                os_geometry::openings::component_mesh(&p, &w.parameters, w.elevation).unwrap();
            assert_closed(&mesh);
            let y = os_geometry::openings::pane_offset(&p, &w.parameters);
            let d = os_geometry::openings::depth(&p, &w.parameters);
            for (t, surface) in mesh.triangles.iter().zip(&mesh.surfaces) {
                if surface.material != Some(material) {
                    continue;
                }
                for &i in t {
                    let v = mesh.vertices[i as usize];
                    let radius = Point2::new(v.x, v.y).distance(Point2::new(3., -2.));
                    let offset = (4. - radius) * sweep.signum();
                    assert!(((offset - y).abs() - d / 2.).abs() < 1e-10);
                    let station = w.parameters.path.project(Point2::new(v.x, v.y));
                    assert!(
                        station >= p.offset + 0.04 - 1e-10
                            && station <= p.offset + p.width - 0.04 + 1e-10
                    );
                }
            }
        }
        let basis = HorizontalBasis {
            origin: Point2::new(1., 2.),
            rotation: 0.4,
        };
        let pick = basis.world_to_plane(w.parameters.path.point(0.5)).unwrap();
        let crop = PlanCrop {
            min: Point2::new(pick.x - 0.3, pick.y - 0.3),
            max: Point2::new(pick.x + 0.3, pick.y + 0.3),
        };
        let parts = w
            .layer_plan_footprints(
                PlanRange {
                    top: 11.,
                    cut: 8.5,
                    bottom: 7.,
                    depth: 6.,
                },
                basis,
                Some(crop),
            )
            .unwrap();
        assert!(
            parts
                .iter()
                .flat_map(|(_, parts)| parts)
                .any(|f| f.contains(pick))
        );
        for f in parts.iter().flat_map(|(_, parts)| parts) {
            assert!(f.vertices().iter().all(|p| p.x >= crop.min.x - 1e-9
                && p.x <= crop.max.x + 1e-9
                && p.y >= crop.min.y - 1e-9
                && p.y <= crop.max.y + 1e-9));
        }
    }
}

#[test]
fn arc_apertures_remove_layer_volume_plan_and_section_material() {
    use os_geometry::{plan::*, section::*};
    for sweep in [PI / 2., -PI / 2., 1.5 * PI, -1.5 * PI] {
        for kind in [OpeningKind::Door, OpeningKind::Window] {
            let w = aperture(sweep, kind);
            let p = &w.openings[0];
            let mesh = w.mesh().unwrap();
            assert_closed(&mesh);
            let net_area = w.parameters.length() * 3. - p.width * p.height;
            for (layer, q) in w.layers.iter().zip(w.layer_quantities().unwrap()) {
                let exact = net_area
                    * (layer.max - layer.min)
                    * (1. - sweep.signum() * (layer.min + layer.max) / 8.);
                assert!((q.volume_m3 - exact).abs() < 1e-11);
                assert_eq!(q.layer, layer.id);
                assert_eq!(q.material, layer.material);
                assert!((q.mass_kg.unwrap() - exact * layer.density_kg_m3.unwrap()).abs() < 1e-8);
                assert!(
                    mesh.surfaces
                        .iter()
                        .any(|s| s.layer == layer.id && s.material == layer.material)
                );
            }
            assert_eq!(mesh.surfaces.len(), mesh.triangles.len());
            assert!(
                (mesh.signed_volume() - w.net_volume().unwrap()).abs() / w.net_volume().unwrap()
                    < 0.001
            );
            // A radial section through the aperture must have no material at
            // its middle height, but must retain the head (and window sill).
            let station = p.offset + p.width * 0.5;
            let origin = w.parameters.path.point(station);
            let tangent = w.parameters.path.tangent(station);
            let plane = VerticalSectionPlane {
                origin,
                direction: Point2::new(-tangent.y, tangent.x),
            };
            for layer in &w.layers {
                let mut part = w.clone();
                part.layers = vec![layer.clone()];
                let shell = part.mesh().unwrap();
                // The complete render shell must pass vertex-fan manifold
                // validation too, not merely have paired triangle edges.
                vertical_section(&shell, plane).unwrap();
                let euler = shell.vertices.len() as isize - shell.triangles.len() as isize / 2;
                assert_eq!(euler, if kind == OpeningKind::Window { 0 } else { 2 });
                for triangle in &shell.triangles {
                    for (a, b) in [
                        (triangle[0], triangle[1]),
                        (triangle[1], triangle[2]),
                        (triangle[2], triangle[0]),
                    ] {
                        let a = shell.vertices[a as usize];
                        let b = shell.vertices[b as usize];
                        let a = Point2::new(a.x, a.y);
                        let b = Point2::new(b.x, b.y);
                        let center = Point2::new(3., -2.);
                        let r = a.distance(center);
                        if (r - b.distance(center)).abs() < 1e-10 {
                            let midpoint = Point2::new((a.x + b.x) / 2., (a.y + b.y) / 2.);
                            assert!(r - midpoint.distance(center) <= WALL_DISPLAY_SAGITTA + 1e-10);
                        }
                    }
                }
            }
            for (_, regions) in w.layer_mesh_regions().unwrap() {
                let mut contours = Vec::new();
                for region in regions {
                    assert_closed(&region);
                    contours.extend(vertical_section(&region, plane).unwrap());
                }
                let mut local_area = 0.;
                for c in contours
                    .iter()
                    .filter(|c| c.points.iter().all(|v| v.x.abs() < 0.3))
                {
                    let lo = c.points.iter().map(|v| v.y).fold(f64::INFINITY, f64::min);
                    let hi = c
                        .points
                        .iter()
                        .map(|v| v.y)
                        .fold(f64::NEG_INFINITY, f64::max);
                    assert!(
                        hi <= w.elevation + p.sill + 1e-8
                            || lo >= w.elevation + p.sill + p.height - 1e-8
                    );
                    local_area += os_geometry::floors::signed_area(&c.points).abs();
                }
                assert!(local_area > 0.);
            }
            let range = PlanRange {
                top: 11.,
                cut: w.elevation + p.sill + 0.9,
                bottom: w.elevation,
                depth: w.elevation - 1.,
            };
            let footprints = w
                .layer_plan_footprints(range, Default::default(), None)
                .unwrap();
            for (layer, parts) in footprints {
                let y = (layer.min + layer.max) / 2.;
                let hole = w.parameters.path.offset_point(station, y);
                let solid = w.parameters.path.offset_point(0.5, y);
                assert!(
                    !parts
                        .iter()
                        .any(|f| f.role == PlanRole::Cut && f.contains(hole))
                );
                assert!(
                    parts
                        .iter()
                        .any(|f| f.role == PlanRole::Cut && f.contains(solid))
                );
                assert_eq!(
                    parts
                        .iter()
                        .any(|f| f.role == PlanRole::Projected && f.contains(hole)),
                    kind == OpeningKind::Window
                );
            }
        }
    }
}

#[test]
fn arc_profile_cuts_are_authored_not_rectangular_and_quantities_ignore_tessellation() {
    for sweep in [PI / 2., -PI / 2., 1.5 * PI, -1.5 * PI] {
        for reverse in [false, true] {
            let mut w = aperture(sweep, OpeningKind::Window);
            let mut profile = vec![
                Point2::new(0., 0.),
                Point2::new(1., 0.),
                Point2::new(0.5, 1.),
            ];
            if reverse {
                profile.reverse();
            }
            w.openings[0].family.host_cut = OpeningHostCut::Profile;
            w.openings[0].family.cut_profile = profile.clone();
            w.openings[0].family.profile = profile;
            let mesh = w.mesh().unwrap();
            assert_closed(&mesh);
            let expected = (w.parameters.length() * 3. - 1.2 * 1.8 * 0.5) * 0.3;
            assert!((w.net_volume().unwrap() - expected).abs() < 1e-11);
            let pane =
                os_geometry::openings::component_mesh(&w.openings[0], &w.parameters, w.elevation)
                    .unwrap();
            assert_closed(&pane);
            let parts = w
                .layer_plan_footprints(
                    os_geometry::plan::PlanRange {
                        top: 11.,
                        cut: 8.5,
                        bottom: 7.,
                        depth: 6.,
                    },
                    Default::default(),
                    None,
                )
                .unwrap();
            for (layer, parts) in parts {
                let y = (layer.min + layer.max) / 2.;
                for (station, filled) in [(1.1, true), (1.6, false), (2.1, true)] {
                    let point = w.parameters.path.offset_point(station, y);
                    assert_eq!(
                        parts.iter().any(
                            |f| f.role == os_geometry::plan::PlanRole::Cut && f.contains(point)
                        ),
                        filled
                    );
                }
            }
        }
    }
}

#[test]
fn single_arc_door_leaf_preserves_authored_profile_and_euclidean_volume() {
    for sweep in [PI / 2., -PI / 2., 1.5 * PI, -1.5 * PI] {
        let w = aperture(sweep, OpeningKind::Door);
        for frame_width in [0., 0.05] {
            for hinge in [DoorHinge::Start, DoorHinge::End] {
                for swing in [DoorSwing::Left, DoorSwing::Right] {
                    let mut p = w.openings[0].clone();
                    let material = os_core::Id::new();
                    p.family.panel_material = Some(material);
                    p.family.frame_width = frame_width;
                    p.family.profile = vec![
                        Point2::new(0., 0.),
                        Point2::new(1., 0.),
                        Point2::new(0.5, 1.),
                    ];
                    p.hinge = hinge;
                    p.swing = swing;
                    let mesh =
                        os_geometry::openings::component_mesh(&p, &w.parameters, w.elevation)
                            .unwrap();
                    assert_closed(&mesh);
                    let leaf = os_geometry::Mesh {
                        vertices: mesh.vertices.clone(),
                        triangles: mesh
                            .triangles
                            .iter()
                            .zip(&mesh.surfaces)
                            .filter(|(_, s)| s.material == Some(material))
                            .map(|(t, _)| *t)
                            .collect(),
                        surfaces: vec![],
                    };
                    let expected = 0.5
                        * (p.width - 2. * frame_width)
                        * (p.height - frame_width)
                        * os_geometry::openings::depth(&p, &w.parameters);
                    assert!((leaf.signed_volume() - expected).abs() < 1e-10);
                    assert_eq!(leaf.triangles.len(), 8);
                }
            }
        }
    }
}

#[test]
fn arc_components_have_curved_panes_frames_lites_and_rigid_leaves() {
    for sweep in [PI / 2., -PI / 2., 1.5 * PI, -1.5 * PI] {
        for kind in [OpeningKind::Door, OpeningKind::Window] {
            for side in [LiteSide::Start, LiteSide::End] {
                for hinge in [DoorHinge::Start, DoorHinge::End] {
                    for swing in [DoorSwing::Left, DoorSwing::Right] {
                        let w = aperture(sweep, kind);
                        let mut p = w.openings[0].clone();
                        p.hinge = hinge;
                        p.swing = swing;
                        let panel = os_core::Id::new();
                        let lite = os_core::Id::new();
                        let frame = os_core::Id::new();
                        p.family.frame_width = 0.05;
                        p.family.panel_material = Some(panel);
                        p.family.frame_material = Some(frame);
                        p.family.side_lite = Some(SideLite {
                            side,
                            width_fraction: 0.25,
                            mullion_width: 0.05,
                            material: Some(lite),
                        });
                        let mesh =
                            os_geometry::openings::component_mesh(&p, &w.parameters, w.elevation)
                                .unwrap();
                        assert_closed(&mesh);
                        for material in [panel, lite, frame] {
                            assert!(
                                mesh.surfaces
                                    .iter()
                                    .any(|s| s.material == Some(material) && s.layer.is_none())
                            );
                        }
                        let primary = os_geometry::openings::primary_bay(&p).unwrap();
                        let station = primary.offset
                            + if hinge == DoorHinge::End {
                                primary.width
                            } else {
                                0.
                            };
                        let t = w.parameters.path.tangent(station);
                        let h = w.parameters.path.point(station);
                        let panel_vertices: Vec<_> = mesh
                            .triangles
                            .iter()
                            .zip(&mesh.surfaces)
                            .filter(|(_, s)| s.material == Some(panel))
                            .flat_map(|(t, _)| t.iter().map(|&i| mesh.vertices[i as usize]))
                            .collect();
                        if kind == OpeningKind::Door {
                            assert_eq!(panel_vertices.len(), 36); // rigid eight-vertex prism
                            let xs: Vec<_> = panel_vertices
                                .iter()
                                .map(|v| (v.x - h.x) * t.x + (v.y - h.y) * t.y)
                                .collect();
                            let span = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max)
                                - xs.iter().copied().fold(f64::INFINITY, f64::min);
                            assert!(
                                (span - os_geometry::openings::depth(&p, &w.parameters)).abs()
                                    < 1e-10
                            );
                            let ys: Vec<_> = panel_vertices
                                .iter()
                                .map(|v| -(v.x - h.x) * t.y + (v.y - h.y) * t.x)
                                .collect();
                            let span = ys.iter().copied().fold(f64::NEG_INFINITY, f64::max)
                                - ys.iter().copied().fold(f64::INFINITY, f64::min);
                            assert!((span - primary.width).abs() < 1e-10);
                        } else {
                            assert!(panel_vertices.len() > 36);
                        }
                        for (triangle, surface) in mesh.triangles.iter().zip(&mesh.surfaces) {
                            if surface.material == Some(panel) && kind == OpeningKind::Door {
                                continue;
                            }
                            let d = if surface.material == Some(frame) {
                                os_geometry::openings::frame_thickness(&p, &w.parameters)
                            } else {
                                os_geometry::openings::depth(&p, &w.parameters)
                            };
                            for &i in triangle {
                                let v = mesh.vertices[i as usize];
                                let radius = Point2::new(v.x, v.y).distance(Point2::new(3., -2.));
                                assert!(((radius - 4.).abs() - d / 2.).abs() < 1e-10);
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn annular_mesh_is_closed_bounded_and_quantities_are_analytic() {
    for sweep in [PI / 2.0, -PI / 2.0, 1.5 * PI, -1.5 * PI] {
        let mut model = Model::new("Arc geometry");
        let wall = Wall::new(
            "org.openstructure.walls.wall",
            WallParams {
                name: "Arc".into(),
                path: WallPath::CircularArc {
                    center: Point2::default(),
                    radius: 4.0,
                    start_angle_rad: 0.0,
                    signed_sweep_rad: sweep,
                },
                thickness: 0.2,
                height: 3.0,
                level: *model.levels.keys().next().unwrap(),
                material: None,
            },
        );
        let id = wall.id();
        model.walls.insert(id, wall);
        let native = os_geometry::walls::NativeWall::from_model(&model, id).unwrap();
        let mesh = native.mesh().unwrap();
        assert!(mesh.vertices.len() <= 4 * (MAX_WALL_ARC_SEGMENTS + 1));
        let mut edges = std::collections::BTreeMap::new();
        for t in &mesh.triangles {
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                *edges.entry((a, b)).or_insert(0) += 1;
            }
        }
        assert!(
            edges
                .iter()
                .all(|(&(a, b), &n)| n == 1 && edges.get(&(b, a)) == Some(&1))
        );
        let exact = 4.0 * sweep.abs() * 0.2 * 3.0;
        assert!((native.net_volume().unwrap() - exact).abs() < 1e-12);
        assert!((mesh.signed_volume() - exact).abs() / exact < 0.001);
        assert!(native.cells().is_err());
        assert!(os_geometry::walls::wall_solid(&native.parameters, 0.0).is_err());
        let parts = native
            .layer_plan_footprints(Default::default(), Default::default(), None)
            .unwrap();
        assert!(parts[0].1.len() > 8);
        let plane = os_geometry::section::VerticalSectionPlane {
            origin: Point2::new(1.0, 0.0),
            direction: Point2::new(0.0, 1.0),
        };
        // The public section extractor validates watertight topology too.
        assert!(
            !os_geometry::section::vertical_section(&mesh, plane)
                .unwrap()
                .is_empty()
        );
    }
}
