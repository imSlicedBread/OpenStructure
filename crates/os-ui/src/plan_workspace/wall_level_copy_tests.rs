//! Cross-level UI acceptance reuses the joined, typed assembly fixture.
use super::*;
use crate::plan_workspace::wall_level_copy::Draft as LevelDraft;

fn destination(model: &mut Model, source: Id, name: &str, elevation: f64) -> (Id, Id) {
    let source_level = model.views[&source].parameters.level.unwrap();
    let mut params = model.levels[&source_level].parameters.clone();
    params.name = format!("{name} level");
    params.elevation = elevation;
    let level = os_model::Level::new("core.level", params);
    let level_id = level.id();
    model.levels.insert(level_id, level);
    let view = os_model::View::new(
        "core.view",
        os_model::ViewParams::floor_plan(name, level_id),
    );
    let view_id = view.id();
    model.views.insert(view_id, view);
    (view_id, level_id)
}

fn click(h: &mut Harness, label: &str) {
    h.click_text_in_band(label, 0.0, h.size.y);
    h.frame(vec![]);
    h.frame(vec![]);
}

fn choose_upper(h: &mut Harness) {
    click(h, "Choose floor plan");
    click(h, "Upper plan · Upper plan level");
}

#[test]
fn aligned_level_copy_ui_dependencies_preview_history_and_reopen_both_dpis() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        let (mut model, bay) = fixture(&mut h);
        let (upper, level) = destination(&mut model, h.view, "Upper plan", 4.0);
        let source_level = model.views[&h.view].parameters.level.unwrap();
        for id in &bay.walls {
            model
                .walls
                .get_mut(id)
                .unwrap()
                .header
                .relationships
                .insert("level".into(), vec![source_level]);
        }
        let settings = model
            .views
            .get_mut(&upper)
            .unwrap()
            .parameters
            .plan
            .as_mut()
            .unwrap();
        settings.basis.rotation = 0.37;
        settings.basis.origin = Point2::new(5.0, -2.0);
        load(&mut h, model, &bay.walls);
        h.app.plans.split = true;
        h.frame(vec![]);
        let before = h.app.editor.document.model().clone();
        let scene = h.app.editor.scene.clone();
        let history = h.app.editor.document.history_stats();
        let revision = h.app.editor.document.revision();
        click(&mut h, "Copy to level…");
        assert!(h.app.wall_level_copy_active(), "{}", h.app.status);
        choose_upper(&mut h);
        assert!(
            h.app
                .plans
                .wall_level_copy
                .as_ref()
                .unwrap()
                .preview
                .is_ok()
        );
        assert!(h.output.shapes.iter().any(|s| matches!(&s.shape, egui::Shape::LineSegment { stroke, .. } if stroke.color == theme::ACCENT)));
        assert_eq!(h.app.editor.document.model(), &before);
        assert_eq!(h.app.editor.scene, scene);
        assert_eq!(h.app.editor.document.revision(), revision);
        assert_eq!(h.app.editor.document.history_stats(), history);
        assert_eq!(h.app.plans.active, Some(h.view));
        assert_eq!(h.app.selected_ids, bay.walls);
        // Global undo must not reach the source document through this modal.
        h.frame(vec![egui::Event::Key {
            key: egui::Key::Z,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND,
        }]);
        assert!(h.app.wall_level_copy_active());
        assert_eq!(h.app.editor.document.model(), &before);
        click(&mut h, "Apply copy to level");
        h.settle();
        assert!(!h.app.status_error, "{}", h.app.status);
        assert!(!h.app.wall_level_copy_active());
        let after = h.app.editor.document.model().clone();
        let copies = copied_walls(&before, &after);
        assert_eq!(copies.len(), bay.walls.len());
        assert_eq!(h.app.plans.active, Some(upper));
        assert_eq!(h.app.active_level, level);
        assert_eq!(h.app.selected_ids, copies);
        assert!(copies.contains(&h.app.selected.unwrap()));
        assert_eq!(after.opening_tags, before.opening_tags);
        assert_eq!(after.rooms, before.rooms);
        assert_eq!(after.room_tags, before.room_tags);
        assert_eq!(after.dimensions, before.dimensions);
        assert_eq!(after.wall_types, before.wall_types);
        assert_eq!(after.opening_types, before.opening_types);
        for (id, wall) in &before.walls {
            assert_eq!(&after.walls[id], wall);
        }
        for (id, opening) in &before.openings {
            assert_eq!(&after.openings[id], opening);
        }
        for (id, join) in &before.wall_joins {
            assert_eq!(&after.wall_joins[id], join);
        }
        let context = h.app.editor.native_plan_context(upper).unwrap();
        let drawing = h.app.plans.drawing.as_ref().unwrap();
        let mut wall_map = std::collections::BTreeMap::new();
        for source in &bay.walls {
            let old = &before.walls[source];
            let id = *copies
                .iter()
                .find(|id| after.walls[id].parameters.path == old.parameters.path)
                .unwrap();
            wall_map.insert(*source, id);
            let mut expected = old.parameters.clone();
            expected.level = level;
            assert_eq!(after.walls[&id].parameters, expected);
            assert_eq!(after.walls[&id].header.properties, old.header.properties);
            assert_eq!(after.walls[&id].header.relationships["level"], vec![level]);
            assert_eq!(
                after.wall_type_assignments[&id],
                before.wall_type_assignments[source]
            );
            assert_eq!(
                after.element_lifecycle(id),
                before.element_lifecycle(*source)
            );
            assert!(h.app.editor.scene.contains_key(&id));
            let item = drawing
                .items(context)
                .unwrap()
                .iter()
                .find(|i| i.entity == id)
                .unwrap();
            let vertices = item.footprint.vertices();
            let point = Point2::new(
                vertices.iter().map(|p| p.x).sum::<f64>() / vertices.len() as f64,
                vertices.iter().map(|p| p.y).sum::<f64>() / vertices.len() as f64,
            );
            assert_eq!(drawing.pick(context, point).unwrap(), Some(id));
        }
        let new_openings: Vec<_> = after
            .openings
            .values()
            .filter(|o| !before.openings.contains_key(&o.id()))
            .collect();
        assert_eq!(new_openings.len(), 2);
        for source in bay.openings {
            let old = &before.openings[&source];
            let copy = new_openings
                .iter()
                .find(|o| o.parameters.name == old.parameters.name)
                .unwrap();
            let mut expected = old.parameters.clone();
            expected.host = wall_map[&expected.host];
            near_opening_params(&copy.parameters, &expected);
            assert_eq!(
                after.opening_clearances[&copy.id()],
                before.opening_clearances[&source]
            );
            assert_eq!(
                after.element_lifecycle(copy.id()),
                before.element_lifecycle(source)
            );
            assert!(h.app.editor.scene.contains_key(&copy.id()));
        }
        let joins: Vec<_> = after
            .wall_joins
            .values()
            .filter(|j| !before.wall_joins.contains_key(&j.id()))
            .collect();
        assert_eq!(joins.len(), bay.joins.len());
        for join in joins {
            assert!(
                join.parameters
                    .members()
                    .iter()
                    .all(|id| copies.contains(id))
            );
            if let WallJoinParams::Corner { owner, .. } = join.parameters {
                assert!(copies.contains(&owner));
            }
        }
        assert_eq!(
            h.app.editor.document.history_stats().undo_entries,
            history.undo_entries + 1
        );
        let copied_scene = h.app.editor.scene.clone();
        h.app.history(false);
        h.settle();
        assert_eq!(h.app.editor.document.model(), &before);
        assert_eq!(h.app.editor.scene, scene);
        h.app.history(true);
        h.settle();
        assert_eq!(h.app.editor.document.model(), &after);
        assert_eq!(h.app.editor.scene, copied_scene);
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("wall-level-copy.osb");
        h.app.editor.save(&path).unwrap();
        h.app.editor.open(&path).unwrap();
        assert_eq!(h.app.editor.document.model(), &after);
        assert!(copies.iter().all(|id| h.app.editor.scene.contains_key(id)));
    }
}

