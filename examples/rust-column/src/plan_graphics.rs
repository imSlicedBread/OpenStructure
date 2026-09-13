//! Independently authored rectangular-column plan outline; no host/kernel types.
use os_plugin_api::{generic, geometry::Recipe, plan};

fn error(message: &str) -> generic::PluginError {
    generic::PluginError {
        code: generic::ErrorCode::Unsupported,
        message: message.into(),
        field: None,
    }
}
fn graphics(request: &plan::Request) -> Result<Vec<plan::Segment>, generic::PluginError> {
    let v = &request.view;
    if request.api_version != generic::VERSION
        || request.service_version != plan::VERSION
        || request.provider_id != "org.example.columns.plan"
        || request.snapshot.elements.len() != 1
        || request.snapshot.levels.len() != 1
    {
        return Err(error("Unsupported plan service or scope"));
    }
    let [top, cut, bottom, depth] = v.range;
    if !v.origin.iter().chain(&v.range).all(|n| n.is_finite())
        || !v.yaw.is_finite()
        || !v.level_elevation.is_finite()
        || depth > bottom
        || bottom > cut
        || cut > top
        || top - depth <= 1e-9
        || !(top - depth).is_finite()
        || !v.scale_denominator.is_finite()
        || !(0.001..=1_000_000.0).contains(&v.scale_denominator)
        || v.crop.is_some_and(|[a, b, c, d]| {
            ![a, b, c, d].iter().all(|n| n.is_finite()) || a >= c || b >= d
        })
    {
        return Err(error("Invalid plan context"));
    }
    let level = &request.snapshot.levels[0];
    if level.id != v.level_id || level.elevation_metres != v.level_elevation {
        return Err(error("Plan level not in scoped snapshot"));
    }
    let reply = crate::column::invoke(&generic::Request {
        api_version: generic::VERSION,
        request_id: request.request_id.clone(),
        context: Some(request.context.clone()),
        operation: generic::Operation::GenerateGeometry {
            element_id: request.element_id.clone(),
            snapshot: request.snapshot.clone(),
        },
    })?;
    let generic::Reply::Geometry {
        recipe: Recipe::RectangularPrism(prism),
        ..
    } = reply
    else {
        return Err(error("Unsupported column geometry"));
    };
    let [top, cut, bottom, depth] = v.range.map(|n| n + v.level_elevation);
    let base = prism.translation[2];
    let end = base + prism.height;
    if ![top, cut, bottom, depth, end].iter().all(|n| n.is_finite()) || top - depth <= 1e-9 {
        return Err(error("Plan height overflow"));
    }
    // Same documented 1 nm boundary convention as the native straight-prism subset.
    let tolerance = 1e-9;
    let role = if !v.show_extensions || end <= depth + tolerance || base >= top - tolerance {
        return Ok(vec![]);
    } else if base <= cut + tolerance && end > cut + tolerance {
        plan::Role::Cut
    } else if end <= cut + tolerance && end > bottom + tolerance {
        plan::Role::Projected
    } else if end <= bottom + tolerance && end > depth + tolerance {
        plan::Role::Depth
    } else {
        return Ok(vec![]);
    };
    let (sin, cos) = v.yaw.sin_cos();
    let corners = [
        [0.0, 0.0],
        [prism.width, 0.0],
        [prism.width, prism.depth],
        [0.0, prism.depth],
    ]
    .map(|[x, y]| {
        let dx = x - v.origin[0];
        let dy = y - v.origin[1];
        [dx * cos + dy * sin, -dx * sin + dy * cos]
    });
    let lines: Vec<_> = (0..4)
        .map(|i| plan::Segment {
            feature: i as u32,
            start: corners[i],
            end: corners[(i + 1) % 4],
            role,
        })
        .collect();
    plan::validate_segments(&lines).map_err(|_| error("Plan coordinate precision lost"))?;
    Ok(lines)
}

