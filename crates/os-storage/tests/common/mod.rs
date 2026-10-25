use serde_json::Value;

/// Advance only native document/entity headers and additive schema-80/81/82 maps.
/// Never recurse into parameters, properties, or extension-owned payloads.
#[allow(dead_code)]
pub fn current_native_headers(value: &mut Value) {
    value["plan_graphics_rules"] = serde_json::json!({});
    value["schema_version"] = os_model::SCHEMA_VERSION.into();
    for (key, data) in value.as_object_mut().unwrap() {
        if key == "project" {
            data["header"]["schema_version"] = os_model::SCHEMA_VERSION.into();
        } else if key != "extensions"
            && let Some(map) = data.as_object_mut()
        {
            for entity in map.values_mut().filter(|v| v.get("header").is_some()) {
                entity["header"]["schema_version"] = os_model::SCHEMA_VERSION.into();
            }
        }
    }
    value
        .as_object_mut()
        .unwrap()
        .entry("plan_hidden_openings")
        .or_insert_with(|| serde_json::json!({}));
    for key in ["group_definitions", "group_instances"] {
        value
            .as_object_mut()
            .unwrap()
            .entry(key)
            .or_insert_with(|| serde_json::json!({}));
    }
    value
        .as_object_mut()
        .unwrap()
        .entry("plan_element_graphics_overrides")
        .or_insert_with(|| serde_json::json!({}));
}

/// Separate expectations for the existing schema 76 -> 77 migration.
/// Call only after earlier family migrations have been accounted for.
#[allow(dead_code)]
pub fn apply_schema_77_defaults(value: &mut Value) {
    if let Some(types) = value
        .get_mut("opening_types")
        .and_then(Value::as_object_mut)
    {
        for ty in types.values_mut() {
            let family = &mut ty["parameters"]["family"];
            assert_eq!(family["version"], 9);
            family["version"] = 10.into();
            for solid in family["extrusions"].as_array_mut().unwrap() {
                assert!(solid.get("bindings").is_none());
                solid["bindings"] = serde_json::json!({
                    "x": null, "y": null, "depth": null, "normal_offset": null
                });
            }
        }
    }
    if let Some(parameters) = value
        .get_mut("length_parameters")
        .and_then(Value::as_object_mut)
    {
        for parameter in parameters.values_mut() {
            assert!(parameter["parameters"].get("formula").is_none());
            parameter["parameters"]["formula"] = Value::Null;
        }
    }
}

#[allow(dead_code)]
pub fn remove_curtains(value: &mut Value) {
    value.as_object_mut().unwrap().remove("plan_graphics_rules");
    for key in [
        "curtain_systems",
        "curtain_panel_types",
        "curtain_mullion_types",
    ] {
        if let Some(removed) = value.as_object_mut().unwrap().remove(key) {
            assert_eq!(
                removed,
                serde_json::json!({}),
                "legacy fixture cannot discard curtain data"
            );
        }
    }
}

#[allow(dead_code)]
pub fn remove_railings(value: &mut Value) {
    value.as_object_mut().unwrap().remove("plan_graphics_rules");
    remove_extrusions(value);
    for key in ["group_definitions", "group_instances"] {
        if let Some(removed) = value.as_object_mut().unwrap().remove(key) {
            assert_eq!(
                removed,
                serde_json::json!({}),
                "legacy fixture cannot discard model-group data"
            );
        }
    }
    for key in ["railings", "railing_types"] {
        if let Some(removed) = value.as_object_mut().unwrap().remove(key) {
            assert_eq!(
                removed,
                serde_json::json!({}),
                "legacy fixture cannot discard railing data"
            );
        }
    }
    remove_ramps(value);
    remove_casework(value);
}

#[allow(dead_code)]
pub fn remove_extrusions(value: &mut Value) {
    value.as_object_mut().unwrap().remove("plan_graphics_rules");
    if let Some(hidden) = value
        .as_object_mut()
        .unwrap()
        .remove("plan_hidden_openings")
    {
        assert_eq!(
            hidden,
            serde_json::json!({}),
            "legacy fixture cannot discard hidden openings"
        );
    }
    if let Some(types) = value
        .get_mut("opening_types")
        .and_then(Value::as_object_mut)
    {
        for ty in types.values_mut() {
            let family = &mut ty["parameters"]["family"];
            if let Some(extrusions) = family.as_object_mut().unwrap().remove("extrusions") {
                assert_eq!(extrusions, serde_json::json!([]));
            }
            if matches!(family["version"].as_u64(), Some(7..=10)) {
                family["version"] = 6.into();
            }
        }
    }
}

