use super::*;
use os_model::*;

struct Harness {
    app: DesktopApp,
    ctx: egui::Context,
    output: egui::FullOutput,
    size: egui::Vec2,
    scale: f32,
    time: f64,
}
impl Harness {
    fn new(size: egui::Vec2, scale: f32) -> Self {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        Self {
            app: DesktopApp::new().unwrap(),
            ctx,
            output: Default::default(),
            size,
            scale,
            time: 0.,
        }
    }
    fn frame(&mut self, events: Vec<egui::Event>) {
        self.time += 0.1;
        let mut raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, self.size)),
            time: Some(self.time),
            events,
            ..Default::default()
        };
        raw.viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .native_pixels_per_point = Some(self.scale);
        self.output = self.ctx.run(raw, |ctx| self.app.opening_type_dialog(ctx));
    }
    fn click_at(&mut self, pos: egui::Pos2) {
        for pressed in [true, false] {
            self.frame(vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    pressed,
                    button: egui::PointerButton::Primary,
                    modifiers: egui::Modifiers::NONE,
                },
            ]);
        }
        self.frame(vec![]);
    }
    fn click(&mut self, label: &str) {
        for _ in 0..3 {
            self.frame(vec![]);
        }
        let at = self
            .output
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.job.text == label => {
                    let r = egui::Rect::from_min_size(t.pos, t.galley.size());
                    s.clip_rect.contains_rect(r).then_some(r.center())
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing visible {label}"));
        assert!(egui::Rect::from_min_size(egui::Pos2::ZERO, self.size).contains(at));
        self.click_at(at);
    }
    fn drag_profile(&mut self) {
        for _ in 0..20 {
            self.frame(vec![]);
        }
        for _ in 0..12 {
            let handle = self.output.shapes.iter().find_map(|s| match &s.shape {
                egui::Shape::Circle(c)
                    if (c.radius == 3. || c.radius == 5.)
                        && s.clip_rect.shrink(8.).contains(c.center) =>
                {
                    Some(c.center)
                }
                _ => None,
            });
            if let Some(at) = handle {
                self.frame(vec![
                    egui::Event::PointerMoved(at),
                    egui::Event::PointerButton {
                        pos: at,
                        pressed: true,
                        button: egui::PointerButton::Primary,
                        modifiers: egui::Modifiers::NONE,
                    },
                ]);
                let to = at + egui::vec2(12., -8.);
                self.frame(vec![egui::Event::PointerMoved(to)]);
                self.frame(vec![egui::Event::PointerMoved(to)]);
                self.frame(vec![egui::Event::PointerButton {
                    pos: to,
                    pressed: false,
                    button: egui::PointerButton::Primary,
                    modifiers: egui::Modifiers::NONE,
                }]);
                return;
            }
            self.frame(vec![
                egui::Event::PointerMoved(self.size.to_pos2() * 0.5),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0., -80.),
                    modifiers: egui::Modifiers::NONE,
                },
            ]);
        }
        panic!("profile handles unavailable");
    }
}
fn fixture(kind: OpeningKind) -> (Editor, Id, Id, Id) {
    let mut e = Editor::new().unwrap();
    let level = *e.document.model().levels.keys().next().unwrap();
    let wall = Wall::new(
        os_walls::WALL_TYPE,
        WallParams {
            name: "Host".into(),
            path: WallPath::Straight {
                start: Point2::new(0., 0.),
                end: Point2::new(8., 0.),
            },
            height: 4.,
            thickness: 0.2,
            level,
            material: None,
        },
    );
    let host = wall.id();
    e.command("Host", Command::AddWall(wall)).unwrap();
    let draft = OpeningTypeDraft::new(&e, kind, None, None);
    let ty = draft.id;
    draft.apply(&mut e).unwrap();
    let opening = Opening::new(
        "core.opening",
        OpeningParams {
            name: "Placed".into(),
            host,
            offset: 1.,
            definition: OpeningDefinition::Typed { type_id: ty },
            hinge: DoorHinge::Start,
            swing: DoorSwing::Left,
            open_state: if kind == OpeningKind::Door {
                OpeningState::DoorAngle(0.)
            } else {
                OpeningState::Default
            },
            width_override: None,
            height_override: None,
            sill_override: None,
            pane_position_override: None,
            lite_side_override: None,
        },
    );
    let id = opening.id();
    e.command("Opening", Command::AddOpening(opening)).unwrap();
    (e, ty, id, host)
}

