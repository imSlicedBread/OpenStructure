use super::*;
use crate::plan_workspace::transforms::{Draft, Mode};
use os_geometry::rooms::{FaceKey, derive_faces};
use os_model::{
    Dimension, DimensionEndpoint, DimensionLayout, DimensionParams, DimensionReference, Model,
};

#[test]
fn wall_split_rejects_joins_and_invisible_sources() {
    use os_model::{WallAnchor, WallEndpoint, WallJoin, WallJoinParams};
    for kind in [
        "butt",
        "corner",
        "tee host",
        "tee branch",
        "hidden",
        "crop",
        "partial crop",
        "multiple",
    ] {
        let mut h = Harness::new(PROFILES[0].0, PROFILES[0].1);
        let mut model = h.app.editor.document.model().clone();
        let mut p = model.walls[&h.wall].parameters.clone();
        p.start = Point2::new(1.0, 0.0);
        p.end = Point2::new(1.0, 2.0);
        if kind == "butt" {
            p.end = Point2::new(3.0, 0.0);
        }
        if kind.starts_with("tee") {
            p.start.x = 0.0;
            p.end.x = 0.0;
        }
        let peer = os_model::Wall::new(os_walls::WALL_TYPE, p);
        let peer_id = peer.id();
        model.walls.insert(peer_id, peer);
        let a = WallAnchor {
            wall: h.wall,
            endpoint: WallEndpoint::End,
        };
        let b = WallAnchor {
            wall: peer_id,
            endpoint: WallEndpoint::Start,
        };
        let join = match kind {
            "butt" => Some(WallJoinParams::Butt { a, b }),
            "corner" => Some(WallJoinParams::Corner {
                a,
                b,
                owner: h.wall,
            }),
            "tee host" | "tee branch" => Some(WallJoinParams::Tee {
                host: h.wall,
                station: 1.0,
                branch: b,
            }),
            _ => None,
        };
        if let Some(p) = join {
            let join = WallJoin::new("core.wall_join", p);
            model.wall_joins.insert(join.id(), join);
        }
        let settings = model
            .views
            .get_mut(&h.view)
            .unwrap()
            .parameters
            .plan
            .as_mut()
            .unwrap();
        match kind {
            "hidden" => settings.visibility.walls = false,
            "crop" => {
                settings.crop = Some(os_model::PlanViewCrop {
                    min: Point2::new(10.0, 10.0),
                    max: Point2::new(20.0, 20.0),
                })
            }
            "partial crop" => {
                settings.crop = Some(os_model::PlanViewCrop {
                    min: Point2::new(0.0, -0.5),
                    max: Point2::new(0.8, 0.5),
                })
            }
            _ => (),
        }
        load(&mut h, model.clone());
        if kind == "tee branch" {
            h.app.select(Some(peer_id));
        }
        if kind == "multiple" {
            h.app.selected_ids.insert(peer_id);
        }
        if kind == "partial crop" {
            let draft = Draft::begin(&h.app, Mode::Split).unwrap();
            assert!(draft.candidate(Point2::new(-0.5, 0.0)).is_err());
            assert!(draft.candidate(Point2::new(0.5, 0.0)).is_ok());
        } else {
            assert!(Draft::begin(&h.app, Mode::Split).is_err(), "{kind}");
        }
        assert_eq!(h.app.editor.document.model(), &model);
        assert!(!h.app.editor.document.can_undo());
    }
}

#[test]
fn wall_split_escape_pointer_loss_and_stale_release_at_both_dpis() {
    for (size, scale) in PROFILES {
        for cause in [
            "escape",
            "pointer",
            "selection",
            "drawing",
            "revision",
            "session",
            "view",
            "provider",
        ] {
            let mut h = Harness::new(size, scale);
            h.app.begin_wall_transform(Mode::Split);
            h.frame(vec![]);
            let target = h.point(Point2::new(0.0, 0.0));
            h.press(target);
            match cause {
                "escape" => h.frame(vec![escape()]),
                "pointer" => h.frame(vec![egui::Event::PointerGone]),
                "selection" => h.app.select(None),
                "drawing" => {
                    h.app.plans.drawing = Some(h.app.editor.native_wall_plan(h.view).unwrap())
                }
                "revision" => {
                    h.app
                        .editor
                        .command("Concurrent", Command::RenameProject("Changed".into()))
                        .unwrap();
                }
                "session" => {
                    h.app.editor.document =
                        Document::from_model(h.app.editor.document.model().clone()).unwrap()
                }
                "view" => h.app.focus_plan(None),
                "provider" => h.app.editor.host.unload(os_walls::PLUGIN_ID).unwrap(),
                _ => unreachable!(),
            }
            let before = h.app.editor.document.model().clone();
            let revision = h.app.editor.document.revision();
            h.release(target);
            h.frame(vec![]);
            assert!(h.app.plans.transform.is_none(), "{cause}");
            assert_eq!(h.app.editor.document.model(), &before, "{cause}");
            assert_eq!(h.app.editor.document.revision(), revision, "{cause}");
        }
    }
}

