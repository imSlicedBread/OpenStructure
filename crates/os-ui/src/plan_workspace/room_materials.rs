//! Display-only floor finish tint. Room geometry comes from the checked drawing.
use super::*;

pub(super) fn paint(
    painter: &egui::Painter,
    model: &os_model::Model,
    room: &os_render::plan::PlanRoomItem,
    context: PlanContext,
    camera: PlanCamera,
    rect: egui::Rect,
) -> Result<()> {
    if room.diagnostic.is_some() || room.boundary.len() < 3 {
        return Ok(());
    }
    let Some(material) = model
        .rooms
        .get(&room.entity)
        .and_then(|r| r.parameters.floor_material)
        .and_then(|id| model.materials.get(&id))
    else {
        return Ok(());
    };
    let size = [f64::from(rect.width()), f64::from(rect.height())];
    let project = |point| -> Result<egui::Pos2> {
        let p = camera.project(point, size)?;
        os_core::ensure(
            p.is_finite() && p.x.abs() < 1e8 && p.y.abs() < 1e8,
            "room fill exceeds screen coordinate range",
        )?;
        Ok(rect.min + egui::vec2(p.x as f32, p.y as f32))
    };
    let clip = if let Some(crop) = context.crop {
        rect.intersect(egui::Rect::from_two_pos(
            project(crop.min)?,
            project(crop.max)?,
        ))
    } else {
        rect
    };
    if !clip.is_positive() {
        return Ok(());
    }
    let triangles = os_geometry::floors::triangulate_plan_polygon(&room.boundary)?;
    let [r, g, b] = material.parameters.color;
    let fill = egui::Color32::from_rgba_unmultiplied(r, g, b, 45);
    let mut mesh = egui::Mesh::default();
    for point in &room.boundary {
        mesh.colored_vertex(project(*point)?, fill);
    }
    for triangle in triangles {
        mesh.add_triangle(triangle[0], triangle[1], triangle[2]);
    }
    painter.with_clip_rect(clip).add(egui::Shape::mesh(mesh));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::opening_schedule::rooms::tests::{add_material, fixture};

    fn settle(app: &mut DesktopApp) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            app.plans.poll(&app.editor);
            if app.plans.ready() {
                return;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
    }

    fn frame(
        app: &mut DesktopApp,
        ctx: &egui::Context,
        size: egui::Vec2,
        scale: f32,
    ) -> egui::FullOutput {
        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            ..Default::default()
        };
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .native_pixels_per_point = Some(scale);
        ctx.run(input, |ctx| app.plan_workspace(ctx))
    }

    fn fills(output: &egui::FullOutput) -> Vec<(usize, egui::Rect, &egui::Mesh)> {
        output
            .shapes
            .iter()
            .enumerate()
            .filter_map(|(i, shape)| match &shape.shape {
                egui::Shape::Mesh(mesh)
                    if !mesh.vertices.is_empty()
                        && mesh.vertices.iter().all(|v| v.color.a() == 45) =>
                {
                    Some((i, shape.clip_rect, mesh.as_ref()))
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn room_material_plan_fill_crop_selection_live_color_and_unresolved_at_both_dpis() {
        for (size, scale) in [
            (egui::vec2(1280., 800.), 1.),
            (egui::vec2(1000., 650.), 1.5),
        ] {
            let (mut app, room, _, view, partition) = fixture();
            let material = add_material(&mut app);
            app.select(Some(room));
            app.room_finish_draft = ["F-01".into(), "W-02".into(), "C-03".into()];
            app.room_material_draft = [Some(material); 3];
            app.apply_room_properties();
            let intent = app.editor.document.model().rooms[&room].clone();
            app.editor
                .command(
                    "Room tag",
                    Command::AddRoomTag(os_model::RoomTag::new(
                        "core.room_tag",
                        os_model::RoomTagParams {
                            view,
                            room,
                            position: Point2::new(2., 2.),
                        },
                    )),
                )
                .unwrap();
            let mut parameters = app.editor.document.model().views[&view].parameters.clone();
            parameters.plan.as_mut().unwrap().crop = Some(os_model::PlanViewCrop {
                min: Point2::new(-0.5, -0.5),
                max: Point2::new(3.5, 2.5),
            });
            app.editor
                .command(
                    "Crop",
                    Command::UpdateView {
                        id: view,
                        parameters,
                    },
                )
                .unwrap();
            app.select(None);
            app.plans.poll(&app.editor);
            app.focus_plan(Some(view));
            settle(&mut app);
            let ctx = egui::Context::default();
            theme::apply(&ctx);
            frame(&mut app, &ctx, size, scale);
            let output = frame(&mut app, &ctx, size, scale);
            assert_eq!(ctx.pixels_per_point(), scale);
            let fill = fills(&output);
            assert_eq!(fill.len(), 1, "unassigned room has no fill");
            let (index, clip, mesh) = fill[0];
            assert!(
                mesh.vertices
                    .iter()
                    .all(|v| v.color == egui::Color32::from_rgba_unmultiplied(160, 100, 60, 45))
            );
            let context = app.editor.native_plan_context(view).unwrap();
            let rect = app.plans.canvas_rect.unwrap();
            let camera = app.plans.cameras[&view];
            let project = |p| {
                let p = camera
                    .project(p, [f64::from(rect.width()), f64::from(rect.height())])
                    .unwrap();
                rect.min + egui::vec2(p.x as f32, p.y as f32)
            };
            let crop = context.crop.unwrap();
            assert_eq!(
                clip,
                rect.intersect(egui::Rect::from_two_pos(
                    project(crop.min),
                    project(crop.max)
                ))
            );
            assert!(
                mesh.vertices.iter().any(|v| !clip.contains(v.pos)),
                "scissor clips boundary beyond crop"
            );
            assert!(output.shapes.iter().skip(index + 1).any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.job.text.contains("F-01") || t.galley.job.text.contains("12.00"))));
            let original_mesh = mesh.clone();
            assert!(output.shapes.iter().skip(index + 1).any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.job.text.contains("Room 1"))), "room tag is above the fill");
            assert!(
                output
                    .shapes
                    .iter()
                    .skip(index + 1)
                    .any(|s| matches!(&s.shape, egui::Shape::Path(p) if p.fill == theme::SURFACE)),
                "wall fills are above the finish tint"
            );
            app.select(Some(room));
            let selected = frame(&mut app, &ctx, size, scale);
            assert_eq!(fills(&selected)[0].2, &original_mesh);
            assert!(selected.shapes.iter().skip(fills(&selected)[0].0 + 1).any(|s| matches!(&s.shape, egui::Shape::LineSegment { stroke, .. } if stroke.color == theme::ACCENT && stroke.width == 2.)));
            let mut parameters = app.editor.document.model().materials[&material]
                .parameters
                .clone();
            parameters.color = [30, 90, 150];
            app.editor
                .command(
                    "Recolor",
                    Command::UpdateMaterial {
                        id: material,
                        parameters,
                    },
                )
                .unwrap();
            settle(&mut app);
            let recolored = frame(&mut app, &ctx, size, scale);
            let mesh = fills(&recolored)[0].2;
            assert_eq!(mesh.indices, original_mesh.indices);
            assert_eq!(
                mesh.vertices.iter().map(|v| v.pos).collect::<Vec<_>>(),
                original_mesh
                    .vertices
                    .iter()
                    .map(|v| v.pos)
                    .collect::<Vec<_>>()
            );
            assert!(
                mesh.vertices
                    .iter()
                    .all(|v| v.color == egui::Color32::from_rgba_unmultiplied(30, 90, 150, 45))
            );
            app.editor
                .command(
                    "Break enclosure",
                    Command::RemoveRoomSeparationLine(partition),
                )
                .unwrap();
            assert!(
                fills(&frame(&mut app, &ctx, size, scale)).is_empty(),
                "no stale fill during regeneration"
            );
            settle(&mut app);
            assert!(fills(&frame(&mut app, &ctx, size, scale)).is_empty());
            let context = app.editor.native_plan_context(view).unwrap();
            let drawing = app.plans.drawing.as_ref().unwrap();
            assert!(
                drawing
                    .rooms(context)
                    .unwrap()
                    .iter()
                    .all(|r| r.boundary.is_empty() && r.diagnostic.is_some())
            );
            assert_eq!(app.editor.document.model().rooms[&room], intent);
            assert!(app.editor.document.model().floors.is_empty());
            app.editor.undo().unwrap();
            settle(&mut app);
            assert_eq!(fills(&frame(&mut app, &ctx, size, scale)).len(), 1);
            app.editor.redo().unwrap();
            settle(&mut app);
            assert!(fills(&frame(&mut app, &ctx, size, scale)).is_empty());
            app.editor.undo().unwrap();
            app.select(Some(room));
            app.room_material_draft[0] = None;
            app.apply_room_properties();
            settle(&mut app);
            assert!(
                fills(&frame(&mut app, &ctx, size, scale)).is_empty(),
                "wall/ceiling refs do not paint a floor"
            );
        }
    }

    #[test]
    fn room_material_concave_and_large_rings_have_exact_fill_area() {
        use os_geometry::floors::{signed_area, triangulate_plan_polygon};
        let concave = vec![
            Point2::new(0., 0.),
            Point2::new(4., 0.),
            Point2::new(4., 1.),
            Point2::new(1., 1.),
            Point2::new(1., 4.),
            Point2::new(0., 4.),
        ];
        let large = (0..300)
            .map(|i| {
                let angle = f64::from(i) * std::f64::consts::TAU / 300.;
                Point2::new(angle.cos(), angle.sin())
            })
            .collect::<Vec<_>>();
        for mut ring in [concave, large] {
            for _ in 0..2 {
                let area: f64 = triangulate_plan_polygon(&ring)
                    .unwrap()
                    .into_iter()
                    .map(|[a, b, c]| {
                        signed_area(&[ring[a as usize], ring[b as usize], ring[c as usize]])
                    })
                    .sum();
                assert!((area - signed_area(&ring).abs()).abs() < 1e-9);
                ring.reverse();
            }
        }
    }
}
