//! Real egui phase settings and shared canvas/paper acceptance at both desktop DPIs.
use super::*;
use crate::plan::phase_tests::{fixture, set_phase};
use os_model::PhaseFilter;
use os_render::sheet::PaperMarkKind;

fn target_phase_combo(h: &mut Harness) {
    // Egui retains the settings scroll offset/focus across modal opens.
    let p = h.text_rect("Floor plan settings").center() + egui::vec2(0., 100.);
    h.frame(vec![
        egui::Event::PointerMoved(p),
        egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0., 2000.),
            modifiers: egui::Modifiers::NONE,
        },
    ]);
    for _ in 0..20 {
        h.frame(vec![]);
    }
    let target = h.text_rect("Target phase");
    h.click_at(egui::pos2(target.right() + 100., target.center().y));
}

#[test]
fn phase_stair_canvas_strokes_and_split_3d_use_the_committed_geometry_at_both_dpis() {
    for (size, scale) in [
        (egui::vec2(1280., 800.), 1.),
        (egui::vec2(1000., 650.), 1.5),
    ] {
        let mut h = Harness::at_size(size, scale);
        let level = h.app.active_level;
        let mut model = h.app.editor.document.model().clone();
        let mut upper = model.levels[&level].clone();
        upper.header.id = Id::new();
        upper.parameters.name = "Upper".into();
        upper.parameters.elevation = 3.;
        let upper_id = upper.id();
        model.levels.insert(upper_id, upper);
        h.app.editor.document = Document::from_model(model).unwrap();
        let stair = os_model::Stair::new(
            "core.stair",
            os_model::StairParams {
                name: "Phase stair".into(),
                lower_level: level,
                upper_level: upper_id,
                start: Point2::new(0., 0.),
                end: Point2::new(4., 0.),
                width: 1.,
                riser_count: 12,
                structural_thickness: 0.15,
                material: None,
            },
        );
        let id = stair.id();
        h.app
            .editor
            .command("Stair", Command::AddStair(stair))
            .unwrap();
        let view = h.app.editor.create_floor_plan("Stair plan", level).unwrap();
        h.frame(vec![]);
        h.app.focus_plan(Some(view));
        h.app.select(None);
        h.settle_plan();
        h.click("Fit plan");
        h.settle_plan();
        let c = h.app.editor.native_plan_context(view).unwrap();
        let d = h.app.editor.native_drawing(view).unwrap();
        let stair = &d.stairs(c).unwrap()[0];
        let arrow = &stair.lines[0];
        let a = h.plan_point(view, arrow.start);
        let b = h.plan_point(view, arrow.end);
        let expected = egui::Color32::from_rgb(35, 65, 88);
        assert!(
            h.output.shapes.iter().any(
                |s| matches!(&s.shape,egui::Shape::LineSegment {points,stroke}
            if points[0].distance(a)<0.1 && points[1].distance(b)<0.1 && stroke.color==expected)
            ),
            "committed stair arrow uses phase appearance"
        );
        let tread = &stair.visible_tread_lines[0];
        let a = h.plan_point(view, tread.start);
        let b = h.plan_point(view, tread.end);
        assert!(
            h.output.shapes.iter().any(
                |s| matches!(&s.shape,egui::Shape::LineSegment {points,stroke}
            if points[0].distance(a)<0.1 && points[1].distance(b)<0.1 && stroke.color==expected)
            ),
            "committed stair footprint uses phase appearance"
        );
        h.click("Split 2D / 3D");
        h.settle_plan();
        let scene = h.app.editor.scene.clone();
        let phase = h.app.editor.document.model().latest_phase().unwrap();
        set_phase(&mut h.app.editor, view, phase, PhaseFilter::ShowExisting);
        h.settle_plan();
        let c = h.app.editor.native_plan_context(view).unwrap();
        assert!(
            h.app
                .editor
                .native_drawing(view)
                .unwrap()
                .stairs(c)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            h.app.editor.scene, scene,
            "plan filter must not change committed 3D"
        );
        assert!(h.app.editor.document.model().stairs.contains_key(&id));
    }
}

