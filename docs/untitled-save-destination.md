# Untitled quick-Save destination correction

2026-09-12 follow-up to the defect reproduced in [native wall edit inspection](native-wall-edits.md).

Quick-access Save and Ctrl+S now distinguish the current project file from the
File menu's prospective path. An untitled project opens a modal destination
form with no implicit relative filename. The form requires an absolute `.osb`
path, supports Cancel/Escape, blocks global history shortcuts, and does not
write until Save project is clicked. Existing foreign files retain the existing
replacement-confirmation flow. A failed save preserves dirty state and keeps
the destination form available for correction, displaying the error.

After a successful save/open, quick Save writes the current project file even
if the prospective File-menu path was edited. File-menu Save remains an explicit
save-to-the-entered-path operation; it is not the quick-save command. This is
still a typed path form, not an OS file browser, recent-files UI or full Save As
workflow. Those production usability capabilities remain open.

## Evidence

`desktop_tests::untitled_quick_save_requires_destination_and_tracks_successful_file`
feeds actual egui input through the desktop frame. It verifies initial prompting,
cancellation, modal undo isolation, relative-path rejection, failed save to a
missing parent, retry to a valid temporary destination, dirty-state preservation,
and subsequent quick-save routing with independent Editor reopen comparison.
Existing overwrite confirmation/cancel tests still pass.

Full offline all-feature workspace tests, strict all-target/all-feature Clippy,
formatting and default workspace build pass. A native-window recheck of this new
form has not yet been performed. No file-format or dependency changes.

Follow-up [native inspection](native-offset-save.md) now verifies quick Save and
Ctrl+S prompting, Cancel preserving unsaved work, successful destination save,
and reopening the resulting two-wall model without the original path error.
