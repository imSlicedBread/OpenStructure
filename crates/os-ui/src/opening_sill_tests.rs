use super::*;

#[test]
fn instance_dimension_forms_inherit_pin_reset_preview_history_cancel_and_stale_both_dpis() {
    for (size, scale) in [
        (egui::vec2(1280., 800.), 1.),
        (egui::vec2(1000., 650.), 1.5),
    ] {
        for kind in [OpeningKind::Door, OpeningKind::Window] {
            let mut app = DesktopApp::new().unwrap();
            let view = app
                .editor
                .create_floor_plan("Dimensions", app.active_level)
                .unwrap();
            let wall = os_model::Wall::new(os_walls::WALL_TYPE, default_wall(app.active_level));
            let host = wall.id();
            app.editor.command("Host", Command::AddWall(wall)).unwrap();
            let create =
                OpeningDraft::begin(&app.editor, Some(view), Some(host), Some(kind), None).unwrap();
            let id = create
                .apply(&mut app.editor, Some(view), Some(host), false)
                .unwrap();
            app.plans.active = Some(view);
            app.select(Some(id));
            let before = app.editor.document.model().clone();
            let scene = app.editor.scene.clone();
            let stats = app.editor.document.history_stats();
            let original = before
                .resolve_opening(&before.openings[&id].parameters)
                .unwrap();
            let ctx = egui::Context::default();
            theme::apply(&ctx);
            app.begin_opening(None);
            assert_eq!(
                app.opening_draft.as_ref().unwrap().dimension_overrides,
                [false, false]
            );
            for (i, name) in [(1, "width"), (2, "height")] {
                click(&mut app, &ctx, size, scale, &format!("Override {name}"));
                for value in ["NaN", "0.0005", "-1", "999", "invalid"] {
                    app.opening_draft.as_mut().unwrap().values[i] = value.into();
                    click(&mut app, &ctx, size, scale, "Apply opening");
                    assert!(app.opening_draft.is_some());
                    assert_eq!(app.editor.document.model(), &before);
                    assert_eq!(app.editor.document.history_stats(), stats);
                }
                click(&mut app, &ctx, size, scale, &format!("Inherit {name}"));
                // Disabled invalid text does not participate in parsing.
                app.opening_draft
                    .as_ref()
                    .unwrap()
                    .preview(&app.editor, Some(view), Some(id))
                    .unwrap();
            }
            for name in ["width", "height"] {
                click(&mut app, &ctx, size, scale, &format!("Override {name}"));
            }
            app.opening_draft.as_mut().unwrap().values[1] = (original.width + 0.2).to_string();
            app.opening_draft.as_mut().unwrap().values[2] = (original.height - 0.2).to_string();
            app.opening_draft
                .as_ref()
                .unwrap()
                .preview(&app.editor, Some(view), Some(id))
                .unwrap();
            assert_eq!(app.editor.document.model(), &before);
            assert_eq!(app.editor.scene, scene);
            click(&mut app, &ctx, size, scale, "Cancel opening");
            assert_eq!(app.editor.document.history_stats(), stats);
            app.begin_opening(None);
            for name in ["width", "height"] {
                click(&mut app, &ctx, size, scale, &format!("Override {name}"));
            }
            app.opening_draft.as_mut().unwrap().values[1] = (original.width + 0.2).to_string();
            app.opening_draft.as_mut().unwrap().values[2] = (original.height - 0.2).to_string();
            click(&mut app, &ctx, size, scale, "Apply opening");
            assert!(app.opening_draft.is_none(), "{}", app.status);
            let after = app.editor.document.model().clone();
            let p = &after.openings[&id].parameters;
            assert_eq!(p.width_override, Some(original.width + 0.2));
            assert_eq!(p.height_override, Some(original.height - 0.2));
            assert_eq!(after.opening_types, before.opening_types);
            assert_eq!(after.openings[&id].header, before.openings[&id].header);
            assert_ne!(app.editor.scene[&host], scene[&host]);
            assert_ne!(app.editor.scene[&id], scene[&id]);
            assert_eq!(
                app.editor.document.history_stats().undo_entries,
                stats.undo_entries + 1
            );
            app.editor.undo().unwrap();
            assert_eq!(app.editor.document.model(), &before);
            assert_eq!(app.editor.scene, scene);
            app.editor.redo().unwrap();
            assert_eq!(app.editor.document.model(), &after);
            app.begin_opening(None);
            click(&mut app, &ctx, size, scale, "Reset width to type default");
            assert_eq!(
                app.opening_draft.as_ref().unwrap().dimension_overrides,
                [false, true]
            );
            click(&mut app, &ctx, size, scale, "Reset height to type default");
            click(&mut app, &ctx, size, scale, "Apply opening");
            assert_eq!(app.editor.document.model(), &before);
            assert_eq!(app.editor.scene, scene);
            app.begin_opening(None);
            click(&mut app, &ctx, size, scale, "Override width");
            // Explicit Some(default) is not inheritance.
            click(&mut app, &ctx, size, scale, "Apply opening");
            assert_eq!(
                app.editor.document.model().openings[&id]
                    .parameters
                    .width_override,
                Some(original.width)
            );
            for reason in 0..5 {
                app.plans.active = Some(view);
                app.select(Some(id));
                app.begin_opening(None);
                match reason {
                    0 => app.selected = Some(host),
                    1 => app.plans.active = None,
                    2 => app.opening_draft.as_mut().unwrap().activation = Some(Id::new()),
                    3 => app
                        .editor
                        .command("Revision", Command::RenameProject("Changed".into()))
                        .unwrap(),
                    _ => {
                        app.editor.document =
                            Document::from_model(app.editor.document.model().clone()).unwrap()
                    }
                }
                let model = app.editor.document.model().clone();
                let stats = app.editor.document.history_stats();
                frame(&mut app, &ctx, size, scale, vec![]);
                assert!(app.opening_draft.is_none());
                assert_eq!(app.editor.document.model(), &model);
                assert_eq!(app.editor.document.history_stats(), stats);
            }
        }
    }
}

