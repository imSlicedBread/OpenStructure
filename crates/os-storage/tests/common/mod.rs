use serde_json::Value;

#[allow(dead_code)]
pub fn remove_phase_fields(value: &mut Value) {
    if let Some(views) = value["views"].as_object_mut() {
        for view in views.values_mut() {
            if let Some(plan) = view["parameters"]["plan"].as_object_mut() {
                plan.remove("target_phase");
                plan.remove("phase_filter");
                if plan["schema_version"] == 3 {
                    plan.insert("schema_version".into(), 2.into());
                }
            }
        }
    }
    if let Some(object) = value.as_object_mut() {
        object.remove("phases");
        object.remove("element_lifecycles");
    }
}

#[allow(dead_code)]
pub fn apply_schema_43_dimension_references(model: &mut Value) {
    if let Some(dimensions) = model
        .get_mut("dimensions")
        .and_then(serde_json::Value::as_object_mut)
    {
        for dimension in dimensions.values_mut() {
            let Some(parameters) = dimension.get_mut("parameters") else {
                continue;
            };
            for key in ["first", "second"] {
                parameters[key] = serde_json::json!({"WallEndpoint": parameters[key].clone()});
            }
            if let Some(additional) = parameters
                .get_mut("additional")
                .and_then(serde_json::Value::as_array_mut)
            {
                for reference in additional {
                    *reference = serde_json::json!({"WallEndpoint": reference.clone()});
                }
            }
        }
    }
}

#[allow(dead_code)]
pub fn apply_schema_38_defaults(model: &mut Value) {
    let latest = model["phases"]
        .as_object()
        .and_then(|phases| {
            phases
                .values()
                .max_by_key(|p| p["parameters"]["order"].as_u64())
        })
        .map(|p| p["header"]["id"].clone())
        .unwrap_or(Value::Null);
    model["ceilings"] = serde_json::json!({});
    if let Some(views) = model
        .get_mut("views")
        .and_then(serde_json::Value::as_object_mut)
    {
        for view in views.values_mut() {
            if let Some(plan) = view
                .get_mut("parameters")
                .and_then(|parameters| parameters.get_mut("plan"))
                .filter(|plan| !plan.is_null())
            {
                plan["schema_version"] = 3.into();
                plan["target_phase"] = latest.clone();
                plan["phase_filter"] = "ShowAll".into();
                plan["view_type"] = "FloorPlan".into();
                plan["visibility"]["ceilings"] = true.into();
            }
        }
    }
}

#[allow(dead_code)]
pub fn reverse_schema_38_migration(model: &mut Value, original: &Value) {
    if original["schema_version"]
        .as_u64()
        .is_some_and(|version| version < 44)
    {
        remove_phase_fields(model);
    }
    if original["schema_version"]
        .as_u64()
        .is_some_and(|version| version < 43)
        && let Some(dimensions) = model
            .get_mut("dimensions")
            .and_then(serde_json::Value::as_object_mut)
    {
        for dimension in dimensions.values_mut() {
            let Some(parameters) = dimension.get_mut("parameters") else {
                continue;
            };
            for key in ["first", "second"] {
                if let Some(reference) = parameters[key].get("WallEndpoint").cloned() {
                    parameters[key] = reference;
                }
            }
            if let Some(additional) = parameters
                .get_mut("additional")
                .and_then(serde_json::Value::as_array_mut)
            {
                for reference in additional {
                    if let Some(wall_endpoint) = reference.get("WallEndpoint").cloned() {
                        *reference = wall_endpoint;
                    }
                }
            }
        }
    }
    if original.get("ceilings").is_none() {
        model.as_object_mut().unwrap().remove("ceilings");
    }
    if let (Some(original_views), Some(views)) = (
        original.get("views").and_then(serde_json::Value::as_object),
        model
            .get_mut("views")
            .and_then(serde_json::Value::as_object_mut),
    ) {
        for (id, original_view) in original_views {
            if let Some(original_plan) = original_view
                .get("parameters")
                .and_then(|parameters| parameters.get("plan"))
                && let Some(parameters) = views
                    .get_mut(id)
                    .and_then(|view| view.get_mut("parameters"))
                    .and_then(serde_json::Value::as_object_mut)
            {
                parameters.insert("plan".into(), original_plan.clone());
            }
        }
    }
}
