use super::*;
#[path = "../../../os-model/tests/support/plan_graphics_rules.rs"]
mod support;
use os_model::PlanRuleCategory;
use os_render::sheet::PaperMarkKind;

fn click_settings(h: &mut Harness, text: &str) {
    for _ in 0..20 {
        h.frame(vec![]);
    }
    for _ in 0..40 {
        let target = h
            .output
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.job.text == text => Some((
                    egui::Rect::from_min_size(t.pos, t.galley.size()),
                    s.clip_rect,
                )),
                _ => None,
            })
            .next_back();
        if let Some((rect, clip)) = target
            && clip.contains_rect(rect)
        {
            h.click_at(rect.center());
            return;
        }
        let heading = h.text_rect("Floor plan settings");
        let delta = if target.is_some_and(|(rect, _)| rect.top() < heading.bottom()) {
            180.
        } else {
            -90.
        };
        let p = heading.center() + egui::vec2(0., 110.);
        h.frame(vec![
            egui::Event::PointerMoved(p),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0., delta),
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        for _ in 0..10 {
            h.frame(vec![]);
        }
    }
    panic!("settings control unreachable: {text}");
}

#[test]
fn rule_graphics_desktop_create_edit_reorder_cancel_canvas_sheet_pdf_at_both_dpis() {
    for (size, scale) in [
        (egui::vec2(1280., 800.), 1.),
        (egui::vec2(1000., 650.), 1.5),
    ] {
        let mut h = Harness::at_size(size, scale);
        let (model, view, [_, door, _, _]) = support::fixture();
        h.app.editor.document = Document::from_model(model).unwrap();
        h.app.editor.regenerate().unwrap();
        h.app.active_level = h.app.editor.document.model().views[&view]
            .parameters
            .level
            .unwrap();
        h.frame(vec![]);
        h.app.focus_plan(Some(view));
        h.app.select(None);
        h.settle_plan();
        h.click("Fit plan");
        h.settle_plan();
        let before = h.app.editor.document.model().clone();
        let history = h.app.editor.document.history_stats();
        h.click("Plan settings");
        h.frame(vec![]);
        h.frame(vec![]);
        click_settings(&mut h, "Rule-based graphics");
        click_settings(&mut h, "Add graphics rule");
        click_settings(&mut h, "Walls");
        h.click("Doors");
        // This typed family emits cut symbols at the current view range.
        click_settings(&mut h, "Cut");
        let cut = h.text_rect("Cut");
        h.click_at(egui::pos2(cut.right() + 16., cut.center().y));
        assert_eq!(h.app.editor.document.model(), &before);
        h.click("Apply plan settings");
        assert!(!h.app.status_error, "{}", h.app.status);
        h.settle_plan();
        assert_eq!(
            h.app.editor.document.history_stats().undo_entries,
            history.undo_entries + 1
        );
        let first = h.app.editor.document.model().plan_graphics_rules[&view][0].clone();
        assert_eq!(first.category, PlanRuleCategory::Doors);
        let c = h.app.editor.native_plan_context(view).unwrap();
        let appearance = h
            .app
            .editor
            .native_drawing(view)
            .unwrap()
            .appearance(c, door, os_geometry::plan::PlanRole::Cut)
            .unwrap()
            .unwrap();
        assert_eq!(appearance.color, [0, 0, 0]);
        assert_eq!(appearance.weight_mm, 0.35);
        assert!(h.output.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::LineSegment {stroke,..} if stroke.color==egui::Color32::BLACK)),"rule reaches canvas");
        h.click("Plan settings");
        h.frame(vec![]);
        h.frame(vec![]);
        click_settings(&mut h, "Add graphics rule");
        click_settings(&mut h, "Walls");
        h.click("Windows");
        click_settings(&mut h, "Up");
        h.click("Apply plan settings");
        assert!(!h.app.status_error, "{}", h.app.status);
        h.settle_plan();
        let rows = &h.app.editor.document.model().plan_graphics_rules[&view];
        assert_eq!(
            rows[0].category,
            PlanRuleCategory::Windows,
            "saved order: {rows:?}"
        );
        assert_eq!(rows[1].id, first.id);
        let committed = h.app.editor.document.model().clone();
        h.click("Undo");
        h.settle_plan();
        assert_eq!(
            h.app.editor.document.model().plan_graphics_rules[&view],
            vec![first.clone()]
        );
        h.click("Redo");
        h.settle_plan();
        assert_eq!(h.app.editor.document.model(), &committed);
        h.click("Plan settings");
        h.frame(vec![]);
        h.frame(vec![]);
        click_settings(&mut h, "Enabled");
        h.frame(vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }]);
        h.frame(vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }]);
        assert!(h.app.plan_draft.is_none());
        assert_eq!(h.app.editor.document.model(), &committed);
        h.settle_plan();
        h.app.create_sheet_from_active_view();
        h.settle_plan();
        let page = h.app.sheet_page().unwrap();
        assert!(page.marks().iter().any(|m|matches!(&m.kind,PaperMarkKind::Path {stroke:Some(s),..} if [s.color.red,s.color.green,s.color.blue]==appearance.color && s.width_mm==appearance.weight_mm && s.dashed==appearance.dashed)));
        let pdf = String::from_utf8(page.to_pdf().unwrap()).unwrap();
        assert!(
            pdf.contains("0.000000 0.000000 0.000000 RG"),
            "vector PDF contains rule RGB"
        );
    }
}
