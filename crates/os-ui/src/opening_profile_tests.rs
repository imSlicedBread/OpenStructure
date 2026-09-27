use super::*;
use os_core::Point2;
use os_model::{DoorHinge, DoorSwing, Opening, OpeningParams, Wall};

fn concave() -> Vec<Point2> {
    [
        (0., 0.),
        (1., 0.),
        (1., 1.),
        (0.7, 1.),
        (0.7, 0.3),
        (0.3, 0.3),
        (0.3, 1.),
        (0., 1.),
    ]
    .map(|(x, y)| Point2::new(x, y))
    .to_vec()
}

#[test]
fn material_color_shared_wall_opening_render_preview_geometry_and_history() {
    use os_model::{
        LayerFunction, Material, MaterialParams, WallLayer, WallType, WallTypeAssignment,
        WallTypeParams,
    };
    use os_render::{Camera, RasterFrame, RasterStyle, Scene};
    let (mut e, host, view, types, ids) = fixture();
    let materials = [[220, 30, 20], [20, 200, 30]].map(|color| {
        Material::new(
            "core.material",
            MaterialParams {
                name: "Shared".into(),
                density_kg_m3: 650.,
                color,
            },
        )
    });
    let mids = materials.each_ref().map(|m| m.id());
    let wall_type = WallType::new(
        "core.wall_type",
        WallTypeParams {
            name: "Colored layers".into(),
            layers: mids
                .map(|id| WallLayer {
                    id: Id::new(),
                    name: "Layer".into(),
                    thickness: 0.1,
                    function: LayerFunction::Finish,
                    material: Some(id),
                })
                .to_vec(),
        },
    );
    let wall_type_id = wall_type.id();
    let mut commands: Vec<_> = materials.into_iter().map(Command::AddMaterial).collect();
    commands.push(Command::AddWallType(wall_type));
    commands.push(Command::AssignWallType {
        wall: host,
        assignment: Some(WallTypeAssignment {
            type_id: wall_type_id,
            flipped: false,
        }),
    });
    e.document
        .execute("Assign shared colors", commands)
        .unwrap();
    for ty in types {
        let mut draft = OpeningTypeDraft::edit(&e, ty).unwrap();
        draft.family.frame_width = 0.05;
        draft.family.panel_material = Some(mids[0]);
        draft.family.frame_material = Some(mids[1]);
        draft.apply(&mut e).unwrap();
    }
    let scene = e.scene.clone();
    let quantities = os_geometry::walls::NativeWall::from_model(e.document.model(), host)
        .unwrap()
        .layer_quantities()
        .unwrap();
    for mid in mids {
        let before = e.document.model().clone();
        let stats = e.document.history_stats();
        let color = [15, 40, 235];
        let mut p = before.materials[&mid].parameters.clone();
        p.color = color;
        e.command(
            "Edit shared material color",
            Command::UpdateMaterial {
                id: mid,
                parameters: p,
            },
        )
        .unwrap();
        let after = e.document.model().clone();
        assert_eq!(e.scene, scene, "mesh and surface IDs must not change");
        let mut restored = after.clone();
        restored.materials = before.materials.clone();
        assert_eq!(restored, before, "only the material changes");
        assert_eq!(
            os_geometry::walls::NativeWall::from_model(&after, host)
                .unwrap()
                .layer_quantities()
                .unwrap(),
            quantities
        );
        assert_eq!(
            e.document.history_stats().undo_entries,
            stats.undo_entries + 1
        );
        let colors = |model: &Model| {
            model
                .materials
                .iter()
                .map(|(id, m)| (*id, m.parameters.color))
                .collect()
        };
        for id in std::iter::once(host).chain(ids) {
            let isolated = Scene::from([(id, scene[&id].clone())]);
            let mut camera = Camera::default();
            camera.fit(&isolated, 320., 240.);
            let mut triangles = os_render::project_scene(&isolated, &camera, 320., 240.);
            for triangle in &mut triangles {
                triangle.shade = 1.;
            }
            let render = |model: &Model, selected| {
                RasterFrame::render_with_materials(
                    &triangles,
                    [320., 240.],
                    1.,
                    selected,
                    RasterStyle::default(),
                    &colors(model),
                )
                .unwrap()
            };
            let old = render(&before, None);
            let new = render(&after, None);
            assert_ne!(old.rgba, new.rgba, "assigned object {id} must recolor");
            assert!(new.rgba.contains(&[15, 40, 235, 255]));
            assert_eq!(
                render(&before, Some(id)).rgba,
                render(&after, Some(id)).rgba,
                "selection tint wins over color"
            );
            for y in 0..new.height {
                for x in 0..new.width {
                    assert_eq!(old.depth_at(x, y), new.depth_at(x, y));
                    assert_eq!(
                        old.pick(x as f32 + 0.5, y as f32 + 0.5),
                        new.pick(x as f32 + 0.5, y as f32 + 0.5)
                    );
                }
            }
        }
        for ty in types {
            let mut draft = OpeningTypeDraft::edit(&e, ty).unwrap();
            let (preview, _) = draft.preview_geometry(&e, Some(view)).unwrap();
            assert!(preview.surfaces.iter().any(|s| s.material == Some(mid)));
            let ctx = egui::Context::default();
            let output = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default()
                    .show(ctx, |ui| draft.profile_editor(ui, &e, Some(view)));
            });
            assert!(output.shapes.iter().any(|s| matches!(&s.shape,
                egui::Shape::LineSegment { stroke, .. } if stroke.color == egui::Color32::from_rgb(15, 40, 235))),
                "native family preview draws the material color");
        }
        e.undo().unwrap();
        assert_eq!(e.document.model(), &before);
        assert_eq!(e.scene, scene);
        e.redo().unwrap();
        assert_eq!(e.document.model(), &after);
        assert_eq!(e.scene, scene);
    }
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("shared-color.osb");
    e.save(&path).unwrap();
    let mut reopened = Editor::new().unwrap();
    reopened.open(&path).unwrap();
    assert_eq!(reopened.document.model(), e.document.model());
    assert_eq!(reopened.scene, e.scene);
}

