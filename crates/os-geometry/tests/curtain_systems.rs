#[path = "../../os-model/tests/support/curtains.rs"]
mod support;
use os_core::{Id, Point2};
use os_geometry::{
    Mesh,
    curtain_systems::{curtain_geometry, curtain_mesh},
};
use os_model::{Material, MaterialParams};

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-8, "{a} != {b}");
}

#[test]
fn diagonal_curtain_prisms_preserve_envelopes_materials_and_analytic_volume() {
    let (mut model, mut assembly) = support::fixture();
    let materials = [700., 2500.].map(|density| {
        Material::new(
            "core.material",
            MaterialParams {
                name: format!("Density {density}"),
                density_kg_m3: density,
                color: [60, 90, 120],
            },
        )
    });
    model
        .curtain_panel_types
        .get_mut(&assembly.parameters.panel_type)
        .unwrap()
        .parameters
        .material = Some(materials[0].id());
    model
        .curtain_mullion_types
        .get_mut(&assembly.parameters.mullion_type)
        .unwrap()
        .parameters
        .material = Some(materials[1].id());
    for material in materials {
        model.materials.insert(material.id(), material);
    }
    model
        .levels
        .get_mut(&assembly.parameters.level)
        .unwrap()
        .parameters
        .elevation = 8.5;
    assembly.parameters.base_offset = -0.25;
    assembly.parameters.start = Point2::new(11., -7.);
    assembly.parameters.end = Point2::new(13.4, -3.8);
    // Test both directed orientations and both normal conventions.
    for reverse in [false, true] {
        let mut p = assembly.parameters.clone();
        if reverse {
            std::mem::swap(&mut p.start, &mut p.end);
        }
        for flip in [false, true] {
            p.normal_flip = flip;
            let boxes = p.resolve(&model).unwrap();
            let derived = curtain_geometry(&p, &model).unwrap();
            let mesh = &derived.mesh;
            mesh.validate().unwrap();
            assert_eq!(mesh.vertices.len(), boxes.len() * 8);
            assert_eq!(mesh.triangles.len(), boxes.len() * 12);
            assert_eq!(mesh.surfaces.len(), mesh.triangles.len());
            close(mesh.signed_volume(), boxes.iter().map(|b| b.volume).sum());
            let dx = (p.end.x - p.start.x) / 4.;
            let dy = (p.end.y - p.start.y) / 4.;
            for (i, component) in boxes.iter().enumerate() {
                let mut low = [f64::INFINITY; 3];
                let mut high = [f64::NEG_INFINITY; 3];
                for vertex in &mesh.vertices[i * 8..(i + 1) * 8] {
                    let x = vertex.x - p.start.x;
                    let y = vertex.y - p.start.y;
                    let local = [x * dx + y * dy, -x * dy + y * dx, vertex.z - 8.25];
                    for axis in 0..3 {
                        low[axis] = low[axis].min(local[axis]);
                        high[axis] = high[axis].max(local[axis]);
                    }
                }
                for axis in 0..3 {
                    close(low[axis], component.min[axis]);
                    close(high[axis], component.max[axis]);
                }
                let piece = Mesh {
                    vertices: mesh.vertices[i * 8..(i + 1) * 8].to_vec(),
                    triangles: mesh.triangles[i * 12..(i + 1) * 12]
                        .iter()
                        .map(|t| t.map(|v| v - (i * 8) as u32))
                        .collect(),
                    surfaces: mesh.surfaces[i * 12..(i + 1) * 12].to_vec(),
                };
                piece.validate().unwrap();
                assert_eq!(derived.components[i].component, *component);
                assert_eq!(derived.components[i].mesh, piece);
                close(piece.signed_volume(), component.volume);
                assert!(
                    piece
                        .surfaces
                        .iter()
                        .all(|s| s.layer.is_none() && s.material == component.material)
                );
                close(
                    component.mass_kg.unwrap(),
                    component.volume
                        * model.materials[&component.material.unwrap()]
                            .parameters
                            .density_kg_m3,
                );
                for other in boxes.iter().skip(i + 1) {
                    assert!((0..3).any(|axis| component.max[axis] <= other.min[axis]
                        || other.max[axis] <= component.min[axis]));
                }
            }
            // The center of a bay beyond its thin panel is empty, even though
            // it lies inside the assembly's overall bounding prism.
            let aperture = [1., 0.08, 0.75];
            assert!(!boxes.iter().any(|b| {
                (0..3).all(|axis| aperture[axis] > b.min[axis] && aperture[axis] < b.max[axis])
            }));
            assert!(mesh.signed_volume() < 4. * 3. * 0.2);
            let mut opposite = p.clone();
            opposite.normal_flip = !p.normal_flip;
            assert_eq!(*mesh, curtain_mesh(&opposite, &model).unwrap());
        }
    }
}

