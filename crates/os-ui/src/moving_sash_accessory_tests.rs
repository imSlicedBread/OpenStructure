use super::*;
use os_model::*;

fn attachment(operation: WindowOperation) -> OpeningAttachment {
    if operation == WindowOperation::Fixed {
        OpeningAttachment::InactiveLeaf
    } else {
        OpeningAttachment::MovingSash
    }
}

struct Harness {
    app: DesktopApp,
    ctx: egui::Context,
    output: egui::FullOutput,
    size: egui::Vec2,
    scale: f32,
    time: f64,
}
impl Harness {
    fn new(size: egui::Vec2, scale: f32, editor: Editor, ty: Id) -> Self {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let mut app = DesktopApp::new().unwrap();
        app.editor = editor;
        app.opening_type_draft = Some(OpeningTypeDraft::edit(&app.editor, ty).unwrap());
        Self {
            app,
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
        self.output = self.ctx.run(raw, |ctx| {
            self.app.opening_type_dialog(ctx);
            self.app.show_opening_type_package_dialog(ctx);
        });
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
        for pressed in [true, false] {
            self.frame(vec![
                egui::Event::PointerMoved(at),
                egui::Event::PointerButton {
                    pos: at,
                    pressed,
                    button: egui::PointerButton::Primary,
                    modifiers: egui::Modifiers::NONE,
                },
            ]);
        }
        self.frame(vec![]);
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
                let to = at + egui::vec2(-8., 12.);
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

fn fixture(operation: WindowOperation, frame: f64) -> (Editor, Id, Id, Id) {
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
            thickness: 0.3,
            level,
            material: None,
        },
    );
    let host = wall.id();
    e.command("Host", Command::AddWall(wall)).unwrap();
    // Fixed is the paired-door case; operable variants retain the sash regression.
    let paired = operation == WindowOperation::Fixed;
    let mut draft = OpeningTypeDraft::new(
        &e,
        if paired {
            OpeningKind::Door
        } else {
            OpeningKind::Window
        },
        None,
        None,
    );
    draft.window_operation = operation;
    draft.family.frame_width = frame;
    if paired {
        draft.values[0] = "1.8".into();
        draft.family.door_leaves = DoorLeaves::Paired {
            active_fraction: 0.63,
        };
    }
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
            open_state: if paired {
                OpeningState::DoorPairAngles {
                    active_degrees: 30.,
                    inactive_degrees: 90.,
                }
            } else if operation == WindowOperation::Casement {
                OpeningState::CasementAngle(45.)
            } else {
                OpeningState::SlidingFraction(0.5)
            },
            width_override: None,
            height_override: None,
            sill_override: None,
            pane_position_override: if paired {
                None
            } else {
                Some(WindowPanePosition::LeftFace)
            },
            lite_side_override: None,
        },
    );
    let id = opening.id();
    e.command("Opening", Command::AddOpening(opening)).unwrap();
    (e, ty, id, host)
}

