//! Shared preparation/validation for synchronous and worker API-2 geometry.
use crate::{
    PluginHost,
    generic::{decode, entity_to_wire},
    require,
};
use os_core::{Id, Point2, Result, ensure};
use os_document::Document;
use os_geometry::{GeometryKernel, Mesh, PrismKernel, Profile, Solid, Transform, Vec3};
use os_plugin_api::{Capability, Permission, generic as wire, geometry::Recipe};
use std::{
    collections::BTreeSet,
    time::{Duration, Instant},
};

pub struct GeometryResult {
    element: Id,
    session: Id,
    revision: u64,
    solid: Solid,
    mesh: Mesh,
}
pub(super) struct PreparedGeometry {
    #[cfg(feature = "wasm")]
    pub(super) id: Id,
    pub(super) input: String,
    pub(super) deadline: Instant,
    request: wire::Request,
    element: Id,
}
impl GeometryResult {
    /// Revalidate at consumption. This is model geometry, not view-specific output.
    pub fn get(&self, document: &Document) -> Result<(Id, &Solid, &Mesh)> {
        ensure(
            self.session == document.session_id() && self.revision == document.revision(),
            "stale generic geometry",
        )?;
        Ok((self.element, &self.solid, &self.mesh))
    }
}

fn tessellate(recipe: Recipe) -> Result<(Solid, Mesh)> {
    recipe.validate()?;
    let Recipe::RectangularPrism(p) = recipe;
    let solid = Solid {
        profile: Profile {
            vertices: vec![
                Point2::new(0.0, 0.0),
                Point2::new(p.width, 0.0),
                Point2::new(p.width, p.depth),
                Point2::new(0.0, p.depth),
            ],
        },
        height: p.height,
        transform: Transform {
            translation: Vec3::new(p.translation[0], p.translation[1], p.translation[2]),
            rotation_z: p.rotation_z,
        },
    };
    let mesh = PrismKernel.tessellate(&solid)?;
    Ok((solid, mesh))
}

impl PluginHost {
    /// Only granted read-scope objects are serialized. Geometry cannot return edits.
    pub fn generate_generic_geometry(
        &self,
        plugin: &str,
        doc: &Document,
        element: Id,
        read: BTreeSet<Id>,
        timeout: Duration,
    ) -> Result<GeometryResult> {
        let prepared = self.prepare_generic_geometry(plugin, doc, element, read, timeout)?;
        let output = self.plugin(plugin)?.plugin.invoke_json(&prepared.input)?;
        Self::finish_generic_geometry(doc, &prepared, &output)
    }

