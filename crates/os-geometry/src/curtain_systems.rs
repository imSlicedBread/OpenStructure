//! Native curtain solids derived from the model's clear component envelopes.
//! Each panel/member remains a separate closed prism in the assembly mesh;
//! touching faces are retained, and no assembly bounding box is substituted.
use crate::{GeometryKernel, Mesh, PrismKernel, Profile, Solid, SurfaceIdentity, Transform, Vec3};
use os_core::{Point2, Result, ensure};
use os_model::{CurtainComponent, CurtainSystemParams, Model};

/// An independently closed solid with its stable component UUID, material and
/// analytic quantities. Section consumers must cut these meshes independently:
/// neighboring members have touching contours in the combined display mesh.
#[derive(Clone, Debug, PartialEq)]
pub struct CurtainComponentMesh {
    pub component: CurtainComponent,
    pub mesh: Mesh,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CurtainGeometry {
    pub components: Vec<CurtainComponentMesh>,
    /// Display mesh selected under the assembly UUID, not a sectionable union.
    pub mesh: Mesh,
}

/// World-space assembly geometry, ordered exactly as `CurtainSystemParams::resolve`.
/// Each component contributes eight vertices and twelve triangles. Material IDs
/// classify surfaces; component IDs are not compound-wall layer identities.
pub fn curtain_mesh(parameters: &CurtainSystemParams, model: &Model) -> Result<Mesh> {
    Ok(curtain_geometry(parameters, model)?.mesh)
}

pub fn curtain_geometry(
    parameters: &CurtainSystemParams,
    model: &Model,
) -> Result<CurtainGeometry> {
    let components = parameters.resolve(model)?;
    let elevation = model.levels[&parameters.level].parameters.elevation;
    let mut result = Mesh::default();
    let mut derived = Vec::with_capacity(components.len());
    for component in components {
        ensure(
            component.area.is_finite()
                && component.area > 0.0
                && component.volume.is_finite()
                && component.volume > 0.0
                && component.length.is_finite()
                && component.length >= 0.0
                && component
                    .mass_kg
                    .is_none_or(|mass| mass.is_finite() && mass >= 0.0),
            "invalid curtain component quantity",
        )?;
        ensure(
            component
                .material
                .is_none_or(|id| model.materials.contains_key(&id)),
            "curtain component material missing",
        )?;
        let [x0, mut y0, z0] = component.min;
        let [x1, mut y1, z1] = component.max;
        // Use the reflected local depths in canonical order before tessellation;
        // map through the same signed-depth basis used by model validation below.
        if parameters.normal_flip {
            (y0, y1) = (-y1, -y0);
        }
        let solid = Solid {
            profile: Profile {
                vertices: vec![
                    Point2::new(x0, y0),
                    Point2::new(x1, y0),
                    Point2::new(x1, y1),
                    Point2::new(x0, y1),
                ],
            },
            height: z1 - z0,
            transform: Transform {
                translation: Vec3::new(0.0, 0.0, z0),
                rotation_z: 0.0,
            },
        };
        let mut mesh = PrismKernel.tessellate(&solid)?;
        for vertex in &mut mesh.vertices {
            // The prism already has reflected depth and CCW winding. Undo that
            // sign for the shared signed-depth mapper, which reflects it once.
            let depth = if parameters.normal_flip {
                -vertex.y
            } else {
                vertex.y
            };
            let [x, y, z] = parameters.world_point([vertex.x, depth, vertex.z], elevation);
            *vertex = Vec3::new(x, y, z);
        }
        ensure(
            mesh.vertices.iter().all(|v| {
                [v.x, v.y, v.z].into_iter().all(|coordinate| {
                    coordinate.is_finite()
                        && coordinate.abs() <= crate::section::MAX_SECTION_COORDINATE
                })
            }),
            "curtain component outside finite coordinate envelope",
        )?;
        let offset = u32::try_from(result.vertices.len())
            .map_err(|_| os_core::Error::Invalid("curtain mesh too large".into()))?;
        mesh.surfaces = vec![
            SurfaceIdentity {
                layer: None,
                material: component.material
            };
            mesh.triangles.len()
        ];
        result.surfaces.extend_from_slice(&mesh.surfaces);
        result
            .triangles
            .extend(mesh.triangles.iter().map(|t| t.map(|i| i + offset)));
        result.vertices.extend_from_slice(&mesh.vertices);
        derived.push(CurtainComponentMesh { component, mesh });
    }
    result.validate()?;
    Ok(CurtainGeometry {
        components: derived,
        mesh: result,
    })
}
