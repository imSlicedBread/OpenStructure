# ADR 0002: Close persistence and plugin-registry verification gaps

Status: accepted.

The completion audit found that schema migration was tested as an in-memory
function, plugin namespaces could overlap between independently loaded manifests,
and ZIP name lookup cannot prove absence of duplicate central-directory entries.

Keep the existing model, storage and transport design. Add container-level
migration and corruption tests; inspect central-directory names before looking up
required entries so an archive cannot silently choose between two model files.
The host owns a global registration index, rejects collisions atomically and
exposes registrations by kind, including all future service categories.

Application saves must first finish pending geometry regeneration. A failed
regeneration must not overwrite a usable file with a project that the application
cannot reopen. The semantic document remains undoable so the user can correct it.

These changes strengthen the existing first-slice contracts. SQLite, an isolated
plugin transport and IFC remain the explicit alternatives/deferred tasks allowed
by the master prompt; this decision does not claim those adapters are implemented.
