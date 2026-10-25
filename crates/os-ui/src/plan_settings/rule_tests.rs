use super::*;
#[path = "../../../os-model/tests/support/plan_graphics_rules.rs"]
mod support;
use os_geometry::plan::PlanRole;
use os_model::*;
use support::*;

fn editor() -> (Editor, Id, [Id; 4]) {
    let (model, view, ids) = fixture();
    let mut editor = Editor::new().unwrap();
    editor.document = os_document::Document::from_model(model).unwrap();
    editor.regenerate().unwrap();
    (editor, view, ids)
}
fn appearance(
    editor: &Editor,
    view: Id,
    element: Id,
    role: PlanRole,
) -> Option<os_render::plan::PlanStroke> {
    editor
        .native_drawing(view)
        .unwrap()
        .appearance(editor.native_plan_context(view).unwrap(), element, role)
        .unwrap()
}

#[test]
fn rule_drafts_dynamic_output_priority_visibility_history_and_stale_snapshots() {
    let (mut editor, view, [wall, door, window, slab]) = editor();
    let before = editor.document.model().clone();
    let mut draft = PlanDraft::begin(&editor, view).unwrap();
    for (index, category) in [
        PlanRuleCategory::Walls,
        PlanRuleCategory::Doors,
        PlanRuleCategory::Windows,
        PlanRuleCategory::Slabs,
    ]
    .into_iter()
    .enumerate()
    {
        let mut r = rule(category);
        r.projected = Some(stroke([20 + index as u8, 30, 180]));
        if category != PlanRuleCategory::Slabs {
            r.cut = Some(stroke([100 + index as u8, 20, 30]));
        }
        draft.rules.rules.push(r);
    }
    assert_eq!(editor.document.model(), &before);
    let old = editor.plan_snapshot(view).unwrap();
    draft.apply(&mut editor, Some(view)).unwrap();
    let saved = editor.document.model().clone();
    let context = editor.native_plan_context(view).unwrap();
    assert!(old.derive().unwrap().items(context).is_err());
    for (i, id) in [wall, door, window].into_iter().enumerate() {
        assert_eq!(
            appearance(&editor, view, id, PlanRole::Cut).unwrap().color,
            [100 + i as u8, 20, 30]
        );
    }
    assert_eq!(
        appearance(&editor, view, slab, PlanRole::Projected)
            .unwrap()
            .color,
        [23, 30, 180]
    );
    let drawing = editor.native_drawing(view).unwrap();
    assert!(
        drawing
            .provider_lines(context)
            .unwrap()
            .iter()
            .any(|l| l.entity == door)
    );
    assert!(
        drawing
            .provider_lines(context)
            .unwrap()
            .iter()
            .any(|l| l.entity == window)
    );
    // The host aperture retains the host appearance, distinct from the symbol.
    assert_ne!(
        appearance(&editor, view, wall, PlanRole::Cut),
        appearance(&editor, view, door, PlanRole::Cut)
    );
    editor.undo().unwrap();
    assert_eq!(editor.document.model(), &before);
    editor.redo().unwrap();
    assert_eq!(editor.document.model(), &saved);
    let mut explicit = PlanDraft::begin(&editor, view).unwrap();
    explicit.element_overrides.overrides.insert(
        door,
        PlanElementGraphicsOverride {
            cut: Some(stroke([0, 255, 0])),
            projected: None,
        },
    );
    explicit.apply(&mut editor, Some(view)).unwrap();
    assert_eq!(
        appearance(&editor, view, door, PlanRole::Cut)
            .unwrap()
            .color,
        [0, 255, 0]
    );
    assert!(
        appearance(&editor, view, door, PlanRole::Projected).is_none(),
        "rules cannot create an absent symbol role"
    );
    let mut hidden = PlanDraft::begin(&editor, view).unwrap();
    hidden.visibility.doors = false;
    hidden.visibility.windows = false;
    hidden.apply(&mut editor, Some(view)).unwrap();
    let drawing = editor.native_drawing(view).unwrap();
    let c = editor.native_plan_context(view).unwrap();
    assert!(
        !drawing
            .provider_lines(c)
            .unwrap()
            .iter()
            .any(|l| l.entity == door || l.entity == window)
    );
    assert_eq!(
        editor.document.model().plan_graphics_rules[&view],
        saved.plan_graphics_rules[&view]
    );
    editor.undo().unwrap();
    let mut crop = PlanDraft::begin(&editor, view).unwrap();
    crop.crop = true;
    crop.values[8..12].clone_from_slice(&["100", "100", "101", "101"].map(str::to_owned));
    crop.apply(&mut editor, Some(view)).unwrap();
    assert!(appearance(&editor, view, wall, PlanRole::Cut).is_none());
    // Slab meshes retain their authored rings; canvas/paper clip them at render time.
    let c = editor.native_plan_context(view).unwrap();
    let d = editor.native_drawing(view).unwrap();
    assert!(d.pick_floor(c, Point2::new(0., 0.)).unwrap().is_none());
    use os_render::sheet::{PaperMarkKind, PaperSheetInfo, PaperViewport, compose_view_sheet};
    let page = compose_view_sheet(
        PaperSheetInfo {
            width_mm: 420.,
            height_mm: 297.,
            number: "A1",
            name: "Crop",
        },
        "Crop",
        PaperViewport {
            center_mm: Point2::new(210., 148.5),
            width_mm: 300.,
            height_mm: 250.,
            model_center_m: Point2::new(100.5, 100.5),
            scale_denominator: c.scale_denominator,
        },
        c,
        &d,
    )
    .unwrap();
    assert!(!page.marks().iter().any(|m|matches!(&m.kind,PaperMarkKind::Path {stroke:Some(s),..} if [s.color.red,s.color.green,s.color.blue]==[23,30,180])),"crop cannot reveal rule-styled slab edges");
    editor.undo().unwrap();
    let mut phase = PlanDraft::begin(&editor, view).unwrap();
    phase.phase_filter = PhaseFilter::ShowNew;
    phase.apply(&mut editor, Some(view)).unwrap();
    assert!(
        editor
            .native_drawing(view)
            .unwrap()
            .items(editor.native_plan_context(view).unwrap())
            .unwrap()
            .is_empty()
    );
    editor.undo().unwrap();
    let mut stale = PlanDraft::begin(&editor, view).unwrap();
    stale.rules.rules.clear();
    editor
        .command("Concurrent edit", Command::RenameProject("Updated".into()))
        .unwrap();
    let committed = editor.document.model().clone();
    assert!(stale.apply(&mut editor, Some(view)).is_err());
    assert_eq!(editor.document.model(), &committed);
    let mut cancelled = PlanDraft::begin(&editor, view).unwrap();
    cancelled.rules.rules.clear();
    drop(cancelled);
    assert_eq!(editor.document.model(), &committed);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rules.osb");
    editor.save(&path).unwrap();
    editor.open(&path).unwrap();
    assert_eq!(editor.document.model(), &committed);
    assert_eq!(
        appearance(&editor, view, door, PlanRole::Cut)
            .unwrap()
            .color,
        [0, 255, 0]
    );
}