#[test]
fn opening_materials_shared_types_preview_failure_history_and_reopen() {
    let (mut e, host, view, types, ids) = fixture();
    let materials = ["Panel", "Frame"].map(|name| {
        os_model::Material::new(
            "core.material",
            os_model::MaterialParams {
                name: name.into(),
                density_kg_m3: 500.,
                color: [180, 180, 180],
            },
        )
    });
    let [panel, frame] = materials.each_ref().map(|m| m.id());
    e.document
        .execute(
            "Materials",
            materials.into_iter().map(Command::AddMaterial).collect(),
        )
        .unwrap();
    for id in ids {
        let mut p = e.document.model().openings[&id].parameters.clone();
        p.width_override = Some(0.95);
        p.height_override = Some(1.1);
        if e.document.model().resolve_opening(&p).unwrap().kind == OpeningKind::Window {
            p.sill_override = Some(0.7);
        }
        e.command("Pin instance", Command::UpdateOpening { id, parameters: p })
            .unwrap();
    }
    for type_id in types {
        // Establish frames first so the material edit can prove unchanged geometry.
        let mut draft = OpeningTypeDraft::edit(&e, type_id).unwrap();
        draft.family.frame_width = 0.05;
        draft.apply(&mut e).unwrap();
        let before = e.document.model().clone();
        let scene = e.scene.clone();
        let history = e.document.history_stats();
        let mut draft = OpeningTypeDraft::edit(&e, type_id).unwrap();
        draft.family.panel_material = Some(panel);
        draft.family.frame_material = Some(frame);
        let preview = draft.preview_model(&e).unwrap();
        let (mesh, _) = draft.preview_geometry(&e, Some(view)).unwrap();
        assert!(mesh.surfaces.iter().any(|s| s.material == Some(panel)));
        assert!(mesh.surfaces.iter().any(|s| s.material == Some(frame)));
        assert_eq!(e.document.model(), &before);
        assert_eq!(e.document.history_stats(), history);
        assert_eq!(e.scene, scene);
        draft.family.frame_material = Some(Id::new());
        assert!(draft.preview_model(&e).is_err());
        assert!(draft.apply(&mut e).is_err());
        assert_eq!(e.document.model(), &before);
        assert_eq!(e.document.history_stats(), history);
        assert_eq!(e.scene, scene);
        draft.family.frame_material = Some(frame);
        draft.apply(&mut e).unwrap();
        assert_eq!(e.document.model(), &preview);
        assert_eq!(e.document.model().openings, before.openings);
        assert_eq!(e.scene[&host], scene[&host]);
        for id in ids {
            let current = &e.scene[&id];
            assert_eq!(current.vertices, scene[&id].vertices);
            assert_eq!(current.triangles, scene[&id].triangles);
            if before.openings[&id].parameters.type_id() == Some(type_id) {
                assert!(current.surfaces.iter().any(|s| s.material == Some(panel)));
                assert!(current.surfaces.iter().any(|s| s.material == Some(frame)));
                assert_eq!(current.surfaces.len(), current.triangles.len());
            }
        }
        let committed = e.scene.clone();
        assert_eq!(
            e.document.history_stats().undo_entries,
            history.undo_entries + 1
        );
        e.undo().unwrap();
        assert_eq!(e.document.model(), &before);
        assert_eq!(e.scene, scene);
        e.redo().unwrap();
        assert_eq!(e.document.model(), &preview);
        assert_eq!(e.scene, committed);
    }
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("opening-materials.osb");
    e.save(&path).unwrap();
    let mut reopened = Editor::new().unwrap();
    reopened.open(&path).unwrap();
    assert_eq!(reopened.document.model(), e.document.model());
    assert_eq!(reopened.scene, e.scene);
}

