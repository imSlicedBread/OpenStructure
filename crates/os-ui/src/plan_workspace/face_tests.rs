//! Associative face authoring through real egui frames, in logical pixels.
use super::*;
use os_model::{DimensionLayout, DimensionWallSide};

fn reload(h: &mut Harness, model: os_model::Model) {
    h.app.editor.document = Document::from_model(model).unwrap();
    h.app.editor.regenerate().unwrap();
    h.app.plans.poll(&h.app.editor);
    h.app.focus_plan(Some(h.view));
    h.app.select(None);
    h.settle();
}

fn pick(h: &Harness, pointer: egui::Pos2) -> Result<DimensionAnchorDraft> {
    h.app.dimension_anchor_at(
        h.app.plans.drawing.as_ref().unwrap(),
        h.app.editor.native_plan_context(h.view).unwrap(),
        h.app.plans.cameras[&h.view],
        h.app.plans.canvas_rect.unwrap(),
        pointer,
    )
}

#[test]
fn wall_face_pick_place_preview_and_history_all_linear_layouts_both_dpis() {
    for (size, scale) in PROFILES {
        for layout in [
            DimensionLayout::Aligned,
            DimensionLayout::Chain,
            DimensionLayout::Baseline,
        ] {
            for unit in [
                Point2::new(1.0, 0.0),
                Point2::new(-1.0, 0.0),
                Point2::new(0.6, 0.8),
            ] {
                let mut h = Harness::new(size, scale);
                let mut model = h.app.editor.document.model().clone();
                let wall = &mut model.walls.get_mut(&h.wall).unwrap().parameters;
                *wall.path.straight_start_mut().unwrap() = Point2::new(-unit.x, -unit.y);
                *wall.path.straight_end_mut().unwrap() = unit;
                reload(&mut h, model);
                let point = |station: f64| {
                    Point2::new(
                        (station - 1.0) * unit.x - 0.135 * unit.y,
                        (station - 1.0) * unit.y + 0.135 * unit.x,
                    )
                };
                for side in [DimensionWallSide::Left, DimensionWallSide::Right] {
                    let sign = if side == DimensionWallSide::Left {
                        1.0
                    } else {
                        -1.0
                    };
                    let p = Point2::new(-sign * 0.135 * unit.y, sign * 0.135 * unit.x);
                    assert!(matches!(pick(&h, h.point(p)).unwrap().reference,
                        DimensionReference::WallFace { side: actual, station_m, .. }
                        if actual == side && (station_m - 1.0).abs() < 1e-5));
                }
                let before = h.app.editor.document.model().clone();
                let scene = h.app.editor.scene.clone();
                let revision = h.app.editor.document.revision();
                h.app.begin_dimension(h.view, layout);
                h.frame(vec![]);
                for station in if layout == DimensionLayout::Aligned {
                    vec![0.4, 1.0]
                } else {
                    vec![0.4, 1.0, 1.6]
                } {
                    h.click(h.point(point(station)));
                    h.frame(vec![]);
                }
                if layout != DimensionLayout::Aligned {
                    h.click_text_in_band("Finish anchors", 0.0, h.size.y);
                    h.frame(vec![]);
                }
                let placement = h.point(Point2::new(-0.65 * unit.y, 0.65 * unit.x));
                h.frame(vec![egui::Event::PointerMoved(placement)]);
                assert_eq!(h.app.editor.document.model(), &before);
                assert_eq!(h.app.editor.document.revision(), revision);
                assert_eq!(h.app.editor.scene, scene);
                h.click(placement);
                h.settle();
                h.clean();
                let id = h.app.selected.unwrap();
                let committed = h.app.editor.document.model().clone();
                let p = &committed.dimensions[&id].parameters;
                assert_eq!(p.layout, layout);
                assert!(p.references().all(|r| matches!(
                    r,
                    DimensionReference::WallFace {
                        side: DimensionWallSide::Left,
                        ..
                    }
                )));
                assert!((p.resolve(&committed).unwrap().length_metres - 0.6).abs() < 1e-5);
                assert_eq!(h.app.editor.document.revision(), revision + 1);
                h.app.history(false);
                assert_eq!(h.app.editor.document.model(), &before);
                h.app.history(true);
                assert_eq!(h.app.editor.document.model(), &committed);
            }
        }
    }
}

