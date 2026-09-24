//! Derived rectangular column prisms, shared by scene and plan.
use crate::{GeometryKernel, Mesh, PrismKernel, Profile, Solid, SurfaceIdentity, Transform, Vec3};
use os_core::{Point2, Result};
use os_model::ColumnParams;

pub fn column_solid(parameters: &ColumnParams, level_elevation: f64) -> Result<Solid> {
    parameters.validate()?;
    let (bottom, _) = parameters.elevations(level_elevation)?;
    let (w, d) = (parameters.width / 2.0, parameters.depth / 2.0);
    Ok(Solid {
        profile: Profile {
            vertices: vec![
                Point2::new(-w, -d),
                Point2::new(w, -d),
                Point2::new(w, d),
                Point2::new(-w, d),
            ],
        },
        height: parameters.height,
        transform: Transform {
            translation: Vec3::new(parameters.center.x, parameters.center.y, bottom),
            rotation_z: 0.0,
        },
    })
}

pub fn column_mesh(parameters: &ColumnParams, level_elevation: f64) -> Result<Mesh> {
    let mut mesh = PrismKernel.tessellate(&column_solid(parameters, level_elevation)?)?;
    mesh.surfaces = vec![
        SurfaceIdentity {
            layer: None,
            material: parameters.material
        };
        mesh.triangles.len()
    ];
    mesh.validate()?;
    Ok(mesh)
}

#[cfg(test)]
mod tests {
    use super::*;
    use os_core::Id;

    #[test]
    fn rectangular_column_mesh_preserves_identity_material_and_level_offset() {
        let material = Id::new();
        let parameters = ColumnParams {
            name: "C1".into(),
            level: Id::new(),
            center: Point2::new(5.0, -2.0),
            width: 0.4,
            depth: 0.6,
            height: 3.2,
            base_offset: 0.15,
            material: Some(material),
        };
        let mesh = column_mesh(&parameters, 3.0).unwrap();
        assert_eq!(mesh.vertices.len(), 8);
        assert_eq!(mesh.triangles.len(), 12);
        assert!((mesh.signed_volume() - 0.4 * 0.6 * 3.2).abs() < 1e-10);
        assert!(
            mesh.vertices
                .iter()
                .all(|point| point.x >= 4.8 && point.x <= 5.2)
        );
        assert!(
            mesh.vertices
                .iter()
                .all(|point| point.y >= -2.3 && point.y <= -1.7)
        );
        assert!(
            mesh.vertices
                .iter()
                .all(|point| point.z >= 3.15 - 1e-10 && point.z <= 6.35 + 1e-10)
        );
        assert_eq!(mesh.surfaces.len(), mesh.triangles.len());
        assert!(
            mesh.surfaces
                .iter()
                .all(|surface| surface.material == Some(material))
        );
    }
}