#[test]
fn authored_host_cut_preview_atomic_rejection_history_stale_and_reopen() {
    let (mut e, host, view, types, _) = fixture();
    let original = e.document.model().clone();
    let original_mesh = e.scene[&host].clone();
    let stats = e.document.history_stats();
    let mut draft = OpeningTypeDraft::edit(&e, types[1]).unwrap();
    draft.family.cut_profile = vec![
        Point2::new(0., 0.),
        Point2::new(1., 0.),
        Point2::new(0.5, 1.),
    ];
    draft.family.host_cut = os_model::OpeningHostCut::Profile;
    assert!(
        draft.apply(&mut e).is_err(),
        "rectangular pane cannot protrude into triangular host"
    );
    assert_eq!(e.document.model(), &original);
    assert_eq!(e.document.history_stats(), stats);
    draft.family.profile = draft.family.cut_profile.clone();
    draft.family.frame_width = 0.05;
    assert!(
        draft.apply(&mut e).is_err(),
        "rectangular rails cannot silently collide"
    );
    assert_eq!(e.document.model(), &original);
    draft.family.frame_width = 0.;
    let preview = draft.preview_model(&e).unwrap();
    draft.preview_geometry(&e, Some(view)).unwrap();
    assert_eq!(e.document.model(), &original);
    assert_eq!(e.scene[&host], original_mesh);
    draft.apply(&mut e).unwrap();
    assert_eq!(e.document.model(), &preview);
    assert_ne!(e.scene[&host], original_mesh);
    assert_eq!(
        e.document.history_stats().undo_entries,
        stats.undo_entries + 1
    );
    assert!(draft.apply(&mut e).is_err());
    let committed = e.scene[&host].clone();
    e.undo().unwrap();
    assert_eq!(e.document.model(), &original);
    assert_eq!(e.scene[&host], original_mesh);
    e.redo().unwrap();
    assert_eq!(e.scene[&host], committed);
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("shaped-host.osb");
    e.save(&path).unwrap();
    let mut reopened = Editor::new().unwrap();
    reopened.open(&path).unwrap();
    assert_eq!(reopened.document.model(), e.document.model());
    assert_eq!(reopened.scene, e.scene);
    let context = reopened.native_plan_context(view).unwrap();
    assert_eq!(
        reopened
            .native_wall_plan(view)
            .unwrap()
            .items(context)
            .unwrap(),
        e.native_wall_plan(view)
            .unwrap()
            .items(e.native_plan_context(view).unwrap())
            .unwrap()
    );
}

