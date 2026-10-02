//! Opening-tag acceptance through the existing desktop egui frame harness.

#[test]
fn shared_lengths_parameter_only_changes_reach_canvas_tags_sheet_and_scene() {
    use os_model::*;
    for (size, scale) in PROFILES {
        let (mut h, view, opening) = setup(size, scale);
        let ty = OpeningType::new(
            "core.opening_type",
            OpeningTypeParams {
                name: "Shared window".into(),
                kind: OpeningKind::Window,
                width: 0.9,
                height: 1.2,
                sill: 0.8,
                family: Default::default(),
                window_operation: Default::default(),
                pane_position: Default::default(),
            },
        );
        let type_id = ty.id();
        let p = LengthParameter::new(
            "core.length_parameter",
            LengthParameterParams {
                name: "Width".into(),
                unit: LengthUnit::Metres,
                value: 0.9,
            },
        );
        let pid = p.id();
        let mut params = h.app.editor.document.model().openings[&opening]
            .parameters
            .clone();
        params.definition = OpeningDefinition::Typed { type_id };
        h.app
            .editor
            .document
            .execute(
                "Shared type",
                vec![
                    Command::AddOpeningType(ty),
                    Command::AddLengthParameter(p),
                    Command::SetOpeningTypeLengthBindings {
                        id: type_id,
                        bindings: OpeningTypeLengthBindings {
                            width: Some(pid),
                            ..Default::default()
                        },
                    },
                    Command::UpdateOpening {
                        id: opening,
                        parameters: params,
                    },
                ],
            )
            .unwrap();
        h.app.editor.regenerate().unwrap();
        h.settle_plan();
        place(&mut h, view, opening);
        h.app.create_sheet_from_active_view();
        h.settle_plan();
        let before = h.app.editor.scene[&opening].clone();
        let before_label = tag_graphic(&h, view).label;
        h.app
            .editor
            .command(
                "Shared width",
                Command::UpdateLengthParameter {
                    id: pid,
                    parameters: LengthParameterParams {
                        name: "Width".into(),
                        unit: LengthUnit::Metres,
                        value: 1.1,
                    },
                },
            )
            .unwrap();
        h.settle_plan();
        let label = tag_graphic(&h, view).label;
        assert_ne!(label, before_label);
        assert!(label.contains("1.100 × 1.200"));
        assert_ne!(h.app.editor.scene[&opening], before);
        let page = h.app.sheet_page().unwrap();
        assert!(page.marks().iter().any(|m| matches!(&m.kind, os_render::sheet::PaperMarkKind::Text { text, .. } if text == &label)));
    }
}

#[test]
fn opening_tag_only_orphan_plan_has_no_empty_geometry_warning() {
    for (size, scale) in PROFILES {
        let (mut h, view, opening) = setup(size, scale);
        let tag = place(&mut h, view, opening);
        let mut commands = vec![Command::RemoveOpening(opening)];
        commands.extend(
            h.app
                .editor
                .document
                .model()
                .walls
                .keys()
                .copied()
                .map(Command::RemoveWall),
        );
        h.app
            .editor
            .document
            .execute("Remove geometry", commands)
            .unwrap();
        h.settle_plan();
        assert!(h.app.editor.document.model().walls.is_empty());
        assert_eq!(canvas_text(&h, "Opening tag · Missing opening"), 1);
        assert_eq!(
            canvas_text(&h, "No visible plan geometry with these settings"),
            0
        );
        h.app.select(None);
        h.frame(vec![]);
        h.click_at(h.plan_point(view, Point2::new(2., 1.5)));
        assert_eq!(h.app.selected, Some(tag));
    }
}

