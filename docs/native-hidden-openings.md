# Persistent hidden openings in floor plans

Native schema 80 / API 61 adds persistent, independent hiding of native doors
and windows in authored FloorPlans, including enlarged-plan targets. Select
1–256 visible openings of one category and choose **Hide openings**. Mixed,
stale, non-native, off-level or invisible selections fail as a whole.

**Hidden openings…** lists each opening's name, kind and UUID. **Restore selected**
and **Restore all** stage changes; **Apply restore** commits one transaction.
Cancel, Escape, active-view changes and document session/revision changes discard
the draft. The modal cancels authoring gestures and isolates canvas/shortcut
input. Restoring only clears instance hide flags: category, phase, view-range
and crop rules still determine visibility.

The required `plan_hidden_openings: BTreeMap<Id, BTreeSet<Id>>` stores at most
4096 IDs per view and 10000 assignments per project. Empty entries are omitted.
Only native openings with hosts on the owning FloorPlan's level are valid.
`SetPlanHiddenOpenings` replaces the full set atomically, validates explicit
requests before lifecycle pruning and against the final transaction graph, and
does not record a no-op. Both plain and detailed duplication copy the same
opening IDs into the new independent view's set. View deletion removes its set;
opening deletion, rehosting, host-level and plan-level edits prune incompatible
assignments within the same undoable transaction.

Drawing exclusions apply after wall-aperture construction. Hidden openings lose
their symbols, tags, attached dimensions, click/overlap/marquee picks and snaps;
the host aperture stays open. Native canvas, sheet preview and vector PDF share
that drawing. Model geometry, rooms, schedules, 3D and other views remain intact.

Schema 79 migration rejects any pre-existing hide map, including null and empty,
then inserts `{}` and advances native headers. Current wire requires the map and
rejects duplicate IDs/keys, empty entries, invalid references and excess limits.
The frozen schema-79 fixture and strict storage tests cover migration and archive
round trips. Native API 61 and older full-model guests must rebuild/reinstall.
Opening family/package 10, generic API 2, container 2 and issue bundles are unchanged.

Evidence is in document/storage `tests/hidden_openings.rs` and UI
`plan_workspace/hidden_openings/tests.rs`: atomic failures, no-op/history,
duplication, lifecycle pruning, strict wire bounds, persistence, straight and
both signed-arc hosts, aperture and other-view preservation, tags/dimensions,
selection/snapping and restored PDF equality. Real-egui event tests exercise
1280×800 at 100% and 1000×650 at 150%, selected/all restores, cancellation,
stale drafts, gesture cancellation and modal canvas/shortcut isolation.

Validation on 2026-10-11 passed all `os-storage --tests --no-fail-fast` tests,
the three document and three UI hidden-opening tests, two storage hidden-opening
tests, the migration-helper regression, six document/two UI duplication tests,
six plugin API tests, and all 26 Wasm transport tests. The document/storage/UI
all-features check and `git diff --check` passed. Wasm fixture loading required
running outside the sandbox's Windows path restrictions. New feature files and
the shared migration helper were formatted via rustfmt stdout and apply_patch.
Scoped Clippy with warnings denied stops on pre-existing package-assignment
formatting and annotation-crop nested-if lints; a clean Clippy gate is not claimed.

Historical migration expectations now share a shallow native-header/schema-80
helper that preserves opaque parameters, properties, extensions and existing
hide sets. Existing schema-77 family/binding/formula defaults are accounted for
separately. Old-package fixture setup removes the newer nullable formula field
and explicitly verifies that prepopulating it is still rejected. No source
strictness or older package dispatch was relaxed.

V02.24/V02.25 remain partial. Other categories/view kinds, temporary hide/isolate,
canvas reveal overlays, graphic overrides and rule/filter hiding are excluded.
This is not a full-workspace, native-window visual or physical-print qualification.