fn frame(
    app: &mut DesktopApp,
    ctx: &egui::Context,
    size: egui::Vec2,
    scale: f32,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    let mut input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
        events,
        ..Default::default()
    };
    input
        .viewports
        .get_mut(&egui::ViewportId::ROOT)
        .unwrap()
        .native_pixels_per_point = Some(scale);
    ctx.run(input, |ctx| app.opening_dialog(ctx))
}

fn click(app: &mut DesktopApp, ctx: &egui::Context, size: egui::Vec2, scale: f32, label: &str) {
    frame(app, ctx, size, scale, vec![]);
    let mut output = frame(app, ctx, size, scale, vec![]);
    for attempt in 0..30 {
        let target = output.shapes.iter().find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.job.text == label => {
                Some((t.pos + egui::vec2(4., 4.), s.clip_rect))
            }
            _ => None,
        });
        if target.is_some_and(|(pos, clip)| clip.contains(pos)) {
            break;
        }
        let (pos, clip) = target.unwrap_or((
            egui::pos2(size.x / 2., size.y),
            egui::Rect::from_center_size(size.to_pos2() / 2., egui::vec2(250., 150.)),
        ));
        let delta = if attempt == 0 {
            2000.
        } else if pos.y < clip.min.y {
            45.
        } else {
            -45.
        };
        frame(
            app,
            ctx,
            size,
            scale,
            vec![
                egui::Event::PointerMoved(clip.center()),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0., delta),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        for _ in 0..8 {
            output = frame(app, ctx, size, scale, vec![]);
        }
    }
    let pos = output
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t)
                if t.galley.job.text == label
                    && s.clip_rect.contains(t.pos + egui::vec2(4., 4.)) =>
            {
                Some(t.pos + egui::vec2(4., 4.))
            }
            _ => None,
        })
        .unwrap_or_else(|| {
            panic!(
                "missing visible control: {label}: {:?}",
                output
                    .shapes
                    .iter()
                    .filter_map(|s| match &s.shape {
                        egui::Shape::Text(t) => Some((&t.galley.job.text, t.pos, s.clip_rect)),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
            )
        });
    for pressed in [true, false] {
        frame(
            app,
            ctx,
            size,
            scale,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
}

#[test]
fn window_sill_form_inherit_override_reset_cancel_stale_and_regeneration_at_both_dpis() {
    for (size, scale) in [
        (egui::vec2(1280., 800.), 1.),
        (egui::vec2(1000., 650.), 1.5),
    ] {
        let mut app = DesktopApp::new().unwrap();
        let level = app.active_level;
        let view = app.editor.create_floor_plan("Sill plan", level).unwrap();
        let wall = os_model::Wall::new(os_walls::WALL_TYPE, default_wall(level));
        let host = wall.id();
        app.editor.command("Host", Command::AddWall(wall)).unwrap();
        let create = OpeningDraft::begin(
            &app.editor,
            Some(view),
            Some(host),
            Some(OpeningKind::Window),
            None,
        )
        .unwrap();
        let id = create
            .apply(&mut app.editor, Some(view), Some(host), false)
            .unwrap();
        let section = app
            .editor
            .create_section_view(
                "Sill section",
                level,
                os_model::SectionViewSettings::new(
                    Point2::new(0., 0.),
                    Point2::new(5., 0.),
                    -0.5,
                    4.,
                ),
            )
            .unwrap();
        let section_before = app
            .editor
            .native_drawing(section)
            .unwrap()
            .provider_lines(app.editor.native_section_context(section).unwrap())
            .unwrap()
            .to_vec();
        let plan_before = app
            .editor
            .native_wall_plan(view)
            .unwrap()
            .provider_lines(app.editor.native_plan_context(view).unwrap())
            .unwrap()
            .to_vec();
        app.plans.active = Some(view);
        app.select(Some(id));
        let baseline = app.editor.document.model().clone();
        let scene = app.editor.scene.clone();
        let history = app.editor.document.history_stats();
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        app.begin_opening(None);
        assert!(!app.opening_draft.as_ref().unwrap().override_sill);
        click(&mut app, &ctx, size, scale, "Override sill");
        assert!(app.opening_draft.as_ref().unwrap().override_sill);
        app.opening_draft.as_mut().unwrap().values[3] = "1.3".into();
        let draft = app.opening_draft.as_ref().unwrap();
        draft.preview(&app.editor, Some(view), Some(id)).unwrap();
        assert_eq!(app.editor.document.model(), &baseline);
        assert_eq!(app.editor.document.history_stats(), history);
        assert_eq!(app.editor.scene, scene);
        click(&mut app, &ctx, size, scale, "Cancel opening");
        assert!(app.opening_draft.is_none());
        assert_eq!(app.editor.document.model(), &baseline);
        assert_eq!(app.editor.document.history_stats(), history);
        app.begin_opening(None);
        click(&mut app, &ctx, size, scale, "Override sill");
        for invalid in ["NaN", "-1", "0.0005", "2.9", "not a number"] {
            app.opening_draft.as_mut().unwrap().values[3] = invalid.into();
            click(&mut app, &ctx, size, scale, "Apply opening");
            assert!(app.opening_draft.is_some());
            assert_eq!(app.editor.document.model(), &baseline);
            assert_eq!(app.editor.document.history_stats(), history);
        }
        // Disabled stale invalid text must not prevent inheriting the default.
        click(&mut app, &ctx, size, scale, "Use type default");
        let draft = app.opening_draft.as_ref().unwrap();
        assert_eq!(draft.parameters().unwrap().sill_override, None);
        draft.preview(&app.editor, Some(view), Some(id)).unwrap();
        click(&mut app, &ctx, size, scale, "Override sill");
        app.opening_draft.as_mut().unwrap().values[3] = "1.3".into();
        click(&mut app, &ctx, size, scale, "Apply opening");
        assert!(app.opening_draft.is_none());
        let changed = app.editor.document.model().clone();
        let changed_scene = app.editor.scene.clone();
        assert_eq!(changed.openings[&id].parameters.sill_override, Some(1.3));
        assert_eq!(changed.opening_types, baseline.opening_types);
        assert_eq!(changed.openings[&id].header, baseline.openings[&id].header);
        assert_eq!(
            app.editor.document.history_stats().undo_entries,
            history.undo_entries + 1
        );
        assert_ne!(changed_scene[&host], scene[&host]);
        assert_ne!(changed_scene[&id], scene[&id]);
        let section_after = app
            .editor
            .native_drawing(section)
            .unwrap()
            .provider_lines(app.editor.native_section_context(section).unwrap())
            .unwrap()
            .to_vec();
        let plan_after = app
            .editor
            .native_wall_plan(view)
            .unwrap()
            .provider_lines(app.editor.native_plan_context(view).unwrap())
            .unwrap()
            .to_vec();
        assert_ne!(section_before, section_after);
        assert_ne!(plan_before, plan_after);
        let elevation = changed.levels[&level].parameters.elevation;
        let bottom = changed_scene[&id]
            .vertices
            .iter()
            .map(|v| v.z)
            .fold(f64::INFINITY, f64::min);
        assert!((bottom - elevation - 1.3).abs() < 1e-10);
        let before_volume = os_geometry::walls::NativeWall::from_model(&baseline, host)
            .unwrap()
            .net_volume()
            .unwrap();
        let after_volume = os_geometry::walls::NativeWall::from_model(&changed, host)
            .unwrap()
            .net_volume()
            .unwrap();
        assert!((before_volume - after_volume).abs() < 1e-10);
        assert!((after_volume - changed_scene[&host].signed_volume()).abs() < 1e-10);
        app.editor.undo().unwrap();
        assert_eq!(app.editor.document.model(), &baseline);
        assert_eq!(app.editor.scene, scene);
        app.editor.redo().unwrap();
        assert_eq!(app.editor.document.model(), &changed);
        assert_eq!(app.editor.scene, changed_scene);
        app.begin_opening(None);
        click(&mut app, &ctx, size, scale, "Reset sill to type default");
        click(&mut app, &ctx, size, scale, "Apply opening");
        assert_eq!(app.editor.document.model(), &baseline);
        assert_eq!(app.editor.scene, scene);
        app.begin_opening(None);
        app.opening_draft.as_mut().unwrap().override_sill = true;
        app.opening_draft.as_mut().unwrap().values[3] = "1.0".into();
        frame(
            &mut app,
            &ctx,
            size,
            scale,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(app.opening_draft.is_none());
        assert_eq!(app.editor.document.model(), &baseline);
        for cause in 0..5 {
            app.plans.active = Some(view);
            app.select(Some(id));
            app.begin_opening(None);
            match cause {
                0 => app.selected = Some(host),
                1 => app.plans.active = None,
                2 => app.opening_draft.as_mut().unwrap().activation = Some(Id::new()),
                3 => app
                    .editor
                    .command("Change revision", Command::RenameProject("Changed".into()))
                    .unwrap(),
                _ => {
                    app.editor.document =
                        Document::from_model(app.editor.document.model().clone()).unwrap()
                }
            }
            let before = app.editor.document.model().clone();
            let stats = app.editor.document.history_stats();
            frame(&mut app, &ctx, size, scale, vec![]);
            assert!(app.opening_draft.is_none());
            assert_eq!(app.editor.document.model(), &before);
            assert_eq!(app.editor.document.history_stats(), stats);
        }
    }
}
