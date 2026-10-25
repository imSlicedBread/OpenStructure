use super::*;
use crate::plan_workspace::sheet_tests::raw_input;

struct Harness {
    app: DesktopApp,
    ctx: egui::Context,
    size: egui::Vec2,
    dpi: f32,
    time: f64,
}
impl Harness {
    fn frame(&mut self, events: Vec<egui::Event>) -> egui::FullOutput {
        self.time += 0.1;
        self.ctx
            .run(raw_input(self.size, self.dpi, self.time, events), |ctx| {
                self.app.show(ctx)
            })
    }
    fn click(&mut self, p: egui::Pos2) {
        self.frame(vec![egui::Event::PointerMoved(p)]);
        for pressed in [true, false] {
            self.frame(vec![egui::Event::PointerButton {
                pos: p,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            }]);
        }
    }
    fn label(&mut self, label: &str) {
        self.frame(vec![]);
        let output = self.frame(vec![]);
        let p = output
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.job.text == label => {
                    Some(egui::Rect::from_min_size(t.pos, t.galley.size()).center())
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing {label}"));
        assert!(egui::Rect::from_min_size(egui::Pos2::ZERO, self.size).contains(p));
        self.click(p);
    }
}
fn key(key: egui::Key) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}

#[test]
fn duplicate_dialog_both_profiles_cancel_apply_stale_input_history_and_delete() {
    for (size, dpi) in [
        (egui::vec2(1280., 800.), 1.),
        (egui::vec2(1000., 650.), 1.5),
    ] {
        let mut app = DesktopApp::new().unwrap();
        let source = app
            .editor
            .create_floor_plan("Duplicate source", app.active_level)
            .unwrap();
        let note = os_model::TextNote::new("core.text_note", os_model::TextNoteParams::new(source));
        app.editor
            .command("Fixture", Command::AddTextNote(note))
            .unwrap();
        app.plans.poll(&app.editor);
        app.focus_plan(Some(source));
        let mut h = Harness {
            app,
            ctx: egui::Context::default(),
            size,
            dpi,
            time: 0.,
        };
        theme::apply(&h.ctx);
        let before = h.app.editor.document.model().clone();
        let history = h.app.editor.document.history_stats();
        h.app.begin_detail_line();
        assert!(h.app.plans.detail_line_draft.is_some());
        h.label("Duplicate plan…");
        assert!(h.app.plans.plan_duplicate.is_some());
        assert!(h.app.plans.detail_line_draft.is_none());
        h.label("Duplicate source Copy");
        h.frame(vec![egui::Event::Text("Edited".into())]);
        assert!(
            h.app
                .plans
                .plan_duplicate
                .as_ref()
                .unwrap()
                .name
                .contains("Edited")
        );
        h.frame(vec![key(egui::Key::Delete), key(egui::Key::D)]);
        assert_eq!(h.app.editor.document.model(), &before);
        assert!(h.app.plans.detail_line_draft.is_none());
        let selected = h.app.selected;
        if let Some(rect) = h.app.plans.canvas_rect {
            h.click(rect.left_bottom() + egui::vec2(20., -20.));
        }
        assert_eq!(h.app.selected, selected);
        assert_eq!(h.app.editor.document.model(), &before);
        h.label("Cancel duplication");
        assert!(h.app.plans.plan_duplicate.is_none());
        assert_eq!(h.app.editor.document.history_stats(), history);
        h.label("Duplicate plan…");
        h.frame(vec![key(egui::Key::Escape)]);
        assert!(h.app.plans.plan_duplicate.is_none());
        assert_eq!(h.app.editor.document.model(), &before);
        h.label("Duplicate plan…");
        h.label("Duplicate with detailing");
        h.label("Apply duplication");
        assert!(h.app.plans.plan_duplicate.is_none(), "{}", h.app.status);
        let copy = h.app.plans.active.unwrap();
        assert_ne!(copy, source);
        let after = h.app.editor.document.model().clone();
        assert_eq!(after.text_notes.len(), before.text_notes.len() + 1);
        assert_eq!(
            h.app.editor.document.history_stats().undo_entries,
            history.undo_entries + 1
        );
        h.app.history(false);
        assert_eq!(h.app.editor.document.model(), &before);
        h.app.history(true);
        assert_eq!(h.app.editor.document.model(), &after);
        h.app.focus_plan(Some(copy));
        h.label("Delete plan");
        assert_eq!(h.app.editor.document.model(), &before);
        h.app.history(false);
        assert_eq!(h.app.editor.document.model(), &after);
        h.app.focus_plan(Some(source));
        h.label("Duplicate plan…");
        h.app
            .editor
            .command("External", Command::RenameProject("External".into()))
            .unwrap();
        let external = h.app.editor.document.model().clone();
        h.frame(vec![]);
        assert!(h.app.plans.plan_duplicate.is_none());
        assert_eq!(h.app.editor.document.model(), &external);
        h.label("Duplicate plan…");
        h.app.focus_plan(Some(copy));
        h.frame(vec![]);
        assert!(h.app.plans.plan_duplicate.is_none());
        h.app.begin_plan_duplicate(source);
        h.app.plans.active = Some(source);
        h.app.editor.document = os_document::Document::from_model(external.clone()).unwrap();
        h.frame(vec![]);
        assert!(h.app.plans.plan_duplicate.is_none());
        assert_eq!(h.app.editor.document.model(), &external);
    }
}

