#[path = "support/hidden_openings.rs"]
mod support;
use os_core::Id;
use os_document::{Command, Document, PlanDuplicateMode};
use os_model::*;
use std::collections::BTreeSet;

#[test]
fn hidden_openings_atomic_noop_history_duplicate_delete_and_lifecycle_pruning() {
    let (m, view, ids) = support::fixture();
    let mut d = Document::from_model(m.clone()).unwrap();
    let hidden: BTreeSet<_> = ids.iter().copied().collect();
    d.execute(
        "Hide",
        vec![Command::SetPlanHiddenOpenings {
            view,
            hidden: hidden.clone(),
        }],
    )
    .unwrap();
    let after = d.model().clone();
    let history = d.history_stats();
    let revision = d.revision();
    let mut expected = m.clone();
    expected.plan_hidden_openings.insert(view, hidden.clone());
    assert_eq!(after, expected);
    d.drain_events();
    d.execute(
        "Noop",
        vec![Command::SetPlanHiddenOpenings {
            view,
            hidden: hidden.clone(),
        }],
    )
    .unwrap();
    assert_eq!(d.history_stats(), history);
    assert_eq!(d.revision(), revision);
    assert!(d.drain_events().is_empty());
    d.undo();
    assert_eq!(d.model(), &m);
    d.redo();
    assert_eq!(d.model(), &after);
    for mode in [PlanDuplicateMode::Plain, PlanDuplicateMode::WithDetailing] {
        let draft = d.prepare_plan_duplicate(view, "Copy".into(), mode).unwrap();
        let copy = d.apply_plan_duplicate(draft).unwrap();
        assert_eq!(d.model().plan_hidden_openings[&copy], hidden);
        d.execute(
            "Restore copy",
            vec![Command::SetPlanHiddenOpenings {
                view: copy,
                hidden: BTreeSet::new(),
            }],
        )
        .unwrap();
        assert_eq!(d.model().plan_hidden_openings[&view], hidden);
        assert!(!d.model().plan_hidden_openings.contains_key(&copy));
        d.undo();
        d.delete_plan(copy).unwrap();
        assert_eq!(d.model(), &after);
    }
    d.execute("Delete opening", vec![Command::RemoveOpening(ids[0])])
        .unwrap();
    assert!(!d.model().plan_hidden_openings[&view].contains(&ids[0]));
    let deleted = d.model().clone();
    d.undo();
    assert_eq!(d.model(), &after);
    d.redo();
    assert_eq!(d.model(), &deleted);
    d.undo();
    let level = Level::new(
        "core.level",
        LevelParams {
            name: "Upper".into(),
            elevation: 4.,
            building: *m.buildings.keys().next().unwrap(),
        },
    );
    let upper = level.id();
    d.execute("Level", vec![Command::AddLevel(level)]).unwrap();
    let before = d.model().clone();
    let host = m.openings[&ids[0]].parameters.host;
    let mut wall = m.walls[&host].parameters.clone();
    wall.level = upper;
    d.execute(
        "Change host level",
        vec![Command::UpdateWall {
            id: host,
            parameters: wall,
        }],
    )
    .unwrap();
    assert!(!d.model().plan_hidden_openings[&view].contains(&ids[0]));
    assert!(!d.model().plan_hidden_openings[&view].contains(&ids[1]));
    d.undo();
    assert_eq!(d.model(), &before);
    let mut p = m.views[&view].parameters.clone();
    p.level = Some(upper);
    d.execute(
        "Change plan level",
        vec![Command::UpdateView {
            id: view,
            parameters: p,
        }],
    )
    .unwrap();
    assert!(!d.model().plan_hidden_openings.contains_key(&view));
    d.undo();
    assert_eq!(d.model(), &before);
    let mut host_copy = m.walls[&host].clone();
    host_copy.header.id = Id::new();
    host_copy.parameters.level = upper;
    let mut opening = m.openings[&ids[0]].parameters.clone();
    opening.host = host_copy.id();
    d.execute(
        "Rehost",
        vec![
            Command::AddWall(host_copy),
            Command::UpdateOpening {
                id: ids[0],
                parameters: opening,
            },
        ],
    )
    .unwrap();
    assert!(!d.model().plan_hidden_openings[&view].contains(&ids[0]));
    d.undo();
    assert_eq!(d.model(), &before);
}

#[test]
fn hidden_openings_invalid_explicit_input_rejects_whole_transaction_before_pruning() {
    let (m, view, ids) = support::fixture();
    let mut d = Document::from_model(m.clone()).unwrap();
    let history = d.history_stats();
    for (view, hidden) in [
        (view, BTreeSet::from([ids[0], Id::new()])),
        (Id::new(), BTreeSet::new()),
        (view, BTreeSet::from([*m.walls.keys().next().unwrap()])),
    ] {
        assert!(
            d.execute(
                "Invalid",
                vec![
                    Command::RenameProject("Must roll back".into()),
                    Command::SetPlanHiddenOpenings { view, hidden }
                ]
            )
            .is_err()
        );
        assert_eq!(d.model(), &m);
        assert_eq!(d.history_stats(), history);
        assert!(d.drain_events().is_empty());
    }
    assert!(
        d.execute(
            "Do not prune explicit input",
            vec![
                Command::SetPlanHiddenOpenings {
                    view,
                    hidden: BTreeSet::from([ids[0]])
                },
                Command::RemoveOpening(ids[0])
            ]
        )
        .is_err()
    );
    assert_eq!(d.model(), &m);
    let drafting = View::new("core.view", ViewParams::drafting("Detail"));
    let id = drafting.id();
    assert!(
        d.execute(
            "Wrong kind",
            vec![
                Command::AddView(drafting),
                Command::SetPlanHiddenOpenings {
                    view: id,
                    hidden: BTreeSet::new()
                }
            ]
        )
        .is_err()
    );
    assert_eq!(d.model(), &m);
}

#[test]
fn hidden_openings_model_limits_and_wire_shape_are_strict() {
    let (mut m, view, ids) = support::fixture();
    let excessive = (0..4097).map(|_| Id::new()).collect();
    assert!(
        m.validate_plan_hidden_openings_replacement(view, &excessive)
            .unwrap_err()
            .to_string()
            .contains("4096")
    );
    let prototype = m.openings[&ids[0]].clone();
    let mut hidden = BTreeSet::new();
    for _ in 0..4096 {
        let mut opening = prototype.clone();
        opening.header.id = Id::new();
        hidden.insert(opening.id());
        m.openings.insert(opening.id(), opening);
    }
    for i in 0..3 {
        let mut v = m.views[&view].clone();
        v.header.id = Id::new();
        v.parameters.name = format!("Plan {i}");
        m.plan_hidden_openings.insert(v.id(), hidden.clone());
        m.views.insert(v.id(), v);
    }
    assert!(
        m.validate_plan_hidden_openings()
            .unwrap_err()
            .to_string()
            .contains("10000")
    );
    let (m, view, ids) = support::fixture();
    let original = serde_json::to_value(m).unwrap();
    for value in [
        serde_json::Value::Null,
        serde_json::json!([]),
        serde_json::json!({view.to_string(): []}),
        serde_json::json!({view.to_string(): [ids[0], ids[0]]}),
        serde_json::json!({view.to_string(): ["bad"]}),
    ] {
        let mut wire = original.clone();
        wire["plan_hidden_openings"] = value;
        assert!(serde_json::from_value::<Model>(wire).is_err());
    }
}
