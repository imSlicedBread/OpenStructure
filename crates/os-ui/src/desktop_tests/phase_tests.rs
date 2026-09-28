//! Phase acceptance through the native desktop's real egui frame/input harness.
use super::*;
use os_model::{ElementLifecycle, PhaseStatus};

fn key(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }
}

fn manage(h: &mut Harness) {
    h.click("Manage");
    h.click("Manage phases…");
    h.frame(vec![]);
}

fn name(h: &mut Harness, old: &str, new: &str) {
    h.click(old);
    h.replace_focused(new);
}

fn choose(h: &mut Harness, label: &str, value: &str) {
    let rect = h.text_rect(label);
    h.click_at(egui::pos2(rect.right() + 45.0, rect.center().y));
    h.click(value);
}

fn properties(h: &mut Harness, id: Id) {
    h.app.select(Some(id));
    h.frame(vec![]);
    if h.visible_text_rect("Created in").is_none() {
        h.click("Phasing");
    }
    h.frame(vec![]);
}

fn snapshot(h: &Harness) -> (Model, u64, os_document::HistoryStats) {
    (
        h.app.editor.document.model().clone(),
        h.app.editor.document.revision(),
        h.app.editor.document.history_stats(),
    )
}

fn apply_and_undo(h: &mut Harness, before: &Model, button: &str) -> Model {
    let history = h.app.editor.document.history_stats().undo_entries;
    h.click(button);
    assert!(!h.app.status_error, "{}", h.app.status);
    let after = h.app.editor.document.model().clone();
    assert_ne!(&after, before);
    assert_eq!(
        h.app.editor.document.history_stats().undo_entries,
        history + 1
    );
    h.click("Undo");
    assert_eq!(h.app.editor.document.model(), before);
    h.click("Redo");
    assert_eq!(h.app.editor.document.model(), &after);
    after
}

#[test]
fn phase_management_add_rename_adjacent_move_middle_delete_persistence_and_history() {
    for (size, scale) in [
        (egui::vec2(1280., 800.), 1.),
        (egui::vec2(1000., 650.), 1.5),
    ] {
        let mut h = Harness::at_size(size, scale);
        let original = h.app.editor.document.model().clone();
        let first = original.existing_phase().unwrap();
        let second = original.latest_phase().unwrap();
        manage(&mut h);
        h.click("Add phase");
        name(&mut h, "Phase 3", "Fit out");
        assert_eq!(h.app.editor.document.model(), &original);
        let added = apply_and_undo(&mut h, &original, "Apply phases");
        let added_id = added.latest_phase().unwrap();
        manage(&mut h);
        h.click("3. Fit out");
        name(&mut h, "Fit out", "Tenant fit out");
        let renamed = apply_and_undo(&mut h, &added, "Apply phases");
        assert_eq!(renamed.latest_phase(), Some(added_id));
        manage(&mut h);
        h.click("3. Tenant fit out");
        h.click("Move up");
        let moved = apply_and_undo(&mut h, &renamed, "Apply phases");
        assert_eq!(
            moved
                .ordered_phases()
                .iter()
                .map(|p| p.id())
                .collect::<Vec<_>>(),
            vec![first, added_id, second]
        );
        manage(&mut h);
        h.click("2. Tenant fit out");
        h.click("Move down");
        let moved_back = apply_and_undo(&mut h, &moved, "Apply phases");
        assert_eq!(moved_back.phases, renamed.phases);
        manage(&mut h);
        h.click("2. New Construction");
        h.click("Delete phase");
        let deleted = apply_and_undo(&mut h, &moved_back, "Apply phases");
        assert_eq!(
            deleted
                .ordered_phases()
                .iter()
                .map(|p| p.id())
                .collect::<Vec<_>>(),
            vec![first, added_id]
        );
        assert_eq!(deleted.phases[&added_id].parameters.order, 1);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("phases.osb");
        h.app.editor.save(&path).unwrap();
        h.app.open_path(&path).unwrap();
        h.frame(vec![]);
        assert_eq!(h.app.editor.document.model(), &deleted);
    }
}