fn load(h: &mut Harness, model: Model) {
    h.app.editor.document = Document::from_model(model).unwrap();
    h.app.editor.regenerate().unwrap();
    h.app.plans.poll(&h.app.editor);
    h.app.focus_plan(Some(h.view));
    h.app.select(Some(h.wall));
    h.settle();
}

fn dependencies(h: &mut Harness, reversed: bool) -> (Id, Id, Id) {
    let (door, window, _) =
        install_transform_openings(h, os_model::WindowPanePosition::LeftFace, true);
    let mut model = h.app.editor.document.model().clone();
    let source = model.walls.get_mut(&h.wall).unwrap();
    source
        .header
        .properties
        .insert("mark".into(), serde_json::json!("W-01"));
    if reversed {
        std::mem::swap(&mut source.parameters.start, &mut source.parameters.end);
    }
    model
        .wall_type_assignments
        .get_mut(&h.wall)
        .unwrap()
        .flipped = true;
    let params = model.walls[&h.wall].parameters.clone();
    let mut peers = Vec::new();
    for (a, b) in [
        ((1.0, 0.0), (1.0, 2.0)),
        ((1.0, 2.0), (-1.0, 2.0)),
        ((-1.0, 2.0), (-1.0, 0.0)),
        ((2.0, 0.0), (3.0, 0.0)),
    ] {
        let mut p = params.clone();
        p.start = Point2::new(a.0, a.1);
        p.end = Point2::new(b.0, b.1);
        let wall = os_model::Wall::new(os_walls::WALL_TYPE, p);
        peers.push(wall.id());
        model.walls.insert(wall.id(), wall);
    }
    let reference = |wall, endpoint| DimensionReference::WallEndpoint { wall, endpoint };
    for layout in [
        DimensionLayout::Aligned,
        DimensionLayout::Chain,
        DimensionLayout::Baseline,
        DimensionLayout::Angular,
    ] {
        let mut p = DimensionParams {
            layout,
            view: h.view,
            baseline_spacing_m: 0.25,
            first: reference(h.wall, DimensionEndpoint::Start),
            second: reference(h.wall, DimensionEndpoint::End),
            additional: vec![],
            offset_m: 0.3,
            orphan_hint: Point2::new(0.0, -0.3),
        };
        if matches!(layout, DimensionLayout::Chain | DimensionLayout::Baseline) {
            // Always order anchors left-to-right, including a reversed source.
            if reversed {
                std::mem::swap(&mut p.first, &mut p.second);
            }
            p.additional
                .push(reference(peers[3], DimensionEndpoint::End));
        }
        if layout == DimensionLayout::Angular {
            p.second = reference(peers[0], DimensionEndpoint::End);
        }
        p.validate_creation(&model).unwrap();
        let d = Dimension::new("core.dimension", p);
        model.dimensions.insert(d.id(), d);
    }
    let tag = os_model::OpeningTag::new(
        "core.opening_tag",
        os_model::OpeningTagParams {
            view: h.view,
            opening: window,
            position: Point2::new(0.2, 0.4),
            label_preset: Default::default(),
        },
    );
    model.opening_tags.insert(tag.id(), tag);
    let seed = Point2::new(0.0, 1.0);
    let faces = derive_faces(&model.room_boundary_segments(params.level).unwrap()).unwrap();
    let signature = faces.assign_seed(seed).unwrap().key.as_signature().to_vec();
    let room = os_model::Room::new(
        "core.room",
        os_model::RoomParams {
            name: "Split room".into(),
            number: "101".into(),
            level: params.level,
            seed,
            boundary_signature: signature,
            floor_finish: None,
            wall_finish: None,
            ceiling_finish: None,
            floor_material: None,
            wall_material: None,
            ceiling_material: None,
        },
    );
    let room_id = room.id();
    model.rooms.insert(room_id, room);
    load(h, model);
    (door, window, room_id)
}

