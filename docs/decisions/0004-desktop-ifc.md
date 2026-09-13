# ADR 0004: desktop IFC exchange workflow

Use the existing Rust wall-subset adapter after the independent viewer gate.
IFC has its own path, distinct from the native save path. Export is an exchange
copy and never clears native dirty state. Always show scope/loss acknowledgement
and explicit replacement confirmation for an existing destination.

Prepare imported document and all meshes before asking to replace the current
document. Hold that prepared snapshot through confirmation; do not reread a
changed source file after consent. On acceptance, clear the native save path and
mark the imported document unsaved. Import failure/cancellation preserves the
current model, history, geometry, paths, selection and drafts. Ordinary native
save/open behavior remains unchanged.

Prepare export bytes before confirmation too. Freeze path and bytes; modal
shortcuts cannot change the document under the acknowledgement. New-file writes
use no-clobber persistence; existing-file replacement uses atomic persistence and
explicit user consent. A newly appeared file must not be overwritten without
another confirmation. Display loss diagnostics even when there are no current
native edits. Inputs remain limited to the documented IFC4 wall subset.

Tests cover actual egui input, cancellation, dirty/import/save lifecycle, overwrite
confirmation and controller-level failure atomicity. Independent viewer inspection
must be recorded before the desktop availability flag is enabled.