#[allow(dead_code)]
pub fn remove_casework(value: &mut Value) {
    value.as_object_mut().unwrap().remove("plan_graphics_rules");
    if let Some(patterns) = value.as_object_mut().unwrap().remove("material_patterns") {
        assert_eq!(patterns, serde_json::json!({}));
    }
    for key in ["casework_types", "casework"] {
        if let Some(removed) = value.as_object_mut().unwrap().remove(key) {
            assert_eq!(
                removed,
                serde_json::json!({}),
                "legacy fixture cannot discard casework data"
            );
        }
    }
}

#[allow(dead_code)]
pub fn remove_ramps(value: &mut Value) {
    value.as_object_mut().unwrap().remove("plan_graphics_rules");
    if let Some(removed) = value.as_object_mut().unwrap().remove("ramps") {
        assert_eq!(
            removed,
            serde_json::json!({}),
            "legacy fixture cannot discard ramp data"
        );
    }
}

#[allow(dead_code)]
pub fn remove_door_leaves(value: &mut Value) {
    value.as_object_mut().unwrap().remove("plan_graphics_rules");
    remove_railings(value);
    remove_curtains(value);
    if let Some(types) = value
        .get_mut("opening_types")
        .and_then(Value::as_object_mut)
    {
        for ty in types.values_mut() {
            let family = &mut ty["parameters"]["family"];
            if family["version"] == 6 {
                family["version"] = 5.into();
            }
            family.as_object_mut().unwrap().remove("door_leaves");
        }
    }
}

/// Test-only reversal of the documented 53 -> 54 representation change.
/// Assert Straight: a legacy fixture must never silently chord-convert an arc.
#[allow(dead_code)]
pub fn reverse_wall_paths(value: &mut Value) {
    value.as_object_mut().unwrap().remove("plan_graphics_rules");
    remove_railings(value);
    remove_door_leaves(value);
    for key in ["group_definitions", "group_instances"] {
        if let Some(removed) = value.as_object_mut().unwrap().remove(key) {
            assert_eq!(
                removed,
                serde_json::json!({}),
                "legacy fixture cannot discard model-group data"
            );
        }
    }
    for key in [
        "sheet_sets",
        "drawing_issues",
        "plan_callouts",
        "section_callouts",
        "text_notes",
    ] {
        if let Some(removed) = value.as_object_mut().unwrap().remove(key) {
            assert_eq!(
                removed,
                serde_json::json!({}),
                "legacy fixture cannot discard drawing issue data"
            );
        }
    }
    if let Some(openings) = value.get_mut("openings").and_then(Value::as_object_mut) {
        for opening in openings.values_mut() {
            opening["parameters"]
                .as_object_mut()
                .unwrap()
                .remove("open_state");
        }
    }
    value.as_object_mut().unwrap().remove("length_parameters");
    value
        .as_object_mut()
        .unwrap()
        .remove("opening_type_length_bindings");
    value.as_object_mut().unwrap().remove("opening_clearances");
    if let Some(walls) = value.get_mut("walls").and_then(Value::as_object_mut) {
        for wall in walls.values_mut() {
            let p = wall["parameters"].as_object_mut().unwrap();
            if let Some(path) = p.remove("path") {
                assert_eq!(path["kind"], "Straight");
                assert!(!p.contains_key("start") && !p.contains_key("end"));
                p.insert("start".into(), path["start"].clone());
                p.insert("end".into(), path["end"].clone());
            }
        }
    }
}
#[allow(dead_code)]
pub fn apply_wall_paths(value: &mut Value) {
    value["plan_hidden_openings"] = serde_json::json!({});
    value["group_definitions"] = serde_json::json!({});
    value["group_instances"] = serde_json::json!({});
    // Current-schema collections added after the frozen migration fixtures.
    value["sheet_sets"] = serde_json::json!({});
    value["drawing_issues"] = serde_json::json!({});
    value["plan_callouts"] = serde_json::json!({});
    value["section_callouts"] = serde_json::json!({});
    value["text_notes"] = serde_json::json!({});
    value["material_patterns"] = serde_json::json!({});
    value["railings"] = serde_json::json!({});
    value["railing_types"] = serde_json::json!({});
    value["ramps"] = serde_json::json!({});
    value["casework_types"] = serde_json::json!({});
    value["casework"] = serde_json::json!({});
    for key in [
        "curtain_systems",
        "curtain_panel_types",
        "curtain_mullion_types",
    ] {
        value[key] = serde_json::json!({});
    }
    if let Some(types) = value
        .get_mut("opening_types")
        .and_then(Value::as_object_mut)
    {
        for ty in types.values_mut() {
            ty["parameters"]["family"]["version"] = 10.into();
            ty["parameters"]["family"]["extrusions"] = serde_json::json!([]);
            ty["parameters"]["family"]["door_leaves"] = "Single".into();
        }
    }
    if let Some(openings) = value.get_mut("openings").and_then(Value::as_object_mut) {
        for opening in openings.values_mut() {
            opening["parameters"]["open_state"] = serde_json::json!("Default");
        }
    }
    if let Some(walls) = value.get_mut("walls").and_then(Value::as_object_mut) {
        for wall in walls.values_mut() {
            let p = wall["parameters"].as_object_mut().unwrap();
            if !p.contains_key("path") {
                let start = p.remove("start").unwrap();
                let end = p.remove("end").unwrap();
                p.insert(
                    "path".into(),
                    serde_json::json!({"kind":"Straight","start":start,"end":end}),
                );
            }
        }
    }
    // Expected current-schema form for fixtures predating persistent opening
    // clearances: old projects migrate with no authored locks.
    value["opening_clearances"] = serde_json::json!({});
    value["length_parameters"] = serde_json::json!({});
    value["opening_type_length_bindings"] = serde_json::json!({});
}

