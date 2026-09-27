mod common;

use os_core::{Id, Point2};
use os_document::{Command, Document};
use os_model::*;
use os_storage::{StorageBackend, ZipJsonStorage, migrate};
use serde_json::{Value, json};

fn schema16() -> Value {
    let mut model = Model::new("Tables");
    let schedule = Schedule::new(
        "core.schedule",
        ScheduleParams::new("Doors", ScheduleCategory::Door),
    );
    model.schedules.insert(schedule.id(), schedule);
    let sheet = Sheet::new("core.sheet", SheetParams::new("A101", "Plan"));
    model.sheets.insert(sheet.id(), sheet);
    let mut value = serde_json::to_value(model).unwrap();
    common::remove_phase_fields(&mut value);
    value.as_object_mut().unwrap().remove("stairs");
    value.as_object_mut().unwrap().remove("roofs");
    value.as_object_mut().unwrap().remove("opening_tags");
    value.as_object_mut().unwrap().remove("detail_lines");
    value
        .as_object_mut()
        .unwrap()
        .remove("room_separation_lines");
    value.as_object_mut().unwrap().remove("wall_joins");
    value.as_object_mut().unwrap().remove("wall_types");
    value
        .as_object_mut()
        .unwrap()
        .remove("wall_type_assignments");
    value.as_object_mut().unwrap().remove("columns");
    value
        .as_object_mut()
        .unwrap()
        .remove("plan_graphics_templates");
    value.as_object_mut().unwrap().remove("plan_graphics");
    value["schema_version"] = json!(16);
    value["project"]["header"]["schema_version"] = json!(16);
    for map in [
        "sites",
        "buildings",
        "levels",
        "walls",
        "openings",
        "opening_types",
        "rooms",
        "room_tags",
        "dimensions",
        "grids",
        "materials",
        "views",
        "floors",
        "sheets",
        "schedules",
    ] {
        for entity in value[map].as_object_mut().unwrap().values_mut() {
            entity["header"]["schema_version"] = json!(16);
            if map == "sheets" {
                entity["parameters"]
                    .as_object_mut()
                    .unwrap()
                    .remove("schedule_placements");
            } else if map == "schedules" {
                assert_eq!(entity["parameters"]["filters"], json!([]));
                entity["parameters"]
                    .as_object_mut()
                    .unwrap()
                    .remove("filters");
            }
        }
    }
    value
}

#[test]
fn schema16_migrates_tables_and_schedule_headers_rejecting_ambiguous_shapes() {
    let original = schema16();
    let mut value = original.clone();
    migrate(&mut value, 16).unwrap();
    let model: Model = serde_json::from_value(value).unwrap();
    assert_eq!(model.schema_version, SCHEMA_VERSION);
    assert!(
        model
            .sheets
            .values()
            .all(|s| s.header.schema_version == SCHEMA_VERSION
                && s.parameters.schedule_placements.is_empty())
    );
    assert!(
        model
            .schedules
            .values()
            .all(|s| s.header.schema_version == SCHEMA_VERSION)
    );
    for shape in [Value::Null, json!([]), json!({})] {
        let mut ambiguous = original.clone();
        let sheet = ambiguous["sheets"]
            .as_object_mut()
            .unwrap()
            .values_mut()
            .next()
            .unwrap();
        sheet["parameters"]["schedule_placements"] = shape;
        let before = ambiguous.clone();
        assert!(
            migrate(&mut ambiguous, 16)
                .unwrap_err()
                .to_string()
                .contains("ambiguous schema 16")
        );
        assert_eq!(ambiguous, before);
    }
}