#[test]
fn phase_management_rejects_first_referenced_names_capacity_and_invalid_reorder_atomically() {
    let mut h = Harness::new();
    h.click("Create wall");
    let wall = h.app.selected.unwrap();
    let before = snapshot(&h);
    manage(&mut h);
    h.click("Delete phase"); // disabled: earliest implicit fallback stays pinned
    h.click("Move down");
    assert_eq!(snapshot(&h), before);
    assert!(h.visible_text_rect("1. Existing").is_some());
    h.click("2. New Construction");
    h.click("Move up"); // cannot move across the pinned phase
    h.click("Delete phase");
    assert!(
        h.visible_text_rect(
            "Invalid data: Phase is referenced by an element lifecycle or plan and cannot be deleted"
        )
        .is_some()
    );
    assert_eq!(snapshot(&h), before);
    for bad in ["", " Existing", "existing", &"x".repeat(129)] {
        // The field stays focused across a rejected Apply only after explicitly clicking it.
        let rect = h.text_rect("Phase name");
        h.click_at(rect.left_bottom() + egui::vec2(35., 12.));
        if bad.is_empty() {
            h.frame(vec![key(egui::Key::A, egui::Modifiers::COMMAND)]);
            h.frame(vec![key(egui::Key::Backspace, egui::Modifiers::NONE)]);
        } else {
            h.replace_focused(bad);
        }
        h.click("Apply phases");
        assert!(h.app.status_error, "accepted {bad:?}");
        assert!(h.app.phase_draft.is_some());
        assert_eq!(snapshot(&h), before);
    }
    h.click("Cancel phases");
    // A swap that would put demolition before creation fails as a single transaction.
    let later = os_model::new_phase("Later", 2);
    let later_id = later.id();
    let created_in = h.app.editor.document.model().latest_phase().unwrap();
    h.app
        .editor
        .document
        .execute(
            "Fixture",
            vec![
                Command::AddPhase(later),
                Command::SetElementLifecycle {
                    element: wall,
                    lifecycle: ElementLifecycle {
                        created_in,
                        demolished_in: Some(later_id),
                    },
                },
            ],
        )
        .unwrap();
    let before = snapshot(&h);
    manage(&mut h);
    h.click("3. Later");
    h.click("Delete phase"); // referenced only by demolition, not creation
    assert_eq!(snapshot(&h), before);
    assert!(h.visible_text_rect("3. Later").is_some());
    h.click("Move up");
    h.click("Apply phases");
    assert!(h.app.status.contains("before it is created"));
    assert_eq!(snapshot(&h), before);
    h.click("Move down");
    name(&mut h, "Later", "Later corrected");
    h.click("Apply phases");
    assert!(!h.app.status_error);
    let count = h.app.editor.document.model().phases.len();
    h.app
        .editor
        .document
        .execute(
            "Fill phases",
            (count..os_model::MAX_PHASES)
                .map(|i| Command::AddPhase(os_model::new_phase(format!("Capacity {i}"), i as u32)))
                .collect(),
        )
        .unwrap();
    let before = snapshot(&h);
    manage(&mut h);
    h.click("Add phase");
    assert!(
        h.visible_text_rect("Invalid data: Project requires 1 to 64 phases")
            .is_some()
    );
    assert_eq!(snapshot(&h), before);
    h.click("Cancel phases");
}