#[allow(dead_code)]
pub fn remove_phase_fields(value: &mut Value) {
    value.as_object_mut().unwrap().remove("plan_graphics_rules");
    reverse_wall_paths(value);
    remove_grouping(value);
    remove_window_operation(value);
    if let Some(openings) = value.get_mut("openings").and_then(Value::as_object_mut) {
        for opening in openings.values_mut() {
            opening["parameters"]
                .as_object_mut()
                .unwrap()
                .remove("pane_position_override");
        }
    }
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
        object.remove("sheet_sets");
        object.remove("drawing_issues");
        object.remove("plan_callouts");
        object.remove("section_callouts");
        object.remove("text_notes");
        object.remove("plan_hidden_openings");
        object.remove("group_definitions");
        object.remove("group_instances");
    }
}

#[allow(dead_code)]
pub fn remove_window_operation(value: &mut Value) {
    value.as_object_mut().unwrap().remove("plan_graphics_rules");
    if let Some(types) = value
        .get_mut("opening_types")
        .and_then(serde_json::Value::as_object_mut)
    {
        for ty in types.values_mut() {
            if let Some(parameters) = ty
                .get_mut("parameters")
                .and_then(serde_json::Value::as_object_mut)
            {
                parameters.remove("window_operation");
            }
        }
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
                plan["schema_version"] = 4.into();
                plan["target_phase"] = latest.clone();
                plan["phase_filter"] = "ShowAll".into();
                plan["view_type"] = "FloorPlan".into();
                plan["visibility"]["ceilings"] = true.into();
                plan["visibility"]["doors"] = true.into();
                plan["visibility"]["windows"] = true.into();
            }
        }
    }
}