#[test]
fn wall_split_dependencies_preview_commit_and_history_at_both_dpis() {
    for (size, scale) in PROFILES {
        for reversed in [false, true] {
            let mut h = Harness::new(size, scale);
            let (door, window, room) = dependencies(&mut h, reversed);
            h.app.plans.split = true;
            h.frame(vec![]);
            let before = h.app.editor.document.model().clone();
            let scene = h.app.editor.scene.clone();
            let revision = h.app.editor.document.revision();
            let drawing = h.app.plans.drawing.as_ref().unwrap().identity();
            let camera = h.app.plans.cameras[&h.view];
            h.click_text_in_band("Snaps", 0.0, h.size.y);
            h.frame(vec![]);
            h.click_text_in_band("Wall transforms", 0.0, h.size.y);
            h.frame(vec![]);
            h.click_text_in_band("Split wall", 0.0, h.size.y);
            h.frame(vec![]);
            assert!(h.app.plans.transform.is_some());
            let target = h.point(Point2::new(0.0, 0.03));
            h.frame(vec![egui::Event::PointerMoved(target)]);
            h.press(h.point(before.walls[&h.wall].parameters.start));
            h.frame(vec![egui::Event::PointerMoved(target)]);
            assert_eq!(h.app.editor.document.model(), &before);
            assert_eq!(h.app.editor.document.revision(), revision);
            assert!(!h.app.editor.document.can_undo());
            assert_eq!(h.app.editor.scene, scene);
            assert_eq!(h.app.plans.cameras[&h.view], camera);
            assert_eq!(h.app.selected, Some(h.wall));
            assert!(h.app.plans.endpoint_drag.is_none());
            assert!(h.app.wall_gesture.is_none());
            assert!(h.output.shapes.iter().any(|s| matches!(&s.shape,
                egui::Shape::LineSegment { stroke,.. } if stroke.width == 2.0 && stroke.color == theme::ACCENT)));
            h.release(target);
            h.settle();
            assert!(!h.app.status_error, "{}", h.app.status);
            let after = h.app.editor.document.model().clone();
            let new_ids: Vec<_> = after
                .walls
                .keys()
                .filter(|id| !before.walls.contains_key(id))
                .copied()
                .collect();
            assert_eq!(new_ids.len(), 1);
            let new = new_ids[0];
            let mut expected = before.clone();
            let mut end = before.walls[&h.wall].clone();
            end.header.id = new;
            end.parameters.start = Point2::new(0.0, 0.0);
            expected.walls.get_mut(&h.wall).unwrap().parameters.end = end.parameters.start;
            expected.walls.insert(new, end);
            expected
                .element_lifecycles
                .insert(new, before.element_lifecycle(h.wall).unwrap());
            expected
                .wall_type_assignments
                .insert(new, before.wall_type_assignments[&h.wall]);
            let p = &mut expected.openings.get_mut(&window).unwrap().parameters;
            p.host = new;
            p.offset -= 1.0;
            assert_eq!(after.openings[&door], before.openings[&door]);
            for d in expected.dimensions.values_mut() {
                for r in [&mut d.parameters.first, &mut d.parameters.second]
                    .into_iter()
                    .chain(d.parameters.additional.iter_mut())
                {
                    if let DimensionReference::WallEndpoint {
                        wall,
                        endpoint: DimensionEndpoint::End,
                    } = r
                        && *wall == h.wall
                    {
                        *wall = new;
                    }
                }
                d.parameters.validate_creation(&after).unwrap();
            }
            let rp = &after.rooms[&room].parameters;
            let faces = derive_faces(&after.room_boundary_segments(rp.level).unwrap()).unwrap();
            let face = faces
                .resolve_seed(
                    rp.seed,
                    &FaceKey::from_signature(&rp.boundary_signature).unwrap(),
                )
                .unwrap();
            assert_eq!(face.area_m2, 4.0);
            assert_eq!(rp.boundary_signature.len(), 5);
            expected
                .rooms
                .get_mut(&room)
                .unwrap()
                .parameters
                .boundary_signature = rp.boundary_signature.clone();
            // Full model equality checks all UUIDs, metadata, openings, types, tags and room intent.
            assert_eq!(after, expected);
            for tag in after.opening_tags.values() {
                tag.parameters.resolve(&after).unwrap();
                assert_eq!(tag.parameters.label(&after), tag.parameters.label(&before));
            }
            assert_eq!(h.app.editor.document.revision(), revision + 1);
            assert_ne!(h.app.editor.scene, scene);
            assert_ne!(h.app.plans.drawing.as_ref().unwrap().identity(), drawing);
            assert!(h.app.plans.transform.is_none());
            h.app.history(false);
            assert_eq!(h.app.editor.document.model(), &before);
            assert!(!h.app.editor.document.can_undo());
            h.app.history(true);
            assert_eq!(h.app.editor.document.model(), &after);
            {
                use os_storage::{StorageBackend, ZipJsonStorage};
                let temp = tempfile::tempdir().unwrap();
                let path = temp.path().join("split.osproj");
                ZipJsonStorage.save(&h.app.editor.document, &path).unwrap();
                assert_eq!(ZipJsonStorage.open(&path).unwrap().model(), &after);
            }
            h.settle();
            let p = h.point(Point2::new(0.0, -0.7));
            let camera = h.app.plans.cameras[&h.view];
            h.press(p);
            h.frame(vec![egui::Event::PointerMoved(p + egui::vec2(30.0, 20.0))]);
            h.release(p + egui::vec2(30.0, 20.0));
            assert_ne!(h.app.plans.cameras[&h.view], camera);
            assert_eq!(h.app.editor.document.model(), &after);
        }
    }
}