#[test]
fn authored_extrusions_real_egui_handle_grille_draft_apply_invalid_cancel_history_both_dpis() {
    for (size, scale) in [
        (egui::vec2(1280., 800.), 1.),
        (egui::vec2(1000., 650.), 1.5),
    ] {
        for kind in [OpeningKind::Door, OpeningKind::Window] {
            let mut h = Harness::new(size, scale);
            let (editor, ty, id, _) = fixture(kind);
            h.app.editor = editor;
            h.app.opening_type_draft = Some(OpeningTypeDraft::edit(&h.app.editor, ty).unwrap());
            let before = h.app.editor.document.model().clone();
            let history = h.app.editor.document.history_stats();
            let scene = h.app.editor.scene.clone();
            h.click("Solids…");
            h.click("Add extrusion");
            h.click("Duplicate extrusion");
            h.click("Delete extrusion");
            let initial = h.app.opening_type_draft.as_ref().unwrap().family.extrusions[0]
                .profile
                .clone();
            h.drag_profile();
            assert_ne!(
                h.app.opening_type_draft.as_ref().unwrap().family.extrusions[0].profile,
                initial
            );
            let draft = h.app.opening_type_draft.as_mut().unwrap();
            let solid = &mut draft.family.extrusions[0];
            solid.profile = initial;
            solid.name = if kind == OpeningKind::Door {
                "Handle"
            } else {
                "Grille"
            }
            .into();
            if kind == OpeningKind::Window {
                solid.profile = vec![
                    Point2::new(0.45, 0.05),
                    Point2::new(0.5, 0.05),
                    Point2::new(0.5, 0.95),
                    Point2::new(0.45, 0.95),
                ];
            }
            solid.depth = 0.025;
            solid.normal_offset = 0.15;
            draft.preview_geometry(&h.app.editor, None).unwrap();
            assert_eq!(h.app.editor.document.model(), &before);
            assert_eq!(h.app.editor.document.history_stats(), history);
            assert_eq!(h.app.editor.scene, scene);
            h.click("Apply type");
            assert!(h.app.opening_type_draft.is_none(), "{}", h.app.status);
            let after = h.app.editor.document.model().clone();
            assert_eq!(after.openings[&id], before.openings[&id]);
            assert_eq!(
                h.app.editor.document.history_stats().undo_entries,
                history.undo_entries + 1
            );
            h.app.editor.undo().unwrap();
            assert_eq!(h.app.editor.document.model(), &before);
            h.app.editor.redo().unwrap();
            assert_eq!(h.app.editor.document.model(), &after);
            h.app.opening_type_draft = Some(OpeningTypeDraft::edit(&h.app.editor, ty).unwrap());
            h.click("Solids…");
            h.app.opening_type_draft.as_mut().unwrap().family.extrusions[0].depth = -1.;
            h.click("Apply type");
            assert!(h.app.opening_type_draft.is_some());
            assert_eq!(h.app.editor.document.model(), &after);
            h.click("Cancel type");
            assert!(h.app.opening_type_draft.is_none());
            h.app.opening_type_draft = Some(OpeningTypeDraft::edit(&h.app.editor, ty).unwrap());
            h.frame(vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }]);
            assert!(h.app.opening_type_draft.is_none());
            h.app.opening_type_draft = Some(OpeningTypeDraft::edit(&h.app.editor, ty).unwrap());
            h.app.editor.undo().unwrap();
            h.frame(vec![]);
            assert!(h.app.opening_type_draft.is_none());
        }
    }
}