#[test]
fn aligned_level_copy_cancel_stale_and_invalid_targets_both_dpis() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        let (mut model, bay) = fixture(&mut h);
        let (upper, _) = destination(&mut model, h.view, "Upper plan", 4.0);
        let (collision, _) = destination(&mut model, h.view, "Overlap", 2.0);
        let (occupied, occupied_level) = destination(&mut model, h.view, "Occupied", 12.0);
        let mut obstacle = model.walls[&h.wall].clone();
        obstacle.header.id = Id::new();
        obstacle.parameters.level = occupied_level;
        model.walls.insert(obstacle.id(), obstacle);
        let (ceiling, _) = destination(&mut model, h.view, "Ceiling", 8.0);
        model.views.get_mut(&ceiling).unwrap().parameters.plan =
            Some(os_model::PlanSettings::reflected_ceiling());
        let (foreign, foreign_level) = destination(&mut model, h.view, "Foreign", 8.0);
        let building = os_model::Building::new(
            "core.building",
            model.buildings.values().next().unwrap().parameters.clone(),
        );
        model
            .levels
            .get_mut(&foreign_level)
            .unwrap()
            .parameters
            .building = building.id();
        model.buildings.insert(building.id(), building);
        load(&mut h, model, &bay.walls);
        let before = h.app.editor.document.model().clone();
        let scene = h.app.editor.scene.clone();
        let history = h.app.editor.document.history_stats();
        for invalid in [h.view, Id::new(), ceiling, foreign, collision, occupied] {
            let mut draft = LevelDraft::begin(&h.app).unwrap();
            draft.choose(&h.app, invalid);
            assert!(draft.preview.is_err(), "invalid destination {invalid}");
            assert!(draft.commit(&mut h.app).is_err());
            assert_eq!(h.app.editor.document.model(), &before);
            assert_eq!(h.app.editor.scene, scene);
            assert_eq!(h.app.editor.document.history_stats(), history);
        }
        h.app.selected_ids = [*bay.walls.first().unwrap()].into_iter().collect();
        assert!(LevelDraft::begin(&h.app).is_err());
        h.app.selected_ids = bay.walls.clone();
        for cancel_button in [true, false] {
            click(&mut h, "Copy to level…");
            choose_upper(&mut h);
            if cancel_button {
                click(&mut h, "Cancel copy to level");
            } else {
                h.frame(vec![escape()]);
            }
            h.frame(vec![]);
            assert!(!h.app.wall_level_copy_active());
            assert_eq!(h.app.editor.document.model(), &before);
            assert_eq!(h.app.editor.scene, scene);
            assert_eq!(h.app.editor.document.history_stats(), history);
            assert_eq!(h.app.selected_ids, bay.walls);
        }
        // Selection and document revisions independently revoke the prepared copy.
        h.app.begin_wall_level_copy();
        h.app.selected_ids.clear();
        h.frame(vec![]);
        assert!(!h.app.wall_level_copy_active());
        assert_eq!(h.app.editor.document.model(), &before);
        h.app.selected_ids = bay.walls.clone();
        h.app.begin_wall_level_copy();
        h.app.focus_plan(Some(upper));
        assert!(!h.app.wall_level_copy_active());
        assert_eq!(h.app.editor.document.model(), &before);
        assert_eq!(h.app.editor.scene, scene);
        assert_eq!(h.app.editor.document.history_stats(), history);
        h.app.focus_plan(Some(h.view));
        h.settle();
        let mut draft = LevelDraft::begin(&h.app).unwrap();
        draft.choose(&h.app, upper);
        h.app.plans.wall_level_copy = Some(draft);
        let mut parameters = before.views[&upper].parameters.clone();
        parameters.name = "Changed target".into();
        h.app
            .editor
            .command(
                "Change target",
                Command::UpdateView {
                    id: upper,
                    parameters,
                },
            )
            .unwrap();
        let edited = h.app.editor.document.model().clone();
        let edited_scene = h.app.editor.scene.clone();
        let edited_history = h.app.editor.document.history_stats();
        h.frame(vec![]);
        assert!(!h.app.wall_level_copy_active());
        assert_eq!(h.app.editor.document.model(), &edited);
        assert_eq!(h.app.editor.scene, edited_scene);
        assert_eq!(h.app.editor.document.history_stats(), edited_history);
    }
}

