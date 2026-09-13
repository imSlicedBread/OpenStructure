# Desktop plugin form view provenance

2026-09-12 prerequisite for D's independent plan interaction route.

Inspection found that host worker tickets already validated `ViewContext`, but
desktop plugin forms passed `None` when starting and polling their jobs. Forms
now bind the active plan UUID and persisted settings revision, pass that context
through ordinary commands, migrations and polling, and permanently revoke a
reviewed draft when its view context changes. Switching back does not revive it.
Revocation cancels pending tool work, disables Apply and explains that a new
review is needed. Draft inputs are retained. Three-dimensional workspace context
is still `None`; document session/revision, provider activation and selection/
level checks continue independently. Navigation does not change these model-space
command stamps.

`plugin_forms/view_tests.rs` verifies two same-level plans, unchanged document,
retained values, revocation and no authorization resurrection on return. Existing
host worker tests cover stale-view reply rejection. Full offline all-feature
workspace tests and strict all-target/all-feature Clippy pass; the default
workspace build passes. No new wire fields, registrations or capabilities are
advertised by this change.

This is **not** independent plugin pointer authoring. Begin/update/commit/cancel,
semantic preview/graphics/snap responses, explicit negotiation and installed Rust
Wall/column plan acceptance remain required by `plugin-plan-contract-design.md`.
No native-window plugin form inspection was performed in this checkpoint.