#[test]
fn authored_extrusions_owner_material_cross_view_sheet_pdf_save_ifc_and_type_duplicate() {
    use os_render::sheet::{PaperMarkKind, PaperSheetInfo, PaperViewport, compose_view_sheet};
    for kind in [OpeningKind::Door, OpeningKind::Window] {
        let (mut e, ty, id, host) = fixture(kind);
        let level = e.document.model().walls[&host].parameters.level;
        let mat = Material::new(
            "core.material",
            MaterialParams {
                cut_pattern: None,
                name: "Bronze handle".into(),
                density_kg_m3: 8500.,
                color: [150, 90, 40],
            },
        );
        let material = mat.id();
        e.command("Material", Command::AddMaterial(mat)).unwrap();
        let parameter = LengthParameter::new(
            "core.length_parameter",
            LengthParameterParams {
                name: "Normal offset".into(),
                unit: LengthUnit::Metres,
                value: 0.15,
                formula: Some("0.1m + 0.05m".into()),
            },
        );
        let pid = parameter.id();
        e.command("Driver", Command::AddLengthParameter(parameter))
            .unwrap();
        let mut draft = OpeningTypeDraft::edit(&e, ty).unwrap();
        let mut s = OpeningExtrusion::new("Authored handle/grille");
        if kind == OpeningKind::Window {
            s.profile = vec![
                Point2::new(0.45, 0.05),
                Point2::new(0.5, 0.05),
                Point2::new(0.5, 0.95),
                Point2::new(0.45, 0.95),
            ];
        }
        s.material = Some(material);
        s.normal_offset = 0.15;
        s.bindings.normal_offset = Some(pid);
        let solid_id = s.id;
        let old_host = crate::opening_tools::host_mesh(e.document.model(), host).unwrap();
        draft.family.extrusions.push(s);
        draft.apply(&mut e).unwrap();
        let old_scene = e.scene[&id].clone();
        let mut parameters = e.document.model().length_parameters[&pid]
            .parameters
            .clone();
        parameters.formula = Some("0.1m + 0.06m".into());
        e.command(
            "Driven offset",
            Command::UpdateLengthParameter {
                id: pid,
                parameters,
            },
        )
        .unwrap();
        assert_ne!(e.scene[&id], old_scene);
        assert_eq!(
            crate::opening_tools::host_mesh(e.document.model(), host).unwrap(),
            old_host
        );
        let p = e
            .document
            .model()
            .resolve_opening(&e.document.model().openings[&id].parameters)
            .unwrap();
        let wall = e.document.model().resolve_wall(host).unwrap();
        let mesh = os_geometry::openings::authored_solids(&p, &wall.parameters, 0.)
            .unwrap()
            .remove(0);
        let center = mesh.vertices.iter().fold(Point2::new(0., 0.), |a, v| {
            Point2::new(a.x + v.x / 8., a.y + v.y / 8.)
        });
        let plan = e.create_floor_plan("Plan", level).unwrap();
        let section = e
            .create_section_view(
                "Section",
                level,
                SectionViewSettings::new(
                    Point2::new(0., center.y),
                    Point2::new(8., center.y),
                    -1.,
                    5.,
                ),
            )
            .unwrap();
        let elevation = e
            .create_wall_elevation("Elevation", host, ElevationSide::Left)
            .unwrap();
        let facade = e
            .create_facade_view(
                "Facade",
                level,
                FacadeViewSettings::new(Point2::new(0., 5.), Point2::new(8., 5.), -1., 5.).unwrap(),
            )
            .unwrap();
        for view in [plan, section, elevation, facade] {
            let context = e.native_view_context(view).unwrap();
            let drawing = e.native_drawing(view).unwrap_or_else(|error| {
                panic!(
                    "{:?}: {error}",
                    e.document.model().views[&view].parameters.kind
                )
            });
            let lines: Vec<_> = drawing
                .provider_lines(context)
                .unwrap()
                .iter()
                .filter(|l| {
                    l.entity == id && drawing.line_surface(id, l.feature).layer == Some(solid_id)
                })
                .collect();
            assert!(
                !lines.is_empty(),
                "missing authored solid in {:?}; range {:?}; lines {:?}",
                e.document.model().views[&view].parameters.kind,
                context.range,
                drawing
                    .provider_lines(context)
                    .unwrap()
                    .iter()
                    .filter(|l| l.entity == id)
                    .map(|l| (l.feature, drawing.line_surface(id, l.feature)))
                    .collect::<Vec<_>>()
            );
            assert!(
                lines
                    .iter()
                    .all(|l| drawing.line_surface(id, l.feature).material == Some(material))
            );
            let l = lines[0];
            let point = Point2::new((l.start.x + l.end.x) / 2., (l.start.y + l.end.y) / 2.);
            let camera = os_render::plan::PlanCamera {
                center: point,
                pixels_per_metre: 1000.,
            };
            assert!(
                drawing
                    .hits_screen(
                        context,
                        camera,
                        [800., 600.],
                        camera.project(point, [800., 600.]).unwrap(),
                        2.
                    )
                    .unwrap()
                    .contains(&id)
            );
            let viewport = PaperViewport {
                model_center_m: Point2::new(4., 2.),
                center_mm: Point2::new(148.5, 105.),
                width_mm: 200.,
                height_mm: 140.,
                scale_denominator: 100.,
            };
            let page = compose_view_sheet(
                PaperSheetInfo {
                    width_mm: 297.,
                    height_mm: 210.,
                    number: "A1",
                    name: "Authored solids",
                },
                "View",
                viewport,
                context,
                &drawing,
            )
            .unwrap();
            let pdf = String::from_utf8(page.to_pdf().unwrap()).unwrap();
            for l in lines {
                let points = [
                    viewport.model_to_paper(l.start).unwrap(),
                    viewport.model_to_paper(l.end).unwrap(),
                ];
                assert!(page.marks().iter().any(|m|matches!(&m.kind,PaperMarkKind::Path{points_mm,closed:false,..} if points_mm.as_slice()==points)));
                let pt = 72. / 25.4;
                assert!(pdf.contains(&format!(
                    "{:.6} {:.6} m\n{:.6} {:.6} l",
                    points[0].x * pt,
                    (210. - points[0].y) * pt,
                    points[1].x * pt,
                    (210. - points[1].y) * pt
                )));
            }
        }
        assert!(
            os_ifc::WallIfc
                .export_report(e.document.model())
                .unwrap_err()
                .to_string()
                .contains("authored opening extrusions")
        );
        let original = e.document.model().clone();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("authored.osb");
        e.save(&path).unwrap();
        e.open(&path).unwrap();
        assert_eq!(e.document.model(), &original);
        let mut duplicate = OpeningTypeDraft::edit(&e, ty).unwrap();
        duplicate.duplicate();
        assert_ne!(duplicate.family.extrusions[0].id, solid_id);
        duplicate.apply(&mut e).unwrap();
    }
}
