//! Stable semantic projection for the native straight-wall adapter, not model layout.
use serde::{Deserialize, Serialize};
pub const OWNER: &str = "org.openstructure.walls";
pub const TYPE: &str = "org.openstructure.walls.wall";
pub const PAYLOAD_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Parameters {
    pub start_x: f64,
    pub start_y: f64,
    pub end_x: f64,
    pub end_y: f64,
    pub thickness: f64,
    pub height: f64,
    pub level: String,
}
