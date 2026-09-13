//! Reference built-in plugin using the same versioned messages as future runtimes.
use os_core::{Error, Point2, Result, ensure};
use os_document::Command;
use os_geometry::{Profile, Solid, Transform, Vec3};
use os_model::{Wall, WallParams};
use os_plugin_api::*;

pub const PLUGIN_ID: &str = "org.openstructure.walls";
pub const WALL_TYPE: &str = "org.openstructure.walls.wall";
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
                let wall = model
                    .walls
                    .get(&id)
                    .ok_or_else(|| Error::Invalid("wall not found".into()))?;
                let level = model
                    .levels
                    .get(&wall.parameters.level)
                    .ok_or_else(|| Error::Invalid("level not found".into()))?;
                Response::Solid(wall_solid(&wall.parameters, level.parameters.elevation)?)
            }
        };
        serde_json::to_string(&ResponseEnvelope {
            api_version: API_VERSION,
            response,
        })
        .map_err(|e| Error::Invalid(e.to_string()))
    }
}
pub fn wall_solid(wall: &WallParams, elevation: f64) -> Result<Solid> {
    wall.validate()?;
    ensure(elevation.is_finite(), "invalid level elevation")?;
    let length = wall.length();
    let half = wall.thickness / 2.0;
    Ok(Solid {
        profile: Profile {
            vertices: vec![
                Point2::new(0.0, -half),
                Point2::new(length, -half),
                Point2::new(length, half),
                Point2::new(0.0, half),
            ],
        },
        height: wall.height,
        transform: Transform {
            translation: Vec3::new(wall.start.x, wall.start.y, elevation),
            rotation_z: (wall.end.y - wall.start.y).atan2(wall.end.x - wall.start.x),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
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