#[test]
fn paired_and_moving_sash_accessory_real_egui_draft_apply_one_undo_redo_escape_stale_both_dpis() {
    for (size, scale) in [
        (egui::vec2(1280., 800.), 1.),
        (egui::vec2(1000., 650.), 1.5),
    ] {
        for operation in [
            WindowOperation::Fixed,
            WindowOperation::Casement,
            WindowOperation::Sliding,
        ] {
            let (editor, ty, id, _) = fixture(operation, 0.04);
            let mut h = Harness::new(size, scale, editor, ty);
            let before = h.app.editor.document.model().clone();
            let history = h.app.editor.document.history_stats();
            let scene = h.app.editor.scene.clone();
            h.click("Solids…");
            h.click("Add extrusion");
            h.click("Opening fixed");
            if operation == WindowOperation::Fixed {
                h.click("Active leaf");
            }
            h.click(if operation == WindowOperation::Fixed {
                "Inactive leaf"
            } else if operation == WindowOperation::Sliding {
                "Moving Start sash"
            } else {
                "Moving sash"
            });
            assert_eq!(
                h.app.opening_type_draft.as_ref().unwrap().family.extrusions[0].attachment,
                attachment(operation)
            );
            h.click("Duplicate extrusion");
            let solids = &h.app.opening_type_draft.as_ref().unwrap().family.extrusions;
            assert_ne!(solids[0].id, solids[1].id);
            assert_eq!(solids[1].attachment, attachment(operation));
            h.click("Delete extrusion");
            let initial = h.app.opening_type_draft.as_ref().unwrap().family.extrusions[0]
                .profile
                .clone();
            h.drag_profile();
            assert_ne!(
                h.app.opening_type_draft.as_ref().unwrap().family.extrusions[0].profile,
                initial
            );
            h.app.opening_type_draft.as_mut().unwrap().family.extrusions[0].profile = initial;
            h.app
                .opening_type_draft
                .as_ref()
                .unwrap()
                .preview_geometry(&h.app.editor, None)
                .unwrap();
            assert_eq!(h.app.editor.document.model(), &before);
            assert_eq!(h.app.editor.document.history_stats(), history);
            assert_eq!(h.app.editor.scene, scene);
            h.click("Apply type");
            assert!(h.app.opening_type_draft.is_none(), "{}", h.app.status);
            let after = h.app.editor.document.model().clone();
            if operation == WindowOperation::Fixed {
                let mut incompatible = OpeningTypeDraft::edit(&h.app.editor, ty).unwrap();
                incompatible.family.door_leaves = DoorLeaves::Single;
                assert!(incompatible.apply(&mut h.app.editor).is_err());
                assert_eq!(h.app.editor.document.model(), &after);
            }
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
            h.app.opening_type_draft.as_mut().unwrap().family.extrusions[0]
                .position
                .x = 5.;
            h.click("Apply type");
            assert!(h.app.opening_type_draft.is_some());
            assert_eq!(h.app.editor.document.model(), &after);
            h.escape();
            assert!(h.app.opening_type_draft.is_none());
            h.app.opening_type_draft = Some(OpeningTypeDraft::edit(&h.app.editor, ty).unwrap());
            h.app.editor.undo().unwrap();
            h.frame(vec![]);
            assert!(h.app.opening_type_draft.is_none());
            // Reopen the same saved project twice: UUIDs and revision stay equal,
            // but the draft must belong exclusively to its original open session.
            h.app.editor.redo().unwrap();
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("stale-session.osb");
            h.app.editor.save(&path).unwrap();
            h.app.editor.open(&path).unwrap();
            h.app.opening_type_draft = Some(OpeningTypeDraft::edit(&h.app.editor, ty).unwrap());
            h.app.opening_type_draft.as_mut().unwrap().family.extrusions[0].normal_offset += 0.01;
            let session = h.app.editor.document.session_id();
            let revision = h.app.editor.document.revision();
            h.app.editor.open(&path).unwrap();
            assert_ne!(h.app.editor.document.session_id(), session);
            assert_eq!(h.app.editor.document.revision(), revision);
            let loaded = h.app.editor.document.model().clone();
            let loaded_history = h.app.editor.document.history_stats();
            let loaded_scene = h.app.editor.scene.clone();
            assert!(
                !h.app
                    .opening_type_draft
                    .as_ref()
                    .unwrap()
                    .current(&h.app.editor)
            );
            assert!(
                h.app
                    .opening_type_draft
                    .as_ref()
                    .unwrap()
                    .apply(&mut h.app.editor)
                    .is_err()
            );
            h.frame(vec![]);
            assert!(h.app.opening_type_draft.is_none());
            assert_eq!(h.app.editor.document.model(), &loaded);
            assert_eq!(h.app.editor.document.history_stats(), loaded_history);
            assert_eq!(h.app.editor.scene, loaded_scene);
        }
    }
}