fn fixture() -> (Editor, Id, Id, [Id; 2], [Id; 4]) {
    let mut e = Editor::new().unwrap();
    let level = *e.document.model().levels.keys().next().unwrap();
    let view = e.create_floor_plan("Family plan", level).unwrap();
    let wall = Wall::new(
        os_walls::WALL_TYPE,
        WallParams {
            name: "Family host".into(),
            start: Point2::new(2., 3.),
            end: Point2::new(2., 17.),
            thickness: 0.2,
            height: 3.,
            level,
            material: None,
        },
    );
    let host = wall.id();
    e.command("Host", Command::AddWall(wall)).unwrap();
    let mut types = Vec::new();
    for kind in [OpeningKind::Door, OpeningKind::Window] {
        let mut draft = OpeningTypeDraft::new(&e, kind, None, None);
        draft.values[0] = "1".into();
        if kind == OpeningKind::Window {
            draft.pane_position = WindowPanePosition::RightFace;
            draft.values[2] = "0.8".into();
        }
        types.push(draft.id);
        draft.apply(&mut e).unwrap();
    }
    let instances = [1., 4., 7., 10.].map(|offset| {
        let door = offset < 6.;
        Opening::new(
            "core.opening",
            OpeningParams {
                width_override: None,
                height_override: None,
                sill_override: None,
                name: "Shared instance".into(),
                host,
                offset,
                definition: OpeningDefinition::Typed {
                    type_id: types[usize::from(!door)],
                },
                hinge: if offset == 4. {
                    DoorHinge::End
                } else {
                    DoorHinge::Start
                },
                swing: if offset == 4. {
                    DoorSwing::Right
                } else {
                    DoorSwing::Left
                },
            },
        )
    });
    let ids = instances.each_ref().map(|o| o.id());
    e.document
        .execute(
            "Place four",
            instances.into_iter().map(Command::AddOpening).collect(),
        )
        .unwrap();
    e.regenerate().unwrap();
    (e, host, view, [types[0], types[1]], ids)
}