fn elements(h: &mut Harness) -> [Id; 3] {
    h.click("Create wall");
    let wall = h.app.selected.unwrap();
    let opening = os_model::Opening::new(
        "core.opening",
        os_model::OpeningParams {
            name: "Phase door".into(),
            host: wall,
            offset: 0.5,
            definition: os_model::OpeningDefinition::Legacy {
                kind: os_model::OpeningKind::Door,
                width: 0.9,
                height: 2.1,
                sill: 0.,
            },
            width_override: None,
            height_override: None,
            sill_override: None,
            pane_position_override: None,
            lite_side_override: None,
            hinge: Default::default(),
            swing: Default::default(),
        },
    );
    let room = os_model::Room::new(
        "core.room",
        os_model::RoomParams {
            number: "1".into(),
            name: "Phase room".into(),
            level: h.app.active_level,
            seed: Point2::new(1., 1.),
            boundary_signature: vec![(wall, true), (Id::new(), true), (Id::new(), true)],
            floor_finish: None,
            wall_finish: None,
            ceiling_finish: None,
            floor_material: None,
            wall_material: None,
            ceiling_material: None,
        },
    );
    let ids = [wall, opening.id(), room.id()];
    h.app
        .editor
        .document
        .execute(
            "Fixture elements",
            vec![Command::AddOpening(opening), Command::AddRoom(room)],
        )
        .unwrap();
    ids
}

#[test]
fn phase_lifecycle_wall_hosted_opening_room_temporary_clear_and_one_step_history() {
    let mut h = Harness::new();
    let ids = elements(&mut h);
    // Demolish the opening before its host; clear the host before the opening.
    for id in [ids[2], ids[1], ids[0]] {
        properties(&mut h, id);
        let before = h.app.editor.document.model().clone();
        let phase = before.latest_phase().unwrap();
        choose(&mut h, "Demolished in", "New Construction");
        assert!(h.visible_text_rect("Temporary").is_some());
        assert_eq!(h.app.editor.document.model(), &before);
        let after = apply_and_undo(&mut h, &before, "Apply lifecycle");
        assert_eq!(
            after.phase_status(id, phase).unwrap(),
            PhaseStatus::Temporary
        );
        if id == ids[1] {
            continue;
        }
        choose(&mut h, "Demolished in", "None");
        let cleared = apply_and_undo(&mut h, &after, "Apply lifecycle");
        assert_eq!(cleared.element_lifecycle(id).unwrap().demolished_in, None);
    }
    properties(&mut h, ids[1]);
    let before = h.app.editor.document.model().clone();
    choose(&mut h, "Demolished in", "None");
    let cleared = apply_and_undo(&mut h, &before, "Apply lifecycle");
    assert_eq!(
        cleared.element_lifecycle(ids[1]).unwrap().demolished_in,
        None
    );
    // Explicit opening creation cannot precede its host, and a rejected draft stays editable.
    properties(&mut h, ids[1]);
    let before = snapshot(&h);
    choose(&mut h, "Created in", "Existing");
    h.click("Apply lifecycle");
    assert!(h.app.status.contains("before its wall"));
    assert_eq!(snapshot(&h), before);
    assert!(h.app.lifecycle_draft.is_some());
    choose(&mut h, "Created in", "New Construction");
    choose(&mut h, "Demolished in", "New Construction");
    h.click("Apply lifecycle");
    assert!(!h.app.status_error);
    properties(&mut h, ids[2]);
    let before = snapshot(&h);
    choose(&mut h, "Demolished in", "Existing");
    h.click("Apply lifecycle");
    assert!(h.app.status.contains("before it is created"));
    assert_eq!(snapshot(&h), before);
    h.click("Cancel lifecycle");
}