#[test]
fn wall_split_rejects_openings_end_clearance_and_unresolved_dependencies_atomically() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        dependencies(&mut h, false);
        let before = h.app.editor.document.model().clone();
        for x in [-1.0, -0.9995, -0.5, -0.0505, 0.1995, 0.4, 0.9995, 1.0] {
            h.app.begin_wall_transform(Mode::Split);
            h.frame(vec![]);
            h.click(h.point(Point2::new(x, 0.0)));
            assert!(h.app.status_error, "x={x}: {}", h.app.status);
            assert_eq!(h.app.editor.document.model(), &before);
            assert!(!h.app.editor.document.can_undo());
        }
        for bad_room in [false, true] {
            let mut broken = before.clone();
            if bad_room {
                broken.rooms.values_mut().next().unwrap().parameters.seed = Point2::new(0.0, 3.0);
            } else {
                broken
                    .dimensions
                    .values_mut()
                    .next()
                    .unwrap()
                    .parameters
                    .second = DimensionReference::WallEndpoint {
                    wall: Id::new(),
                    endpoint: DimensionEndpoint::End,
                };
            }
            load(&mut h, broken.clone());
            let d = Draft::begin(&h.app, Mode::Split).unwrap();
            assert!(d.commit(&mut h.app, Point2::new(0.0, 0.0)).is_err());
            assert_eq!(h.app.editor.document.model(), &broken);
            assert!(!h.app.editor.document.can_undo());
        }
    }
}

#[test]
fn wall_split_rotated_basis_large_coordinates_and_legacy_wall() {
    for reverse in [false, true] {
        let mut h = Harness::new(PROFILES[0].0, PROFILES[0].1);
        let mut model = h.app.editor.document.model().clone();
        let base = 100_000_000.0;
        let p = &mut model.walls.get_mut(&h.wall).unwrap().parameters;
        p.start = Point2::new(base, base);
        p.end = Point2::new(base + 4.0, base + 3.0);
        if reverse {
            std::mem::swap(&mut p.start, &mut p.end);
        }
        model
            .views
            .get_mut(&h.view)
            .unwrap()
            .parameters
            .plan
            .as_mut()
            .unwrap()
            .basis = os_model::PlanViewBasis {
            origin: Point2::new(base, base),
            rotation: 0.63,
        };
        load(&mut h, model.clone());
        let context = h.app.editor.native_plan_context(h.view).unwrap();
        let draft = Draft::begin(&h.app, Mode::Split).unwrap();
        let candidate = draft
            .candidate(
                context
                    .basis
                    .world_to_plane(Point2::new(base + 2.0, base + 1.5))
                    .unwrap(),
            )
            .unwrap();
        let p = &candidate.model.walls[&h.wall].parameters;
        assert!(p.end.distance(Point2::new(base + 2.0, base + 1.5)) < 1e-7);
        assert_eq!(p.start, model.walls[&h.wall].parameters.start);
        assert_eq!(candidate.model.walls.len(), 2);
        assert_eq!(h.app.editor.document.model(), &model);
        for p in [Point2::new(f64::NAN, 0.0), Point2::new(100.0, 100.0)] {
            assert!(draft.candidate(p).is_err());
        }
    }
}