pub(super) fn respond(request: plan::Request) -> Option<Vec<u8>> {
    let result = match graphics(&request) {
        Ok(lines) => plan::Reply::Graphics(lines),
        Err(e) => plan::Reply::Error(e),
    };
    let response = plan::Response {
        api_version: generic::VERSION,
        service: plan::Service::Graphics,
        service_version: plan::VERSION,
        request_id: request.request_id,
        context: request.context,
        element_id: request.element_id,
        view: request.view,
        result,
    };
    let bytes = serde_json::to_vec(&response).ok()?;
    (bytes.len() <= generic::MAX_BYTES).then_some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> plan::Request {
        let height_key = if cfg!(feature = "migration-v2") {
            "height_m"
        } else {
            "height"
        };
        plan::Request {
            api_version: 2,
            service: plan::Service::Graphics,
            service_version: 1,
            request_id: "r".into(),
            provider_id: "org.example.columns.plan".into(),
            element_id: "e".into(),
            context: generic::Stamp {
                project: "p".into(),
                session: "s".into(),
                revision: 0,
            },
            view: plan::Context {
                view_id: "v".into(),
                settings_revision: 0,
                level_id: "l".into(),
                level_elevation: 0.0,
                origin: [0.0, 0.0],
                yaw: 0.0,
                range: [2.5, 1.2, 0.0, -1.0],
                crop: None,
                scale_denominator: 100.0,
                show_walls: true,
                show_extensions: true,
            },
            snapshot: generic::Snapshot {
                levels: vec![generic::Level {
                    id: "l".into(),
                    name: "Level".into(),
                    elevation_metres: 0.0,
                }],
                elements: vec![generic::Element {
                    id: "e".into(),
                    owner: "org.example.columns".into(),
                    type_id: "org.example.columns.rectangular".into(),
                    name: "Column".into(),
                    payload_schema_version: if cfg!(feature = "migration-v2") { 2 } else { 1 },
                    payload: serde_json::json!({"width":0.4,"depth":0.6,height_key:3.0}),
                    relationships: Default::default(),
                    depends_on: ["l".into()].into(),
                }],
            },
        }
    }
    #[test]
    fn independent_outline_roles_and_basis_match_numeric_expectations() {
        let mut r = request();
        let lines = graphics(&r).unwrap();
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0].start, [0.0, 0.0]);
        assert_eq!(lines[0].end, [0.4, 0.0]);
        assert_eq!(lines[2].start, [0.4, 0.6]);
        assert!(lines.iter().all(|l| l.role == plan::Role::Cut));
        for (range, role) in [
            ([5.0, 4.0, 2.0, 0.0], plan::Role::Projected),
            ([5.0, 4.0, 3.0, 0.0], plan::Role::Depth),
        ] {
            r.view.range = range;
            assert!(graphics(&r).unwrap().iter().all(|l| l.role == role));
        }
        r.view.range = [6.0, 5.0, 4.0, 3.0];
        assert!(graphics(&r).unwrap().is_empty());
        r.view.range = [2.5, 1.2, 0.0, -1.0];
        r.view.origin = [10.0, -20.0];
        r.view.yaw = std::f64::consts::FRAC_PI_2;
        let lines = graphics(&r).unwrap();
        assert!((lines[0].start[0] - 20.0).abs() < 1e-12);
        assert!((lines[0].start[1] - 10.0).abs() < 1e-12);
        assert!((lines[0].end[1] - 9.6).abs() < 1e-12);
    }
    #[test]
    fn independent_service_routes_and_rejects_bad_versions_scopes_and_ranges() {
        let good = request();
        let bytes = crate::respond(&serde_json::to_vec(&good).unwrap()).unwrap();
        let response: plan::Response = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(response.request_id, good.request_id);
        assert!(matches!(response.result, plan::Reply::Graphics(_)));
        for case in 0..5 {
            let mut r = good.clone();
            match case {
                0 => r.service_version = 2,
                1 => r.view.range[1] = 9.0,
                2 => r.snapshot.levels.clear(),
                3 => r.view.level_elevation = 1.0,
                4 => r.view.origin[0] = f64::MAX,
                _ => unreachable!(),
            }
            assert!(graphics(&r).is_err());
        }
    }
}
