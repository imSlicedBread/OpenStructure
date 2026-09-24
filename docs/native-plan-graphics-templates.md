# Native plan graphics templates

The first template slice persists named floor-plan graphics standards with cut
and projected color, line weight, and solid/dashed values for walls, doors,
windows, and slabs. A plan can retain its local values or link to one project
template. Linked template values take precedence; unlinking exposes the retained
local values again. Templates are edited in the Plan settings draft and become
document state only when the user applies the form.

Template creation, edits, links, and unlinking are document transactions, so a
single Apply is one undo step. A template cannot be removed while another plan
uses it. Editing a linked template invalidates every linked plan and dependent
sheet viewport. The schema-27-to-28 migration initializes empty graphics maps;
older documents retain their existing appearance.

Resolved values now flow into the native plan drawing and are shared by the plan
canvas, sheet preview, and vector PDF exporter. Wall outlines use the wall
category; native door/window symbols use their opening kind; slab boundaries use
the slab projected style. Cut/projected roles select the corresponding stroke,
and paper line weight plus the fixed 3 mm dash / 1.5 mm gap are retained in sheet
output. Unbound views keep their legacy appearance, and selection accents remain
visible instead of being hidden by standards.

This remains a bounded native-plan graphics slice, not a complete graphics
system. Host-wall apertures use the wall category; door/window categories style
their plan symbols. There are no element/filter overrides, apply-once or batch
assignment, transfer between projects, hatches, monochrome output, or section
graphics. Native-window and physical-print qualification are still outstanding.

Focused evidence includes the linked-template settings/history/reopen test,
native wall/door/window/slab appearance mapping, a two-scale egui dash/phase
test, and a sheet/PDF vector-stroke test. The storage migration test separately
covers schema 27→28. These results do not constitute native-window or
physical-print qualification.