#[test]
fn duplicate_native_drawing_sheet_and_pdf_keep_annotation_geometry() {
    use os_model::*;
    use os_render::sheet::{PaperSheetInfo, PaperViewport, compose_view_sheet};
    let mut app = DesktopApp::new().unwrap();
    let source = app
        .editor
        .create_floor_plan("Source", app.active_level)
        .unwrap();
    let mut p = app.editor.document.model().views[&source]
        .parameters
        .clone();
    let plan = p.plan.as_mut().unwrap();
    plan.basis.rotation = 0.6;
    plan.crop = Some(PlanViewCrop {
        min: Point2::new(-3., -3.),
        max: Point2::new(3., 3.),
    });
    plan.annotation_crop = Some(PlanViewCrop {
        min: Point2::new(-4., -4.),
        max: Point2::new(4., 4.),
    });
    plan.scale_denominator = 25.;
    let mut note = TextNote::new("core.text_note", TextNoteParams::new(source));
    note.parameters.text = "AB".into();
    note.parameters.position = Point2::new(-1., 1.);
    let line = DetailLine::new(
        "core.detail_line",
        DetailLineParams {
            view: source,
            start: Point2::new(-1., -1.),
            end: Point2::new(1., 1.),
        },
    );
    app.editor
        .commands(
            "Fixture",
            vec![
                Command::UpdateView {
                    id: source,
                    parameters: p,
                },
                Command::AddTextNote(note),
                Command::AddDetailLine(line),
            ],
        )
        .unwrap();
    let draft = app
        .editor
        .document
        .prepare_plan_duplicate(source, "Copy".into(), PlanDuplicateMode::WithDetailing)
        .unwrap();
    let copy = app.editor.document.apply_plan_duplicate(draft).unwrap();
    let mut pages = Vec::new();
    let mut notes = Vec::new();
    for id in [source, copy] {
        let context = app.editor.native_plan_context(id).unwrap();
        let drawing = app.editor.native_drawing(id).unwrap();
        let graphics = drawing.text_notes(context).unwrap();
        assert_eq!(graphics.len(), 1);
        notes.push(
            graphics[0]
                .layout(context.scale_denominator)
                .unwrap()
                .glyphs,
        );
        let page = compose_view_sheet(
            PaperSheetInfo {
                width_mm: 420.,
                height_mm: 297.,
                number: "A1",
                name: "Comparison",
            },
            "Comparison",
            PaperViewport {
                center_mm: Point2::new(210., 148.5),
                width_mm: 300.,
                height_mm: 250.,
                model_center_m: Point2::default(),
                scale_denominator: 25.,
            },
            context,
            &drawing,
        )
        .unwrap();
        let pdf = page.to_pdf().unwrap();
        assert!(pdf.starts_with(b"%PDF"));
        pages.push(pdf);
    }
    assert_eq!(notes[0], notes[1]);
    assert_eq!(pages[0], pages[1]);
}
