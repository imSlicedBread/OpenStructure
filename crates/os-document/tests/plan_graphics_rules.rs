#[path = "../../os-model/tests/support/plan_graphics_rules.rs"]
mod support;
use os_core::{Id, Point2};
use os_document::{Command, Document, PlanDuplicateMode};
use os_model::*;
use support::*;

#[test]
fn rule_replacements_are_atomic_undoable_and_validate_final_graph() {
    let (model, view, [wall, ..]) = fixture();
    let mut doc = Document::from_model(model).unwrap();
    let row = rule(PlanRuleCategory::Walls);
    let before = doc.model().clone();
    let stats = doc.history_stats();
    doc.execute(
        "Rules",
        vec![Command::SetPlanGraphicsRules {
            view,
            rules: vec![row.clone()],
        }],
    )
    .unwrap();
    assert_eq!(doc.history_stats().undo_entries, stats.undo_entries + 1);
    assert!(doc.undo());
    assert_eq!(doc.model(), &before);
    assert!(doc.redo());
    let saved = doc.model().clone();
    let history = doc.history_stats();
    let mut invalid = row.clone();
    invalid.predicates.clear();
    assert!(
        doc.execute(
            "Bad rules",
            vec![
                Command::RenameProject("Must roll back".into()),
                Command::SetPlanGraphicsRules {
                    view,
                    rules: vec![invalid]
                }
            ]
        )
        .is_err()
    );
    assert_eq!(doc.model(), &saved);
    assert_eq!(doc.history_stats(), history);
    let mut copy = doc.model().views[&view].clone();
    copy.header.id = Id::new();
    let new_view = copy.id();
    doc.execute(
        "Final graph",
        vec![
            Command::SetPlanGraphicsRules {
                view: new_view,
                rules: vec![rule(PlanRuleCategory::Walls)],
            },
            Command::AddView(copy),
        ],
    )
    .unwrap();
    let mut removals: Vec<_> = doc
        .model()
        .openings
        .keys()
        .copied()
        .map(Command::RemoveOpening)
        .collect();
    removals.push(Command::RemoveWall(wall));
    doc.execute("Delete matched wall and hosted openings", removals)
        .unwrap();
    assert_eq!(doc.model().plan_graphics_rules[&view], vec![row]);
    assert!(
        doc.model()
            .resolve_plan_graphics_rules(view)
            .unwrap()
            .is_empty()
    );
    assert!(doc.undo());
    assert!(
        doc.model()
            .resolve_plan_graphics_rules(view)
            .unwrap()
            .contains_key(&wall)
    );
    doc.execute("Remove owner", vec![Command::RemoveView(new_view)])
        .unwrap();
    assert!(!doc.model().plan_graphics_rules.contains_key(&new_view));
    assert!(doc.undo());
    assert!(doc.model().plan_graphics_rules.contains_key(&new_view));
    let mut changed = doc.model().views[&new_view].parameters.clone();
    changed.plan.as_mut().unwrap().view_type = PlanViewType::ReflectedCeilingPlan;
    changed.plan.as_mut().unwrap().range = PlanViewRange {
        bottom: 0.,
        cut: 1.2,
        top: 3.,
        depth: 4.,
    };
    doc.execute(
        "Incompatible owner",
        vec![Command::UpdateView {
            id: new_view,
            parameters: changed,
        }],
    )
    .unwrap();
    assert!(!doc.model().plan_graphics_rules.contains_key(&new_view));
}

#[test]
fn rule_duplicates_and_callouts_copy_with_fresh_ids_and_independent_edits() {
    let (mut model, view, _) = fixture();
    let row = rule(PlanRuleCategory::Walls);
    model.plan_graphics_rules.insert(view, vec![row.clone()]);
    let mut doc = Document::from_model(model).unwrap();
    for (mode, name) in [
        (PlanDuplicateMode::Plain, "Plain"),
        (PlanDuplicateMode::WithDetailing, "Detailing"),
    ] {
        let prepared = doc.prepare_plan_duplicate(view, name.into(), mode).unwrap();
        let copy = prepared.target();
        doc.apply_plan_duplicate(prepared).unwrap();
        let mut actual = doc.model().plan_graphics_rules[&copy][0].clone();
        assert_ne!(actual.id, row.id);
        actual.id = row.id;
        assert_eq!(actual, row);
        doc.execute(
            "Clear copy",
            vec![Command::SetPlanGraphicsRules {
                view: copy,
                rules: vec![],
            }],
        )
        .unwrap();
        assert_eq!(doc.model().plan_graphics_rules[&view], vec![row.clone()]);
    }
    let (_, target, commands) = doc
        .enlarged_plan_commands(
            view,
            "Callout".into(),
            PlanViewCrop {
                min: Point2::new(-1., -1.),
                max: Point2::new(3., 3.),
            },
            25.,
        )
        .unwrap();
    doc.execute("Callout", commands).unwrap();
    let mut actual = doc.model().plan_graphics_rules[&target][0].clone();
    assert_ne!(actual.id, row.id);
    actual.id = row.id;
    assert_eq!(actual, row);
    let before = doc.model().clone();
    assert!(doc.undo());
    assert!(!doc.model().plan_graphics_rules.contains_key(&target));
    assert!(doc.redo());
    assert_eq!(doc.model(), &before);
}

#[test]
fn rule_project_limit_uses_final_owner_graph_before_cleanup() {
    let (mut model, view, _) = fixture();
    for _ in 0..8 {
        let mut copy = model.views[&view].clone();
        copy.header.id = Id::new();
        let id = copy.id();
        model.views.insert(id, copy);
        model
            .plan_graphics_rules
            .insert(id, (0..32).map(|_| rule(PlanRuleCategory::Walls)).collect());
    }
    let removed = *model.plan_graphics_rules.keys().next().unwrap();
    let replacement = model.copied_plan_graphics_rules(removed);
    let mut document = Document::from_model(model).unwrap();
    document
        .execute(
            "Replace owner at project limit",
            vec![
                Command::SetPlanGraphicsRules {
                    view,
                    rules: replacement,
                },
                Command::RemoveView(removed),
            ],
        )
        .unwrap();
    assert_eq!(
        document
            .model()
            .plan_graphics_rules
            .values()
            .map(Vec::len)
            .sum::<usize>(),
        256
    );
    assert!(!document.model().plan_graphics_rules.contains_key(&removed));
    assert!(document.undo());
    assert!(document.model().plan_graphics_rules.contains_key(&removed));
    assert!(!document.model().plan_graphics_rules.contains_key(&view));
}
