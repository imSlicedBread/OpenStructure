use os_core::Id;
use os_model::{Model, PhaseFilter, PhaseStatus, ScheduleCategory, ScheduleParams, SchedulePhase};

#[test]
fn schedule_phase_filters_cover_all_six_statuses_and_five_filters() {
    use PhaseStatus::*;
    let statuses = [
        Existing,
        New,
        Demolished,
        Temporary,
        Future,
        PreviouslyDemolished,
    ];
    for (filter, expected) in [
        (PhaseFilter::ShowAll, [true, true, true, true, false, false]),
        (
            PhaseFilter::ShowExisting,
            [true, false, false, false, false, false],
        ),
        (
            PhaseFilter::ShowNew,
            [false, true, false, false, false, false],
        ),
        (
            PhaseFilter::ShowDemolished,
            [false, false, true, false, false, false],
        ),
        (
            PhaseFilter::ShowTemporary,
            [false, false, false, true, false, false],
        ),
    ] {
        assert_eq!(statuses.map(|s| filter.includes(s)), expected);
    }
}

#[test]
fn schedule_phase_construction_strict_wire_room_exclusion_and_uuid_references() {
    let mut model = Model::new("Schedules");
    for category in [
        ScheduleCategory::Door,
        ScheduleCategory::Window,
        ScheduleCategory::All,
        ScheduleCategory::RoomFinish,
    ] {
        let legacy = ScheduleParams::new("Legacy", category);
        assert_eq!(legacy.phase, SchedulePhase::LegacyUnphased);
        let mut params = ScheduleParams::new_for_model("New", category, &model).unwrap();
        assert_eq!(
            params.phase,
            if category == ScheduleCategory::RoomFinish {
                SchedulePhase::LegacyUnphased
            } else {
                SchedulePhase::PhaseAware {
                    target: model.latest_phase(),
                    filter: PhaseFilter::ShowAll,
                }
            }
        );
        let mut wire = serde_json::to_value(&params).unwrap();
        wire.as_object_mut().unwrap().remove("phase");
        assert!(serde_json::from_value::<ScheduleParams>(wire).is_err());
        params.phase = SchedulePhase::PhaseAware {
            target: Some(Id::new()),
            filter: PhaseFilter::ShowNew,
        };
        assert!(params.phase.resolve(&model).is_err());
        assert_eq!(
            params.validate().is_ok(),
            category != ScheduleCategory::RoomFinish
        );
    }
    let pinned = ScheduleParams::new_for_model("Pinned", ScheduleCategory::All, &model)
        .unwrap()
        .phase;
    let latest = SchedulePhase::PhaseAware {
        target: None,
        filter: PhaseFilter::ShowAll,
    };
    let old = model.latest_phase().unwrap();
    let phase = os_model::new_phase("Later", 2);
    let added = phase.id();
    model.phases.insert(added, phase);
    assert_eq!(pinned.resolve(&model).unwrap().unwrap().0, old);
    assert_eq!(latest.resolve(&model).unwrap().unwrap().0, added);
    model.phases.get_mut(&old).unwrap().parameters.order = 2;
    model.phases.get_mut(&added).unwrap().parameters.order = 1;
    model.validate().unwrap();
    assert_eq!(pinned.resolve(&model).unwrap().unwrap().0, old);
    assert_eq!(latest.resolve(&model).unwrap().unwrap().0, old);
    model.phases.clear();
    assert!(
        ScheduleParams::new_for_model("Missing phases", ScheduleCategory::Door, &model).is_err()
    );
    assert!(latest.resolve(&model).is_err());
    assert!(ScheduleParams::new_for_model("Rooms", ScheduleCategory::RoomFinish, &model).is_ok());
}