    pub(super) fn prepare_generic_geometry(
        &self,
        plugin: &str,
        doc: &Document,
        element: Id,
        read: BTreeSet<Id>,
        timeout: Duration,
    ) -> Result<PreparedGeometry> {
        let started = Instant::now();
        ensure(
            !timeout.is_zero() && timeout <= Duration::from_secs(30),
            "invalid geometry deadline",
        )?;
        let p = self.plugin(plugin)?;
        ensure(
            p.manifest.api_version == wire::VERSION,
            "geometry requires API 2",
        )?;
        require(p, Permission::ModelRead)?;
        ensure(
            p.manifest.capabilities.contains(&Capability::Geometry),
            "plugin has no geometry capability",
        )?;
        #[cfg(feature = "wasm")]
        ensure(
            !p.worker.as_ref().is_some_and(|r| r.is_busy()),
            "plugin already has a live worker",
        )?;
        ensure(
            read.contains(&element) && read.len() <= wire::MAX_SCOPE,
            "invalid geometry scope",
        )?;
        let native = doc.model().walls.get(&element);
        let entity = if let Some(wall) = native {
            crate::native_wall::project_checked(doc.model(), wall)?
        } else {
            entity_to_wire(doc.model().extensions.get(&element).ok_or_else(|| {
                crate::invalid("geometry target is not an extension or native wall")
            })?)
        };
        ensure(
            entity.owner == plugin,
            "geometry target belongs to another owner",
        )?;
        let kind = p
            .catalog
            .as_ref()
            .and_then(|c| c.types.iter().find(|t| t.id == entity.type_id))
            .ok_or_else(|| crate::invalid("geometry type not registered"))?;
        ensure(
            kind.payload_schema_version == entity.payload_schema_version,
            "geometry target requires payload migration",
        )?;
        ensure(
            doc.model()
                .plugin_requirements
                .get(plugin)
                .map_or(native.is_some(), |r| r.version == p.manifest.version),
            "geometry target requires another plugin version",
        )?;
        let mut snapshot = wire::Snapshot::default();
        for id in read {
            if let Some(e) = doc.model().extensions.get(&id) {
                snapshot.elements.push(entity_to_wire(e));
            } else if let Some(wall) = doc.model().walls.get(&id) {
                snapshot
                    .elements
                    .push(crate::native_wall::project_checked(doc.model(), wall)?);
            } else if let Some(level) = doc.model().levels.get(&id) {
                snapshot.levels.push(wire::Level {
                    id: id.to_string(),
                    name: level.parameters.name.clone(),
                    elevation_metres: level.parameters.elevation,
                });
            } else {
                return Err(crate::invalid("unsupported geometry scope object"));
            }
        }
        let request_id = Id::new();
        let request = wire::Request {
            api_version: wire::VERSION,
            request_id: request_id.to_string(),
            context: Some(wire::Stamp {
                project: doc.model().project.id().to_string(),
                session: doc.session_id().to_string(),
                revision: doc.revision(),
            }),
            operation: wire::Operation::GenerateGeometry {
                element_id: element.to_string(),
                snapshot,
            },
        };
        let input = serde_json::to_string(&request).map_err(crate::invalid)?;
        ensure(
            input.len() <= wire::MAX_BYTES && started.elapsed() < timeout,
            "geometry request too large or expired",
        )?;
        Ok(PreparedGeometry {
            #[cfg(feature = "wasm")]
            id: request_id,
            input,
            deadline: started + timeout,
            request,
            element,
        })
    }

    pub(super) fn finish_generic_geometry(
        doc: &Document,
        prepared: &PreparedGeometry,
        output: &str,
    ) -> Result<GeometryResult> {
        ensure(
            Instant::now() < prepared.deadline,
            "geometry acceptance deadline expired",
        )?;
        ensure(
            prepared.request.context.as_ref()
                == Some(&wire::Stamp {
                    project: doc.model().project.id().to_string(),
                    session: doc.session_id().to_string(),
                    revision: doc.revision(),
                }),
            "geometry document changed",
        )?;
        let wire::Reply::Geometry { element_id, recipe } = decode(output, &prepared.request)?
        else {
            return Err(crate::invalid("expected geometry, not edits"));
        };
        ensure(
            element_id == prepared.element.to_string(),
            "geometry target mismatch",
        )?;
        let (solid, mesh) = tessellate(recipe)?;
        ensure(
            Instant::now() < prepared.deadline,
            "geometry acceptance deadline expired",
        )?;
        Ok(GeometryResult {
            element: prepared.element,
            session: doc.session_id(),
            revision: doc.revision(),
            solid,
            mesh,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use os_plugin_api::geometry::RectangularPrism;
    #[test]
    fn independent_recipe_maps_to_checked_rotated_kernel_geometry() {
        let recipe = Recipe::RectangularPrism(RectangularPrism {
            width: 2.0,
            depth: 0.5,
            height: 3.0,
            translation: [10.0, -5.0, 7.0],
            rotation_z: std::f64::consts::FRAC_PI_2,
        });
        let (_, mesh) = tessellate(recipe).unwrap();
        assert_eq!(mesh.triangles.len(), 12);
        assert!((mesh.signed_volume() - 3.0).abs() < 1e-9);
        assert!((mesh.vertices[2].x - 9.5).abs() < 1e-9);
        assert!((mesh.vertices[2].y + 3.0).abs() < 1e-9);
        let bad = Recipe::RectangularPrism(RectangularPrism {
            width: 1.0,
            depth: 1.0,
            height: 1.0,
            translation: [f64::MAX; 3],
            rotation_z: 0.0,
        });
        assert!(tessellate(bad).is_err());
    }
}