#[test]
fn phase_drafts_discard_stale_selection_revision_session_and_modal_shortcuts() {
    let mut h = Harness::new();
    let [wall, opening, room] = elements(&mut h);
    properties(&mut h, wall);
    let before_cancel = snapshot(&h);
    choose(&mut h, "Demolished in", "New Construction");
    h.click("Cancel lifecycle");
    assert!(h.visible_text_rect("None").is_some());
    assert_eq!(snapshot(&h), before_cancel);
    choose(&mut h, "Demolished in", "New Construction");
    h.frame(vec![
        key(egui::Key::Escape, egui::Modifiers::NONE),
        key(egui::Key::Z, egui::Modifiers::COMMAND),
    ]);
    assert_eq!(snapshot(&h), before_cancel);
    assert!(h.visible_text_rect("None").is_some());
    choose(&mut h, "Demolished in", "New Construction");
    properties(&mut h, room);
    assert!(h.visible_text_rect("None").is_some());
    properties(&mut h, wall);
    assert!(h.visible_text_rect("None").is_some());
    choose(&mut h, "Demolished in", "New Construction");
    h.app
        .editor
        .document
        .execute(
            "External edit",
            vec![Command::RenameProject("Revision".into())],
        )
        .unwrap();
    h.frame(vec![]);
    assert!(h.visible_text_rect("None").is_some());
    choose(&mut h, "Demolished in", "New Construction");
    let model = h.app.editor.document.model().clone();
    h.app.editor.document = Document::from_model(model).unwrap();
    h.frame(vec![]);
    assert!(h.visible_text_rect("None").is_some());
    // Session identity must invalidate a draft even if revision and element IDs match.
    choose(&mut h, "Demolished in", "New Construction");
    let revision = h.app.editor.document.revision();
    h.app.editor.document = Document::from_model(h.app.editor.document.model().clone()).unwrap();
    assert_eq!(h.app.editor.document.revision(), revision);
    h.frame(vec![]);
    assert!(h.visible_text_rect("None").is_some());
    h.app.selected_ids.insert(opening);
    h.frame(vec![]);
    assert!(h.app.lifecycle_draft.is_none());
    assert!(h.visible_text_rect("Created in").is_none());
    properties(&mut h, wall);
    for title in ["Undo target", "Redo target"] {
        h.app
            .editor
            .document
            .execute(
                "History fixture",
                vec![Command::RenameProject(title.into())],
            )
            .unwrap();
    }
    assert!(h.app.editor.document.undo());
    assert!(h.app.editor.document.can_undo() && h.app.editor.document.can_redo());
    h.frame(vec![]);
    let before = snapshot(&h);
    manage(&mut h);
    h.click("Add phase");
    h.click("Project phases"); // exercise document shortcuts without TextEdit focus
    for key_code in [egui::Key::Z, egui::Key::Y, egui::Key::S, egui::Key::Delete] {
        h.frame(vec![key(key_code, egui::Modifiers::COMMAND)]);
        assert_eq!(snapshot(&h), before);
        assert!(h.app.save_destination.is_none());
    }
    h.frame(vec![key(egui::Key::Delete, egui::Modifiers::NONE)]);
    assert_eq!(snapshot(&h), before);
    h.frame(vec![
        key(egui::Key::Escape, egui::Modifiers::NONE),
        key(egui::Key::Z, egui::Modifiers::COMMAND),
    ]);
    assert!(h.app.phase_draft.is_none());
    assert_eq!(snapshot(&h), before);
    manage(&mut h);
    h.click("Add phase");
    h.click("Cancel phases");
    assert_eq!(snapshot(&h), before);
    // A manager opened in an earlier revision or another session cannot Apply.
    for replace_session in [false, true] {
        if replace_session {
            h.app.editor.document =
                Document::from_model(h.app.editor.document.model().clone()).unwrap();
            h.frame(vec![]);
        }
        manage(&mut h);
        h.click("Add phase");
        if replace_session {
            let revision = h.app.editor.document.revision();
            h.app.editor.document =
                Document::from_model(h.app.editor.document.model().clone()).unwrap();
            assert_eq!(h.app.editor.document.revision(), revision);
        } else {
            h.app
                .editor
                .document
                .execute("External", vec![Command::RenameProject("External".into())])
                .unwrap();
        }
        let before = snapshot(&h);
        h.frame(vec![]);
        assert!(h.app.status.contains("Phase edits discarded"));
        assert!(h.app.phase_draft.is_none());
        assert_eq!(snapshot(&h), before);
    }
}
