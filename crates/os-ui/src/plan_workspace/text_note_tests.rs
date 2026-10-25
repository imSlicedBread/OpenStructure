use super::super::sheet_tests::raw_input;
use super::*;
struct Harness {
    app: DesktopApp,
    ctx: egui::Context,
    size: egui::Vec2,
    dpi: f32,
    time: f64,
    view: Id,
}
impl Harness {
    fn new(section: u8, dpi: f32) -> Self {
        let mut app = DesktopApp::new().unwrap();
        let view = if section == 2 {
            app.editor.create_drafting_view("Note drafting").unwrap()
        } else if section == 1 {
            let mut s = os_model::SectionViewSettings::new(
                Point2::new(18., 20.),
                Point2::new(26., 26.),
                -5.,
                5.,
            );
            s.min_station = -5.;
            s.max_station = 5.;
            if dpi > 1. {
                std::mem::swap(&mut s.start, &mut s.end);
            }
            app.editor
                .create_section_view("Note section", app.active_level, s)
                .unwrap()
        } else {
            app.editor
                .create_floor_plan("Note plan", app.active_level)
                .unwrap()
        };
        let mut parameters = app.editor.document.model().views[&view].parameters.clone();
        if let Some(plan) = &mut parameters.plan {
            plan.basis.rotation = -0.4;
            plan.scale_denominator = if dpi > 1. { 25. } else { 100. };
        }
        if let Some(section) = &mut parameters.section {
            section.scale_denominator = if dpi > 1. { 25. } else { 100. };
        }
        if let Some(drafting) = &mut parameters.drafting {
            drafting.scale_denominator = if dpi > 1. { 25. } else { 100. };
        }
        app.editor
            .command(
                "Fixture scale",
                Command::UpdateView {
                    id: view,
                    parameters,
                },
            )
            .unwrap();
        app.plans.poll(&app.editor);
        app.focus_plan(Some(view));
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let mut h = Self {
            app,
            ctx,
            size: if dpi > 1. {
                egui::vec2(1000., 650.)
            } else {
                egui::vec2(1280., 800.)
            },
            dpi,
            time: 0.,
            view,
        };
        h.settle();
        h
    }
    fn frame(&mut self, events: Vec<egui::Event>) -> egui::FullOutput {
        self.time += 0.1;
        self.ctx
            .run(raw_input(self.size, self.dpi, self.time, events), |ctx| {
                self.app.show(ctx)
            })
    }
    fn settle(&mut self) {
        let end = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            self.frame(vec![]);
            if self.app.plans.ready() {
                break;
            }
            assert!(std::time::Instant::now() < end);
            std::thread::yield_now();
        }
        self.frame(vec![]);
    }
    fn point(&self, p: Point2) -> egui::Pos2 {
        let r = self.app.plans.canvas_rect.unwrap();
        let c = self.app.editor.native_view_context(self.view).unwrap();
        let p = self.app.plans.cameras[&self.view]
            .project(
                c.basis.world_to_plane(p).unwrap(),
                [f64::from(r.width()), f64::from(r.height())],
            )
            .unwrap();
        r.min + egui::vec2(p.x as f32, p.y as f32)
    }
    fn click(&mut self, p: egui::Pos2) {
        self.frame(vec![egui::Event::PointerMoved(p)]);
        for pressed in [true, false] {
            self.frame(vec![
                egui::Event::PointerMoved(p),
                egui::Event::PointerButton {
                    pos: p,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ]);
        }
    }
    fn label(&mut self, label: &str) {
        self.frame(vec![]);
        let out = self.frame(vec![]);
        let p = out
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.job.text == label => {
                    Some(egui::Rect::from_min_size(t.pos, t.galley.size()).center())
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing {label}"));
        self.click(p);
    }
    fn escape(&mut self) {
        self.frame(vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }]);
    }
}
#[test]
fn text_notes_real_egui_plan_section_both_dpis_preview_edit_move_delete_and_history() {
    for section in [0, 1, 2] {
        for dpi in [1., 1.5] {
            let mut h = Harness::new(section, dpi);
            let before = h.app.editor.document.model().clone();
            let history = h.app.editor.document.history_stats();
            h.app.begin_text_note(None);
            h.frame(vec![]);
            let p = &mut h.app.plans.text_notes.draft.as_mut().unwrap().parameters;
            p.text = "Alpha\nBeta".into();
            p.width_mm = 20.;
            p.text_height_mm = 2.;
            h.label("Place / move on drawing");
            let at = h.point(Point2::new(-1., 1.));
            h.frame(vec![egui::Event::PointerMoved(at)]);
            assert_eq!(h.app.editor.document.model(), &before);
            assert_eq!(h.app.editor.document.history_stats(), history);
            h.click(at);
            h.settle();
            assert_eq!(
                h.app.editor.document.model().text_notes.len(),
                1,
                "{}",
                h.app.status
            );
            let note = h
                .app
                .editor
                .document
                .model()
                .text_notes
                .values()
                .next()
                .unwrap()
                .clone();
            assert!(note.parameters.position.distance(Point2::new(-1., 1.)) < 1e-5);
            let context = h.app.editor.native_view_context(h.view).unwrap();
            let drawing = h.app.editor.native_drawing(h.view).unwrap();
            let anchor = drawing.text_notes(context).unwrap()[0]
                .layout(context.scale_denominator)
                .unwrap()
                .glyphs[0]
                .0;
            let expected = h.point(context.basis.plane_to_world(anchor).unwrap());
            let output = h.frame(vec![]);
            assert!(output.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t) if t.galley.job.text=="A" && egui::Rect::from_min_size(t.pos,t.galley.size()).left_bottom().distance(expected)<1.)));
            assert_eq!(
                h.app.editor.document.history_stats().undo_entries,
                history.undo_entries + 1
            );
            let committed = h.app.editor.document.model().clone();
            h.app.history(false);
            h.settle();
            assert_eq!(h.app.editor.document.model(), &before);
            h.app.history(true);
            h.settle();
            assert_eq!(h.app.editor.document.model(), &committed);
            h.app.begin_text_note(Some(note.id()));
            h.app
                .plans
                .text_notes
                .draft
                .as_mut()
                .unwrap()
                .parameters
                .text = "Canceled".into();
            h.label("Cancel note");
            assert_eq!(h.app.editor.document.model(), &committed);
            h.app.begin_text_note(Some(note.id()));
            h.app
                .plans
                .text_notes
                .draft
                .as_mut()
                .unwrap()
                .parameters
                .text = "Edited".into();
            h.label("Apply note");
            h.settle();
            assert_eq!(
                h.app.editor.document.model().text_notes[&note.id()]
                    .parameters
                    .text,
                "Edited"
            );
            h.app.begin_text_note(Some(note.id()));
            h.label("Place / move on drawing");
            let at = h.point(Point2::new(0.5, 0.5));
            assert!(h.app.plans.canvas_rect.unwrap().contains(at));
            h.click(at);
            h.settle();
            assert!(
                h.app.editor.document.model().text_notes[&note.id()]
                    .parameters
                    .position
                    .distance(Point2::new(0.5, 0.5))
                    < 1e-5,
                "section={section} dpi={dpi} actual={:?} status={} draft={}",
                h.app.editor.document.model().text_notes[&note.id()]
                    .parameters
                    .position,
                h.app.status,
                h.app.plans.text_notes.draft.is_some()
            );
            // Canvas picking opens the same edit draft without changing the model.
            let stable = h.app.editor.document.model().clone();
            let c = h.app.editor.native_view_context(h.view).unwrap();
            let anchor = c
                .basis
                .world_to_plane(stable.text_notes[&note.id()].parameters.position)
                .unwrap();
            let hit = c
                .basis
                .plane_to_world(Point2::new(anchor.x + 0.01, anchor.y - 0.01))
                .unwrap();
            h.click(h.point(hit));
            h.frame(vec![]);
            assert_eq!(h.app.selected, Some(note.id()));
            assert!(h.app.plans.text_notes.draft.is_some());
            assert_eq!(h.app.editor.document.model(), &stable);
            h.label("Delete note");
            h.settle();
            assert!(h.app.editor.document.model().text_notes.is_empty());
            h.app.history(false);
            h.settle();
            assert_eq!(h.app.editor.document.model(), &stable);
        }
    }
}
#[test]
fn text_note_escape_stale_revision_and_text_focus_do_not_mutate_workspace() {
    let mut h = Harness::new(2, 1.5);
    h.app.begin_text_note(None);
    h.label("Place / move on drawing");
    h.frame(vec![egui::Event::PointerMoved(
        h.point(Point2::new(1., 1.)),
    )]);
    let before = h.app.editor.document.model().clone();
    h.escape();
    assert!(h.app.plans.text_notes.draft.is_none());
    assert_eq!(h.app.editor.document.model(), &before);
    h.app.begin_text_note(None);
    h.label("Place / move on drawing");
    h.app
        .editor
        .document
        .execute("External", vec![Command::RenameProject("External".into())])
        .unwrap();
    let before = h.app.editor.document.model().clone();
    h.frame(vec![]);
    assert!(h.app.plans.text_notes.draft.is_none());
    assert_eq!(h.app.editor.document.model(), &before);
    h.app.begin_text_note(None);
    h.label("Note");
    assert!(h.ctx.wants_keyboard_input());
    let before = h.app.editor.document.model().clone();
    h.frame(vec![
        egui::Event::Text("d".into()),
        egui::Event::Key {
            key: egui::Key::Delete,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        },
    ]);
    assert_eq!(h.app.editor.document.model(), &before);
    assert!(h.app.plans.detail_line_draft.is_none());
    assert!(h.app.plans.text_notes.draft.is_some());
}