#[test]
fn curtain_world_corners_enforce_section_envelope_including_depth() {
    let (mut model, assembly) = support::fixture();
    let limit = os_geometry::section::MAX_SECTION_COORDINATE;
    for sign in [-1., 1.] {
        for axis in 0..2 {
            let mut p = assembly.parameters.clone();
            if axis == 0 {
                p.start = Point2::new(sign * limit, 0.);
                p.end = Point2::new(sign * limit, 4.);
            } else {
                p.start = Point2::new(0., sign * limit);
                p.end = Point2::new(4., sign * limit);
            }
            assert!(
                p.resolve(&model).is_err(),
                "model must reject escaping corners"
            );
            for flip in [false, true] {
                p.normal_flip = flip;
                assert!(
                    curtain_geometry(&p, &model).is_err(),
                    "member depth must not escape the envelope"
                );
            }
            if axis == 0 {
                p.start.x -= sign * 0.2;
                p.end.x -= sign * 0.2;
            } else {
                p.start.y -= sign * 0.2;
                p.end.y -= sign * 0.2;
            }
            curtain_geometry(&p, &model).unwrap();
        }
    }
    let mut p = assembly.parameters;
    p.start = Point2::new(limit - 2.4, 0.);
    p.end = Point2::new(limit, 3.2);
    assert!(p.resolve(&model).is_err());
    assert!(curtain_geometry(&p, &model).is_err());
    p.start = Point2::new(0., 0.);
    p.end = Point2::new(4., 0.);
    p.base_offset = 0.;
    model.levels.get_mut(&p.level).unwrap().parameters.elevation = limit - p.height;
    let mesh = curtain_mesh(&p, &model).unwrap();
    assert!(mesh.vertices.iter().any(|v| v.z == limit));
    model.levels.get_mut(&p.level).unwrap().parameters.elevation += 0.01;
    assert!(curtain_mesh(&p, &model).is_err());
}

#[test]
fn curtain_geometry_rejects_invalid_context_and_nonfinite_quantities() {
    let (model, assembly) = support::fixture();
    for case in 0..5 {
        let mut model = model.clone();
        let mut p = assembly.parameters.clone();
        match case {
            0 => p.level = Id::new(),
            1 => p.end = p.start,
            2 => p.base_offset = f64::NAN,
            3 => {
                model
                    .curtain_panel_types
                    .get_mut(&p.panel_type)
                    .unwrap()
                    .parameters
                    .material = Some(Id::new())
            }
            _ => {
                let material = Material::new(
                    "core.material",
                    MaterialParams {
                        name: "Invalid density".into(),
                        density_kg_m3: f64::INFINITY,
                        color: [0; 3],
                    },
                );
                model
                    .curtain_panel_types
                    .get_mut(&p.panel_type)
                    .unwrap()
                    .parameters
                    .material = Some(material.id());
                model.materials.insert(material.id(), material);
            }
        }
        assert!(curtain_mesh(&p, &model).is_err());
    }
}

#[test]
fn maximum_grid_topology_has_bounded_disjoint_prism_mesh() {
    let (model, assembly) = support::fixture();
    let mut p = assembly.parameters;
    p.vertical = (0..os_model::MAX_CURTAIN_GRIDS)
        .map(|i| os_model::CurtainGrid {
            id: Id::new(),
            position: i as f64 * 4. / 17.,
        })
        .collect();
    p.horizontal = (0..os_model::MAX_CURTAIN_GRIDS)
        .map(|i| os_model::CurtainGrid {
            id: Id::new(),
            position: i as f64 * 3. / 17.,
        })
        .collect();
    p.reconcile().unwrap();
    let mesh = curtain_mesh(&p, &model).unwrap();
    let components = 18 + 17 * 18 + 17 * 17;
    assert_eq!(mesh.vertices.len(), components * 8);
    assert_eq!(mesh.triangles.len(), components * 12);
    close(
        mesh.signed_volume(),
        p.resolve(&model).unwrap().iter().map(|c| c.volume).sum(),
    );
}
