//! Derived cabinet envelopes. Detailed doors, drawers and interiors are not modeled.
use crate::plan::{HorizontalBasis, PlanCrop, PlanFootprint, PlanRange, rectangular_plan};
use crate::{GeometryKernel, Mesh, PrismKernel, Profile, Solid, SurfaceIdentity, Transform, Vec3};
use os_core::{Point2, Result};
use os_model::{CaseworkParams, CaseworkTypeParams};

/// Geometry uses resolved type parameters; callers validate model references first.
pub fn casework_solid(
    parameters: &CaseworkParams,
    casework_type: &CaseworkTypeParams,
    level_elevation: f64,
) -> Result<Solid> {
    let (bottom, _) = parameters.elevations(casework_type, level_elevation)?;
    let (w, d) = (casework_type.width / 2.0, casework_type.depth / 2.0);
    Ok(Solid {
        profile: Profile {
            vertices: vec![
                Point2::new(-w, -d),
                Point2::new(w, -d),
                Point2::new(w, d),
                Point2::new(-w, d),
            ],
        },
        height: casework_type.height,
        transform: Transform {
            translation: Vec3::new(parameters.center.x, parameters.center.y, bottom),
            rotation_z: parameters.yaw,
        },
    })
}

pub fn casework_mesh(
    parameters: &CaseworkParams,
    casework_type: &CaseworkTypeParams,
    level_elevation: f64,
) -> Result<Mesh> {
    let mut mesh =
        PrismKernel.tessellate(&casework_solid(parameters, casework_type, level_elevation)?)?;
    mesh.surfaces = vec![
        SurfaceIdentity {
            layer: None,
            material: casework_type.material
        };
        mesh.triangles.len()
    ];
    mesh.validate()?;
    Ok(mesh)
}

