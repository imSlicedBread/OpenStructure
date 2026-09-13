# Native wall edit inspection

2026-09-12, current default desktop build, native window 11535656, 1280×800
client area. Computer-use skill drove actual pointer and keyboard input; no
synthetic controller calls were used for this checkpoint.

## Observed workflow

1. Created the default 5 m wall and Floor plan 1. Selected-wall toolbar exposed
   Move wall, Resize start and Resize end.
2. Move acquired the original start endpoint, then a destination 65 logical
   pixels right and up. The wall moved without changing its 5 m length.
   Undo restored its original position; Redo restored the moved position.
3. Enabled Split 2D / 3D. Resize end accepted exact length 3 and angle 90.
   The destination click committed the exact perpendicular wall; both views
   changed together and Properties showed 3 m length and 1.800 m³ volume.
4. Undo restored the moved 5 m wall in both views; Redo restored the 3 m wall.
   Started Resize start and pressed Escape; the committed wall stayed intact.
5. Saved a new `outputs/native-wall-edits.osb`, reopened it through File/Open,
   selected the saved floor plan and observed the same selected perpendicular
   wall in both views with the same Properties values. Closed the clean window.

## Independently inspected saved data

Read `model.json` directly from the saved ZIP after native Save:

- Model schema 4, one wall, no grids or extensions.
- Wall UUID `77008e45-cfd6-48be-b5b0-77717c7bd539`.
- Start `(1, 1)`, end `(1.0000000000000002, 4)` metres.
- Height 3, thickness 0.2, material null.
- Ground level `4bb1ffba-690f-4fb4-876e-4d53588bf763`.
- Plan `0f8f63d4-fcc1-479d-ba0e-f1c40df62a71`, associated with that level,
  default basis/range, settings version 1 and revision 0.

Identity across edits is proven by the controller/input regression tests, not
by comparing before/after ZIPs in this native run. Resize-start commit, invalid
drafts and rotated plans remain automated-test evidence rather than native
inspection evidence. This checkpoint does not qualify production editing.

## Discovered save-path defect

The initial quick Save on an untitled project used relative `project.osb`.
Launching through the desktop tool inherited the Codex application directory,
so the attempted temporary save in Program Files failed with access denied.
The application retained the unsaved model and displayed the error. Entering
the explicit workspace output path in File and saving succeeded. No existing
artifact was overwritten.

Untitled Save should request a destination instead of silently relying on the
process working directory. This remains an open usability defect for the next
implementation pass. The skill-based inspection exposed it; no permissions or
system settings were changed to work around it.

Follow-up: [untitled quick-Save correction](untitled-save-destination.md) now
implements destination prompting and routing with passing desktop input tests.
The observations above describe the original inspected build, not the correction.
