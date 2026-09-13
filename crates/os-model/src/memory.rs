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
named_parameters!(MaterialParams, density_kg_m3);
// Plan settings contain only fixed-size scalars/points/options; inline entity
// size already accounts for them. Keep this pattern exhaustive on schema changes.
named_parameters!(ViewParams, kind, level, settings_revision, plan);
named_parameters!(WallParams, start, end, thickness, height, level, material);

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
            grids,
            materials,
            views,
            extensions,
            plugin_requirements,
        } = self;
        let mut estimate = Estimate(size_of::<Self>());
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
        entities!(walls, Wall);
        entities!(grids, Grid);
        entities!(materials, Material);
        entities!(views, View);
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