#[test]
fn opening_family_two_types_shared_instances_preview_history_and_archive() {
    let (mut e, host, view, types, ids) = fixture();
    for (index, type_id) in types.into_iter().enumerate() {
        let before = e.document.model().clone();
        let scene = e.scene.clone();
        let history = e.document.history_stats();
        let mut draft = OpeningTypeDraft::edit(&e, type_id).unwrap();
        draft.family.profile = concave();
        if index == 1 {
            draft.family.profile.reverse();
            draft.family.depth = 0.04;
        }
        let candidate = draft.preview_model(&e).unwrap();
        let (preview, lines) = draft.preview_geometry(&e, Some(view)).unwrap();
        let preview_id = lines[0].entity;
        assert_eq!(e.document.model(), &before);
        assert_eq!(e.scene, scene);
        assert_eq!(e.document.history_stats(), history);
        draft.apply(&mut e).unwrap();
        assert_eq!(e.document.model(), &candidate);
        assert_eq!(e.scene[&preview_id], preview);
        let context = e.native_plan_context(view).unwrap();
        let drawing = e.native_wall_plan(view).unwrap();
        let actual: Vec<_> = drawing
            .provider_lines(context)
            .unwrap()
            .iter()
            .filter(|l| l.entity == preview_id)
            .cloned()
            .collect();
        assert_eq!(actual, lines);
        assert_eq!(
            e.scene[&host], scene[&host],
            "positive profile never changes the rectangular host cut"
        );
        assert_eq!(e.document.model().openings, before.openings);
        assert_eq!(e.document.model().levels, before.levels);
        assert_eq!(
            e.document.model().opening_types[&types[1 - index]],
            before.opening_types[&types[1 - index]]
        );
        assert_eq!(
            e.document.history_stats().undo_entries,
            history.undo_entries + 1
        );
        for id in &ids[index * 2..index * 2 + 2] {
            let p = e
                .document
                .model()
                .resolve_opening(&e.document.model().openings[id].parameters)
                .unwrap();
            let wall = &e.document.model().walls[&host].parameters;
            assert!(
                (e.scene[id].signed_volume()
                    - 0.72 * p.width * p.height * os_geometry::openings::depth(&p, wall))
                .abs()
                    < 1e-10
            );
            // Every directed triangle edge has exactly one opposite: closed,
            // outward, manifold shells for concavity and both authored windings.
            let mut edges = std::collections::BTreeMap::new();
            for t in &e.scene[id].triangles {
                for j in 0..3 {
                    *edges.entry((t[j], t[(j + 1) % 3])).or_insert(0) += 1;
                }
            }
            for (&(a, b), &count) in &edges {
                assert_eq!(count, 1);
                assert_eq!(edges.get(&(b, a)), Some(&1));
            }
        }
        assert!(
            draft.preview_model(&e).is_err(),
            "applied draft becomes stale"
        );
        let after = e.document.model().clone();
        let after_scene = e.scene.clone();
        e.undo().unwrap();
        assert_eq!(e.document.model(), &before);
        assert_eq!(e.scene, scene);
        e.redo().unwrap();
        assert_eq!(e.document.model(), &after);
        assert_eq!(e.scene, after_scene);
    }
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("family.osb");
    e.save(&path).unwrap();
    let mut reopened = Editor::new().unwrap();
    reopened.open(&path).unwrap();
    assert_eq!(reopened.document.model(), e.document.model());
    assert_eq!(reopened.scene, e.scene);
    let context = reopened.native_plan_context(view).unwrap();
    assert_eq!(
        reopened
            .native_wall_plan(view)
            .unwrap()
            .provider_lines(context)
            .unwrap(),
        e.native_wall_plan(view)
            .unwrap()
            .provider_lines(e.native_plan_context(view).unwrap())
            .unwrap()
    );
}