#[cfg(feature = "external-plugins")]
#[test]
#[ignore = "requires OPENSTRUCTURE_WALL_TEST_PLUGIN installed guest directory"]
fn aligned_level_copy_rejects_installed_wall_provider_and_cancels_provider_change() {
    let mut h = Harness::new(PROFILES[0].0, PROFILES[0].1);
    let (mut model, bay) = fixture(&mut h);
    let (upper, _) = destination(&mut model, h.view, "Upper plan", 4.0);
    load(&mut h, model, &bay.walls);
    let before = h.app.editor.document.model().clone();
    let scene = h.app.editor.scene.clone();
    let history = h.app.editor.document.history_stats();
    let mut draft = LevelDraft::begin(&h.app).unwrap();
    draft.choose(&h.app, upper);
    assert!(draft.preview.is_ok());
    let directory =
        std::env::var_os("OPENSTRUCTURE_WALL_TEST_PLUGIN").expect("installed guest directory");
    h.app.editor.host.unload(os_walls::PLUGIN_ID).unwrap();
    h.app
        .editor
        .host
        .load_wasm_directory(
            std::path::Path::new(&directory),
            [
                os_plugin_api::Permission::ModelRead,
                os_plugin_api::Permission::ModelWrite,
                os_plugin_api::Permission::UiTool,
            ]
            .into(),
        )
        .unwrap();
    assert!(h.app.editor.host.worker_supported(os_walls::PLUGIN_ID));
    assert!(LevelDraft::begin(&h.app).is_err());
    assert!(draft.commit(&mut h.app).is_err());
    assert_eq!(h.app.editor.document.model(), &before);
    assert_eq!(h.app.editor.scene, scene);
    assert_eq!(h.app.editor.document.history_stats(), history);
}
