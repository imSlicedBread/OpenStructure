# ADR 0009: semantic extension preservation before generic authoring

Status: accepted; milestone C foundation, not completion of the B/C gate.

Write model schema 2 within container 2. Migrate schema 0 through 1, and 1 to 2,
in memory with unchanged UUIDs. Older model readers reject schema 2. Native
headers track the model schema; plugin payload versions are independent.

Store extension entities in a separate UUID map, with envelope version 1,
owner plugin ID, namespaced type ID, name, positive payload schema version,
named relationships, explicit regeneration dependencies and opaque JSON payload.
Record exact numeric major.minor.patch plugin requirements. No project contents
are installed or executed. Unknown payload contents survive semantically (JSON
whitespace/key ordering are not byte-preserved). Unknown envelope fields/versions
are rejected rather than silently lost. Future metadata belongs in the payload
or a separately versioned envelope. This does not promise preservation of arbitrary
unknown fields in existing native entity structs.

Bound each serialized envelope to 1 MiB, combined envelopes to 16 MiB, entity count
to 10,000, requirements to 1,024 and payload nesting to 32. Validate globally unique
non-nil IDs, owner requirements and all references. Explicit `depends_on` edges
between extensions must be acyclic; ordinary named relationships may be cyclic.
All references protect deletion and trigger conservative transitive invalidation.

Add atomic core commands for requirements and extension add/replace/remove.
Replacement checks the previous payload schema and cannot change owner/type/ID;
a migration proposal uses the same copy/validate/commit and undo machinery.
This is a migration commit boundary, not an executable plugin migration service.
V1 plugin replies remain wall-only and cannot use these commands. A future
generic contract must authorize owned types and scoped objects before dispatch.

Until extension providers are callable, all extension geometry/editing is
unavailable, even if an owner manifest is present. The desktop exposes read-only
inspection and a persistent warning; native save preserves the data. IFC wall
export refuses extension entities, including with loss consent, until explicit
mappings or a separately designed element-exclusion workflow exists.
