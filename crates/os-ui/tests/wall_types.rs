use os_core::{Id, Point2};
use os_document::Command;
use os_geometry::{GeometryKernel, PrismKernel, Vec3, walls::NativeWall};
use os_model::*;
use os_ui::Editor;
use std::collections::BTreeSet;

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-8, "{a} != {b}");
}
fn fixture() -> (Editor, Id, [Id; 2], WallType, [Id; 2]) {
    let mut e = Editor::new().unwrap();
    let level = *e.document.model().levels.keys().next().unwrap();
    let plan = e.create_floor_plan("Layers", level).unwrap();
    let materials = [
        Material::new(
            "core.material",
            MaterialParams {
                name: "Masonry".into(),
                density_kg_m3: 1800.0,
            },
        ),
        Material::new(
            "core.material",
            MaterialParams {
                name: "Insulation".into(),
                density_kg_m3: 40.0,
            },
        ),
    ];
    let mids = materials.each_ref().map(|m| m.id());
    let ty = WallType::new(
        "core.wall_type",
        WallTypeParams {
            name: "Compound".into(),
            layers: vec![
                WallLayer {
                    id: Id::new(),
                    name: "Core".into(),
                    thickness: 0.2,
                    function: LayerFunction::Structure,
                    material: Some(mids[0]),
                },
                WallLayer {
                    id: Id::new(),
                    name: "Insulation".into(),
                    thickness: 0.1,
                    function: LayerFunction::Insulation,
                    material: Some(mids[1]),
                },
            ],
        },
    );
    let walls = [0., 3.].map(|y| {
        Wall::new(
            os_walls::WALL_TYPE,
            WallParams {
                name: format!("Wall {y}"),
                start: Point2::new(0., y),
                end: Point2::new(4., y),
                thickness: 0.12,
                height: 3.,
                level,
                material: None,
            },
        )
    });
    let ids = walls.each_ref().map(|w| w.id());
    let mut commands: Vec<_> = materials.into_iter().map(Command::AddMaterial).collect();
    commands.push(Command::AddWallType(ty.clone()));
    commands.extend(walls.into_iter().map(Command::AddWall));
    commands.extend(ids.map(|wall| Command::AssignWallType {
        wall,
        assignment: Some(WallTypeAssignment {
            type_id: ty.id(),
            flipped: false,
        }),
    }));
    e.document
        .execute("Shared compound walls", commands)
        .unwrap();
    e.regenerate().unwrap();
    (e, plan, ids, ty, mids)
}
fn door(host: Id, offset: f64) -> Opening {
    Opening::new(
        "core.opening",
        OpeningParams {
            name: "Door".into(),
            host,
            offset,
            definition: OpeningDefinition::Legacy {
                kind: OpeningKind::Door,
                width: 1.,
                height: 2.,
                sill: 0.,
            },
            hinge: Default::default(),
            swing: Default::default(),
        },
    )
}

#[test]
fn shared_edit_reorder_flip_material_density_history_and_reopen() {
    let (mut e, plan, ids, ty, mids) = fixture();
    let original = e.document.model().clone();
    let mut p = ty.parameters.clone();
    p.layers[0].thickness = 0.3;
    p.layers.swap(0, 1);
    e.document
        .execute(
            "Shared type edit",
            vec![Command::UpdateWallType {
                id: ty.id(),
                parameters: p.clone(),
            }],
        )
        .unwrap();
    let events = e.document.drain_events();
    assert_eq!(events.len(), 1);
    for id in ids.into_iter().chain([plan]) {
        assert!(events[0].invalidated.contains(&id));
    }
    for wall in ids {
        let r = e.document.model().resolve_wall(wall).unwrap();
        close(r.parameters.thickness, 0.4);
        assert_eq!(r.layers[0].id, Some(ty.parameters.layers[1].id));
        assert_eq!(r.layers[0].material, Some(mids[1]));
        assert_eq!(e.document.model().walls[&wall], original.walls[&wall]);
    }
    e.command(
        "Flip",
        Command::AssignWallType {
            wall: ids[0],
            assignment: Some(WallTypeAssignment {
                type_id: ty.id(),
                flipped: true,
            }),
        },
    )
    .unwrap();
    let a = e.document.model().resolve_wall(ids[0]).unwrap();
    let b = e.document.model().resolve_wall(ids[1]).unwrap();
    for (a, b) in a.layers.iter().zip(&b.layers) {
        assert_eq!(a.id, b.id);
        close(a.min, -b.max);
        close(a.max, -b.min);
    }
    let before_density = NativeWall::from_model(e.document.model(), ids[0])
        .unwrap()
        .layer_quantities()
        .unwrap();
    e.command(
        "Density",
        Command::UpdateMaterial {
            id: mids[0],
            parameters: MaterialParams {
                name: "Masonry".into(),
                density_kg_m3: 2000.,
            },
        },
    )
    .unwrap();
    let quantities = NativeWall::from_model(e.document.model(), ids[0])
        .unwrap()
        .layer_quantities()
        .unwrap();
    close(
        quantities[1].mass_kg.unwrap(),
        quantities[1].volume_m3 * 2000.,
    );
    assert_ne!(quantities[1].mass_kg, before_density[1].mass_kg);
    let final_model = e.document.model().clone();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("layers.osb");
    e.save(&path).unwrap();
    e.undo().unwrap();
    e.redo().unwrap();
    assert_eq!(e.document.model(), &final_model);
    e.open(&path).unwrap();
    assert_eq!(e.document.model(), &final_model);
    for id in ids {
        assert!(!e.scene[&id].surfaces.is_empty());
    }
}