#[test]
fn opening_tag_typed_window_live_rehost_orphan_and_sheet_vector_output() {
    use os_model::{
        OpeningDefinition, OpeningFamily, OpeningKind, OpeningType, OpeningTypeParams,
        WindowPanePosition,
    };
    use os_render::sheet::PaperMarkKind;
    for (size, scale) in PROFILES {
        let (mut h, view, opening) = setup(size, scale);
        let ty = OpeningType::new(
            "core.opening_type",
            OpeningTypeParams {
                window_operation: Default::default(),
                name: "W900".into(),
                family: OpeningFamily::default(),
                kind: OpeningKind::Window,
                width: 0.9,
                height: 1.2,
                sill: 0.8,
                pane_position: WindowPanePosition::Center,
            },
        );
        let type_id = ty.id();
        let mut parameters = h.app.editor.document.model().openings[&opening]
            .parameters
            .clone();
        parameters.name = "W-01".into();
        parameters.definition = OpeningDefinition::Typed { type_id };
        h.app
            .editor
            .document
            .execute(
                "Window",
                vec![
                    Command::AddOpeningType(ty),
                    Command::UpdateOpening {
                        id: opening,
                        parameters,
                    },
                ],
            )
            .unwrap();
        h.settle_plan();
        let tag = place(&mut h, view, opening);
        let mut saved = h.app.editor.document.model().opening_tags[&tag].clone();
        assert_eq!(
            tag_graphic(&h, view).leader_source,
            Some(Point2::new(1.45, 0.))
        );
        let mut parameters = h.app.editor.document.model().opening_types[&type_id]
            .parameters
            .clone();
        parameters.name = "W1000".into();
        parameters.width = 1.;
        h.app
            .editor
            .command(
                "Type",
                Command::UpdateOpeningType {
                    id: type_id,
                    parameters,
                },
            )
            .unwrap();
        h.settle_plan();
        let label = "W-01 · W1000 · 1.000 × 1.200 m";
        assert_eq!(canvas_text(&h, label), 1);
        assert_eq!(
            tag_graphic(&h, view).leader_source,
            Some(Point2::new(1.5, 0.))
        );
        assert_tag_leader(&h, view, &tag_graphic(&h, view));
        let target = h
            .app
            .editor
            .document
            .model()
            .walls
            .values()
            .find(|w| w.parameters.start() == Point2::new(4., 3.))
            .unwrap()
            .id();
        let mut parameters = h.app.editor.document.model().openings[&opening]
            .parameters
            .clone();
        parameters.host = target;
        h.app
            .editor
            .command(
                "Rehost",
                Command::UpdateOpening {
                    id: opening,
                    parameters,
                },
            )
            .unwrap();
        h.settle_plan();
        assert_eq!(canvas_text(&h, label), 1);
        assert_eq!(h.app.editor.document.model().opening_tags[&tag], saved);
        assert_eq!(
            tag_graphic(&h, view).leader_source,
            Some(Point2::new(2.5, 3.))
        );
        assert_tag_leader(&h, view, &tag_graphic(&h, view));
        h.app.create_sheet_from_active_view();
        h.settle_plan();
        let page = h.app.sheet_page().unwrap();
        assert!(page.marks().iter().any(|m| m.clip.is_some()
            && matches!(&m.kind, PaperMarkKind::Text { text, .. } if text == label)));
        let index = page
            .marks()
            .iter()
            .position(|m| {
                matches!(&m.kind,
            PaperMarkKind::Text { text, .. } if text == label)
            })
            .unwrap();
        let leader = &page.marks()[index - 1];
        let PaperMarkKind::Path {
            points_mm,
            closed: false,
            stroke: Some(_),
            fill: None,
        } = &leader.kind
        else {
            panic!("tag vector path must precede text")
        };
        assert_eq!(points_mm.len(), 2);
        assert!(!page.marks()[index].clip.unwrap().contains(points_mm[1]));
        let vector_path = format!(
            "{:.6} {:.6} m\n{:.6} {:.6} l",
            points_mm[0].x * 72. / 25.4,
            (page.height_mm - points_mm[0].y) * 72. / 25.4,
            points_mm[1].x * 72. / 25.4,
            (page.height_mm - points_mm[1].y) * 72. / 25.4
        );
        let pdf = String::from_utf8(page.to_pdf().unwrap()).unwrap();
        assert!(pdf.find(&vector_path).unwrap() < pdf.find("572D3031").unwrap());
        assert!(
            pdf.contains("572D3031"),
            "opening tag text must be emitted into vector PDF"
        );
        let mut parameters = h.app.editor.document.model().opening_tags[&tag]
            .parameters
            .clone();
        parameters.label_preset = os_model::OpeningTagLabelPreset::DimensionsOnly;
        h.app
            .editor
            .command(
                "Short tag label",
                Command::UpdateOpeningTag {
                    id: tag,
                    parameters,
                },
            )
            .unwrap();
        h.settle_plan();
        let short_label = "1.000 × 1.200 m";
        assert_eq!(tag_graphic(&h, view).label, short_label);
        let short_page = h.app.sheet_page().unwrap();
        assert!(short_page.marks().iter().any(|mark| matches!(
            &mark.kind,
            PaperMarkKind::Text { text, .. } if text == short_label
        )));
        let short_pdf = String::from_utf8(short_page.to_pdf().unwrap()).unwrap();
        assert!(short_pdf.contains("<312E30303020D720312E323030206D> Tj"));
        saved = h.app.editor.document.model().opening_tags[&tag].clone();
        h.app.history(false);
        h.settle_plan();
        assert_eq!(tag_graphic(&h, view).label, label);
        h.app.history(true);
        h.settle_plan();
        assert_eq!(tag_graphic(&h, view).label, short_label);
        h.app
            .editor
            .command("Delete", Command::RemoveOpening(opening))
            .unwrap();
        h.settle_plan();
        let page = h.app.sheet_page().unwrap();
        assert!(tag_graphic(&h, view).leader_source.is_none());
        assert!(page.marks().iter().any(|m|
            matches!(&m.kind, PaperMarkKind::Text { text, .. } if text == "Opening tag · Missing opening")));
        assert_eq!(h.app.editor.document.model().opening_tags[&tag], saved);
        h.app.history(false);
        h.settle_plan();
        assert!(
            h.app.sheet_page().unwrap().marks().iter().any(
                |m| matches!(&m.kind, PaperMarkKind::Text { text, .. } if text == short_label)
            )
        );
    }
}
use super::*;