#[test]
fn paired_and_moving_sash_accessory_hidden_invalid_instance_blocks_draft_and_type_update_at_both_dpis()
 {
    for (size, scale) in [
        (egui::vec2(1280., 800.), 1.),
        (egui::vec2(1000., 650.), 1.5),
    ] {
        for operation in [WindowOperation::Fixed, WindowOperation::Sliding] {
            let (mut editor, ty, visible, _) = fixture(operation, 0.);
            let mut hidden = editor.document.model().openings[&visible].clone();
            hidden.header.id = Id::new();
            hidden.parameters.name = "Unshown narrow instance".into();
            hidden.parameters.offset = 4.;
            hidden.parameters.width_override = Some(0.01);
            let hidden_id = hidden.id();
            editor
                .command("Hidden instance", Command::AddOpening(hidden))
                .unwrap();
            let level = editor.document.model().walls
                [&editor.document.model().openings[&visible].parameters.host]
                .parameters
                .level;
            let view = editor
                .create_floor_plan("Visible instance only", level)
                .unwrap();
            let mut parameters = editor.document.model().views[&view].parameters.clone();
            parameters.plan.as_mut().unwrap().crop = Some(PlanViewCrop {
                min: Point2::new(0., -1.),
                max: Point2::new(3., 1.),
            });
            editor
                .command(
                    "Hide narrow instance outside plan crop",
                    Command::UpdateView {
                        id: view,
                        parameters,
                    },
                )
                .unwrap();
            let context = editor.native_view_context(view).unwrap();
            let drawing = editor.native_drawing(view).unwrap();
            let lines = drawing.provider_lines(context).unwrap();
            assert!(lines.iter().any(|line| line.entity == visible));
            assert!(lines.iter().all(|line| line.entity != hidden_id));
            let mut h = Harness::new(size, scale, editor, ty);
            h.app.focus_plan(Some(view));
            let before = h.app.editor.document.model().clone();
            let history = h.app.editor.document.history_stats();
            let scene = h.app.editor.scene.clone();
            h.click("Solids…");
            h.click("Add extrusion");
            h.click(if operation == WindowOperation::Fixed {
                "Inactive leaf"
            } else {
                "Moving Start sash"
            });
            assert!(
                h.app
                    .opening_type_draft
                    .as_ref()
                    .unwrap()
                    .preview_geometry(&h.app.editor, Some(view))
                    .is_err()
            );
            h.click("Apply type");
            assert!(h.app.opening_type_draft.is_some());
            assert_eq!(h.app.editor.document.model(), &before);
            assert_eq!(h.app.editor.document.history_stats(), history);
            assert_eq!(h.app.editor.scene, scene);
            h.click("Cancel type");
            assert!(h.app.opening_type_draft.is_none());
        }
    }
}