#[test]
fn placements_update_sheet_history_references_and_archive_roundtrip() {
    let mut value = schema16();
    migrate(&mut value, 16).unwrap();
    let mut doc = Document::from_model(serde_json::from_value(value).unwrap()).unwrap();
    let sheet_id = *doc.model().sheets.keys().next().unwrap();
    let schedule = *doc.model().schedules.keys().next().unwrap();
    for action in 0..3 {
        let before = doc.model().clone();
        let revision = doc.revision();
        let mut parameters = before.sheets[&sheet_id].parameters.clone();
        match action {
            0 => parameters.schedule_placements.push(SheetSchedulePlacement {
                id: Id::new(),
                schedule,
                paper_rect_mm: SheetPaperRect {
                    min_mm: Point2::new(18.0, 154.0),
                    max_mm: Point2::new(402.0, 238.0),
                },
            }),
            1 => parameters.schedule_placements[0].paper_rect_mm.min_mm.y = 160.0,
            _ => parameters.schedule_placements.clear(),
        }
        doc.execute(
            "Table layout",
            vec![Command::UpdateSheet {
                id: sheet_id,
                parameters,
            }],
        )
        .unwrap();
        assert_eq!(doc.revision(), revision + 1);
        let after = doc.model().clone();
        assert!(doc.undo());
        assert_eq!(doc.model(), &before);
        assert!(doc.redo());
        assert_eq!(doc.model(), &after);
        if action < 2 {
            assert!(
                doc.execute(
                    "Delete referenced schedule",
                    vec![Command::RemoveSchedule(schedule)]
                )
                .is_err()
            );
            assert_eq!(doc.model(), &after);
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("table.osb");
        ZipJsonStorage.save(&doc, &path).unwrap();
        assert_eq!(ZipJsonStorage.open(&path).unwrap().model(), &after);
    }
}

#[test]
fn mixed_plan_and_section_viewports_round_trip_with_independent_scales() {
    let mut model = Model::new("Mixed view sheet");
    let level = *model.levels.keys().next().unwrap();
    let plan = View::new("core.view", ViewParams::floor_plan("Ground plan", level));
    let section = View::new(
        "core.view",
        ViewParams {
            name: "Building section".into(),
            kind: ViewKind::Section,
            level: Some(level),
            settings_revision: 0,
            plan: None,
            section: Some(SectionViewSettings::new(
                Point2::new(0.0, 0.0),
                Point2::new(8.0, 0.0),
                -0.5,
                4.0,
            )),
        },
    );
    let plan_id = plan.id();
    let section_id = section.id();
    model.views.insert(plan_id, plan);
    model.views.insert(section_id, section);

    let mut parameters = SheetParams::new("A101", "Plan and section");
    parameters.viewports = vec![
        SheetViewport {
            id: Id::new(),
            view: plan_id,
            model_center_m: Point2::new(12.0, 7.0),
            paper_center_mm: Point2::new(100.0, 80.0),
            width_mm: 180.0,
            height_mm: 124.0,
            scale_denominator: 100.0,
            title_override: None,
        },
        SheetViewport {
            id: Id::new(),
            view: section_id,
            model_center_m: Point2::new(4.0, 1.75),
            paper_center_mm: Point2::new(310.0, 80.0),
            width_mm: 180.0,
            height_mm: 124.0,
            scale_denominator: 50.0,
            title_override: None,
        },
    ];
    let sheet = Sheet::new("core.sheet", parameters);
    let sheet_id = sheet.id();
    model.sheets.insert(sheet_id, sheet);
    let doc = Document::from_model(model).unwrap();

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("mixed-viewports.osb");
    ZipJsonStorage.save(&doc, &path).unwrap();
    let reopened = ZipJsonStorage.open(&path).unwrap();
    assert_eq!(reopened.model(), doc.model());
    let viewports = &reopened.model().sheets[&sheet_id].parameters.viewports;
    assert_eq!(viewports.len(), 2);
    assert_eq!(viewports[0].view, plan_id);
    assert_eq!(viewports[0].scale_denominator, 100.0);
    assert_eq!(viewports[1].view, section_id);
    assert_eq!(viewports[1].scale_denominator, 50.0);
}
