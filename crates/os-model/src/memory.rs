//! Conservative accounting for owned snapshot data, not an allocator/RSS meter.
use crate::*;
use serde_json::Value;
use std::mem::size_of;

#[derive(Default)]
struct Estimate(usize);

impl Estimate {
    fn add(&mut self, bytes: usize) {
        self.0 = self.0.saturating_add(bytes);
    }

    // Charge a generously sized full node per entry, including child pointers.
    // Rust does not guarantee BTreeMap's layout. This is a retention accounting
    // policy, deliberately not a promise about allocator overhead or peak RSS.
    fn tree<K, V>(&mut self, len: usize) {
        let node = size_of::<(K, V)>()
            .saturating_mul(16)
            .saturating_add(16 * size_of::<usize>() + 64);
        self.add(len.saturating_mul(node));
    }

    fn relationships(&mut self, values: &BTreeMap<String, Vec<Id>>) {
        self.tree::<String, Vec<Id>>(values.len());
        for (key, ids) in values {
            self.add(key.capacity());
            self.add(ids.capacity().saturating_mul(size_of::<Id>()));
        }
    }

    fn json(&mut self, value: &Value) {
        match value {
            Value::Null | Value::Bool(_) => {}
            // arbitrary_precision stores the decimal spelling; cloning a Number
            // copies that string. No serialization or precision loss is involved.
            Value::Number(number) => self.add(number.as_str().len()),
            Value::String(text) => self.add(text.capacity()),
            Value::Array(values) => {
                self.add(values.capacity().saturating_mul(size_of::<Value>()));
                for value in values {
                    self.json(value);
                }
            }
            Value::Object(values) => {
                self.tree::<String, Value>(values.len());
                for (key, value) in values {
                    self.add(key.capacity());
                    self.json(value);
                }
            }
        }
    }

    fn header(&mut self, header: &Header) {
        let Header {
            id: _,
            type_id,
            schema_version: _,
            properties,
            relationships,
        } = header;
        self.add(type_id.capacity());
        self.tree::<String, Value>(properties.len());
        for (key, value) in properties {
            self.add(key.capacity());
            self.json(value);
        }
        self.relationships(relationships);
    }

    fn entity<T: NamedParameters>(&mut self, entity: &Entity<T>) {
        let Entity { header, parameters } = entity;
        self.header(header);
        self.add(parameters.name().capacity());
    }
}

trait NamedParameters {
    fn name(&self) -> &String;
}

// Exhaustive patterns make adding a new parameter field require an accounting
// review. The listed fields contain no owned heap allocations.
macro_rules! named_parameters {
    ($ty:ty $(, $fixed:ident)*) => {
        impl NamedParameters for $ty {
            fn name(&self) -> &String {
                let Self { name, $($fixed: _,)* } = self;
                name
            }
        }
    };
}
named_parameters!(ProjectParams);
named_parameters!(SiteParams, project);
named_parameters!(BuildingParams, site);
named_parameters!(LevelParams, elevation, building);
named_parameters!(GridParams, building, start, end);
named_parameters!(MaterialParams, density_kg_m3, color);
named_parameters!(PlanGraphicsTemplateParams, styles);
named_parameters!(PhaseParams, order);
named_parameters!(CurtainPanelTypeParams, kind, thickness, material);
named_parameters!(CurtainMullionTypeParams, width, depth, material);
named_parameters!(StairRailingParams, stair, railing_type, side);
named_parameters!(
    StairRailingTypeParams,
    top_rail_height,
    top_rail_width,
    top_rail_depth,
    post_width,
    post_depth,
    max_post_spacing,
    material
);
// Plan settings contain only fixed-size scalars/points/options; inline entity
// size already accounts for them. Keep this pattern exhaustive on schema changes.
named_parameters!(ViewParams, kind, level, settings_revision, plan, section);
named_parameters!(WallParams, path, thickness, height, level, material);
named_parameters!(
    StairParams,
    lower_level,
    upper_level,
    start,
    end,
    width,
    riser_count,
    structural_thickness,
    material
);
named_parameters!(
    RampParams,
    lower_level,
    upper_level,
    start,
    end,
    width,
    structural_thickness,
    material
);
named_parameters!(
    ColumnParams,
    level,
    center,
    width,
    depth,
    height,
    base_offset,
    material
);
named_parameters!(CaseworkTypeParams, width, depth, height, material);
named_parameters!(CaseworkParams, type_id, level, center, yaw, base_offset);
named_parameters!(
    OpeningParams,
    open_state,
    host,
    offset,
    definition,
    width_override,
    height_override,
    sill_override,
    pane_position_override,
    lite_side_override,
    hinge,
    swing
);
// Family profile allocation is counted explicitly below.
named_parameters!(
    OpeningTypeParams,
    window_operation,
    kind,
    width,
    height,
    sill,
    pane_position,
    family
);