#[test]
fn paired_and_moving_sash_accessory_owner_material_cross_view_native_package_roundtrip_and_ifc_rejection()
 {
    for (operation, selected_attachment) in [
        (WindowOperation::Fixed, OpeningAttachment::ActiveLeaf),
        (WindowOperation::Fixed, OpeningAttachment::InactiveLeaf),
        (WindowOperation::Casement, OpeningAttachment::MovingSash),
        (WindowOperation::Sliding, OpeningAttachment::MovingSash),
    ] {
        let (mut e, ty, id, host) = fixture(operation, 0.04);
        let level = e.document.model().walls[&host].parameters.level;
        let material = Material::new(
            "core.material",
            MaterialParams {
                name: "Bronze latch".into(),
                density_kg_m3: 8500.,
                color: [150, 90, 40],
                cut_pattern: None,
            },
        );
        let mid = material.id();
        e.command("Material", Command::AddMaterial(material))
            .unwrap();
        let mut draft = OpeningTypeDraft::edit(&e, ty).unwrap();
        let mut s = OpeningExtrusion::new("Latch");
        s.profile = vec![
            Point2::new(0.75, 0.15),
            Point2::new(0.9, 0.15),
            Point2::new(0.9, 0.35),
            Point2::new(0.75, 0.35),
        ];
        s.attachment = selected_attachment;
        s.material = Some(mid);
        s.normal_offset = 0.15;
        let sid = s.id;
        draft.family.extrusions.push(s);
        let host_before = crate::opening_tools::host_mesh(e.document.model(), host).unwrap();
        draft.apply(&mut e).unwrap();
        assert!(!e.scene.contains_key(&sid));
        let scene_surfaces: Vec<_> = e.scene[&id]
            .surfaces
            .iter()
            .filter(|surface| surface.layer == Some(sid))
            .collect();
        assert!(!scene_surfaces.is_empty());
        assert!(
            scene_surfaces
                .iter()
                .all(|surface| surface.material == Some(mid))
        );
        assert_eq!(
            crate::opening_tools::host_mesh(e.document.model(), host).unwrap(),
            host_before
        );
        let resolved = e
            .document
            .model()
            .resolve_opening(&e.document.model().openings[&id].parameters)
            .unwrap();
        let wall = e.document.model().resolve_wall(host).unwrap();
        let mesh = os_geometry::openings::authored_solids(&resolved, &wall.parameters, 0.)
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
            let drawing = e.native_drawing(view).unwrap();
            let lines: Vec<_> = drawing
                .provider_lines(context)
                .unwrap()
                .iter()
                .filter(|line| {
                    line.entity == id && drawing.line_surface(id, line.feature).layer == Some(sid)
                })
                .collect();
            assert!(
                !lines.is_empty(),
                "missing {:?} {operation:?}",
                e.document.model().views[&view].parameters.kind
            );
            assert!(
                lines
                    .iter()
                    .all(|line| drawing.line_surface(id, line.feature).material == Some(mid))
            );
            let line = lines[0];
            let point = Point2::new(
                (line.start.x + line.end.x) / 2.,
                (line.start.y + line.end.y) / 2.,
            );
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
            let edges = lines.iter().map(|line| (line.start, line.end)).collect();
            e = assert_linked_sheet_pdf(e, view, edges);
        }
        let bytes = os_storage::export_opening_type_package(e.document.model(), ty).unwrap();
        let package = os_storage::parse_opening_type_package(&bytes).unwrap();
        assert_eq!(
            package.parameters,
            e.document.model().opening_types[&ty].parameters
        );
        assert_eq!(package.materials.len(), 1);
        assert_eq!(package.materials[0].source_id, mid);
        assert_eq!(package.parameters.family.extrusions[0].id, sid);
        assert_eq!(
            package.parameters.family.extrusions[0].attachment,
            selected_attachment
        );
        assert_package_import(&e, ty, sid, mid);
        assert!(
            os_ifc::WallIfc
                .export_report(e.document.model())
                .unwrap_err()
                .to_string()
                .contains("authored opening extrusions")
        );
        let original = e.document.model().clone();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("moving-sash.osb");
        e.save(&path).unwrap();
        e.open(&path).unwrap();
        assert_eq!(e.document.model(), &original);
        let mut duplicate = OpeningTypeDraft::edit(&e, ty).unwrap();
        duplicate.duplicate();
        assert_ne!(duplicate.family.extrusions[0].id, sid);
        assert_eq!(duplicate.family.extrusions[0].material, Some(mid));
        assert_eq!(
            duplicate.family.extrusions[0].attachment,
            selected_attachment
        );
        duplicate.apply(&mut e).unwrap();
    }
}

fn assert_linked_sheet_pdf(editor: Editor, view: Id, edges: Vec<(Point2, Point2)>) -> Editor {
    use os_render::sheet::{PaperMarkKind, PaperViewport};
    let mut app = DesktopApp::new().unwrap();
    app.editor = editor;
    let ctx = egui::Context::default();
    theme::apply(&ctx);
    let frame = |app: &mut DesktopApp| {
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1280., 800.),
            )),
            ..Default::default()
        };
        let _ = ctx.run(raw, |ctx| app.plan_workspace(ctx));
    };
    frame(&mut app); // Establish the workspace's current document session.
    app.focus_plan(Some(view));
    app.create_sheet_from_active_view();
    assert!(!app.status_error, "{}", app.status);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        frame(&mut app);
        if app.plans.ready() {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "sheet view failed to become ready"
        );
        std::thread::yield_now();
    }
    let sheet = &app.editor.document.model().sheets[&app.plans.active_sheet.unwrap()].parameters;
    let linked = &sheet.viewports[0];
    assert_eq!(linked.view, view);
    let viewport = PaperViewport {
        center_mm: linked.paper_center_mm,
        width_mm: linked.width_mm,
        height_mm: linked.height_mm,
        model_center_m: linked.model_center_m,
        scale_denominator: linked.scale_denominator,
    };
    let paper_height = sheet.paper_size.dimensions_mm().y;
    let page = app.sheet_page().unwrap();
    let pdf = String::from_utf8(page.to_pdf().unwrap()).unwrap();
    for (start, end) in edges {
        let points = [
            viewport.model_to_paper(start).unwrap(),
            viewport.model_to_paper(end).unwrap(),
        ];
        assert!(page.marks().iter().any(|mark| matches!(&mark.kind,
            PaperMarkKind::Path { points_mm,closed: false,.. } if points_mm.as_slice() == points)));
        let pt = 72. / 25.4;
        assert!(pdf.contains(&format!(
            "{:.6} {:.6} m\n{:.6} {:.6} l",
            points[0].x * pt,
            (paper_height - points[0].y) * pt,
            points[1].x * pt,
            (paper_height - points[1].y) * pt
        )));
    }
    app.editor
}