/// Uses an absolute plan range and the shared cut/projected/depth policy.
/// As with `rectangular_plan`, geometry entirely above the cut is omitted.
pub fn casework_footprint(
    parameters: &CaseworkParams,
    casework_type: &CaseworkTypeParams,
    level_elevation: f64,
    range: PlanRange,
    basis: HorizontalBasis,
    crop: Option<PlanCrop>,
) -> Result<Option<PlanFootprint>> {
    rectangular_plan(
        &casework_solid(parameters, casework_type, level_elevation)?,
        range,
        basis,
        crop,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::PlanRole;
    use os_core::Id;
    use std::collections::BTreeMap;

    fn fixture() -> (CaseworkParams, CaseworkTypeParams) {
        (
            CaseworkParams {
                name: "Cabinet 1".into(),
                type_id: Id::new(),
                level: Id::new(),
                center: Point2::new(5.0, -2.0),
                yaw: 0.37,
                base_offset: 0.15,
            },
            CaseworkTypeParams {
                name: "Base cabinet".into(),
                width: 1.2,
                depth: 0.6,
                height: 0.9,
                material: Some(Id::new()),
            },
        )
    }

    #[test]
    fn rotated_mesh_is_closed_outward_and_preserves_material_and_extents() {
        let (parameters, cabinet_type) = fixture();
        let solid = casework_solid(&parameters, &cabinet_type, 3.0).unwrap();
        assert_eq!(solid.transform.rotation_z, parameters.yaw);
        let mesh = casework_mesh(&parameters, &cabinet_type, 3.0).unwrap();
        assert_eq!(mesh.vertices.len(), 8);
        assert_eq!(mesh.triangles.len(), 12);
        assert!((mesh.signed_volume() - 1.2 * 0.6 * 0.9).abs() < 1e-10);
        let corners = parameters.boundary(&cabinet_type).unwrap();
        for (i, vertex) in mesh.vertices.iter().enumerate() {
            assert!(Point2::new(vertex.x, vertex.y).distance(corners[i % 4]) < 1e-12);
            assert!((vertex.z - if i < 4 { 3.15 } else { 4.05 }).abs() < 1e-12);
        }
        assert_eq!(mesh.surfaces.len(), mesh.triangles.len());
        assert!(
            mesh.surfaces
                .iter()
                .all(|s| s.material == cabinet_type.material && s.layer.is_none())
        );
        let mut edges = BTreeMap::<(u32, u32), (usize, i32)>::new();
        for [a, b, c] in &mesh.triangles {
            for (from, to) in [(*a, *b), (*b, *c), (*c, *a)] {
                let edge = edges.entry((from.min(to), from.max(to))).or_default();
                edge.0 += 1;
                edge.1 += if from < to { 1 } else { -1 };
            }
        }
        assert!(edges.values().all(|value| *value == (2, 0)));
        let mut unassigned = cabinet_type;
        unassigned.material = None;
        assert!(
            casework_mesh(&parameters, &unassigned, 3.0)
                .unwrap()
                .surfaces
                .iter()
                .all(|s| *s == SurfaceIdentity::default())
        );
    }

    #[test]
    fn footprint_uses_shared_range_basis_and_crop_policy() {
        let (mut parameters, cabinet_type) = fixture();
        let basis = HorizontalBasis {
            origin: parameters.center,
            rotation: parameters.yaw,
        };
        let range = PlanRange::default();
        for (offset, role) in [
            (0.15, Some(PlanRole::Projected)),
            (0.8, Some(PlanRole::Cut)),
            (-1.0, Some(PlanRole::Depth)),
            (1.3, None),
            (-3.0, None),
        ] {
            parameters.base_offset = offset;
            let footprint =
                casework_footprint(&parameters, &cabinet_type, 0.0, range, basis, None).unwrap();
            assert_eq!(footprint.as_ref().map(|f| f.role), role);
            if let Some(footprint) = footprint {
                assert!((footprint.area() - 0.72).abs() < 1e-10);
            }
        }
        parameters.base_offset = 0.15;
        let full = casework_footprint(&parameters, &cabinet_type, 0.0, range, basis, None)
            .unwrap()
            .unwrap();
        assert!(full.vertices()[0].distance(Point2::new(-0.6, -0.3)) < 1e-12);
        let crop = PlanCrop {
            min: Point2::new(0.0, -1.0),
            max: Point2::new(1.0, 1.0),
        };
        let clipped = casework_footprint(&parameters, &cabinet_type, 0.0, range, basis, Some(crop))
            .unwrap()
            .unwrap();
        assert!((clipped.area() - 0.36).abs() < 1e-10);
        let outside = PlanCrop {
            min: Point2::new(10.0, 10.0),
            max: Point2::new(11.0, 11.0),
        };
        assert!(
            casework_footprint(&parameters, &cabinet_type, 0.0, range, basis, Some(outside))
                .unwrap()
                .is_none()
        );
        let upstairs = casework_footprint(
            &parameters,
            &cabinet_type,
            3.0,
            range.at_level(3.0).unwrap(),
            basis,
            None,
        )
        .unwrap()
        .unwrap();
        assert_eq!(upstairs, full);
    }

    #[test]
    fn invalid_dimensions_rotation_bounds_and_elevations_fail_before_geometry() {
        let (parameters, cabinet_type) = fixture();
        let bad_type = CaseworkTypeParams {
            height: 0.0,
            ..cabinet_type.clone()
        };
        assert!(casework_solid(&parameters, &bad_type, 0.0).is_err());
        for bad in [
            CaseworkParams {
                yaw: f64::NAN,
                ..parameters.clone()
            },
            CaseworkParams {
                center: Point2::new(1e6, 0.0),
                ..parameters.clone()
            },
        ] {
            assert!(casework_mesh(&bad, &cabinet_type, 0.0).is_err());
            assert!(
                casework_footprint(
                    &bad,
                    &cabinet_type,
                    0.0,
                    PlanRange::default(),
                    HorizontalBasis::default(),
                    None
                )
                .is_err()
            );
        }
        assert!(casework_mesh(&parameters, &cabinet_type, f64::MAX).is_err());
        assert!(casework_mesh(&parameters, &cabinet_type, 1e6).is_err());
    }
}