#[test]
fn plan_phase_settings_draft_canvas_sheet_pdf_history_and_stale_workers_at_both_dpis() {
    for (size, scale) in [
        (egui::vec2(1280., 800.), 1.),
        (egui::vec2(1000., 650.), 1.5),
    ] {
        let mut h = Harness::at_size(size, scale);
        let (editor, view, walls, phases) = fixture();
        h.app.editor = editor;
        h.app.active_level = h.app.editor.document.model().views[&view]
            .parameters
            .level
            .unwrap();
        h.frame(vec![]);
        h.app.focus_plan(Some(view));
        h.app.select(None);
        h.settle_plan();
        let before = h.app.editor.document.model().clone();
        let history = h.app.editor.document.history_stats();
        let old = h.app.editor.plan_snapshot(view).unwrap();
        h.click("Plan settings");
        h.frame(vec![]);
        h.frame(vec![]);
        h.click("Show all");
        h.click("Demolished");
        assert_eq!(h.app.editor.document.model(), &before);
        h.click("Apply plan settings");
        assert!(!h.app.status_error, "{}", h.app.status);
        h.settle_plan();
        assert_eq!(
            h.app.editor.document.history_stats().undo_entries,
            history.undo_entries + 1
        );
        let context = h.app.editor.native_plan_context(view).unwrap();
        assert!(old.derive().unwrap().items(context).is_err());
        let drawing = h.app.editor.native_drawing(view).unwrap();
        assert!(
            drawing
                .items(context)
                .unwrap()
                .iter()
                .all(|i| i.entity == walls[2])
        );
        let color = egui::Color32::from_rgb(155, 82, 68);
        assert!(
            h.output.shapes.iter().any(
                |s| matches!(&s.shape,egui::Shape::LineSegment {stroke,..} if stroke.color==color)
            ),
            "phase color reaches canvas"
        );
        let after = h.app.editor.document.model().clone();
        h.click("Undo");
        h.settle_plan();
        assert_eq!(h.app.editor.document.model(), &before);
        h.click("Redo");
        h.settle_plan();
        assert_eq!(h.app.editor.document.model(), &after);
        h.click("Plan settings");
        h.frame(vec![]);
        h.frame(vec![]);
        assert!(h.app.plan_draft.is_some(), "plan settings must open");
        target_phase_combo(&mut h);
        h.click("Existing");
        assert_eq!(h.app.editor.document.model(), &after);
        h.click("Apply plan settings");
        h.settle_plan();
        assert_eq!(
            h.app.editor.document.model().views[&view]
                .parameters
                .plan
                .unwrap()
                .target_phase,
            Some(phases[0])
        );
        h.click("Undo");
        h.settle_plan();
        assert_eq!(h.app.editor.document.model(), &after);
        h.click("Plan settings");
        h.frame(vec![]);
        h.frame(vec![]);
        assert!(h.app.plan_draft.is_some(), "plan settings must reopen");
        target_phase_combo(&mut h);
        h.click("Existing");
        h.app
            .editor
            .command(
                "Concurrent edit",
                Command::RenameProject("Edited while open".into()),
            )
            .unwrap();
        let stale_model = h.app.editor.document.model().clone();
        h.click("Apply plan settings");
        assert!(h.app.plan_draft.is_some());
        assert_eq!(h.app.editor.document.model(), &stale_model);
        h.click("Cancel plan settings");
        h.settle_plan();
        // Every visible status resolves the same line color/width/dash in paper/PDF.
        set_phase(&mut h.app.editor, view, phases[1], PhaseFilter::ShowAll);
        h.settle_plan();
        h.app.create_sheet_from_active_view();
        h.settle_plan();
        let page = h.app.sheet_page().unwrap();
        let c = h.app.editor.native_plan_context(view).unwrap();
        let d = h.app.editor.native_drawing(view).unwrap();
        for id in &walls[..4] {
            let appearance = d
                .appearance(c, *id, os_geometry::plan::PlanRole::Cut)
                .unwrap()
                .unwrap();
            assert!(page.marks().iter().any(|m| matches!(&m.kind,PaperMarkKind::Path {stroke:Some(s),..}
                if [s.color.red,s.color.green,s.color.blue]==appearance.color && s.dashed==appearance.dashed && s.width_mm==appearance.weight_mm)));
        }
        let pdf = String::from_utf8(page.to_pdf().unwrap()).unwrap();
        assert!(pdf.contains(" RG") && pdf.contains("] 0 d"));
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("phased-sheet.osb");
        h.app.editor.save(&path).unwrap();
        let saved = h.app.editor.document.model().clone();
        h.app.editor.open(&path).unwrap();
        assert_eq!(h.app.editor.document.model(), &saved);
    }
}