fn assert_package_import(source: &Editor, ty: Id, source_solid: Id, source_material: Id) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("moving-accessory.osot");
    os_storage::write_opening_type_package(source.document.model(), ty, &path).unwrap();
    for (size, scale) in [
        (egui::vec2(1280., 800.), 1.),
        (egui::vec2(1000., 650.), 1.5),
    ] {
        let mut destination = Editor::new().unwrap();
        let mut collision = source.document.model().materials[&source_material].clone();
        collision.parameters.color = [1, 2, 3];
        collision.parameters.density_kg_m3 = 7000.;
        destination
            .command(
                "Material UUID and name collision",
                Command::AddMaterial(collision.clone()),
            )
            .unwrap();
        let mut collision_type = source.document.model().opening_types[&ty].clone();
        collision_type.parameters.family.extrusions.clear();
        destination
            .command(
                "Type UUID and name collision",
                Command::AddOpeningType(collision_type),
            )
            .unwrap();
        let mut h = Harness::new(size, scale, destination, ty);
        h.app.opening_type_draft = None;
        let before = h.app.editor.document.model().clone();
        let history = h.app.editor.document.history_stats();
        let scene = h.app.editor.scene.clone();
        h.app.begin_import_opening_type_package();
        h.app.opening_type_package_dialog.as_mut().unwrap().path = path.display().to_string();
        h.click("Read and preview");
        assert_eq!(h.app.editor.document.model(), &before);
        assert_eq!(h.app.editor.document.history_stats(), history);
        assert_eq!(h.app.editor.scene, scene);
        h.click("Import type");
        assert!(
            h.app.opening_type_package_dialog.is_none(),
            "{}",
            h.app.status
        );
        let imported = h.app.selected.unwrap();
        assert_ne!(imported, ty);
        let model = h.app.editor.document.model();
        let solid = &model.opening_types[&imported].parameters.family.extrusions[0];
        assert_ne!(solid.id, source_solid);
        let material = solid.material.unwrap();
        assert_ne!(material, source_material);
        let mut expected_solid = source.document.model().opening_types[&ty]
            .parameters
            .family
            .extrusions[0]
            .clone();
        expected_solid.id = solid.id;
        expected_solid.material = Some(material);
        assert_eq!(solid, &expected_solid);
        assert_eq!(solid.attachment, expected_solid.attachment);
        let mut expected_material = source.document.model().materials[&source_material]
            .parameters
            .clone();
        expected_material.name.push_str(" (Imported)");
        assert_eq!(model.materials[&material].parameters, expected_material);
        assert_eq!(model.materials[&source_material], collision);
        assert_eq!(model.opening_types[&ty], before.opening_types[&ty]);
        assert_eq!(
            h.app.editor.document.history_stats().undo_entries,
            history.undo_entries + 1
        );
        let after = model.clone();
        let reexport = os_storage::parse_opening_type_package(
            &os_storage::export_opening_type_package(model, imported).unwrap(),
        )
        .unwrap();
        assert_eq!(
            reexport.parameters,
            model.opening_types[&imported].parameters
        );
        assert_eq!(reexport.materials[0].source_id, material);
        h.app.editor.undo().unwrap();
        assert_eq!(h.app.editor.document.model(), &before);
        h.app.editor.redo().unwrap();
        assert_eq!(h.app.editor.document.model(), &after);
    }
}
