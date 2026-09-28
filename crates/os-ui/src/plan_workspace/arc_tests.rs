use super::*;

#[test]
fn arc_authoring_preview_commit_properties_history_both_dpis() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        h.app.plans.snaps.enabled = false;
        h.app.plans.split = true;
        h.settle();
        let before = h.app.editor.document.model().clone();
        let revision = h.app.editor.document.revision();
        let scene = h.app.editor.scene.clone();
        h.app.begin_plan_arc(h.view);
        h.frame(vec![]);
        h.click(h.point(Point2::new(-0.8, 0.5)));
        h.frame(vec![]);
        h.click(h.point(Point2::new(0.0, 1.1)));
        h.frame(vec![egui::Event::PointerMoved(
            h.point(Point2::new(0.8, 0.5)),
        )]);
        assert_eq!(h.app.editor.document.model(), &before);
        assert_eq!(h.app.editor.scene, scene);
        assert_eq!(h.app.editor.document.revision(), revision);
        h.click(h.point(Point2::new(0.8, 0.5)));
        h.settle();
        assert!(!h.app.status_error, "{}", h.app.status);
        assert!(h.app.wall_gesture.is_none());
        let id = h.app.selected.unwrap();
        let committed = h.app.editor.document.model().clone();
        let p = &committed.walls[&id].parameters;
        assert!(!p.path.is_straight());
        assert_eq!(
            (p.height, p.thickness, p.material, p.level),
            (
                3.7,
                0.27,
                before.walls[&h.wall].parameters.material,
                h.app.active_level
            )
        );
        assert_eq!(h.app.editor.document.revision(), revision + 1);
        assert!(h.app.editor.scene[&id].vertices.len() > 16);
        let context = h.app.editor.native_plan_context(h.view).unwrap();
        assert!(
            h.app
                .plans
                .drawing
                .as_ref()
                .unwrap()
                .items(context)
                .unwrap()
                .iter()
                .any(|i| i.entity == id)
        );
        h.app.history(false);
        assert_eq!(h.app.editor.document.model(), &before);
        h.app.history(true);
        assert_eq!(h.app.editor.document.model(), &committed);
        h.app.select(Some(id));
        if let os_model::WallPath::CircularArc { radius, .. } = &mut h.app.draft.path {
            *radius += 0.2;
        }
        h.app.apply_wall();
        assert!(!h.app.status_error, "{}", h.app.status);
        assert_eq!(h.app.selected, Some(id));
        assert_ne!(
            h.app.editor.document.model().walls[&id].parameters.path,
            p.path
        );
    }
}

#[test]
fn arc_escape_invalid_and_stale_do_not_mutate_both_dpis() {
    for (size, scale) in PROFILES {
        for cancel in ["escape", "view", "revision", "invalid"] {
            let mut h = Harness::new(size, scale);
            h.app.plans.snaps.enabled = false;
            let before = h.app.editor.document.model().clone();
            h.app.begin_plan_arc(h.view);
            h.frame(vec![]);
            h.click(h.point(Point2::new(-0.8, 0.5)));
            h.frame(vec![]);
            h.click(h.point(Point2::new(0.0, 0.5)));
            h.frame(vec![]);
            match cancel {
                "escape" => h.frame(vec![escape()]),
                "view" => {
                    h.app.focus_plan(None);
                    h.frame(vec![]);
                }
                "revision" => {
                    h.app.editor.document = Document::from_model(before.clone()).unwrap();
                    h.frame(vec![]);
                }
                _ => {
                    h.click(h.point(Point2::new(0.8, 0.5)));
                    h.frame(vec![escape()]);
                }
            }
            assert_eq!(h.app.editor.document.model(), &before);
            assert!(h.app.wall_gesture.is_none(), "{cancel}");
        }
    }
}
