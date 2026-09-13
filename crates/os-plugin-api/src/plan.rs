//! Optional plan-graphics service 1 alongside generic API 2. No host types.
use crate::{ProtocolResult, ensure, generic};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const VERSION: u32 = 1;
pub const MAX_SEGMENTS: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Service {
    #[serde(rename = "plan.graphics")]
    Graphics,
}

/// Horizontal downward plan. Coordinates are metres; yaw is radians. Range is
/// level-relative [top, cut, bottom, depth]; crop is plane-space [min_x,min_y,max_x,max_y].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Context {
    pub view_id: String,
    pub settings_revision: u64,
    pub level_id: String,
    pub level_elevation: f64,
    pub origin: [f64; 2],
    pub yaw: f64,
    pub range: [f64; 4],
    pub crop: Option<[f64; 4]>,
    pub scale_denominator: f64,
    pub show_walls: bool,
    pub show_extensions: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub api_version: u32,
    pub service: Service,
    pub service_version: u32,
    pub request_id: String,
    pub context: generic::Stamp,
    pub provider_id: String,
    pub element_id: String,
    pub view: Context,
    pub snapshot: generic::Snapshot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    Cut,
    Projected,
    Depth,
}

/// Finite plane-space line with a stable provider-local semantic feature key.
/// Host styles, clipping, screen acquisition and draw ordering remain host-owned.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Segment {
    pub feature: u32,
    pub start: [f64; 2],
    pub end: [f64; 2],
    pub role: Role,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", deny_unknown_fields)]
pub enum Reply {
    Graphics(Vec<Segment>),
    Error(generic::PluginError),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Response {
    pub api_version: u32,
    pub service: Service,
    pub service_version: u32,
    pub request_id: String,
    pub context: generic::Stamp,
    pub element_id: String,
    pub view: Context,
    pub result: Reply,
}

pub fn validate_segments(segments: &[Segment]) -> ProtocolResult<()> {
    ensure(
        segments.len() <= MAX_SEGMENTS,
        "plan graphics exceed 256 segments",
    )?;
    let mut keys = BTreeSet::new();
    for segment in segments {
        let length = (segment.end[0] - segment.start[0]).hypot(segment.end[1] - segment.start[1]);
        ensure(
            segment
                .start
                .iter()
                .chain(&segment.end)
                .all(|v| v.is_finite())
                && length.is_finite()
                && length > 1e-9,
            "plan segment must have finite nonzero length",
        )?;
        ensure(keys.insert(segment.feature), "duplicate plan feature")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_response_mutation_fuzz_has_no_parser_or_geometry_panics() {
        let seed = serde_json::json!({
            "api_version":2,"service":"plan.graphics","service_version":1,
            "request_id":"request","context":{"project":"project","session":"session","revision":0},
            "element_id":"entity","view":{
                "view_id":"view","settings_revision":0,"level_id":"level","level_elevation":0.0,
                "origin":[0.0,0.0],"yaw":0.0,"range":[2.5,1.2,0.0,-1.0],"crop":null,
                "scale_denominator":100.0,"show_walls":true,"show_extensions":true
            },
            "result":{"kind":"Graphics","data":[{"feature":0,"start":[0.0,0.0],"end":[1.0,0.0],"role":"Cut"}]}
        }).to_string().into_bytes();
        let check = |bytes: &[u8]| {
            if let Ok(Response {
                result: Reply::Graphics(segments),
                ..
            }) = serde_json::from_slice::<Response>(bytes)
            {
                let _ = validate_segments(&segments);
            }
        };
        check(&seed);
        for index in 0..seed.len() {
            check(&seed[..index]);
            for replacement in [0, b'"', b'[', b'}', b'9', 255] {
                let mut changed = seed.clone();
                changed[index] = replacement;
                check(&changed);
            }
        }
        let segment = Segment {
            feature: 0,
            start: [0.0; 2],
            end: [1.0, 0.0],
            role: Role::Cut,
        };
        assert!(validate_segments(&vec![segment.clone(); 257]).is_err());
        for value in [f64::NAN, f64::INFINITY, -f64::INFINITY] {
            assert!(
                validate_segments(&[Segment {
                    end: [value, 0.0],
                    ..segment.clone()
                }])
                .is_err()
            );
        }
    }
}
