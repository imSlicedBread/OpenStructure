use os_core::Id;
use os_model::*;

fn fixture() -> (Model, Id) {
    let mut model = Model::new("Schedules");
    let schedule = Schedule::new(
        "core.schedule",
        ScheduleParams::new("Doors", ScheduleCategory::Door),
    );
    let id = schedule.id();
    model.schedules.insert(id, schedule);
    (model, id)
}

#[test]
fn strict_schedule_identity_fields_and_bounds() {
    let (model, id) = fixture();
    model.validate().unwrap();
    for name in [
        "".into(),
        " ".into(),
        " Doors".into(),
        "Doors\n".into(),
        "x".repeat(129),
    ] {
        let mut bad = model.clone();
        bad.schedules.get_mut(&id).unwrap().parameters.name = name;
        assert!(bad.validate().is_err());
    }
    for columns in [
        vec![],
        vec![ScheduleColumn::Name; 2],
        vec![ScheduleColumn::Name; 9],
    ] {
        let mut bad = model.clone();
        bad.schedules.get_mut(&id).unwrap().parameters.columns = columns;
        assert!(bad.validate().is_err());
    }
    for case in 0..5 {
        let mut bad = model.clone();
        let entity = bad.schedules.get_mut(&id).unwrap();
        match case {
            0 => entity.header.id = Id::new(),
            1 => entity.header.type_id = "core.view".into(),
            2 => entity.header.schema_version -= 1,
            3 => {
                let duplicate = Schedule::new(
                    "core.schedule",
                    ScheduleParams::new("doors", ScheduleCategory::All),
                );
                bad.schedules.insert(duplicate.id(), duplicate);
            }
            _ => {
                let mut duplicate = entity.clone();
                duplicate.header.id = bad.project.id();
                bad.schedules.insert(duplicate.id(), duplicate);
            }
        }
        assert!(bad.validate().is_err());
    }
    let mut nil = model.clone();
    let mut schedule = nil.schedules.remove(&id).unwrap();
    schedule.header.id = Id(Default::default());
    nil.schedules.insert(schedule.id(), schedule);
    assert!(nil.validate().is_err());
    let mut value = serde_json::to_value(&model).unwrap();
    value["schedules"][id.to_string()]["parameters"]["columns"] =
        serde_json::json!(["Unsupported"]);
    assert!(serde_json::from_value::<Model>(value).is_err());
    let mut full = model;
    for i in 1..MAX_SCHEDULES {
        let s = Schedule::new(
            "core.schedule",
            ScheduleParams::new(format!("Schedule {i}"), ScheduleCategory::All),
        );
        full.schedules.insert(s.id(), s);
    }
    full.validate().unwrap();
    let s = Schedule::new(
        "core.schedule",
        ScheduleParams::new("Overflow", ScheduleCategory::Window),
    );
    full.schedules.insert(s.id(), s);
    assert!(full.validate().is_err());
}

#[test]
fn schedule_retention_accounts_for_owned_capacity() {
    let (model, id) = fixture();
    let mut larger = model.clone();
    let schedule = larger.schedules.get_mut(&id).unwrap();
    schedule.parameters.name.reserve(4096);
    schedule.parameters.columns.reserve(512);
    schedule.header.properties.insert(
        "opaque".into(),
        serde_json::json!({"text": "x".repeat(2048)}),
    );
    assert!(larger.estimated_memory_bytes() > model.estimated_memory_bytes() + 6000);
}