const PROFILES: [(egui::Vec2, f32); 2] = [
    (egui::vec2(1280., 800.), 1.),
    (egui::vec2(1000., 650.), 1.5),
];

fn button(pos: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        pressed,
        button: egui::PointerButton::Primary,
        modifiers: egui::Modifiers::NONE,
    }
}
fn escape() -> egui::Event {
    egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}
fn setup(size: egui::Vec2, scale: f32) -> (Harness, Id, Id) {
    let mut h = Harness::at_size(size, scale);
    let level = h.app.active_level;
    for (start, end) in [
        (Point2::new(0., 0.), Point2::new(4., 0.)),
        (Point2::new(4., 0.), Point2::new(4., 3.)),
        (Point2::new(4., 3.), Point2::new(0., 3.)),
        (Point2::new(0., 3.), Point2::new(0., 0.)),
    ] {
        h.app
            .editor
            .command(
                "Boundary",
                Command::AddWall(os_model::Wall::new(
                    "org.openstructure.walls.wall",
                    WallParams {
                        name: "Boundary".into(),
                        path: os_model::WallPath::Straight { start, end },
                        height: 3.,
                        thickness: 0.2,
                        level,
                        material: None,
                    },
                )),
            )
            .unwrap();
    }
    let view = h.app.editor.create_floor_plan("Tag plan", level).unwrap();
    h.app.focus_plan(Some(view));
    h.settle_plan();
    let host = h
        .app
        .editor
        .document
        .model()
        .walls
        .values()
        .find(|w| {
            w.parameters.start() == Point2::new(0., 0.) && w.parameters.end() == Point2::new(4., 0.)
        })
        .unwrap()
        .id();
    let opening = os_model::Opening::new(
        "core.opening",
        os_model::OpeningParams {
            open_state: Default::default(),
            name: "D-01".into(),
            host,
            offset: 1.,
            definition: os_model::OpeningDefinition::Legacy {
                kind: os_model::OpeningKind::Door,
                width: 0.9,
                height: 2.1,
                sill: 0.,
            },
            width_override: None,
            height_override: None,
            sill_override: None,
            pane_position_override: None,
            lite_side_override: None,
            hinge: Default::default(),
            swing: Default::default(),
        },
    );
    let opening = {
        let id = opening.id();
        h.app
            .editor
            .command("Opening", Command::AddOpening(opening))
            .unwrap();
        id
    };
    h.frame(vec![escape()]);
    h.settle_plan();
    h.frame(vec![]);
    assert_eq!(h.ctx.pixels_per_point(), scale);
    (h, view, opening)
}
fn place(h: &mut Harness, view: Id, opening: Id) -> Id {
    h.app.select(Some(opening));
    h.frame(vec![]);
    h.click("Opening Tag");
    h.click_at(h.plan_point(view, Point2::new(2., 1.5)));
    h.settle_plan();
    h.frame(vec![]);
    assert_eq!(
        h.app.editor.document.model().opening_tags.len(),
        1,
        "{}",
        h.app.status
    );
    *h.app
        .editor
        .document
        .model()
        .opening_tags
        .keys()
        .next()
        .unwrap()
}
fn start_drag(h: &mut Harness, view: Id, tag: Id) -> egui::Pos2 {
    let position = h.app.editor.document.model().opening_tags[&tag]
        .parameters
        .position;
    let from = h.plan_point(view, position);
    h.frame(vec![egui::Event::PointerMoved(from)]);
    h.frame(vec![button(from, true)]);
    from
}
fn canvas_text(h: &Harness, text: &str) -> usize {
    let canvas = h.app.plans.canvas_rect.unwrap();
    h.output
        .shapes
        .iter()
        .filter(|shape| match &shape.shape {
            egui::Shape::Text(t) => t.galley.job.text == text && canvas.contains(t.pos),
            _ => false,
        })
        .count()
}

fn tag_graphic(h: &Harness, view: Id) -> os_render::plan::PlanOpeningTag {
    let context = h.app.editor.native_plan_context(view).unwrap();
    h.app
        .plans
        .drawing
        .as_ref()
        .unwrap()
        .opening_tags(context)
        .unwrap()[0]
        .clone()
}

fn assert_tag_leader(h: &Harness, view: Id, graphic: &os_render::plan::PlanOpeningTag) {
    assert_tag_leader_count(h, view, graphic, 1);
}