#[test]
fn wall_face_properties_repair_preview_live_thickness_and_history_both_dpis() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        let (id, anchor, _, _) = dimension_repair_fixture(&mut h, DimensionLayout::Aligned);
        let before = h.app.editor.document.model().clone();
        let scene = h.app.editor.scene.clone();
        let revision = h.app.editor.document.revision();
        h.click_text_in_band(&format!("Replace anchor {}", anchor + 1), 0.0, h.size.y);
        h.frame(vec![]);
        let pointer = h.point(Point2::new(0.0, 0.135));
        h.frame(vec![egui::Event::PointerMoved(pointer)]);
        assert_eq!(h.app.editor.document.model(), &before);
        assert_eq!(h.app.editor.scene, scene);
        h.click(pointer);
        h.settle();
        h.clean();
        let committed = h.app.editor.document.model().clone();
        assert!(matches!(
            committed.dimensions[&id].parameters.second,
            DimensionReference::WallFace { .. }
        ));
        assert_eq!(h.app.editor.document.revision(), revision + 1);
        let mut wall = committed.walls[&h.wall].parameters.clone();
        wall.thickness = 0.8;
        h.app
            .editor
            .command(
                "Change dimensioned thickness",
                Command::UpdateWall {
                    id: h.wall,
                    parameters: wall,
                },
            )
            .unwrap();
        h.settle();
        let model = h.app.editor.document.model();
        let value = model.dimensions[&id]
            .parameters
            .resolve(model)
            .unwrap()
            .length_metres;
        assert!((value - 1.16_f64.sqrt()).abs() < 1e-5);
        let context = h.app.editor.native_plan_context(h.view).unwrap();
        let graphics = h
            .app
            .plans
            .drawing
            .as_ref()
            .unwrap()
            .dimensions(context)
            .unwrap();
        assert!(
            (graphics
                .iter()
                .find(|d| d.entity == id)
                .unwrap()
                .value_m
                .unwrap()
                - value)
                .abs()
                < 1e-12
        );
        h.app.history(false);
        assert_eq!(h.app.editor.document.model(), &committed);
        h.app.history(false);
        assert_eq!(h.app.editor.document.model(), &before);
        h.app.history(true);
        assert_eq!(h.app.editor.document.model(), &committed);
    }
}

#[test]
fn wall_face_hits_obey_logical_radius_crop_visibility_voids_and_old_target_priority() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        let face = h.point(Point2::new(0.0, 0.135));
        assert!(pick(&h, face + egui::vec2(0.0, -11.99)).is_ok());
        assert!(pick(&h, face + egui::vec2(0.0, -12.01)).is_err());
        assert!(matches!(
            pick(&h, h.point(Point2::new(-1.0, 0.0))).unwrap().reference,
            DimensionReference::WallEndpoint { .. }
        ));
        let base = h.app.editor.document.model().clone();
        for hidden in [false, true] {
            let mut model = base.clone();
            let plan = model
                .views
                .get_mut(&h.view)
                .unwrap()
                .parameters
                .plan
                .as_mut()
                .unwrap();
            if hidden {
                plan.visibility.walls = false;
            } else {
                plan.crop = Some(os_model::PlanViewCrop {
                    min: Point2::new(-1.0, -0.5),
                    max: Point2::new(-0.5, 0.5),
                });
            }
            reload(&mut h, model);
            assert!(pick(&h, h.point(Point2::new(0.0, 0.135))).is_err());
        }
        reload(&mut h, base);
        let (opening, _) = add_dimension_test_opening(&mut h, os_model::OpeningKind::Door);
        assert!(
            matches!(pick(&h, h.point(Point2::new(-0.7, 0.0))).unwrap().reference,
            DimensionReference::OpeningJamb { opening: actual, .. } if actual == opening)
        );
        assert!(
            pick(&h, h.point(Point2::new(-0.4, 0.135))).is_err(),
            "wall face is absent in aperture"
        );
    }
}

