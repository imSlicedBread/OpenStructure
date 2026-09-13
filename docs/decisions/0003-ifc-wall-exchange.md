# ADR 0003: restricted IFC4 wall exchange

Status: accepted for the second slice.

Implement IFC4 STEP serialization and a bounded, strict subset reader in Rust
behind `IfcAdapter`. Runtime exchange must not require Python or a network.
Use IfcWall (not IfcWallStandardCase, whose material-layer requirements are beyond
this slice), spatial decomposition, SI metres, local placements and rectangular
extrusions. Encode native UUIDs as standard 22-character IFC GlobalIds.

This is not a general IFC importer. Unsupported schema/entity/representation
forms fail explicitly. Do not hide a native JSON model in an IFC property and
call decoding that model an IFC round trip: imported wall parameters must come
from IFC placements, profiles, extrusions and containment.

Expose exchange reports listing native-only data not carried by this subset.
The legacy lossless-looking trait methods reject exports that would lose data;
explicit report APIs permit callers to acknowledge the documented losses.
Import creates a default native view and reports it. Native save remains `.osb`.

Validate the emitted fixture independently using IfcOpenShell (development/CI
only), including EXPRESS rules and actual tessellation volume/bounds. An external
viewer is a separate acceptance gate; do not enable the desktop exchange controls
until it is verified. CLI commands may expose the experimental subset with
explicit diagnostics, no-clobber output and no current-document mutation.

References: public buildingSMART IFC4 ADD2 TC1 schema; IfcOpenShell validation
documentation. No proprietary formats, assets or sources are used.