#[test]
fn openings_cut_every_layer_and_plan_section_mesh_quantities_reconcile() {
    let (mut e, plan, ids, ty, mids) = fixture();
    e.command("Door", Command::AddOpening(door(ids[0], 1.)))
        .unwrap();
    let wall = NativeWall::from_model(e.document.model(), ids[0]).unwrap();
    let quantities = wall.layer_quantities().unwrap();
    close(quantities[0].volume_m3, 2.);
    close(quantities[1].volume_m3, 1.);
    close(
        quantities.iter().map(|q| q.volume_m3).sum(),
        wall.net_volume().unwrap(),
    );
    close(quantities.iter().map(|q| q.mass_kg.unwrap()).sum(), 3640.);
    close(e.scene[&ids[0]].signed_volume(), 3.);
    for (layer, cells) in wall.layer_cells().unwrap() {
        close(
            cells
                .iter()
                .map(|c| PrismKernel.tessellate(c).unwrap().signed_volume())
                .sum(),
            10. * (layer.max - layer.min),
        );
        assert!(cells.iter().all(|c| !(c.profile.vertices[0].x < 1.5
            && c.profile.vertices[1].x > 1.5
            && c.transform.translation.z < 1.
            && c.transform.translation.z + c.height > 1.)));
    }
    let ctx = e.native_plan_context(plan).unwrap();
    let drawing = e.native_wall_plan(plan).unwrap();
    let items: Vec<_> = drawing
        .items(ctx)
        .unwrap()
        .iter()
        .filter(|i| i.entity == ids[0])
        .collect();
    close(items.iter().map(|i| i.footprint.area()).sum(), 0.9);
    assert_eq!(
        items
            .iter()
            .filter_map(|i| i.surface.material)
            .collect::<BTreeSet<_>>(),
        mids.into()
    );
    for y in [-0.1, 0.1] {
        assert_eq!(
            drawing.pick(ctx, Point2::new(0.5, y)).unwrap(),
            Some(ids[0])
        );
    }
    assert_ne!(
        drawing.pick(ctx, Point2::new(1.5, 0.)).unwrap(),
        Some(ids[0])
    );
    let level = e.document.model().walls[&ids[0]].parameters.level;
    let section = e
        .create_section_view(
            "Cross section",
            level,
            SectionViewSettings::new(Point2::new(0.5, -1.), Point2::new(0.5, 1.), 0., 4.),
        )
        .unwrap();
    let ctx = e.native_section_context(section).unwrap();
    let drawing = e.native_drawing(section).unwrap();
    let lines = drawing.provider_lines(ctx).unwrap();
    let surface_ids: BTreeSet<_> = lines
        .iter()
        .filter_map(|l| drawing.line_surface(l.entity, l.feature).layer)
        .collect();
    assert_eq!(
        surface_ids,
        ty.parameters.layers.iter().map(|l| l.id).collect()
    );
    assert!(lines.iter().all(|l| l.entity == ids[0]));
    // A cross section through the aperture contains only the 1m-high lintel.
    let aperture = e
        .create_section_view(
            "Through door",
            level,
            SectionViewSettings::new(Point2::new(1.5, -1.), Point2::new(1.5, 1.), 0., 4.),
        )
        .unwrap();
    let ctx = e.native_section_context(aperture).unwrap();
    let drawing = e.native_drawing(aperture).unwrap();
    assert!(
        drawing
            .provider_lines(ctx)
            .unwrap()
            .iter()
            .all(|l| l.start.y >= 2. - 1e-8 && l.end.y >= 2. - 1e-8)
    );
    let scene = std::collections::BTreeMap::from([(ids[0], e.scene[&ids[0]].clone())]);
    let camera = os_render::Camera {
        yaw: 0.,
        pitch: 0.55,
        target: Vec3::new(2., 0., 1.5),
        pixels_per_metre: 100.,
    };
    let triangles = os_render::project_scene(&scene, &camera, 800., 600.);
    assert!(triangles.iter().all(|t| t.entity == ids[0]));
    assert_eq!(
        triangles
            .iter()
            .filter_map(|t| t.surface.layer)
            .collect::<BTreeSet<_>>(),
        ty.parameters.layers.iter().map(|l| l.id).collect()
    );
    let frame =
        os_render::RasterFrame::render(&triangles, [800., 600.], 1., None, Default::default())
            .unwrap();
    let at = camera.project(Vec3::new(0.5, 0.15, 1.5), 800., 600.);
    assert_eq!(frame.pick(at.x, at.y), Some(ids[0]));
}