#[test]
fn text_note_camera_and_canvas_changes_cancel_only_after_claiming_a_drag() {
    for section in [0, 1, 2] {
        for dpi in [1., 1.5] {
            for resize in [false, true] {
                let mut h = Harness::new(section, dpi);
                h.app.begin_text_note(None);
                h.label("Place / move on drawing");
                // Closing the editor and changing the camera before pressing is allowed.
                h.app.plans.cameras.get_mut(&h.view).unwrap().center.x += 0.1;
                h.frame(vec![]);
                assert!(
                    h.app
                        .plans
                        .text_notes
                        .draft
                        .as_ref()
                        .unwrap()
                        .camera
                        .is_none()
                );
                let before = h.app.editor.document.model().clone();
                let history = h.app.editor.document.history_stats();
                let at = h.point(Point2::new(0.5, 0.5));
                h.frame(vec![egui::Event::PointerMoved(at)]);
                h.frame(vec![egui::Event::PointerButton {
                    pos: at,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                }]);
                assert!(h.app.plans.text_notes.claimed);
                assert!(
                    h.app
                        .plans
                        .text_notes
                        .draft
                        .as_ref()
                        .unwrap()
                        .camera
                        .is_some()
                );
                if resize {
                    h.size += egui::vec2(120., 80.);
                } else {
                    h.app.plans.cameras.get_mut(&h.view).unwrap().center.x += 0.25;
                }
                h.frame(vec![]);
                assert!(h.app.plans.text_notes.draft.is_none());
                h.frame(vec![egui::Event::PointerButton {
                    pos: at,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                }]);
                assert_eq!(h.app.editor.document.model(), &before);
                assert_eq!(h.app.editor.document.history_stats(), history);
            }
        }
    }
}