fn assert_tag_leader_count(
    h: &Harness,
    view: Id,
    graphic: &os_render::plan::PlanOpeningTag,
    count: usize,
) {
    let context = h.app.editor.native_plan_context(view).unwrap();
    let rect = h.app.plans.canvas_rect.unwrap();
    let size = [f64::from(rect.width()), f64::from(rect.height())];
    let origin = h
        .app
        .plans
        .test_screen_point(view, Point2::default())
        .unwrap()
        - rect.min;
    let unit = h
        .app
        .plans
        .test_screen_point(view, Point2::new(1., 0.))
        .unwrap()
        - rect.min;
    let ppm = f64::from(unit.x - origin.x);
    let camera = os_render::plan::PlanCamera {
        center: Point2::new(
            (size[0] * 0.5 - f64::from(origin.x)) / ppm,
            (f64::from(origin.y) - size[1] * 0.5) / ppm,
        ),
        pixels_per_metre: ppm,
    };
    let (a, b) = graphic
        .leader(context, camera, size)
        .unwrap()
        .expect("visible leader");
    let expected = [
        rect.min + egui::vec2(a.x as f32, a.y as f32),
        rect.min + egui::vec2(b.x as f32, b.y as f32),
    ];
    assert_eq!(
        h.output
            .shapes
            .iter()
            .filter(|shape| matches!(&shape.shape,
        egui::Shape::LineSegment { points, .. } if points[0].distance(expected[0]) < 0.05
            && points[1].distance(expected[1]) < 0.05))
            .count(),
        count,
        "derived leader paint count"
    );
    assert!(expected.iter().all(|p| rect.contains(*p)));
    let midpoint = Point2::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5);
    assert!(
        !graphic.hit(context, camera, size, midpoint).unwrap(),
        "leader must not pick the tag"
    );
    assert_eq!(
        h.app
            .plans
            .drawing
            .as_ref()
            .unwrap()
            .pick_opening_tag_screen(context, camera, size, midpoint)
            .unwrap(),
        None
    );
}

