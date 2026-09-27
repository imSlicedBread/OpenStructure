use os_model::*;
use serde_json::json;

fn text(value: &str) -> ScheduleFilter {
    ScheduleFilter::Text {
        field: ScheduleTextField::Name,
        operator: ScheduleTextOperator::Contains,
        value: value.into(),
    }
}

#[test]
fn schedule_filters_validate_at_model_boundary_and_account_for_capacity() {
    let mut model = Model::new("Filters");
    let mut schedule = Schedule::new(
        "core.schedule",
        ScheduleParams::new("Doors", ScheduleCategory::Door),
    );
    assert!(schedule.parameters.filters.is_empty());
    schedule.parameters.filters = vec![text("Entry"); MAX_SCHEDULE_FILTERS];
    let id = schedule.id();
    model.schedules.insert(id, schedule);
    model.validate().unwrap();
    for value in [
        "",
        " ",
        " leading",
        "trailing ",
        "a\nb",
        "a\0b",
        &"é".repeat(129),
    ] {
        let mut bad = model.clone();
        bad.schedules.get_mut(&id).unwrap().parameters.filters = vec![text(value)];
        assert!(bad.validate().is_err(), "{value:?}");
    }
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut bad = model.clone();
        bad.schedules.get_mut(&id).unwrap().parameters.filters = vec![ScheduleFilter::Numeric {
            field: ScheduleNumericField::Width,
            operator: ScheduleNumericOperator::Equals,
            value,
        }];
        assert!(bad.validate().is_err());
    }
    let mut bad = model.clone();
    bad.schedules
        .get_mut(&id)
        .unwrap()
        .parameters
        .filters
        .push(text("overflow"));
    assert!(bad.validate().is_err());
    let mut room = ScheduleParams::new("Rooms", ScheduleCategory::RoomFinish);
    room.filters.push(text("Room"));
    assert!(room.validate().is_err());
    room.filters.clear();
    room.validate().unwrap();
    let before = model.estimated_memory_bytes();
    let filters = &mut model.schedules.get_mut(&id).unwrap().parameters.filters;
    filters.reserve(64);
    if let ScheduleFilter::Text { value, .. } = &mut filters[0] {
        value.reserve(4096);
    }
    assert!(
        model.estimated_memory_bytes()
            >= before + 4096 + 64 * std::mem::size_of::<ScheduleFilter>()
    );
}

#[test]
fn schedule_filters_wire_contract_is_tagged_strict_and_round_trips() {
    let numeric = ScheduleFilter::Numeric {
        field: ScheduleNumericField::Sill,
        operator: ScheduleNumericOperator::GreaterOrEqual,
        value: 0.9000001,
    };
    assert_eq!(
        serde_json::to_value(&numeric).unwrap(),
        json!({"kind":"numeric","field":"sill","operator":"greater_or_equal","value":0.9000001})
    );
    assert_eq!(
        serde_json::to_value(text("Entry")).unwrap(),
        json!({"kind":"text","field":"name","operator":"contains","value":"Entry"})
    );
    for invalid in [
        json!({"kind":"text","field":"width","operator":"equals","value":"1"}),
        json!({"kind":"numeric","field":"name","operator":"equals","value":1}),
        json!({"kind":"numeric","field":"width","operator":"contains","value":1}),
        json!({"kind":"text","field":"name","operator":"less","value":"a"}),
        json!({"kind":"text","field":"name","operator":"equals","value":1}),
        json!({"kind":"numeric","field":"width","operator":"equals","value":"1"}),
        json!({"kind":"text","field":"name","operator":"equals","value":"a","extra":true}),
        json!({"kind":"other","field":"name","operator":"equals","value":"a"}),
    ] {
        assert!(serde_json::from_value::<ScheduleFilter>(invalid).is_err());
    }
    let mut params = ScheduleParams::new("All", ScheduleCategory::All);
    params.filters = vec![text("Entry"), numeric];
    let wire = serde_json::to_value(&params).unwrap();
    assert_eq!(
        serde_json::from_value::<ScheduleParams>(wire.clone()).unwrap(),
        params
    );
    let mut missing = wire;
    missing.as_object_mut().unwrap().remove("filters");
    assert!(serde_json::from_value::<ScheduleParams>(missing).is_err());
}

#[test]
fn schedule_filter_operators_use_lowercase_and_exact_numeric_boundaries() {
    use ScheduleTextOperator as T;
    for (op, expected, matches) in [
        (T::Equals, "ÉNTRÉE", true),
        (T::NotEquals, "ÉNTRÉE", false),
        (T::Contains, "NTR", true),
        (T::StartsWith, "én", true),
        (T::Equals, "entree", false),
        (T::Equals, "e\u{301}ntrée", false),
        (T::Contains, "xyz", false),
        (T::StartsWith, "tr", false),
    ] {
        assert_eq!(op.matches("Éntrée", expected), matches);
    }
    use ScheduleNumericOperator as N;
    for (op, outcomes) in [
        (N::Equals, [false, true, false]),
        (N::NotEquals, [true, false, true]),
        (N::Less, [true, false, false]),
        (N::LessOrEqual, [true, true, false]),
        (N::Greater, [false, false, true]),
        (N::GreaterOrEqual, [false, true, true]),
    ] {
        for (value, expected) in [0.8999999, 0.9, 0.9000001].into_iter().zip(outcomes) {
            assert_eq!(op.matches(value, 0.9), expected, "{op:?} {value}");
        }
    }
}