#[allow(dead_code)]
pub fn reverse_schema_38_migration(model: &mut Value, original: &Value) {
    model.as_object_mut().unwrap().remove("plan_graphics_rules");
    if original.get("material_patterns").is_none() {
        model.as_object_mut().unwrap().remove("material_patterns");
    }
    for collection in ["sheet_sets", "drawing_issues", "plan_callouts"] {
        if original.get(collection).is_none() {
            model.as_object_mut().unwrap().remove(collection);
        }
    }
    if original["schema_version"].as_u64().is_some_and(|v| v < 54) {
        reverse_wall_paths(model);
    }
    if original["schema_version"].as_u64().is_some_and(|v| v < 52)
        && let Some(types) = model
            .get_mut("opening_types")
            .and_then(serde_json::Value::as_object_mut)
    {
        for ty in types.values_mut() {
            ty["parameters"]
                .as_object_mut()
                .unwrap()
                .remove("window_operation");
        }
    }
    if original["schema_version"].as_u64().is_some_and(|v| v < 51) {
        remove_lite_override(model);
    }
    if original["schema_version"].as_u64().is_some_and(|v| v < 48) {
        remove_schedule_phase(model);
    }
    if original["schema_version"].as_u64().is_some_and(|v| v < 47) {
        remove_grouping(model);
    }
    if original["schema_version"].as_u64().is_some_and(|v| v < 46)
        && let Some(openings) = model.get_mut("openings").and_then(Value::as_object_mut)
    {
        for opening in openings.values_mut() {
            opening["parameters"]
                .as_object_mut()
                .unwrap()
                .remove("pane_position_override");
        }
    }
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
    for collection in ["casework_types", "casework"] {
        if original.get(collection).is_none() {
            model.as_object_mut().unwrap().remove(collection);
        }
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
    if original.get("opening_types").is_none() {
        model.as_object_mut().unwrap().remove("opening_types");
    }
}

#[allow(dead_code)]
pub fn remove_grouping(value: &mut Value) {
    value.as_object_mut().unwrap().remove("plan_graphics_rules");
    remove_schedule_phase(value);
    if let Some(schedules) = value.get_mut("schedules").and_then(Value::as_object_mut) {
        for schedule in schedules.values_mut() {
            schedule["parameters"]
                .as_object_mut()
                .unwrap()
                .remove("group_by");
        }
    }
}

#[allow(dead_code)]
pub fn remove_schedule_phase(value: &mut Value) {
    value.as_object_mut().unwrap().remove("plan_graphics_rules");
    remove_opening_visibility(value);
    if let Some(schedules) = value.get_mut("schedules").and_then(Value::as_object_mut) {
        for schedule in schedules.values_mut() {
            schedule["parameters"]
                .as_object_mut()
                .unwrap()
                .remove("phase");
        }
    }
}

#[allow(dead_code)]
pub fn remove_opening_visibility(value: &mut Value) {
    value.as_object_mut().unwrap().remove("plan_graphics_rules");
    remove_door_leaves(value);
    remove_window_operation(value);
    remove_lite_override(value);
    if let Some(types) = value
        .get_mut("opening_types")
        .and_then(Value::as_object_mut)
    {
        for ty in types.values_mut() {
            let family = &mut ty["parameters"]["family"];
            if family["version"] == 5 {
                family["version"] = 4.into();
            }
            family.as_object_mut().unwrap().remove("side_lite");
        }
    }
    if let Some(views) = value["views"].as_object_mut() {
        for view in views.values_mut() {
            if let Some(plan) = view["parameters"]["plan"].as_object_mut() {
                if plan["schema_version"] == 4 {
                    plan.insert("schema_version".into(), 3.into());
                }
                if let Some(visibility) = plan.get_mut("visibility").and_then(Value::as_object_mut)
                {
                    visibility.remove("doors");
                    visibility.remove("windows");
                }
            }
        }
    }
}

#[allow(dead_code)]
pub fn apply_opening_visibility(value: &mut Value) {
    if let Some(types) = value
        .get_mut("opening_types")
        .and_then(Value::as_object_mut)
    {
        for ty in types.values_mut() {
            ty["parameters"]["window_operation"] = "Fixed".into();
        }
    }
    apply_lite_override(value);
    if let Some(types) = value
        .get_mut("opening_types")
        .and_then(Value::as_object_mut)
    {
        for ty in types.values_mut() {
            ty["parameters"]["family"]["version"] = 10.into();
            ty["parameters"]["family"]["extrusions"] = serde_json::json!([]);
            ty["parameters"]["family"]["door_leaves"] = "Single".into();
            ty["parameters"]["family"]["side_lite"] = Value::Null;
        }
    }
    for view in value["views"].as_object_mut().unwrap().values_mut() {
        let plan = &mut view["parameters"]["plan"];
        if !plan.is_null() {
            plan["schema_version"] = 4.into();
            plan["visibility"]["doors"] = true.into();
            plan["visibility"]["windows"] = true.into();
        }
    }
}

#[allow(dead_code)]
pub fn remove_lite_override(value: &mut Value) {
    value.as_object_mut().unwrap().remove("plan_graphics_rules");
    if let Some(openings) = value.get_mut("openings").and_then(Value::as_object_mut) {
        for opening in openings.values_mut() {
            opening["parameters"]
                .as_object_mut()
                .unwrap()
                .remove("lite_side_override");
        }
    }
}

#[allow(dead_code)]
pub fn apply_lite_override(value: &mut Value) {
    if let Some(openings) = value.get_mut("openings").and_then(Value::as_object_mut) {
        for opening in openings.values_mut() {
            opening["parameters"]["lite_side_override"] = Value::Null;
        }
    }
}