#[test]
fn invalid_type_material_assignment_and_deletion_are_atomic() {
    let (mut e, _, ids, ty, mids) = fixture();
    let mut cases = vec![
        Command::RemoveWallType(ty.id()),
        Command::RemoveMaterial(mids[0]),
        Command::AssignWallType {
            wall: ids[0],
            assignment: Some(WallTypeAssignment {
                type_id: Id::new(),
                flipped: false,
            }),
        },
    ];
    for case in 0..7 {
        let mut p = ty.parameters.clone();
        match case {
            0 => p.layers.clear(),
            1 => p.layers[0].thickness = 0.,
            2 => p.layers[0].thickness = f64::NAN,
            3 => p.layers[0].material = Some(Id::new()),
            4 => p.layers[1].id = p.layers[0].id,
            5 => p.layers[0].id = ids[0],
            _ => p.layers = vec![p.layers[0].clone(); 33],
        }
        cases.push(Command::UpdateWallType {
            id: ty.id(),
            parameters: p,
        });
    }
    for command in cases {
        let before = e.document.model().clone();
        let rev = e.document.revision();
        let history = e.document.history_stats();
        assert!(
            e.document
                .execute(
                    "Invalid",
                    vec![Command::RenameProject("must rollback".into()), command]
                )
                .is_err()
        );
        assert_eq!(e.document.model(), &before);
        assert_eq!(e.document.revision(), rev);
        assert_eq!(e.document.history_stats(), history);
        assert!(e.document.drain_events().is_empty());
    }
    e.document
        .execute(
            "Detach and delete",
            ids.map(|wall| Command::AssignWallType {
                wall,
                assignment: None,
            })
            .into_iter()
            .chain([Command::RemoveWallType(ty.id())])
            .collect(),
        )
        .unwrap();
    for id in ids {
        close(
            e.document
                .model()
                .resolve_wall(id)
                .unwrap()
                .parameters
                .thickness,
            0.12,
        );
    }
    assert!(e.document.undo());
    assert!(e.document.model().wall_types.contains_key(&ty.id()));
}

