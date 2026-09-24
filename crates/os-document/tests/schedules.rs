use os_document::{Command, Document};
use os_model::*;

#[test]
fn schedule_commands_preserve_identity_are_atomic_and_one_step_undoable() {
    let mut doc = Document::new("Schedules").unwrap();
    let empty = doc.model().clone();
    let s = Schedule::new(
        "core.schedule",
        ScheduleParams::new("Doors", ScheduleCategory::Door),
    );
    let id = s.id();
    doc.execute("Add", vec![Command::AddSchedule(s)]).unwrap();
    let created = doc.model().clone();
    assert!(doc.undo());
    assert_eq!(doc.model(), &empty);
    assert!(doc.redo());
    assert_eq!(doc.model(), &created);
    let mut params = created.schedules[&id].parameters.clone();
    params.columns = vec![ScheduleColumn::Width, ScheduleColumn::Name];
    params.sort = ScheduleSort::Width;
    doc.execute(
        "Configure",
        vec![Command::UpdateSchedule {
            id,
            parameters: params,
        }],
    )
    .unwrap();
    let edited = doc.model().clone();
    assert_eq!(created.schedules[&id].header, edited.schedules[&id].header);
    assert!(doc.undo());
    assert_eq!(doc.model(), &created);
    assert!(doc.redo());
    assert_eq!(doc.model(), &edited);
    doc.drain_events();
    let revision = doc.revision();
    let history = doc.history_stats();
    let invalid = Schedule::new(
        "core.schedule",
        ScheduleParams::new("doors", ScheduleCategory::Window),
    );
    assert!(
        doc.execute(
            "Invalid batch",
            vec![
                Command::RenameProject("bad".into()),
                Command::AddSchedule(invalid)
            ]
        )
        .is_err()
    );
    assert_eq!(doc.model(), &edited);
    assert_eq!(doc.revision(), revision);
    assert_eq!(doc.history_stats(), history);
    assert!(doc.drain_events().is_empty());
    doc.execute("Remove", vec![Command::RemoveSchedule(id)])
        .unwrap();
    assert!(!doc.drain_events().is_empty());
    assert!(!doc.model().schedules.contains_key(&id));
    assert!(doc.undo());
    assert_eq!(doc.model(), &edited);
    assert!(doc.redo());
    assert!(!doc.model().schedules.contains_key(&id));
}
