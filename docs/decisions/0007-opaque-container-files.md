# ADR 0007: preserve opaque project files before plugin data expansion

Status: accepted; prerequisite of milestone C, not its completion.

Store validated auxiliary file bytes with the document, separately from the
semantic model and transaction snapshots. They are immutable through the current
public editing API: model commands and undo/redo cannot discard or rewrite them.
Opening stages all auxiliary bytes before returning a candidate document. Never
extract or execute them. Future editable assets require transactional references
and dependency/dirty-state rules, not mutation of this preservation map.

Write container version 2, retaining model schema 1 and the ZIP/JSON layout.
Read containers 1 and 2. Old readers reject 2 rather than opening and stripping
opaque files on save. Migrate only in memory; preserve old input until an explicit
save. Preserve names, file bytes and empty directories, not ZIP compression,
timestamps, comments or platform attributes. Reject symlinks/special files,
unsafe/ambiguous paths, malformed or unsupported entries rather than dropping them.

Bound auxiliary files to 16 MiB each, 64 MiB combined, within the existing 1,024
entry archive cap. Validate names and declared sizes before reading contents;
also cap actual decompressed reads. No dependencies or plugin execution are needed.
This does not yet provide plugin entity envelopes, attachment editing, unknown
model-field preservation, missing-plugin geometry or full production recovery.
