use os_core::{Id, Point2};
use os_model::{RoomParams, ScheduleCategory, ScheduleColumn, ScheduleParams, ScheduleSort};

#[test]
fn room_material_refs_validate_each_slot_independently_of_codes() {
    use os_model::{Material, MaterialParams, Model, Room};
    let mut model = Model::new("Room materials");
    let material = Material::new(
        "core.material",
        MaterialParams {
            name: "Oak".into(),
            density_kg_m3: 650.,
            color: [160, 100, 60],
        },
    );
    let material_id = material.id();
    model.materials.insert(material_id, material);
    let room = Room::new(
        "core.room",
        RoomParams {
            number: "1".into(),
            name: "Office".into(),
            level: *model.levels.keys().next().unwrap(),
            seed: Point2::new(1., 1.),
            boundary_signature: vec![(Id::new(), true), (Id::new(), true), (Id::new(), false)],
            floor_finish: Some("F-01".into()),
            wall_finish: None,
            ceiling_finish: Some("C-03".into()),
            floor_material: None,
            wall_material: None,
            ceiling_material: None,
        },
    );
    let id = room.id();
    model.rooms.insert(id, room.clone());
    for slot in 0..3 {
        for assigned in [None, Some(material_id), Some(Id::new())] {
            let p = &mut model.rooms.get_mut(&id).unwrap().parameters;
            *[
                &mut p.floor_material,
                &mut p.wall_material,
                &mut p.ceiling_material,
            ][slot] = assigned;
            assert_eq!(
                model.validate().is_ok(),
                assigned.is_none() || assigned == Some(material_id)
            );
            model.rooms.insert(id, room.clone());
        }
    }
}

#[test]
fn room_finish_codes_are_optional_bounded_and_validated() {
    let mut room = RoomParams {
        number: "101".into(),
        name: "Office".into(),
        level: Id::new(),
        seed: Point2::new(1., 1.),
        boundary_signature: vec![(Id::new(), true), (Id::new(), true), (Id::new(), false)],
        floor_material: None,
        wall_material: None,
        ceiling_material: None,
        floor_finish: None,
        wall_finish: None,
        ceiling_finish: None,
    };
    room.validate().unwrap();
    for field in 0..3 {
        for invalid in [
            "".to_owned(),
            " F1".into(),
            "F1 ".into(),
            "F\n1".into(),
            "x".repeat(129),
        ] {
            let mut bad = room.clone();
            *[
                &mut bad.floor_finish,
                &mut bad.wall_finish,
                &mut bad.ceiling_finish,
            ][field] = Some(invalid);
            assert!(bad.validate().is_err());
        }
    }
    room.floor_finish = Some("F-01".into());
    room.wall_finish = Some("W-02".into());
    room.ceiling_finish = Some("x".repeat(128));
    room.validate().unwrap();
}

#[test]
fn room_schedule_columns_and_sort_are_category_specific() {
    for category in [
        ScheduleCategory::Door,
        ScheduleCategory::Window,
        ScheduleCategory::All,
        ScheduleCategory::RoomFinish,
    ] {
        let base = ScheduleParams::new("Schedule", category);
        base.validate().unwrap();
        for column in ScheduleColumn::ALL.into_iter().chain(ScheduleColumn::ROOM) {
            let mut candidate = base.clone();
            candidate.columns = vec![column];
            assert_eq!(
                candidate.validate().is_ok(),
                base.available_columns().contains(&column)
            );
        }
        for sort in ScheduleSort::ALL.into_iter().chain(ScheduleSort::ROOM) {
            let mut candidate = base.clone();
            candidate.sort = sort;
            assert_eq!(
                candidate.validate().is_ok(),
                base.available_sorts().contains(&sort)
            );
        }
        let mut bad = base.clone();
        bad.columns.clear();
        assert!(bad.validate().is_err());
        bad.columns = vec![ScheduleColumn::Name; 9];
        assert!(bad.validate().is_err());
        bad.columns = vec![ScheduleColumn::Name; 2];
        assert!(bad.validate().is_err());
    }
}