#[test]
fn rule_drafts_template_changes_do_not_erase_rules_and_edits_change_matches() {
    let (mut editor, view, [wall, ..]) = editor();
    let mut draft = PlanDraft::begin(&editor, view).unwrap();
    let mut r = rule(PlanRuleCategory::Walls);
    r.cut = Some(stroke([1, 2, 3]));
    r.predicates = vec![PlanGraphicsPredicate::Number {
        field: PlanRuleNumberField::WallHeight,
        op: PlanRuleNumberOp::Greater,
        value: 2.8,
    }];
    draft.rules.rules = vec![r.clone()];
    draft.apply(&mut editor, Some(view)).unwrap();
    editor
        .command(
            "Local graphics",
            Command::SetPlanGraphics {
                view,
                binding: Some(PlanGraphicsBinding::default()),
            },
        )
        .unwrap();
    assert_eq!(
        appearance(&editor, view, wall, PlanRole::Cut)
            .unwrap()
            .color,
        [1, 2, 3]
    );
    editor
        .command(
            "Unlink",
            Command::SetPlanGraphics {
                view,
                binding: None,
            },
        )
        .unwrap();
    assert_eq!(editor.document.model().plan_graphics_rules[&view], vec![r]);
    let mut p = editor.document.model().walls[&wall].parameters.clone();
    p.height = 2.7;
    editor
        .command(
            "Height",
            Command::UpdateWall {
                id: wall,
                parameters: p,
            },
        )
        .unwrap();
    assert_ne!(
        appearance(&editor, view, wall, PlanRole::Cut)
            .unwrap()
            .color,
        [1, 2, 3]
    );
    editor.undo().unwrap();
    assert_eq!(
        appearance(&editor, view, wall, PlanRole::Cut)
            .unwrap()
            .color,
        [1, 2, 3]
    );
}
