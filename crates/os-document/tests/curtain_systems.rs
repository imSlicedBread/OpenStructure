#[path = "../../os-model/tests/support/curtains.rs"]
mod support;
use os_core::Id;
use os_document::{Command, Document};
use os_model::*;
use std::collections::BTreeSet;

#[test]
fn curtain_lifecycle_and_component_invalidation_survive_history() {
    let (model, assembly) = support::fixture();
    let mut doc = Document::new("Curtains").unwrap();
    let id = assembly.id();
    let mut assembly = assembly;
    assembly.parameters.level = *doc.model().levels.keys().next().unwrap();
    let old_ids: BTreeSet<_> = assembly.parameters.child_ids().collect();
    let mut commands: Vec<_> = model
        .curtain_panel_types
        .into_values()
        .map(Command::AddCurtainPanelType)
        .collect();
    commands.extend(
        model
            .curtain_mullion_types
            .into_values()
            .map(Command::AddCurtainMullionType),
    );
    commands.push(Command::AddCurtainSystem(assembly.clone()));
    doc.execute("Add curtain", commands).unwrap();
    assert_eq!(doc.model().element_lifecycles.len(), 1);
    assert!(doc.model().element_lifecycles.contains_key(&id));
    assert!(old_ids.is_subset(&doc.drain_events()[0].invalidated));
    let before = doc.model().clone();
    let mut p = assembly.parameters.clone();
    p.vertical.insert(
        1,
        CurtainGrid {
            id: Id::new(),
            position: 1.,
        },
    );
    p.reconcile().unwrap();
    let all_ids: BTreeSet<_> = old_ids.iter().copied().chain(p.child_ids()).collect();
    doc.execute(
        "Add grid",
        vec![Command::UpdateCurtainSystem { id, parameters: p }],
    )
    .unwrap();
    assert!(all_ids.is_subset(&doc.drain_events()[0].invalidated));
    let after = doc.model().clone();
    assert!(doc.undo());
    assert_eq!(doc.model(), &before);
    assert!(all_ids.is_subset(&doc.drain_events()[0].invalidated));
    assert!(doc.redo());
    assert_eq!(doc.model(), &after);
    assert!(all_ids.is_subset(&doc.drain_events()[0].invalidated));
    let current_ids: BTreeSet<_> = doc.model().curtain_systems[&id]
        .parameters
        .child_ids()
        .collect();
    doc.execute("Remove curtain", vec![Command::RemoveCurtainSystem(id)])
        .unwrap();
    assert!(doc.model().element_lifecycles.is_empty());
    assert!(current_ids.is_subset(&doc.drain_events()[0].invalidated));
    assert!(doc.undo());
    assert!(current_ids.is_subset(&doc.drain_events()[0].invalidated));
    assert!(doc.redo());
    assert!(current_ids.is_subset(&doc.drain_events()[0].invalidated));
}

#[test]
fn curtain_command_rejects_identity_replacement_and_customization_loss_atomically() {
    let (mut model, mut assembly) = support::fixture();
    let mut custom = model.curtain_panel_types.values().next().unwrap().clone();
    custom.header.id = Id::new();
    custom.parameters.name = "Custom".into();
    assembly.parameters.panels[0].panel_type = custom.id();
    model.curtain_panel_types.insert(custom.id(), custom);
    let id = assembly.id();
    model.curtain_systems.insert(id, assembly.clone());
    let mut doc = Document::from_model(model).unwrap();
    let before = doc.model().clone();
    for case in 0..4 {
        let mut p = assembly.parameters.clone();
        match case {
            0 => p.panels[0].id = Id::new(),
            1 => p.mullions[0].id = Id::new(),
            2 => {
                p.vertical.remove(1);
                // Simulate a caller dropping the old assignment before reconciliation.
                p.panels.clear();
                p.mullions.clear();
                p.reconcile().unwrap();
            }
            _ => {
                p.panel_type = p.panels[0].panel_type;
                p.vertical.remove(1);
                p.panels.clear();
                p.mullions.clear();
                p.reconcile().unwrap();
            }
        }
        let history = doc.history_stats();
        assert!(
            doc.execute(
                "Invalid",
                vec![Command::UpdateCurtainSystem { id, parameters: p }]
            )
            .is_err()
        );
        assert_eq!(doc.model(), &before);
        assert_eq!(doc.history_stats(), history);
        assert!(doc.drain_events().is_empty());
    }
    let mut moved = assembly.parameters;
    moved.vertical[1].position = 2.3;
    doc.execute(
        "Move grid",
        vec![Command::UpdateCurtainSystem {
            id,
            parameters: moved.clone(),
        }],
    )
    .unwrap();
    assert_eq!(doc.model().curtain_systems[&id].parameters, moved);
}
