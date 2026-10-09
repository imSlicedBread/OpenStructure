//! Stroke-free section fill mesh; shared edges receive no antialias fringes.
use super::*;

pub(super) fn mesh(
    drawing: &PlanDrawing,
    context: PlanContext,
    camera: PlanCamera,
    rect: egui::Rect,
) -> Result<egui::Mesh> {
    let mut mesh = egui::Mesh::default();
    let size = [f64::from(rect.width()), f64::from(rect.height())];
    for fill in drawing.section_fills(context)? {
        let [r, g, b] = drawing
            .surface_color(fill.surface)
            .unwrap_or([232, 232, 232]);
        let color = egui::Color32::from_rgb(r, g, b);
        let base = u32::try_from(mesh.vertices.len())
            .map_err(|_| os_core::Error::Invalid("section fill screen vertex overflow".into()))?;
        for point in &fill.points {
            let p = camera.project(*point, size)?;
            os_core::ensure(
                p.x.abs() < 1e8 && p.y.abs() < 1e8,
                "section fill exceeds screen range",
            )?;
            mesh.colored_vertex(rect.min + egui::vec2(p.x as f32, p.y as f32), color);
        }
        for index in 1..fill.points.len() - 1 {
            mesh.add_triangle(base, base + index as u32, base + index as u32 + 1);
        }
    }
    Ok(mesh)
}