#[test]
fn compound_butt_corner_tee_contact_clearance_and_atomic_type_propagation() {
    for kind in 0..3 {
        let (mut e, plan, ids, ty, _) = fixture();
        let mut p = e.document.model().walls[&ids[1]].parameters.clone();
        p.start = Point2::new(if kind == 2 { 2. } else { 4. }, 0.);
        p.end = if kind == 0 {
            Point2::new(8., 0.)
        } else {
            Point2::new(p.start.x, 3.)
        };
        e.command(
            "Position",
            Command::UpdateWall {
                id: ids[1],
                parameters: p,
            },
        )
        .unwrap();
        let a = WallAnchor {
            wall: ids[0],
            endpoint: WallEndpoint::End,
        };
        let b = WallAnchor {
            wall: ids[1],
            endpoint: WallEndpoint::Start,
        };
        let join = match kind {
            0 => WallJoinParams::Butt { a, b },
            1 => WallJoinParams::Corner {
                a,
                b,
                owner: ids[0],
            },
            _ => WallJoinParams::Tee {
                host: ids[0],
                station: 2.,
                branch: b,
            },
        };
        e.join_walls(join).unwrap();
        let mut p = ty.parameters.clone();
        p.layers[0].thickness = 0.3;
        e.command(
            "Resize shared profile",
            Command::UpdateWallType {
                id: ty.id(),
                parameters: p,
            },
        )
        .unwrap();
        let ga = NativeWall::from_model(e.document.model(), ids[0]).unwrap();
        let gb = NativeWall::from_model(e.document.model(), ids[1]).unwrap();
        close(ga.parameters.thickness, 0.4);
        close(gb.parameters.thickness, 0.4);
        assert_eq!(ga.interfaces, gb.interfaces);
        let expected_area = match kind {
            0 => 3.2,
            1 => 2.8,
            _ => 2.72,
        };
        close(
            ga.net_volume().unwrap() + gb.net_volume().unwrap(),
            expected_area * 3.,
        );
        close(
            e.scene.values().map(|m| m.signed_volume()).sum(),
            expected_area * 3.,
        );
        let ctx = e.native_plan_context(plan).unwrap();
        let drawing = e.native_wall_plan(plan).unwrap();
        close(
            drawing
                .items(ctx)
                .unwrap()
                .iter()
                .map(|i| i.footprint.area())
                .sum(),
            expected_area,
        );
        assert!(
            drawing
                .items(ctx)
                .unwrap()
                .iter()
                .all(|i| i.surface.layer.is_some() && ids.contains(&i.entity))
        );
        // All constituent cells stay disjoint across the contact, and their union reaches it.
        let bounds = |s: &os_geometry::Solid| {
            let mesh = PrismKernel.tessellate(s).unwrap();
            let axis = |f: fn(&Vec3) -> f64| {
                (
                    mesh.vertices.iter().map(f).fold(f64::INFINITY, f64::min),
                    mesh.vertices
                        .iter()
                        .map(f)
                        .fold(f64::NEG_INFINITY, f64::max),
                )
            };
            (axis(|v| v.x), axis(|v| v.y))
        };
        let mut contacts = 0;
        for a in ga.cells().unwrap() {
            for b in gb.cells().unwrap() {
                let (a, b) = (bounds(&a), bounds(&b));
                let x = a.0.1.min(b.0.1) - a.0.0.max(b.0.0);
                let y = a.1.1.min(b.1.1) - a.1.0.max(b.1.0);
                close(x.max(0.) * y.max(0.), 0.);
                if x >= -1e-8 && y >= -1e-8 {
                    contacts += 1;
                }
            }
        }
        assert!(contacts > 0);
        if kind == 2 {
            let before = e.document.model().clone();
            assert!(
                e.command("Unsafe door", Command::AddOpening(door(ids[0], 1.5)))
                    .is_err()
            );
            assert_eq!(e.document.model(), &before);
            e.command("Clear door", Command::AddOpening(door(ids[0], 0.1)))
                .unwrap();
            let before = e.document.model().clone();
            let mut p = ty.parameters.clone();
            p.layers[0].thickness = 2.;
            assert!(
                e.command(
                    "Unsafe profile",
                    Command::UpdateWallType {
                        id: ty.id(),
                        parameters: p
                    }
                )
                .is_err()
            );
            assert_eq!(e.document.model(), &before);
        }
    }
}

#[test]
fn native_solid_and_ifc_exports_refuse_typed_walls() {
    let (e, _, ids, _, _) = fixture();
    let before = e.document.model().clone();
    assert!(
        os_ifc::WallIfc
            .export_report(&before)
            .unwrap_err()
            .to_string()
            .contains("compound")
    );
    assert!(
        e.host
            .request(
                os_walls::PLUGIN_ID,
                &before,
                os_plugin_api::Request::GenerateWall { id: ids[0] }
            )
            .is_err()
    );
    assert_eq!(e.document.model(), &before);
}