impl Model {
    /// Retention-policy estimate including inline data and owned allocations.
    ///
    /// Counts string/vector capacities and pessimistic tree-node charges; excludes
    /// allocator bookkeeping, temporary clones, geometry and document attachments.
    /// This is not an exact heap measurement or a process-memory guarantee. New
    /// owned parameter fields must be added here when expanding the model schema.
    pub fn estimated_memory_bytes(&self) -> usize {
        let Model {
            schema_version: _,
            project,
            sites,
            buildings,
            levels,
            walls,
            wall_types,
            wall_type_assignments,
            wall_joins,
            floors,
            stairs,
            ramps,
            railings,
            railing_types,
            curtain_systems,
            curtain_panel_types,
            curtain_mullion_types,
            roofs,
            ceilings,
            columns,
            casework_types,
            casework,
            openings,
            opening_clearances,
            opening_types,
            length_parameters,
            opening_type_length_bindings,
            dimensions,
            rooms,
            room_separation_lines,
            phases,
            element_lifecycles,
            room_tags,
            opening_tags,
            detail_lines,
            grids,
            materials,
            views,
            plan_graphics_templates,
            plan_graphics,
            sheets,
            schedules,
            extensions,
            plugin_requirements,
        } = self;
        let mut estimate = Estimate(size_of::<Self>());
        estimate.tree::<Id, OpeningClearance>(opening_clearances.len());
        estimate.tree::<Id, OpeningTypeLengthBindings>(opening_type_length_bindings.len());
        estimate.tree::<Id, LengthParameter>(length_parameters.len());
        for parameter in length_parameters.values() {
            estimate.header(&parameter.header);
            estimate.add(parameter.parameters.name.capacity());
        }
        estimate.entity(project);
        macro_rules! entities {
            ($values:ident, $ty:ty) => {
                estimate.tree::<Id, $ty>($values.len());
                for entity in $values.values() {
                    estimate.entity(entity);
                }
            };
        }
        entities!(sites, Site);
        entities!(buildings, Building);
        entities!(levels, Level);
        entities!(plan_graphics_templates, PlanGraphicsTemplate);
        estimate.tree::<Id, PlanGraphicsBinding>(plan_graphics.len());
        entities!(walls, Wall);
        entities!(columns, Column);
        entities!(casework_types, CaseworkType);
        entities!(casework, Casework);
        entities!(stairs, Stair);
        entities!(ramps, Ramp);
        entities!(railings, StairRailing);
        entities!(railing_types, StairRailingType);
        entities!(curtain_panel_types, CurtainPanelType);
        entities!(curtain_mullion_types, CurtainMullionType);
        estimate.tree::<Id, CurtainSystem>(curtain_systems.len());
        for assembly in curtain_systems.values() {
            estimate.header(&assembly.header);
            let p = &assembly.parameters;
            estimate.add(p.name.capacity());
            estimate.add(
                p.vertical
                    .capacity()
                    .saturating_add(p.horizontal.capacity())
                    .saturating_mul(size_of::<CurtainGrid>()),
            );
            estimate.add(
                p.panels
                    .capacity()
                    .saturating_mul(size_of::<CurtainPanel>()),
            );
            estimate.add(
                p.mullions
                    .capacity()
                    .saturating_mul(size_of::<CurtainMullion>()),
            );
        }
        estimate.tree::<Id, Ceiling>(ceilings.len());
        for ceiling in ceilings.values() {
            estimate.header(&ceiling.header);
            let CeilingParams {
                name,
                level: _,
                material: _,
                boundary_room: _,
                boundary,
                holes,
                thickness: _,
                elevation_offset: _,
            } = &ceiling.parameters;
            estimate.add(name.capacity());
            estimate.add(
                boundary
                    .capacity()
                    .saturating_mul(size_of::<os_core::Point2>()),
            );
            estimate.add(
                holes
                    .capacity()
                    .saturating_mul(size_of::<Vec<os_core::Point2>>()),
            );
            for hole in holes {
                estimate.add(hole.capacity().saturating_mul(size_of::<os_core::Point2>()));
            }
        }
        estimate.tree::<Id, Roof>(roofs.len());
        for roof in roofs.values() {
            estimate.header(&roof.header);
            let RoofParams {
                name,
                level: _,
                material: _,
                boundary,
                holes,
                thickness: _,
                top_offset: _,
                slope_start: _,
                slope_end: _,
                rise_per_run: _,
            } = &roof.parameters;
            estimate.add(name.capacity());
            estimate.add(
                boundary
                    .capacity()
                    .saturating_mul(size_of::<os_core::Point2>()),
            );
            estimate.add(
                holes
                    .capacity()
                    .saturating_mul(size_of::<Vec<os_core::Point2>>()),
            );
            for hole in holes {
                estimate.add(hole.capacity().saturating_mul(size_of::<os_core::Point2>()));
            }
        }
        estimate.tree::<Id, WallTypeAssignment>(wall_type_assignments.len());
        estimate.tree::<Id, WallType>(wall_types.len());
        for ty in wall_types.values() {
            estimate.header(&ty.header);
            estimate.add(ty.parameters.name.capacity());
            estimate.add(
                ty.parameters
                    .layers
                    .capacity()
                    .saturating_mul(size_of::<WallLayer>()),
            );
            for layer in &ty.parameters.layers {
                estimate.add(layer.name.capacity());
            }
        }
        estimate.tree::<Id, WallJoin>(wall_joins.len());
        for join in wall_joins.values() {
            estimate.header(&join.header);
        }
        estimate.tree::<Id, Floor>(floors.len());
        for floor in floors.values() {
            estimate.header(&floor.header);
            let FloorParams {
                name,
                level: _,
                material: _,
                boundary,
                holes,
                thickness: _,
                top_offset: _,
            } = &floor.parameters;
            estimate.add(name.capacity());
            estimate.add(
                boundary
                    .capacity()
                    .saturating_mul(size_of::<os_core::Point2>()),
            );
            estimate.add(
                holes
                    .capacity()
                    .saturating_mul(size_of::<Vec<os_core::Point2>>()),
            );
            for ring in holes {
                estimate.add(ring.capacity().saturating_mul(size_of::<os_core::Point2>()));
            }
        }
        entities!(openings, Opening);
        entities!(opening_types, OpeningType);
        for ty in opening_types.values() {
            estimate.add(
                ty.parameters
                    .family
                    .cut_profile
                    .capacity()
                    .saturating_mul(size_of::<os_core::Point2>()),
            );
            estimate.add(
                ty.parameters
                    .family
                    .profile
                    .capacity()
                    .saturating_mul(size_of::<os_core::Point2>()),
            );
        }
        estimate.tree::<Id, Dimension>(dimensions.len());
        for dimension in dimensions.values() {
            estimate.header(&dimension.header);
            estimate.add(
                dimension
                    .parameters
                    .additional
                    .capacity()
                    .saturating_mul(size_of::<DimensionReference>()),
            );
        }
        estimate.tree::<Id, RoomSeparationLine>(room_separation_lines.len());
        for line in room_separation_lines.values() {
            estimate.header(&line.header);
        }
        entities!(phases, Phase);
        estimate.tree::<Id, ElementLifecycle>(element_lifecycles.len());
        estimate.tree::<Id, Room>(rooms.len());
        estimate.tree::<Id, RoomTag>(room_tags.len());
        estimate.tree::<Id, OpeningTag>(opening_tags.len());
        for tag in opening_tags.values() {
            estimate.header(&tag.header);
        }
        estimate.tree::<Id, DetailLine>(detail_lines.len());
        for line in detail_lines.values() {
            estimate.header(&line.header);
        }
        for tag in room_tags.values() {
            estimate.header(&tag.header);
        }
        for room in rooms.values() {
            estimate.header(&room.header);
            let RoomParams {
                number,
                name,
                level: _,
                seed: _,
                boundary_signature,
                floor_finish,
                wall_finish,
                ceiling_finish,
                floor_material: _,
                wall_material: _,
                ceiling_material: _,
            } = &room.parameters;
            estimate.add(number.capacity());
            estimate.add(name.capacity());
            for code in [floor_finish, wall_finish, ceiling_finish]
                .into_iter()
                .flatten()
            {
                estimate.add(code.capacity());
            }
            estimate.add(
                boundary_signature
                    .capacity()
                    .saturating_mul(size_of::<(Id, bool)>()),
            );
        }
        entities!(grids, Grid);
        entities!(materials, Material);
        entities!(views, View);
        estimate.tree::<Id, Schedule>(schedules.len());
        for schedule in schedules.values() {
            estimate.header(&schedule.header);
            let ScheduleParams {
                name,
                category: _,
                columns,
                sort: _,
                filters,
                group_by,
                phase: _,
            } = &schedule.parameters;
            estimate.add(
                group_by
                    .capacity()
                    .saturating_mul(size_of::<ScheduleGroupField>()),
            );
            estimate.add(name.capacity());
            estimate.add(
                filters
                    .capacity()
                    .saturating_mul(size_of::<ScheduleFilter>()),
            );
            for filter in filters {
                if let ScheduleFilter::Text { value, .. } = filter {
                    estimate.add(value.capacity());
                }
            }
            estimate.add(
                columns
                    .capacity()
                    .saturating_mul(size_of::<ScheduleColumn>()),
            );
        }
        estimate.tree::<Id, Sheet>(sheets.len());
        for sheet in sheets.values() {
            estimate.header(&sheet.header);
            let SheetParams {
                number,
                name,
                paper_size: _,
                viewports,
                schedule_placements,
            } = &sheet.parameters;
            estimate.add(
                schedule_placements
                    .capacity()
                    .saturating_mul(size_of::<SheetSchedulePlacement>()),
            );
            estimate.add(number.capacity());
            estimate.add(name.capacity());
            estimate.add(
                viewports
                    .capacity()
                    .saturating_mul(size_of::<SheetViewport>()),
            );
            for viewport in viewports {
                let SheetViewport {
                    id: _,
                    view: _,
                    model_center_m: _,
                    paper_center_mm: _,
                    width_mm: _,
                    height_mm: _,
                    scale_denominator: _,
                    title_override,
                } = viewport;
                if let Some(title) = title_override {
                    estimate.add(title.capacity());
                }
            }
        }
        estimate.tree::<Id, ExtensionEntity>(extensions.len());
        for entity in extensions.values() {
            let ExtensionEntity {
                envelope_version: _,
                id: _,
                owner,
                type_id,
                name,
                payload_schema_version: _,
                relationships,
                depends_on,
                payload,
            } = entity;
            estimate.add(owner.capacity());
            estimate.add(type_id.capacity());
            estimate.add(name.capacity());
            estimate.relationships(relationships);
            estimate.tree::<Id, ()>(depends_on.len());
            estimate.json(payload);
        }
        estimate.tree::<String, PluginRequirement>(plugin_requirements.len());
        for (key, PluginRequirement { version }) in plugin_requirements {
            estimate.add(key.capacity());
            estimate.add(version.capacity());
        }
        estimate.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opening_types_account_for_map_headers_and_owned_name_capacity() {
        let mut model = Model::new("Types");
        let empty = model.estimated_memory_bytes();
        let ty = OpeningType::new(
            "core.opening_type",
            OpeningTypeParams {
                window_operation: Default::default(),
                family: Default::default(),
                name: "Door".into(),
                pane_position: Default::default(),
                kind: OpeningKind::Door,
                width: 0.9,
                height: 2.1,
                sill: 0.0,
            },
        );
        let id = ty.id();
        model.opening_types.insert(id, ty);
        let with_type = model.estimated_memory_bytes();
        assert!(with_type > empty + size_of::<OpeningType>());
        model
            .opening_types
            .get_mut(&id)
            .unwrap()
            .parameters
            .name
            .reserve(4096);
        assert!(model.estimated_memory_bytes() >= with_type + 4096);
        let before = model.estimated_memory_bytes();
        model
            .opening_types
            .get_mut(&id)
            .unwrap()
            .header
            .properties
            .insert("note".into(), Value::String("x".repeat(2048)));
        assert!(model.estimated_memory_bytes() >= before + 2048);
    }

    #[test]
    fn accounts_for_owned_payloads_and_capacity_without_changing_numbers() {
        let mut model = Model::new("Memory");
        let base = model.estimated_memory_bytes();
        model.project.parameters.name.reserve(4096);
        assert!(model.estimated_memory_bytes() >= base + 4096);
        let before = model.estimated_memory_bytes();
        let value: Value =
            serde_json::from_str(r#"{"array":["opaque",123456789012345678901234567890.000001]}"#)
                .unwrap();
        model
            .project
            .header
            .properties
            .insert("vendor".into(), value.clone());
        let with_payload = model.estimated_memory_bytes();
        assert!(with_payload > before + 100);
        assert_eq!(model.project.header.properties["vendor"], value);
        model
            .project
            .header
            .relationships
            .insert("self".into(), vec![model.project.id(); 100]);
        assert!(model.estimated_memory_bytes() >= with_payload + 100 * size_of::<Id>());
    }
}
