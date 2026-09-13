# ADR 0014: typed, versioned named plan settings

Status: accepted for the D development milestone.

## Decision

Native model schema 3 stores `ViewParams.settings_revision` and optional typed
`PlanSettings` version 1. Plans require settings; other view kinds reject them.
Settings belong to `os-model`, without geometry, renderer or UI dependencies.
They contain level-relative range, horizontal origin/yaw, rectangular crop,
scale denominator and initial wall/extension visibility. Navigation remains
session-local. These fields are not placed in an unvalidated header property bag.

Document AddView/UpdateView/RemoveView commands validate the complete candidate
graph and use existing bounded snapshot history. UpdateView owns revision
increments and cannot change view kind. Exact no-ops preserve redo. Removing the
last view is rejected unless the same batch adds a replacement; legacy documents
already containing no views remain editable. Editor named-plan helpers use these
commands and derive geometry from persisted settings, not a second view model.

Migration 2→3 supplies documented defaults and updates native headers only.
It preserves legacy unassigned plans without choosing a level on the user's behalf.
Such plans require explicit assignment before derivation/editing. Ambiguous old
fields and unsupported settings versions fail before adoption. Opening checks the
current writer's serialized model budget and does not rewrite the source archive.
Container and plugin protocol/envelope versions do not change.

## Consequences

Settings round-trip and participate in atomic undo/redo with stable entity IDs.
Drawing contexts include actual settings as well as view/session/document
revisions; undo does not make a previously published drawing current again.
Invalidation conservatively includes plan views for model changes that could
affect their contents. This is not yet a background plan cache.

Older readers reject schema 3 explicitly. View parameters are an internal model
API, not the independent plugin wire contract. The existing independent API-2
Wall/column guests need no plan-settings knowledge to keep authoring elements.
Desktop named-view controls, split workspace, plotting, finer visibility and
callable external plan providers remain subsequent work.