#[test]
fn opening_family_invalid_profiles_are_atomic_in_preview_and_commands() {
    let (mut e, _, view, types, _) = fixture();
    let valid = e.document.model().clone();
    let scene = e.scene.clone();
    let stats = e.document.history_stats();
    let revision = e.document.revision();
    for case in 0..16 {
        let mut draft = OpeningTypeDraft::edit(&e, types[0]).unwrap();
        match case {
            0 => draft.family.profile[1].x = f64::NAN,
            1 => draft.family.profile[1].y = f64::INFINITY,
            2 => draft.family.profile[2].x = 1.1,
            3 => draft.family.profile[0].x = -0.1,
            4 => draft.family.profile.swap(1, 2),
            5 => draft.family.profile[1] = draft.family.profile[0],
            6 => draft.family.profile = vec![Point2::new(0., 0.); 33],
            7 => draft.family.profile.truncate(2),
            8 => {
                draft.family.profile = [(0., 0.), (1., 0.), (0.5, 0.), (0., 1.)]
                    .map(|(x, y)| Point2::new(x, y))
                    .to_vec()
            }
            9 => {
                draft.family.profile = [(0., 0.), (0.001, 0.), (0., 0.001)]
                    .map(|(x, y)| Point2::new(x, y))
                    .to_vec()
            }
            10 => {
                draft.family.profile = [(0., 0.), (1., 0.), (1., 1.), (0.5, 0.), (0., 1.)]
                    .map(|(x, y)| Point2::new(x, y))
                    .to_vec()
            }
            11 => draft.family.version = 5,
            12 => draft.family.depth = f64::NAN,
            13 => draft.family.frame_width = 0.0005,
            14 => draft.family.frame_depth = f64::NAN,
            _ => draft.values[0] = "20".into(), // invalidates another placement/host
        }
        assert!(draft.preview_geometry(&e, Some(view)).is_err(), "{case}");
        assert!(draft.apply(&mut e).is_err(), "{case}");
        assert!(
            e.command(
                "Invalid API edit",
                Command::UpdateOpeningType {
                    id: types[0],
                    parameters: draft.parameters().unwrap()
                }
            )
            .is_err(),
            "{case}"
        );
        assert_eq!(e.document.model(), &valid);
        assert_eq!(e.scene, scene);
        assert_eq!(e.document.history_stats(), stats);
        assert_eq!(e.document.revision(), revision);
    }
    let mut canceled = OpeningTypeDraft::edit(&e, types[0]).unwrap();
    canceled.family.profile = concave();
    canceled.preview_geometry(&e, Some(view)).unwrap();
    drop(canceled);
    assert_eq!(e.document.model(), &valid);
    assert_eq!(e.document.history_stats(), stats);
    let stale = OpeningTypeDraft::edit(&e, types[0]).unwrap();
    e.document = Document::from_model(valid.clone()).unwrap();
    assert!(stale.preview_model(&e).is_err());
    assert!(stale.apply(&mut e).is_err());
    assert_eq!(e.document.model(), &valid);
}

