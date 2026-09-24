//! Reference built-in plugin using the same versioned messages as future runtimes.
use os_core::{Error, Result, ensure};
use os_document::Command;
pub use os_geometry::walls::wall_solid;
use os_model::Wall;
use os_plugin_api::*;

pub const PLUGIN_ID: &str = "org.openstructure.walls";
pub const WALL_TYPE: &str = "org.openstructure.walls.wall";
mod openings;
pub use openings::wall_prisms;
pub struct WallsPlugin;
impl Plugin for WallsPlugin {
    fn manifest(&self) -> Manifest {
        Manifest::from_toml(include_str!("../plugin.toml")).expect("bundled manifest must be valid")
    }
    fn invoke_json(&self, text: &str) -> Result<String> {
        ensure(
            text.len() <= MAX_MESSAGE_BYTES,
            "request exceeds plugin limit",
        )?;
        let request: RequestEnvelope =
            serde_json::from_str(text).map_err(|e| Error::Invalid(e.to_string()))?;
        ensure(
            request.api_version == API_VERSION,
            "unsupported request version",
        )?;
        let model = request
            .model
            .ok_or_else(|| Error::Permission("wall plugin requires model.read".into()))?;
        ensure(
            model.schema_version == REQUIRED_MODEL_SCHEMA_VERSION,
            "native Model request requires schema 28",
        )?;
        let response = match request.request {
            Request::CreateWall(parameters) => {
                parameters.validate()?;
                Response::Commands(vec![Command::AddWall(Wall::new(WALL_TYPE, parameters))])
            }
            Request::EditWall { id, parameters } => {
                parameters.validate()?;
                ensure(model.walls.contains_key(&id), "wall not found")?;
                Response::Commands(vec![Command::UpdateWall { id, parameters }])
            }
            Request::GenerateWall { id } => {
                os_core::ensure(
                    !model.wall_type_assignments.contains_key(&id),
                    "single-Solid provider cannot preserve typed wall layers; use native layer geometry",
                )?;
                let mut cells = os_geometry::walls::NativeWall::from_model(&model, id)?.cells()?;
                if cells.len() != 1 {
                    return Err(Error::Unsupported("legacy single-solid response cannot represent hosted openings; use native host geometry".into()));
                }
                Response::Solid(cells.remove(0))
            }
        };
        serde_json::to_string(&ResponseEnvelope {
            api_version: API_VERSION,
            response,
        })
        .map_err(|e| Error::Invalid(e.to_string()))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use os_core::Point2;
    use os_model::WallParams;
    #[test]
    fn butt_join_plugin_geometry_uses_model_aware_derivation() {
        use os_model::{Model, WallAnchor, WallEndpoint, WallJoin, WallJoinParams};
        let mut model = Model::new("Joined plugin walls");
        let level = *model.levels.keys().next().unwrap();
        let a = Wall::new(
            WALL_TYPE,
            WallParams {
                name: "A".into(),
                start: Point2::new(0., 0.),
                end: Point2::new(3., 0.),
                height: 3.,
                thickness: 0.2,
                level,
                material: None,
            },
        );
        let b = Wall::new(
            WALL_TYPE,
            WallParams {
                name: "B".into(),
                start: Point2::new(6., 0.),
                end: Point2::new(3., 0.),
                ..a.parameters.clone()
            },
        );
        let (aid, bid) = (a.id(), b.id());
        model.walls.insert(aid, a);
        model.walls.insert(bid, b);
        let join = WallJoin::new(
            "core.wall_join",
            WallJoinParams::Butt {
                a: WallAnchor {
                    wall: aid,
                    endpoint: WallEndpoint::End,
                },
                b: WallAnchor {
                    wall: bid,
                    endpoint: WallEndpoint::End,
                },
            },
        );
        model.wall_joins.insert(join.id(), join);
        model.validate().unwrap();
        let request = |model: Model, id| {
            serde_json::to_string(&RequestEnvelope {
                api_version: API_VERSION,
                request: Request::GenerateWall { id },
                model: Some(model),
            })
            .unwrap()
        };
        for id in [aid, bid] {
            let response = WallsPlugin
                .invoke_json(&request(model.clone(), id))
                .unwrap();
            let response: ResponseEnvelope = serde_json::from_str(&response).unwrap();
            let Response::Solid(solid) = response.response else {
                panic!("expected solid")
            };
            assert_eq!(
                solid,
                os_geometry::walls::NativeWall::from_model(&model, id)
                    .unwrap()
                    .cells()
                    .unwrap()[0]
            );
        }
        model.walls.get_mut(&bid).unwrap().parameters.end.x += 0.1;
        assert!(WallsPlugin.invoke_json(&request(model, aid)).is_err());
    }
    use os_geometry::{GeometryKernel, PrismKernel};
    #[test]
    fn wall_regenerates_after_parameter_and_elevation_changes() {
        let mut wall = WallParams {
            name: "W".into(),
            start: Point2::new(2.0, 3.0),
            end: Point2::new(5.0, 7.0),
            thickness: 0.2,
            height: 3.0,
            level: os_core::Id::new(),
            material: None,
        };
        let first = PrismKernel
            .tessellate(&wall_solid(&wall, 0.0).unwrap())
            .unwrap();
        assert!((first.signed_volume() - 3.0).abs() < 1e-8);
        wall = wall.with_length(10.0).unwrap();
        wall.height = 4.0;
        wall.thickness = 0.3;
        let next = PrismKernel
            .tessellate(&wall_solid(&wall, 5.0).unwrap())
            .unwrap();
        assert!((next.signed_volume() - 12.0).abs() < 1e-8);
        assert_eq!(
            next.vertices
                .iter()
                .map(|v| v.z)
                .fold(f64::INFINITY, f64::min),
            5.0
        );
    }
}