fn reveal_browser(h: &mut Harness, label: &str) -> egui::Rect {
    for _ in 0..20 {
        if let Some(rect) = h.visible_text_rect(label) {
            return rect;
        }
        let pointer = h.text_rect("Project Browser").center() + egui::vec2(0., 75.);
        h.frame(vec![
            egui::Event::PointerMoved(pointer),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0., -90.),
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        for _ in 0..8 {
            h.frame(vec![]);
        }
    }
    panic!("Browser entry is not reachable: {label}");
}

#[test]
fn opening_tag_real_frames_place_move_live_label_select_and_pan_at_both_dpis() {
    for (size, scale) in PROFILES {
        let (mut h, view, opening) = setup(size, scale);
        h.app.select(Some(opening));
        h.frame(vec![]);
        let before = h.app.editor.document.model().clone();
        let history = h.app.editor.document.history_stats();
        h.click("Opening Tag");
        h.frame(vec![egui::Event::PointerMoved(
            h.plan_point(view, Point2::new(2., 1.5)),
        )]);
        assert_eq!(h.app.editor.document.model(), &before);
        assert_eq!(h.app.editor.document.history_stats(), history);
        let label = "D-01 · Legacy · 0.900 × 2.100 m".to_owned();
        assert!(canvas_text(&h, &label) > 0, "placement preview missing");
        let preview = crate::plan::opening_tag_graphic(
            h.app.editor.document.model(),
            opening,
            &os_model::OpeningTagParams {
                label_preset: Default::default(),
                view,
                opening,
                position: Point2::new(2., 1.5),
            },
            h.app.editor.native_plan_context(view).unwrap(),
        )
        .unwrap();
        assert_tag_leader(&h, view, &preview);
        h.click_at(h.plan_point(view, Point2::new(2., 1.5)));
        h.settle_plan();
        h.frame(vec![]);
        let tag = *h
            .app
            .editor
            .document
            .model()
            .opening_tags
            .keys()
            .next()
            .unwrap();
        let placed = h.app.editor.document.model().clone();
        assert_tag_leader(&h, view, &tag_graphic(&h, view));
        assert_eq!(placed.opening_tags[&tag].parameters.opening, opening);
        assert_eq!(placed.opening_tags[&tag].parameters.view, view);
        assert_eq!(
            h.app.editor.document.history_stats().undo_entries,
            history.undo_entries + 1
        );
        assert_eq!(
            canvas_text(&h, &label),
            1,
            "explicit tag suppresses automatic name/number"
        );
        h.app.history(false);
        assert_eq!(h.app.editor.document.model(), &before);
        h.app.history(true);
        assert_eq!(h.app.editor.document.model(), &placed);
        h.settle_plan();
        h.frame(vec![]);
        h.app.select(None);
        h.frame(vec![]);
        h.click_at(h.plan_point(view, placed.opening_tags[&tag].parameters.position));
        assert_eq!(h.app.selected, Some(tag));
        assert_eq!(h.app.editor.document.model(), &placed);
        h.frame(vec![]);
        let camera_probe = h.plan_point(view, Point2::new(0., 0.));
        let from = start_drag(&mut h, view, tag);
        let to = from + egui::vec2(25., -18.);
        h.frame(vec![egui::Event::PointerMoved(to)]);
        let mut preview = tag_graphic(&h, view);
        let ppm = (h.plan_point(view, Point2::new(1., 0.)) - camera_probe).x;
        preview.anchor.x += f64::from((to.x - from.x) / ppm);
        preview.anchor.y -= f64::from((to.y - from.y) / ppm);
        assert_tag_leader(&h, view, &preview);
        assert_eq!(h.app.editor.document.model(), &placed);
        assert_eq!(
            h.app.editor.document.history_stats().undo_entries,
            history.undo_entries + 1
        );
        assert_eq!(
            h.plan_point(view, Point2::new(0., 0.)),
            camera_probe,
            "tag preview must not pan"
        );
        h.frame(vec![button(to, false)]);
        h.settle_plan();
        h.frame(vec![]);
        let moved = h.app.editor.document.model().clone();
        assert_tag_leader(&h, view, &tag_graphic(&h, view));
        assert_ne!(
            moved.opening_tags[&tag].parameters.position,
            placed.opening_tags[&tag].parameters.position
        );
        assert_eq!(
            moved.opening_tags[&tag].header,
            placed.opening_tags[&tag].header
        );
        assert_eq!(
            h.app.editor.document.history_stats().undo_entries,
            history.undo_entries + 2
        );
        assert_eq!(h.plan_point(view, Point2::new(0., 0.)), camera_probe);
        h.app.history(false);
        assert_eq!(h.app.editor.document.model(), &placed);
        h.app.history(true);
        assert_eq!(h.app.editor.document.model(), &moved);
        h.settle_plan();
        h.frame(vec![]);
        h.app.select(Some(opening));
        h.frame(vec![]);
        let mut parameters = h.app.editor.document.model().openings[&opening]
            .parameters
            .clone();
        parameters.name = "D-02".into();
        h.app
            .editor
            .command(
                "Rename",
                Command::UpdateOpening {
                    id: opening,
                    parameters,
                },
            )
            .unwrap();
        h.settle_plan();
        h.frame(vec![]);
        let ctx = h.app.editor.native_plan_context(view).unwrap();
        let tags = h
            .app
            .plans
            .drawing
            .as_ref()
            .unwrap()
            .opening_tags(ctx)
            .unwrap();
        assert_eq!(tags[0].label, "D-02 · Legacy · 0.900 × 2.100 m");
        assert_eq!(tags[0].entity, tag);
        assert_eq!(canvas_text(&h, "D-02 · Legacy · 0.900 × 2.100 m"), 1);
        assert_eq!(
            h.app.editor.document.model().opening_tags,
            moved.opening_tags
        );
        h.app.select(None);
        h.frame(vec![]);
        let probe = h.plan_point(view, Point2::new(0., 0.));
        let ordinary = h.plan_point(view, Point2::new(0.5, 0.5));
        let before_pan = h.app.editor.document.model().clone();
        h.drag(ordinary, ordinary + egui::vec2(35., 20.));
        assert_ne!(h.plan_point(view, Point2::new(0., 0.)), probe);
        assert_eq!(h.app.editor.document.model(), &before_pan);
    }
}

#[test]
fn opening_tag_real_frames_invalid_release_escape_and_stale_cancel_through_release() {
    for (size, scale) in PROFILES {
        for case in [
            "outside", "escape", "revision", "session", "drawing", "view", "crop", "provider",
        ] {
            if case == "provider" && !cfg!(feature = "external-plugins") {
                continue;
            }
            let (mut h, view, opening) = setup(size, scale);
            let tag = place(&mut h, view, opening);
            let before = h.app.editor.document.model().opening_tags.clone();
            let from = start_drag(&mut h, view, tag);
            let mut to = from + egui::vec2(30., 20.);
            h.frame(vec![egui::Event::PointerMoved(to)]);
            match case {
                "outside" => {
                    to = h.app.plans.canvas_rect.unwrap().right_bottom() + egui::vec2(10., 10.);
                }
                "escape" => h.frame(vec![escape()]),
                "revision" => h
                    .app
                    .editor
                    .command("External edit", Command::RenameProject("Changed".into()))
                    .unwrap(),
                "session" => {
                    h.app.editor.document =
                        Document::from_model(h.app.editor.document.model().clone()).unwrap()
                }
                "drawing" => {
                    h.app.plans.drawing = Some(h.app.editor.native_wall_plan(view).unwrap())
                }
                "view" => h.app.focus_plan(None),
                "provider" => {
                    h.app.editor.host.unload(os_walls::PLUGIN_ID).unwrap();
                }
                "crop" => {
                    let v = h.app.editor.document.model().views[&view]
                        .parameters
                        .clone();
                    let mut settings = v.plan.unwrap();
                    settings.crop = Some(os_model::PlanViewCrop {
                        min: Point2::new(0., 0.),
                        max: Point2::new(4., 3.),
                    });
                    h.app
                        .editor
                        .update_floor_plan(view, "Tag plan", v.level.unwrap(), settings)
                        .unwrap();
                }
                _ => unreachable!(),
            }
            let history = h.app.editor.document.history_stats();
            let model = h.app.editor.document.model().clone();
            h.frame(vec![egui::Event::PointerMoved(to)]);
            let probe =
                (h.app.plans.active == Some(view)).then(|| h.plan_point(view, Point2::new(0., 0.)));
            h.frame(vec![egui::Event::PointerMoved(to + egui::vec2(5., 0.))]);
            h.frame(vec![button(to + egui::vec2(5., 0.), false)]);
            assert_eq!(h.app.editor.document.model(), &model, "{case} at {scale}");
            assert_eq!(h.app.editor.document.model().opening_tags, before);
            assert_eq!(h.app.editor.document.history_stats(), history, "{case}");
            if let Some(probe) = probe {
                assert_eq!(
                    h.plan_point(view, Point2::new(0., 0.)),
                    probe,
                    "cancelled press became pan: {case}"
                );
            }
        }
    }
}

#[test]
fn opening_tag_browser_focus_properties_position_delete_and_orphan_access() {
    for (size, scale) in PROFILES {
        let (mut h, view, opening) = setup(size, scale);
        let tag = place(&mut h, view, opening);
        h.app.properties_fraction = 0.45;
        h.app.select(None);
        h.app.focus_plan(None);
        h.frame(vec![]);
        h.frame(vec![]);
        let label = "D-01 · Legacy · 0.900 × 2.100 m".to_owned();
        // Collapse preceding groups to expose the tag in the existing browser scroll area.
        for label in ["Walls (4)", "Rooms (1)"] {
            if let Some(rect) = h.visible_text_rect(label) {
                h.click_at(rect.center());
            }
        }
        h.frame(vec![]);
        let rect = reveal_browser(&mut h, &label);
        h.click_at(rect.center());
        assert_eq!(h.app.selected, Some(tag));
        assert_eq!(h.app.plans.active, Some(view));
        h.settle_plan();
        h.frame(vec![]);
        assert!(
            h.visible_text_rect("Opening tag · live instance, type and dimensions")
                .is_some()
        );
        let original = h.app.editor.document.model().clone();
        let history = h.app.editor.document.history_stats();
        h.click("Full");
        h.click("Instance name");
        assert_eq!(h.app.editor.document.model(), &original);
        h.reveal_property("Label preview");
        assert!(h.visible_text_rect("D-01").is_some());
        h.reveal_property("Apply tag label");
        h.click("Apply tag label");
        h.settle_plan();
        h.frame(vec![]);
        let styled = h.app.editor.document.model().clone();
        assert_eq!(
            styled.opening_tags[&tag].parameters.label_preset,
            os_model::OpeningTagLabelPreset::InstanceName
        );
        assert_eq!(canvas_text(&h, "D-01"), 1);
        assert_eq!(
            styled.opening_tags[&tag].header,
            original.opening_tags[&tag].header
        );
        assert_eq!(
            h.app.editor.document.history_stats().undo_entries,
            history.undo_entries + 1
        );
        h.app.history(false);
        h.settle_plan();
        assert_eq!(h.app.editor.document.model(), &original);
        h.app.history(true);
        h.settle_plan();
        assert_eq!(h.app.editor.document.model(), &styled);
        let before = h.app.editor.document.model().clone();
        h.dimension("X (m)", "2.5");
        assert_eq!(h.app.editor.document.model(), &before);
        h.click("Apply tag position");
        assert_eq!(
            h.app.editor.document.model().opening_tags[&tag]
                .parameters
                .position
                .x,
            2.5
        );
        assert_eq!(
            h.app.editor.document.model().opening_tags[&tag].header,
            before.opening_tags[&tag].header
        );
        let moved = h.app.editor.document.model().clone();
        assert_eq!(
            moved.opening_tags[&tag].parameters.label_preset,
            os_model::OpeningTagLabelPreset::InstanceName
        );
        h.app.history(false);
        assert_eq!(h.app.editor.document.model(), &before);
        h.app.history(true);
        assert_eq!(h.app.editor.document.model(), &moved);
        h.settle_plan();
        h.frame(vec![]);
        h.reveal_property("Delete opening tag");
        h.click("Delete opening tag");
        assert!(h.app.editor.document.model().opening_tags.is_empty());
        h.app.history(false);
        assert_eq!(h.app.editor.document.model(), &moved);
        h.app
            .editor
            .command("Delete opening", Command::RemoveOpening(opening))
            .unwrap();
        h.settle_plan();
        h.frame(vec![]);
        h.app.select(None);
        h.frame(vec![]);
        let rect = reveal_browser(&mut h, "Opening tag · Missing opening");
        h.click_at(rect.center());
        h.frame(vec![]);
        assert_eq!(h.app.selected, Some(tag));
        assert!(h.visible_text_rect("Missing opening").is_some());
        let ctx = h.app.editor.native_plan_context(view).unwrap();
        assert_eq!(
            h.app
                .plans
                .drawing
                .as_ref()
                .unwrap()
                .opening_tags(ctx)
                .unwrap()[0]
                .diagnostic
                .as_deref(),
            Some("Missing opening")
        );
        let orphan = h.app.editor.document.model().clone();
        let orphan_history = h.app.editor.document.history_stats();
        let from = start_drag(&mut h, view, tag);
        let to = from + egui::vec2(20., -10.);
        h.frame(vec![egui::Event::PointerMoved(to)]);
        assert_eq!(h.app.editor.document.model(), &orphan);
        assert_eq!(h.app.editor.document.history_stats(), orphan_history);
        assert_eq!(canvas_text(&h, "Opening tag · Missing opening"), 1);
        h.frame(vec![button(to, false)]);
        let orphan_moved = h.app.editor.document.model().clone();
        assert_eq!(orphan_moved.openings, orphan.openings);
        assert_eq!(
            orphan_moved.opening_tags[&tag].header,
            orphan.opening_tags[&tag].header
        );
        assert_eq!(
            orphan_moved.opening_tags[&tag].parameters.opening,
            orphan.opening_tags[&tag].parameters.opening
        );
        assert_eq!(
            orphan_moved.opening_tags[&tag].parameters.view,
            orphan.opening_tags[&tag].parameters.view
        );
        assert_ne!(
            orphan_moved.opening_tags[&tag].parameters.position,
            orphan.opening_tags[&tag].parameters.position
        );
        assert_eq!(
            h.app.editor.document.history_stats().undo_entries,
            orphan_history.undo_entries + 1
        );
        h.app.history(false);
        assert_eq!(h.app.editor.document.model(), &orphan);
        h.app.history(true);
        assert_eq!(h.app.editor.document.model(), &orphan_moved);
        h.settle_plan();
        h.frame(vec![]);
        let v = h.app.editor.document.model().views[&view]
            .parameters
            .clone();
        let mut settings = v.plan.unwrap();
        settings.crop = Some(os_model::PlanViewCrop {
            min: Point2::new(0., 0.),
            max: Point2::new(0.5, 0.5),
        });
        h.app
            .editor
            .update_floor_plan(view, "Tag plan", v.level.unwrap(), settings)
            .unwrap();
        h.settle_plan();
        h.frame(vec![]);
        assert_eq!(canvas_text(&h, "Opening tag · Missing opening"), 0);
        h.app.select(None);
        h.frame(vec![]);
        let rect = reveal_browser(&mut h, "Opening tag · Missing opening");
        h.click_at(rect.center());
        assert_eq!(
            h.app.selected,
            Some(tag),
            "cropped orphan remains selectable in Browser"
        );
    }
}

#[test]
fn opening_tag_crop_rejects_drag_release_and_properties_outside_crop() {
    for (size, scale) in PROFILES {
        let (mut h, view, opening) = setup(size, scale);
        let tag = place(&mut h, view, opening);
        let v = h.app.editor.document.model().views[&view]
            .parameters
            .clone();
        let mut settings = v.plan.unwrap();
        settings.crop = Some(os_model::PlanViewCrop {
            min: Point2::new(1., 0.5),
            max: Point2::new(3., 2.5),
        });
        h.app
            .editor
            .update_floor_plan(view, "Tag plan", v.level.unwrap(), settings)
            .unwrap();
        h.settle_plan();
        h.frame(vec![]);
        let before = h.app.editor.document.model().clone();
        let history = h.app.editor.document.history_stats();
        start_drag(&mut h, view, tag);
        let outside = h.plan_point(view, Point2::new(3.5, 1.5));
        assert!(h.app.plans.canvas_rect.unwrap().contains(outside));
        h.frame(vec![egui::Event::PointerMoved(outside)]);
        h.frame(vec![button(outside, false)]);
        assert_eq!(h.app.editor.document.model(), &before);
        assert_eq!(h.app.editor.document.history_stats(), history);
        h.frame(vec![]);
        h.dimension("X (m)", "3.5");
        h.click("Apply tag position");
        assert_eq!(
            h.app.editor.document.model(),
            &before,
            "Properties must reject positions outside owning view crop"
        );
        assert_eq!(h.app.editor.document.history_stats(), history);
        assert!(h.app.status_error);
    }
}

#[test]
fn opening_tag_placement_escape_stale_and_duplicate_leave_model_and_history_intact() {
    for (size, scale) in PROFILES {
        for case in ["escape", "revision", "drawing", "duplicate"] {
            let (mut h, view, opening) = setup(size, scale);
            if case == "duplicate" {
                place(&mut h, view, opening);
            }
            h.app.select(Some(opening));
            h.frame(vec![]);
            h.click("Opening Tag");
            let p = h.plan_point(view, Point2::new(2., 1.5));
            h.frame(vec![egui::Event::PointerMoved(p)]);
            match case {
                "escape" => h.frame(vec![escape()]),
                "revision" => h
                    .app
                    .editor
                    .command("Edit", Command::RenameProject("Changed".into()))
                    .unwrap(),
                "drawing" => {
                    h.app.plans.drawing = Some(h.app.editor.native_wall_plan(view).unwrap())
                }
                _ => (),
            }
            let before = h.app.editor.document.model().clone();
            let history = h.app.editor.document.history_stats();
            h.frame(vec![]);
            h.click_at(h.plan_point(view, Point2::new(2.5, 1.5)));
            assert_eq!(h.app.editor.document.model(), &before, "{case}");
            assert_eq!(h.app.editor.document.history_stats(), history, "{case}");
        }
    }
}

#[test]
fn opening_tag_drag_wins_over_overlapping_selected_wall_endpoint() {
    for (size, scale) in PROFILES {
        let (mut h, view, opening) = setup(size, scale);
        let tag = place(&mut h, view, opening);
        let wall = h
            .app
            .editor
            .document
            .model()
            .walls
            .values()
            .find(|w| w.parameters.start() == Point2::new(0., 0.))
            .unwrap()
            .id();
        h.app.opening_tag_position = Point2::new(0., 0.);
        h.click("Apply tag position");
        h.settle_plan();
        h.app.select(Some(wall));
        h.frame(vec![]);
        let before = h.app.editor.document.model().clone();
        let from = start_drag(&mut h, view, tag);
        let to = from + egui::vec2(30., -25.);
        h.frame(vec![egui::Event::PointerMoved(to)]);
        assert_eq!(h.app.editor.document.model(), &before);
        h.frame(vec![button(to, false)]);
        let after = h.app.editor.document.model();
        assert_eq!(after.walls, before.walls);
        assert_ne!(
            after.opening_tags[&tag].parameters.position,
            before.opening_tags[&tag].parameters.position
        );
        assert_eq!(h.app.selected, Some(tag));
    }
}

#[test]
fn opening_tag_source_move_rotated_plane_crop_and_orphan_at_both_dpis() {
    for (size, scale) in PROFILES {
        let (mut h, view, opening) = setup(size, scale);
        let tag = place(&mut h, view, opening);
        let saved = h.app.editor.document.model().opening_tags[&tag].clone();
        let mut parameters = h.app.editor.document.model().openings[&opening]
            .parameters
            .clone();
        parameters.offset = 2.;
        h.app
            .editor
            .command(
                "Move opening",
                Command::UpdateOpening {
                    id: opening,
                    parameters,
                },
            )
            .unwrap();
        h.settle_plan();
        assert_eq!(
            tag_graphic(&h, view).leader_source,
            Some(Point2::new(2.45, 0.))
        );
        assert_tag_leader(&h, view, &tag_graphic(&h, view));
        assert_eq!(h.app.editor.document.model().opening_tags[&tag], saved);

        let mut settings = h.app.editor.document.model().views[&view]
            .parameters
            .plan
            .unwrap();
        settings.basis.origin = Point2::new(1., -2.);
        settings.basis.rotation = 0.63;
        h.app
            .editor
            .update_floor_plan(view, "Tag plan", h.app.active_level, settings)
            .unwrap();
        h.settle_plan();
        let context = h.app.editor.native_plan_context(view).unwrap();
        let graphic = tag_graphic(&h, view);
        assert_eq!(
            graphic.leader_source,
            Some(context.basis.world_to_plane(Point2::new(2.45, 0.)).unwrap())
        );
        assert_eq!(
            graphic.anchor,
            context
                .basis
                .world_to_plane(saved.parameters.position)
                .unwrap()
        );
        assert_tag_leader(&h, view, &graphic);

        // Exercise an oblique host independently of the rectangle fixture.
        let mut model = h.app.editor.document.model().clone();
        let host = model.openings[&opening].parameters.host;
        let wall = &mut model.walls.get_mut(&host).unwrap().parameters;
        *wall.path.straight_start_mut().unwrap() = Point2::new(1., 1.);
        *wall.path.straight_end_mut().unwrap() = Point2::new(4., 5.);
        let derived =
            crate::plan::opening_tag_graphic(&model, tag, &saved.parameters, context).unwrap();
        assert!(
            derived.leader_source.unwrap().distance(
                context
                    .basis
                    .world_to_plane(Point2::new(1. + 0.6 * 2.45, 1. + 0.8 * 2.45))
                    .unwrap()
            ) < 1e-12
        );
        model.walls.remove(&host);
        let orphan =
            crate::plan::opening_tag_graphic(&model, tag, &saved.parameters, context).unwrap();
        assert_eq!(orphan.diagnostic.as_deref(), Some("Missing host wall"));
        assert!(orphan.leader_source.is_none());

        settings.basis = Default::default();
        settings.crop = Some(os_model::PlanViewCrop {
            min: Point2::new(1., 0.5),
            max: Point2::new(3., 2.5),
        });
        h.app
            .editor
            .update_floor_plan(view, "Tag plan", h.app.active_level, settings)
            .unwrap();
        h.settle_plan();
        assert_tag_leader(&h, view, &tag_graphic(&h, view));
        h.app
            .editor
            .command("Delete opening", Command::RemoveOpening(opening))
            .unwrap();
        h.settle_plan();
        assert!(tag_graphic(&h, view).leader_source.is_none());
        assert_eq!(canvas_text(&h, "Opening tag · Missing opening"), 1);
        let mut phantom = tag_graphic(&h, view);
        phantom.diagnostic = None;
        phantom.leader_source = Some(Point2::new(2.45, 0.));
        assert_tag_leader_count(&h, view, &phantom, 0);
        assert_eq!(h.app.editor.document.model().opening_tags[&tag], saved);
    }
}