#[test]
fn opening_family_plan_cut_matches_mesh_caps_and_rectangular_aperture() {
    let (mut e, host, view, types, ids) = fixture();
    for ty in types {
        let mut d = OpeningTypeDraft::edit(&e, ty).unwrap();
        d.family.profile = concave();
        d.apply(&mut e).unwrap();
    }
    let model = e.document.model();
    let wall = &model.walls[&host].parameters;
    let context = e.native_plan_context(view).unwrap();
    let drawing = e.native_wall_plan(view).unwrap();
    let lines = drawing.provider_lines(context).unwrap();
    for id in ids {
        let p = model
            .resolve_opening(&model.openings[&id].parameters)
            .unwrap();
        let mesh = &e.scene[&id];
        let n = p.family.profile.len() as u32;
        let a = os_geometry::openings::component_point(&p, wall, 0.);
        let b = os_geometry::openings::component_point(&p, wall, 1.);
        let a = os_geometry::openings::world(wall, a.x, a.y);
        let b = os_geometry::openings::world(wall, b.x, b.y);
        let (dx, dy) = ((b.x - a.x) / p.width, (b.y - a.y) / p.width);
        let uv = |v: os_geometry::Vec3| {
            Point2::new(
                ((v.x - a.x) * dx + (v.y - a.y) * dy) / p.width,
                (v.z - p.sill) / p.height,
            )
        };
        // Independently intersect the actual triangulated cap via point-in-triangle.
        let cut = (context.range.cut - p.sill) / p.height;
        let cross =
            |a: Point2, b: Point2, c: Point2| (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
        for i in 1..100 {
            let u = i as f64 / 100.;
            if (u - 0.3).abs() < 1e-8 || (u - 0.7).abs() < 1e-8 {
                continue;
            }
            let q = Point2::new(u, cut);
            let in_mesh = mesh
                .triangles
                .iter()
                .filter(|t| t.iter().all(|i| *i < n))
                .any(|t| {
                    let [a, b, c] = t.map(|i| uv(mesh.vertices[i as usize]));
                    let v = [cross(a, b, q), cross(b, c, q), cross(c, a, q)];
                    v.iter().all(|v| *v >= -1e-10) || v.iter().all(|v| *v <= 1e-10)
                });
            let pos = a; // center line of the extruded component in world plan
            let component_lines: Vec<_> = lines
                .iter()
                .filter(|l| {
                    l.entity == id
                        && if p.kind == OpeningKind::Door {
                            l.feature == 2 || l.feature == 3
                        } else {
                            l.feature == 4 || l.feature == 5
                        }
                })
                .collect();
            let in_plan = component_lines.iter().any(|l| {
                let s = ((l.start.x - pos.x) * dx + (l.start.y - pos.y) * dy) / p.width;
                let t = ((l.end.x - pos.x) * dx + (l.end.y - pos.y) * dy) / p.width;
                u >= s.min(t) - 1e-10 && u <= s.max(t) + 1e-10
            });
            assert_eq!(in_mesh, in_plan, "{id}: u={u}");
            assert_eq!(in_mesh, !(0.3..=0.7).contains(&u));
        }
    }
    assert!(
        (e.scene[&host].signed_volume() - (14. * 3. * 0.2 - 2. * 2.1 * 0.2 - 2. * 1.2 * 0.2)).abs()
            < 1e-10
    );
}

#[test]
fn opening_family_frames_regenerate_shared_instances_plan_and_archive() {
    let (mut e, host, view, types, ids) = fixture();
    let initial_context = e.native_plan_context(view).unwrap();
    let initial_drawing = e.native_wall_plan(view).unwrap();
    let initial_lines = initial_drawing
        .provider_lines(initial_context)
        .unwrap()
        .to_vec();
    let host_mesh = e.scene[&host].clone();

    for (index, type_id) in types.into_iter().enumerate() {
        let before = e.document.model().clone();
        let before_scene = e.scene.clone();
        let before_history = e.document.history_stats();
        let mut draft = OpeningTypeDraft::edit(&e, type_id).unwrap();
        draft.family.frame_width = if index == 0 { 0.05 } else { 0.04 };
        draft.family.frame_depth = if index == 0 { 0.08 } else { 0.06 };
        let candidate = draft.preview_model(&e).unwrap();
        let (preview_mesh, preview_lines) = draft.preview_geometry(&e, Some(view)).unwrap();
        let preview_id = preview_lines[0].entity;
        assert_eq!(e.document.model(), &before);
        assert_eq!(e.scene, before_scene);
        assert_eq!(e.document.history_stats(), before_history);

        draft.apply(&mut e).unwrap();
        assert_eq!(e.document.model(), &candidate);
        assert_eq!(e.scene[&preview_id], preview_mesh);
        assert_eq!(e.scene[&host], host_mesh);
        let assigned = if index == 0 { &ids[..2] } else { &ids[2..] };
        let unaffected = if index == 0 { &ids[2..] } else { &ids[..2] };
        assert!(assigned.iter().all(|id| e.scene[id] != before_scene[id]));
        assert!(unaffected.iter().all(|id| e.scene[id] == before_scene[id]));

        let context = e.native_plan_context(view).unwrap();
        let drawing = e.native_wall_plan(view).unwrap();
        let actual: Vec<_> = drawing
            .provider_lines(context)
            .unwrap()
            .iter()
            .filter(|line| line.entity == preview_id)
            .cloned()
            .collect();
        assert_eq!(actual, preview_lines);
        let old_count = initial_lines
            .iter()
            .filter(|line| line.entity == preview_id)
            .count();
        assert_eq!(actual.len(), old_count + 8, "two four-edge frame jambs");
        assert_eq!(
            e.document.history_stats().undo_entries,
            before_history.undo_entries + 1
        );
        e.undo().unwrap();
        assert_eq!(e.document.model(), &before);
        assert_eq!(e.scene, before_scene);
        e.redo().unwrap();
        assert_eq!(e.document.model(), &candidate);
    }

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("framed-openings.osb");
    e.save(&path).unwrap();
    let mut reopened = Editor::new().unwrap();
    reopened.open(&path).unwrap();
    assert_eq!(reopened.document.model(), e.document.model());
    assert_eq!(reopened.scene, e.scene);
}