#[test]
fn wall_face_typed_thickness_low_zoom_overlap_phase_and_cancel() {
    for (size, scale) in PROFILES {
        let mut h = Harness::new(size, scale);
        let mut model = h.app.editor.document.model().clone();
        let ty = os_model::WallType::new(
            "core.wall_type",
            os_model::WallTypeParams {
                name: "Compound face".into(),
                layers: [0.2, 0.4]
                    .into_iter()
                    .map(|thickness| os_model::WallLayer {
                        id: Id::new(),
                        name: "Layer".into(),
                        thickness,
                        function: os_model::LayerFunction::Structure,
                        material: None,
                    })
                    .collect(),
            },
        );
        model.wall_type_assignments.insert(
            h.wall,
            os_model::WallTypeAssignment {
                type_id: ty.id(),
                flipped: false,
            },
        );
        model.wall_types.insert(ty.id(), ty);
        reload(&mut h, model.clone());
        let anchor = pick(&h, h.point(Point2::new(0.0, 0.3))).unwrap();
        assert!((anchor.point.y - 0.3).abs() < 1e-12);
        let original_camera = h.app.plans.cameras[&h.view];
        h.app
            .plans
            .cameras
            .get_mut(&h.view)
            .unwrap()
            .pixels_per_metre = 20.0;
        for _ in 0..3 {
            assert!(matches!(
                pick(&h, h.point(Point2::new(0.0, 0.0))).unwrap().reference,
                DimensionReference::WallFace {
                    side: DimensionWallSide::Left,
                    ..
                }
            ));
        }
        h.app.plans.cameras.insert(h.view, original_camera);
        let line = os_model::DetailLine::new(
            "core.detail_line",
            os_model::DetailLineParams {
                view: h.view,
                start: Point2::new(-0.5, 0.3),
                end: Point2::new(0.5, 0.3),
            },
        );
        let mut overlap = model.clone();
        overlap.detail_lines.insert(line.id(), line);
        reload(&mut h, overlap);
        assert!(
            pick(&h, h.point(Point2::new(0.0, 0.3))).is_err(),
            "foreground drafting line blocks host face"
        );

        let mut hidden = model.clone();
        hidden.element_lifecycles.insert(
            h.wall,
            os_model::ElementLifecycle {
                created_in: hidden.latest_phase().unwrap(),
                demolished_in: None,
            },
        );
        hidden
            .views
            .get_mut(&h.view)
            .unwrap()
            .parameters
            .plan
            .as_mut()
            .unwrap()
            .target_phase = Some(hidden.existing_phase().unwrap());
        reload(&mut h, hidden);
        assert!(
            pick(&h, h.point(Point2::new(0.0, 0.3))).is_err(),
            "future phase face hidden"
        );

        for stale in [false, true] {
            reload(&mut h, model.clone());
            h.app.begin_dimension(h.view, DimensionLayout::Aligned);
            h.frame(vec![]);
            h.click(h.point(Point2::new(0.0, 0.3)));
            assert!(
                h.app
                    .plans
                    .dimension_draft
                    .as_ref()
                    .unwrap()
                    .first
                    .is_some()
            );
            if stale {
                let mut wall = model.walls[&h.wall].parameters.clone();
                wall.path.straight_end_mut().unwrap().x += 0.1;
                h.app
                    .editor
                    .command(
                        "Invalidate face draft",
                        Command::UpdateWall {
                            id: h.wall,
                            parameters: wall,
                        },
                    )
                    .unwrap();
                h.settle();
            } else {
                h.frame(vec![key_pressed(egui::Key::Escape)]);
                assert_eq!(h.app.editor.document.model(), &model);
                assert!(!h.app.editor.document.can_undo());
            }
            assert!(h.app.plans.dimension_draft.is_none());
            assert!(h.app.editor.document.model().dimensions.is_empty());
        }
    }
}
