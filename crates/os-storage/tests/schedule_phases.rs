use os_document::{Command, Document};
use os_model::{Model, PhaseFilter, SCHEMA_VERSION, SchedulePhase};
use os_storage::{StorageBackend, ZipJsonStorage, migrate};
use serde_json::{Value, json};
mod common;

fn frozen() -> Value {
    serde_json::from_str(include_str!("fixtures/schema-47-schedule-phases.json")).unwrap()
}

#[test]
fn frozen_47_schedule_phases_preserve_all_other_values_and_save_reopen() {
    let original = frozen();
    let mut value = original.clone();
    migrate(&mut value, 47).unwrap();
    let model: Model = serde_json::from_value(value.clone()).unwrap();
    model.validate().unwrap();
    assert_eq!(model.schema_version, SCHEMA_VERSION);
    assert!(!model.schedules.is_empty());
    let mut reversed = value;
    common::reverse_wall_paths(&mut reversed);
    common::remove_opening_visibility(&mut reversed);
    reversed["schema_version"] = 47.into();
    for (key, data) in reversed.as_object_mut().unwrap() {
        if key == "project" {
            assert_eq!(data["header"]["schema_version"], SCHEMA_VERSION);
            data["header"]["schema_version"] = 47.into();
        } else if key != "extensions"
            && let Some(map) = data.as_object_mut()
        {
            for entity in map.values_mut().filter(|e| e.get("header").is_some()) {
                assert_eq!(entity["header"]["schema_version"], SCHEMA_VERSION);
                entity["header"]["schema_version"] = 47.into();
                if key == "schedules" {
                    assert_eq!(
                        entity["parameters"]
                            .as_object_mut()
                            .unwrap()
                            .remove("phase"),
                        Some(json!({"mode": "legacy_unphased"}))
                    );
                }
            }
        }
    }
    assert_eq!(reversed, original);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("schedule-phases.osb");
    for phase in [
        SchedulePhase::LegacyUnphased,
        SchedulePhase::PhaseAware {
            target: model.latest_phase(),
            filter: PhaseFilter::ShowAll,
        },
        SchedulePhase::PhaseAware {
            target: None,
            filter: PhaseFilter::ShowTemporary,
        },
    ] {
        let mut current = model.clone();
        for schedule in current.schedules.values_mut() {
            if schedule.parameters.category != os_model::ScheduleCategory::RoomFinish {
                schedule.parameters.phase = phase;
            }
        }
        let document = Document::from_model(current).unwrap();
        ZipJsonStorage.save(&document, &path).unwrap();
        assert_eq!(
            ZipJsonStorage.open(&path).unwrap().model(),
            document.model()
        );
    }
}

#[test]
fn schedule_phase_migration_and_current_schema_are_strict_and_atomic() {
    let original = frozen();
    let id = original["schedules"]
        .as_object()
        .unwrap()
        .keys()
        .next()
        .unwrap();
    for case in 0..8 {
        let mut bad = original.clone();
        match case {
            0 => bad["schedules"][id]["parameters"]["phase"] = Value::Null,
            1 => bad["schedules"][id]["parameters"]["phase"] = json!({"mode": "legacy_unphased"}),
            2 => bad["schedules"][id]["parameters"] = Value::Null,
            3 => bad["schedules"][id]["header"]["schema_version"] = 46.into(),
            4 => bad["project"]["header"]["schema_version"] = 46.into(),
            5 => bad["schedules"] = json!([]),
            6 => {
                bad.as_object_mut().unwrap().remove("phases");
            }
            _ => bad["schema_version"] = 46.into(),
        }
        let before = bad.clone();
        assert!(migrate(&mut bad, 47).is_err(), "case {case}");
        assert_eq!(bad, before);
    }
    let mut current = original;
    migrate(&mut current, 47).unwrap();
    let id = current["schedules"]
        .as_object()
        .unwrap()
        .keys()
        .next()
        .unwrap()
        .clone();
    for phase in [
        None,
        Some(Value::Null),
        Some(json!({"mode":"unknown"})),
        Some(json!({"mode":"phase_aware","target":null})),
        Some(json!({"mode":"phase_aware","target":null,"filter":"Bad"})),
        Some(json!({"mode":"phase_aware","target":os_core::Id::new(),"filter":"ShowAll"})),
    ] {
        let mut bad = current.clone();
        let params = bad["schedules"][&id]["parameters"].as_object_mut().unwrap();
        if let Some(phase) = phase {
            params.insert("phase".into(), phase);
        } else {
            params.remove("phase");
        }
        let before = bad.clone();
        assert!(migrate(&mut bad, SCHEMA_VERSION).is_err());
        assert_eq!(bad, before);
    }
}

#[test]
fn deleting_pinned_schedule_phase_rejects_atomically_without_history_or_events() {
    let mut model = Model::new("Pinned schedule");
    let phase = model.latest_phase().unwrap();
    let schedule = os_model::Schedule::new(
        "core.schedule",
        os_model::ScheduleParams::new_for_model("Doors", os_model::ScheduleCategory::Door, &model)
            .unwrap(),
    );
    let id = schedule.id();
    model.schedules.insert(id, schedule);
    let mut document = Document::from_model(model).unwrap();
    let before = document.model().clone();
    let history = document.history_stats();
    let revision = document.revision();
    assert!(
        document
            .execute("Delete pinned phase", vec![Command::RemovePhase(phase)])
            .is_err()
    );
    assert_eq!(document.model(), &before);
    assert_eq!(document.history_stats(), history);
    assert_eq!(document.revision(), revision);
    assert!(document.drain_events().is_empty());
    let mut parameters = before.schedules[&id].parameters.clone();
    parameters.phase = SchedulePhase::PhaseAware {
        target: None,
        filter: PhaseFilter::ShowAll,
    };
    document
        .execute(
            "Follow latest and delete",
            vec![
                Command::UpdateSchedule { id, parameters },
                Command::RemovePhase(phase),
            ],
        )
        .unwrap();
    assert_eq!(document.model().phases.len(), 1);
    assert!(document.undo());
    assert_eq!(document.model(), &before);
}
