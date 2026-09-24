//! Native plan workspace. One native worker plus one optional provider composition
//! worker; obsolete work drains before replacement in each lane.
use super::*;
use crate::plan_gesture::WallEdit;
#[cfg(test)]
mod column_tests;
mod columns;
mod crop;
mod detail_lines;
#[cfg(test)]
mod graphics_tests;
#[cfg(test)]
mod room_separation_line_tests;
mod room_separation_lines;
mod room_tags;
use os_model::{
    DimensionEndpoint, DimensionParams, DimensionReference, Opening, OpeningDefinition,
    OpeningKind, OpeningParams, OpeningType, OpeningTypeParams, ResolvedOpening, Sheet,
    SheetParams, SheetViewport,
};
use os_render::{
    plan::{PlanCamera, PlanContext, PlanDimensionItem, PlanDrawing, PlanFloorItem},
    sheet::{
        PaperColor, PaperMarkKind, PaperSheetInfo, PaperViewport, SheetPage, compose_view_sheet,
    },
};
use std::{
    collections::BTreeMap,
    io::Write,
    path::{Path, PathBuf},
    thread::JoinHandle,
};
#[cfg(test)]
mod detail_line_tests;
#[cfg(test)]
mod endpoint_tests;
#[cfg(test)]
mod floor_tests;
#[cfg(test)]
mod opening_tests;
#[cfg(feature = "external-plugins")]
mod providers;
#[cfg(test)]
mod section_tests;
#[cfg(test)]
mod sheet_tests;

const ENDPOINT_RADIUS: f32 = 6.0;
const ENDPOINT_HIT_RADIUS: f32 = 10.0;
const SECTION_VIEW_PADDING_M: f64 = 0.25;
const SHEET_VIEWPORT_CENTER_MM: Point2 = Point2 { x: 210.0, y: 128.0 };
const SHEET_VIEWPORT_SIZE_MM: Point2 = Point2 { x: 360.0, y: 220.0 };

struct PendingSheetPdf {
    path: PathBuf,
    bytes: Vec<u8>,
    replace: bool,
    session: Id,
    revision: u64,
}

fn sheet_pdf_path(text: &str) -> Result<PathBuf> {
    let path = PathBuf::from(text.trim());
    os_core::ensure(
        !path.as_os_str().is_empty()
            && path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("pdf")),
        "Choose a file path ending in .pdf.",
    )?;
    Ok(path)
}

fn write_sheet_pdf(path: &Path, bytes: &[u8], replace: bool) -> Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut file = tempfile::NamedTempFile::new_in(parent)
        .map_err(|error| Error::Storage(error.to_string()))?;
    file.write_all(bytes)
        .and_then(|()| file.as_file_mut().sync_all())
        .map_err(|error| Error::Storage(error.to_string()))?;
    if replace {
        file.persist(path)
            .map_err(|error| Error::Storage(error.to_string()))?;
    } else {
        file.persist_noclobber(path)
            .map_err(|error| Error::Storage(error.to_string()))?;
    }
    Ok(())
}

fn sheet_fit_rect(
    available: egui::Rect,
    width_mm: f64,
    height_mm: f64,
) -> Option<(egui::Rect, f32)> {
    if !width_mm.is_finite() || !height_mm.is_finite() || width_mm <= 0.0 || height_mm <= 0.0 {
        return None;
    }
    let available = available.shrink(24.0);
    if available.width() <= 0.0 || available.height() <= 0.0 {
        return None;
    }
    let scale = (available.width() / width_mm as f32).min(available.height() / height_mm as f32);
    if !scale.is_finite() || scale <= 0.0 {
        return None;
    }
    let size = egui::vec2(width_mm as f32 * scale, height_mm as f32 * scale);
    Some((
        egui::Rect::from_center_size(available.center(), size),
        scale,
    ))
}

fn paper_to_screen(point_mm: Point2, page: egui::Rect, scale: f32) -> egui::Pos2 {
    page.min + egui::vec2(point_mm.x as f32 * scale, point_mm.y as f32 * scale)
}

fn paint_sheet_page(painter: &egui::Painter, bounds: egui::Rect, page: &SheetPage) -> Result<()> {
    let Some((page_rect, scale)) = sheet_fit_rect(bounds, page.width_mm, page.height_mm) else {
        return Err(Error::Invalid("sheet preview has no usable area".into()));
    };
    painter.rect_filled(page_rect, 0.0, egui::Color32::WHITE);
    painter.rect_stroke(
        page_rect,
        0.0,
        egui::Stroke::new(1.0, egui::Color32::DARK_GRAY),
        egui::StrokeKind::Inside,
    );
    let color = |color: PaperColor| egui::Color32::from_rgb(color.red, color.green, color.blue);
    for mark in page.marks() {
        let clip = mark.clip.map_or(page_rect, |clip| {
            egui::Rect::from_min_max(
                paper_to_screen(clip.min_mm, page_rect, scale),
                paper_to_screen(clip.max_mm, page_rect, scale),
            )
            .intersect(page_rect)
        });
        if clip.is_negative() {
            continue;
        }
        let painter = painter.with_clip_rect(clip);
        match &mark.kind {
            PaperMarkKind::Path {
                points_mm,
                closed,
                stroke,
                fill,
            } => {
                let points: Vec<_> = points_mm
                    .iter()
                    .map(|point| paper_to_screen(*point, page_rect, scale))
                    .collect();
                let egui_stroke = stroke.map_or(egui::Stroke::NONE, |stroke| {
                    egui::Stroke::new(
                        (stroke.width_mm as f32 * scale).clamp(0.35, 8.0),
                        color(stroke.color),
                    )
                });
                let dashed = stroke.is_some_and(|stroke| stroke.dashed);
                if *closed && let Some(fill) = fill {
                    painter.add(egui::Shape::convex_polygon(
                        points.clone(),
                        color(*fill),
                        if dashed {
                            egui::Stroke::NONE
                        } else {
                            egui_stroke
                        },
                    ));
                }
                if !(*closed && fill.is_some() && !dashed) {
                    let plan_stroke = stroke.map(|stroke| os_render::plan::PlanStroke {
                        color: [stroke.color.red, stroke.color.green, stroke.color.blue],
                        weight_mm: stroke.width_mm,
                        dashed: stroke.dashed,
                    });
                    let mut phase = 0.0;
                    for segment in points.windows(2) {
                        paint_plan_line(
                            &painter,
                            segment[0],
                            segment[1],
                            plan_stroke,
                            egui_stroke,
                            scale,
                            &mut phase,
                        )?;
                    }
                    if *closed && let (Some(first), Some(last)) = (points.first(), points.last()) {
                        paint_plan_line(
                            &painter,
                            *last,
                            *first,
                            plan_stroke,
                            egui_stroke,
                            scale,
                            &mut phase,
                        )?;
                    }
                }
            }
            PaperMarkKind::Text {
                baseline_mm,
                text,
                size_mm,
                color: text_color,
            } => {
                painter.text(
                    paper_to_screen(*baseline_mm, page_rect, scale),
                    egui::Align2::LEFT_BOTTOM,
                    text,
                    egui::FontId::proportional((*size_mm as f32 * scale).max(1.0)),
                    color(*text_color),
                );
            }
        }
    }
    Ok(())
}

struct EndpointDrag {
    mode: WallEdit,
    origin: egui::Pos2,
    moved: bool,
}

fn endpoint_handles(
    wall: &WallParams,
    context: PlanContext,
    camera: PlanCamera,
    rect: egui::Rect,
) -> Vec<(WallEdit, egui::Pos2)> {
    [
        (WallEdit::ResizeStart, wall.start),
        (WallEdit::ResizeEnd, wall.end),
    ]
    .into_iter()
    .filter_map(|(mode, point)| {
        let point = context.basis.world_to_plane(point).ok()?;
        if context.crop.is_some_and(|crop| {
            point.x < crop.min.x
                || point.x > crop.max.x
                || point.y < crop.min.y
                || point.y > crop.max.y
        }) {
            return None;
        }
        let screen = camera
            .project(point, [f64::from(rect.width()), f64::from(rect.height())])
            .ok()?;
        let pos = rect.min + egui::vec2(screen.x as f32, screen.y as f32);
        (pos.is_finite() && rect.contains(pos)).then_some((mode, pos))
    })
    .collect()
}

fn hit_endpoint(handles: &[(WallEdit, egui::Pos2)], pointer: egui::Pos2) -> Option<WallEdit> {
    handles
        .iter()
        .filter_map(|(mode, pos)| {
            let distance = pos.distance_sq(pointer);
            (distance <= ENDPOINT_HIT_RADIUS * ENDPOINT_HIT_RADIUS).then_some((*mode, distance))
        })
        // Start precedes end in the handles list; min_by preserves the first tie.
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(mode, _)| mode)
}

fn point_in_plan_crop(context: PlanContext, point: Point2) -> bool {
    context.crop.is_none_or(|crop| {
        point.x >= crop.min.x
            && point.x <= crop.max.x
            && point.y >= crop.min.y
            && point.y <= crop.max.y
    })
}

fn screen_rect_inside_plan_crop(
    context: PlanContext,
    camera: PlanCamera,
    rect: egui::Rect,
    size: [f64; 2],
    bounds: egui::Rect,
) -> Result<bool> {
    if context.crop.is_none() {
        return Ok(true);
    }
    for corner in [
        bounds.min,
        egui::pos2(bounds.max.x, bounds.min.y),
        bounds.max,
        egui::pos2(bounds.min.x, bounds.max.y),
    ] {
        let local = corner - rect.min;
        let point = camera.unproject(Point2::new(f64::from(local.x), f64::from(local.y)), size)?;
        if !point_in_plan_crop(context, point) {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(super) fn clip_plan_segment(
    a: Point2,
    b: Point2,
    crop: Option<os_geometry::plan::PlanCrop>,
) -> Option<(Point2, Point2)> {
    let Some(crop) = crop else {
        return Some((a, b));
    };
    let delta = Point2::new(b.x - a.x, b.y - a.y);
    let mut enter: f64 = 0.0;
    let mut leave: f64 = 1.0;
    for (p, q) in [
        (-delta.x, a.x - crop.min.x),
        (delta.x, crop.max.x - a.x),
        (-delta.y, a.y - crop.min.y),
        (delta.y, crop.max.y - a.y),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
            continue;
        }
        let t = q / p;
        if p < 0.0 {
            enter = enter.max(t);
        } else {
            leave = leave.min(t);
        }
        if enter > leave {
            return None;
        }
    }
    Some((
        Point2::new(a.x + delta.x * enter, a.y + delta.y * enter),
        Point2::new(a.x + delta.x * leave, a.y + delta.y * leave),
    ))
}

fn paint_room_outline(
    painter: &egui::Painter,
    boundary: &[Point2],
    context: PlanContext,
    camera: PlanCamera,
    rect: egui::Rect,
    size: [f64; 2],
    stroke: egui::Stroke,
) -> Result<()> {
    for (a, b) in boundary
        .iter()
        .zip(boundary.iter().cycle().skip(1))
        .take(boundary.len())
    {
        let Some((a, b)) = clip_plan_segment(*a, *b, context.crop) else {
            continue;
        };
        let a = camera.project(a, size)?;
        let b = camera.project(b, size)?;
        os_core::ensure(
            [a.x, a.y, b.x, b.y]
                .iter()
                .all(|coordinate| coordinate.abs() < f64::from(f32::MAX) / 2.0),
            "room boundary exceeds screen coordinate range",
        )?;
        painter.line_segment(
            [
                egui::pos2(rect.left() + a.x as f32, rect.top() + a.y as f32),
                egui::pos2(rect.left() + b.x as f32, rect.top() + b.y as f32),
            ],
            stroke,
        );
    }
    Ok(())
}

fn paint_floor_graphic(
    painter: &egui::Painter,
    context: PlanContext,
    camera: PlanCamera,
    rect: egui::Rect,
    floor: &PlanFloorItem,
    selected: bool,
    appearance: Option<os_render::plan::PlanStroke>,
) -> Result<()> {
    let size = [f64::from(rect.width()), f64::from(rect.height())];
    let clip = if let Some(crop) = context.crop {
        let a = camera.project(Point2::new(crop.min.x, crop.max.y), size)?;
        let b = camera.project(Point2::new(crop.max.x, crop.min.y), size)?;
        os_core::ensure(
            [a.x, a.y, b.x, b.y]
                .iter()
                .all(|coordinate| coordinate.is_finite() && coordinate.abs() < 1e8),
            "floor crop exceeds screen coordinate range",
        )?;
        rect.intersect(egui::Rect::from_min_max(
            rect.min + egui::vec2(a.x as f32, a.y as f32),
            rect.min + egui::vec2(b.x as f32, b.y as f32),
        ))
    } else {
        rect
    };
    if clip.is_negative() {
        return Ok(());
    }
    let clipped = painter.with_clip_rect(clip);
    let screen: Vec<_> = floor
        .boundary
        .iter()
        .map(|point| {
            let point = camera.project(*point, size)?;
            os_core::ensure(
                point.x.abs() < f64::from(f32::MAX) / 2.0
                    && point.y.abs() < f64::from(f32::MAX) / 2.0,
                "floor exceeds screen coordinate range",
            )?;
            Ok(rect.min + egui::vec2(point.x as f32, point.y as f32))
        })
        .collect::<Result<_>>()?;
    let fill = if selected {
        theme::SELECTED
    } else {
        theme::SURFACE.gamma_multiply(0.32)
    };
    for [a, b, c] in &floor.triangles {
        clipped.add(egui::Shape::convex_polygon(
            vec![
                screen[*a as usize],
                screen[*b as usize],
                screen[*c as usize],
            ],
            fill,
            egui::Stroke::NONE,
        ));
    }
    let stroke = egui::Stroke::new(
        if selected { 2.0 } else { 1.0 },
        if selected {
            theme::ACCENT
        } else {
            theme::MUTED
        },
    );
    let px_per_paper_mm = (camera.pixels_per_metre * context.scale_denominator / 1000.0) as f32;
    let mut phase_mm = 0.0;
    for (a, b) in screen
        .iter()
        .zip(screen.iter().cycle().skip(1))
        .take(screen.len())
    {
        paint_plan_line(
            &clipped,
            *a,
            *b,
            (!selected).then_some(appearance).flatten(),
            stroke,
            px_per_paper_mm,
            &mut phase_mm,
        )?;
    }
    Ok(())
}

fn paint_plan_line(
    painter: &egui::Painter,
    start: egui::Pos2,
    end: egui::Pos2,
    appearance: Option<os_render::plan::PlanStroke>,
    fallback: egui::Stroke,
    pixels_per_paper_mm: f32,
    phase_mm: &mut f32,
) -> Result<()> {
    let Some(style) = appearance else {
        painter.line_segment([start, end], fallback);
        return Ok(());
    };
    os_core::ensure(
        pixels_per_paper_mm.is_finite() && pixels_per_paper_mm > 0.0,
        "invalid screen scale for plan appearance",
    )?;
    let color = egui::Color32::from_rgb(style.color[0], style.color[1], style.color[2]);
    let stroke = egui::Stroke::new(
        (style.weight_mm as f32 * pixels_per_paper_mm).max(0.1),
        color,
    );
    if !style.dashed {
        painter.line_segment([start, end], stroke);
        return Ok(());
    }
    let delta = end - start;
    let length = delta.length();
    if length <= f32::EPSILON {
        return Ok(());
    }
    let length_mm = length / pixels_per_paper_mm;
    os_core::ensure(
        length_mm.is_finite() && length_mm <= 20_000.0,
        "dashed plan stroke exceeds the display segment budget",
    )?;
    let mut offset = 0.0_f32;
    let mut count = 0usize;
    while offset < length_mm {
        count += 1;
        os_core::ensure(
            count <= 4096,
            "dashed plan stroke exceeds the display segment budget",
        )?;
        let phase = (*phase_mm + offset).rem_euclid(4.5);
        let (advance, draw) = if phase < 3.0 {
            ((3.0 - phase).min(length_mm - offset), true)
        } else {
            ((4.5 - phase).min(length_mm - offset), false)
        };
        if draw {
            let a = start + delta * (offset / length_mm);
            let b = start + delta * ((offset + advance) / length_mm);
            painter.line_segment([a, b], stroke);
        }
        offset += advance.max(f32::EPSILON);
    }
    *phase_mm = (*phase_mm + length_mm).rem_euclid(4.5);
    Ok(())
}

fn paint_dimension_graphic(
    painter: &egui::Painter,
    context: PlanContext,
    camera: PlanCamera,
    rect: egui::Rect,
    size: [f64; 2],
    dimension: &PlanDimensionItem,
    selected: bool,
) -> Result<()> {
    let color = if dimension.diagnostic.is_some() {
        theme::ERROR
    } else if selected {
        theme::ACCENT
    } else {
        theme::TEXT
    };
    let screen = |point: Point2| -> Result<egui::Pos2> {
        let point = camera.project(point, size)?;
        os_core::ensure(
            point.x.abs() < f64::from(f32::MAX) / 2.0 && point.y.abs() < f64::from(f32::MAX) / 2.0,
            "dimension exceeds screen coordinate range",
        )?;
        Ok(egui::pos2(
            rect.left() + point.x as f32,
            rect.top() + point.y as f32,
        ))
    };
    if dimension.value_m.is_some() {
        let stroke = egui::Stroke::new(if selected { 1.8 } else { 1.2 }, color);
        for (start, end) in dimension.lines() {
            if let Some((start, end)) = clip_plan_segment(start, end, context.crop) {
                painter.line_segment([screen(start)?, screen(end)?], stroke);
            }
        }
        if let Some((clipped_start, clipped_end)) =
            clip_plan_segment(dimension.line_start, dimension.line_end, context.crop)
        {
            let start = screen(clipped_start)?;
            let end = screen(clipped_end)?;
            let dx = end.x - start.x;
            let dy = end.y - start.y;
            let length = dx.hypot(dy);
            if length.is_finite() && length > 1e-3 {
                let tick = egui::vec2(-dy / length * 4.0, dx / length * 4.0);
                for (at, original_endpoint) in
                    [(start, dimension.line_start), (end, dimension.line_end)]
                {
                    let tick_start = at - tick;
                    let tick_end = at + tick;
                    let tick_bounds = egui::Rect::from_two_pos(tick_start, tick_end);
                    if point_in_plan_crop(context, original_endpoint)
                        && screen_rect_inside_plan_crop(context, camera, rect, size, tick_bounds)?
                    {
                        painter.line_segment([tick_start, tick_end], stroke);
                    }
                }
            }
        }
    } else if point_in_plan_crop(context, dimension.orphan_hint) {
        let at = screen(dimension.orphan_hint)?;
        let marker_bounds = egui::Rect::from_center_size(at, egui::vec2(12.0, 12.0));
        if screen_rect_inside_plan_crop(context, camera, rect, size, marker_bounds)? {
            painter.circle_filled(at, 6.0, color);
            painter.text(
                at,
                egui::Align2::CENTER_CENTER,
                "!",
                egui::FontId::proportional(11.0),
                theme::CANVAS,
            );
        }
    }
    if let Some([min, max]) = dimension.label_bounds(context, camera, size)? {
        let bounds = egui::Rect::from_min_max(
            rect.min + egui::vec2(min.x as f32, min.y as f32),
            rect.min + egui::vec2(max.x as f32, max.y as f32),
        );
        painter
            .with_clip_rect(painter.clip_rect().intersect(bounds))
            .text(
                bounds.center(),
                egui::Align2::CENTER_CENTER,
                dimension.label(),
                egui::FontId::monospace(12.0),
                color,
            );
    }
    Ok(())
}

fn paint_angular_graphic(
    painter: &egui::Painter,
    context: PlanContext,
    camera: PlanCamera,
    rect: egui::Rect,
    graphic: &os_render::plan::PlanAngularDimension,
    selected: bool,
) -> Result<()> {
    let size = [f64::from(rect.width()), f64::from(rect.height())];
    let color = if graphic.diagnostic.is_some() {
        theme::ERROR
    } else if selected {
        theme::ACCENT
    } else {
        theme::TEXT
    };
    let screen = |p| -> Result<egui::Pos2> {
        let p = camera.project(p, size)?;
        os_core::ensure(
            p.x.abs() < f64::from(f32::MAX) / 2.0 && p.y.abs() < f64::from(f32::MAX) / 2.0,
            "angular screen coordinate overflow",
        )?;
        Ok(rect.min + egui::vec2(p.x as f32, p.y as f32))
    };
    for (a, b) in graphic.segments(context)? {
        painter.line_segment(
            [screen(a)?, screen(b)?],
            egui::Stroke::new(if selected { 1.8 } else { 1.2 }, color),
        );
    }
    if let Some([a, b]) = graphic.label_bounds(context, camera, size)? {
        let bounds = egui::Rect::from_min_max(
            rect.min + egui::vec2(a.x as f32, a.y as f32),
            rect.min + egui::vec2(b.x as f32, b.y as f32),
        );
        painter
            .with_clip_rect(painter.clip_rect().intersect(bounds))
            .text(
                bounds.center(),
                egui::Align2::CENTER_CENTER,
                graphic.label(),
                egui::FontId::monospace(12.0),
                color,
            );
    }
    Ok(())
}

fn preview_dimension(
    first: Point2,
    second: Point2,
    placement: Point2,
    entity: Id,
) -> Option<PlanDimensionItem> {
    let dx = second.x - first.x;
    let dy = second.y - first.y;
    let value_m = dx.hypot(dy);
    if !value_m.is_finite() || value_m <= 1e-6 {
        return None;
    }
    let normal = Point2::new(-dy / value_m, dx / value_m);
    let offset = (placement.x - first.x) * normal.x + (placement.y - first.y) * normal.y;
    let offset = Point2::new(normal.x * offset, normal.y * offset);
    Some(PlanDimensionItem {
        spans: Vec::new(),
        shared_start_witness: false,
        entity,
        witness_start: first,
        witness_end: second,
        line_start: Point2::new(first.x + offset.x, first.y + offset.y),
        line_end: Point2::new(second.x + offset.x, second.y + offset.y),
        value_m: Some(value_m),
        orphan_hint: placement,
        diagnostic: None,
    })
}

/// Session preferences; never stored as model geometry or history.
#[derive(Clone, Copy)]
struct SnapOptions {
    enabled: bool,
    endpoints: bool,
    intersections: bool,
    perpendicular: bool,
    midpoints: bool,
    nearest: bool,
    axis_extensions: bool,
}
impl Default for SnapOptions {
    fn default() -> Self {
        Self {
            enabled: true,
            endpoints: true,
            intersections: true,
            perpendicular: true,
            midpoints: true,
            nearest: true,
            axis_extensions: false,
        }
    }
}

#[derive(Default)]
pub(super) struct PlanWorkspace {
    crop: crop::State,
    room_tag_draft: Option<room_tags::Draft>,
    detail_line_draft: Option<detail_lines::Draft>,
    detail_line_pointer_claimed: bool,
    room_separation_line_draft: Option<room_separation_lines::Draft>,
    room_separation_line_pointer_claimed: bool,
    room_tag_pointer_claimed: bool,
    endpoint_drag: Option<EndpointDrag>,
    opening_move: Option<crate::opening_tools::OpeningMove>,
    opening_move_claimed: bool,
    // Retain ownership after cancellation until button-up, so the same press
    // cannot turn into a pan or selection. This is not a live wall draft.
    endpoint_pointer_claimed: bool,
    section_placement: Option<SectionPlacementDraft>,
    #[cfg(feature = "external-plugins")]
    providers: providers::Providers,
    #[cfg(test)]
    pub(super) canvas_rect: Option<egui::Rect>,
    pub active: Option<Id>,
    pub active_sheet: Option<Id>,
    pub split: bool,
    pub(super) room_placement_active: bool,
    dimension_draft: Option<DimensionDraft>,
    opening_placement: Option<OpeningPlacementDraft>,
    floor_sketch: Option<FloorSketchDraft>,
    column_placement: Option<columns::Placement>,
    column_edit: Option<columns::Edit>,
    column_pointer_claimed: bool,
    toolbar_height: f32,
    session: Option<Id>,
    cameras: BTreeMap<Id, PlanCamera>,
    desired: Option<PlanContext>,
    pending: Option<(JoinHandle<Result<PlanDrawing>>, bool)>,
    pub(super) drawing: Option<PlanDrawing>,
    error: Option<String>,
    attempted: bool,
    snaps: SnapOptions,
    pdf_path: String,
    pending_pdf: Option<PendingSheetPdf>,
    sheet_schedule: Option<Id>,
}

#[derive(Clone)]
struct DimensionAnchorDraft {
    reference: DimensionReference,
    point: Point2,
}

struct SectionPlacementDraft {
    context: PlanContext,
    session: Id,
    revision: u64,
    provider_signature: Vec<(String, Id)>,
    first: Option<Point2>,
    bottom_elevation: f64,
    top_elevation: f64,
}

struct DimensionDraft {
    layout: os_model::DimensionLayout,
    additional: Vec<DimensionAnchorDraft>,
    placing: bool,
    provider_signature: Vec<(String, Id)>,
    drawing_identity: Option<Id>,
    context: PlanContext,
    first: Option<DimensionAnchorDraft>,
    second: Option<DimensionAnchorDraft>,
}

#[derive(Clone, Copy)]
struct OpeningPlacementDraft {
    context: PlanContext,
    activation: Option<Id>,
    session: Id,
    revision: u64,
    kind: OpeningKind,
    type_id: Option<Id>,
}

struct FloorSketchDraft {
    context: PlanContext,
    session: Id,
    revision: u64,
    provider_signature: Vec<(String, Id)>,
    points: Vec<Point2>,
    thickness: String,
    top_offset: String,
}

#[cfg(feature = "external-plugins")]
pub(super) fn plan_provider_signature(editor: &Editor) -> Vec<(String, Id)> {
    editor
        .host
        .manifests()
        .filter_map(|manifest| {
            editor
                .host
                .activation_id(&manifest.id)
                .map(|activation| (manifest.id.clone(), activation))
        })
        .collect()
}

#[cfg(not(feature = "external-plugins"))]
pub(super) fn plan_provider_signature(_editor: &Editor) -> Vec<(String, Id)> {
    Vec::new()
}

struct OpeningPlacementPreview {
    host: Id,
    parameters: OpeningParams,
    resolved: ResolvedOpening,
    message: Option<String>,
}

fn project_to_segment(point: Point2, start: Point2, end: Point2) -> (f64, f64) {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let length_squared = dx * dx + dy * dy;
    let t =
        (((point.x - start.x) * dx + (point.y - start.y) * dy) / length_squared).clamp(0.0, 1.0);
    let projected = Point2::new(start.x + t * dx, start.y + t * dy);
    (t, point.distance(projected))
}

fn floor_snap_target(
    drawing: &PlanDrawing,
    context: PlanContext,
    camera: PlanCamera,
    viewport: [f64; 2],
    pointer: Point2,
    snaps: SnapOptions,
) -> Result<Point2> {
    let unsnapped = camera.unproject(pointer, viewport)?;
    if !snaps.enabled {
        return Ok(unsnapped);
    }
    let query = os_render::snapping::SnapQuery {
        camera,
        viewport,
        pointer,
        radius_pixels: 12.0,
        endpoints: snaps.endpoints,
        midpoints: snaps.midpoints,
        intersections: snaps.intersections,
        perpendicular_from: None,
        nearest: snaps.nearest,
        axis_extensions: snaps.axis_extensions,
        exclude_entity: None,
    };
    Ok(drawing
        .snap(context, query)?
        .candidate(context, query)?
        .map_or(unsnapped, |candidate| candidate.point))
}

fn section_elevation_bounds(model: &os_model::Model, marker_level: Id) -> Result<(f64, f64)> {
    let marker = model
        .levels
        .get(&marker_level)
        .ok_or_else(|| Error::Invalid("section marker level is missing".into()))?;
    let building = marker.parameters.building;
    let levels: Vec<_> = model
        .levels
        .values()
        .filter(|level| level.parameters.building == building)
        .collect();
    os_core::ensure(!levels.is_empty(), "section building has no levels")?;

    let mut bottom = f64::INFINITY;
    let mut top = f64::NEG_INFINITY;
    for level in levels {
        let elevation = level.parameters.elevation;
        bottom = bottom.min(elevation);
        // Keep a useful default range even in an empty starter building.
        top = top.max(elevation + 3.0);
    }
    for wall in model.walls.values() {
        let Some(level) = model.levels.get(&wall.parameters.level) else {
            continue;
        };
        if level.parameters.building == building {
            bottom = bottom.min(level.parameters.elevation);
            top = top.max(level.parameters.elevation + wall.parameters.height);
        }
    }
    for floor in model.floors.values() {
        let Some(level) = model.levels.get(&floor.parameters.level) else {
            continue;
        };
        if level.parameters.building == building {
            let floor_top = level.parameters.elevation + floor.parameters.top_offset;
            bottom = bottom.min(floor_top - floor.parameters.thickness);
            top = top.max(floor_top);
        }
    }

    let bounds = (
        bottom - SECTION_VIEW_PADDING_M,
        top + SECTION_VIEW_PADDING_M,
    );
    os_model::SectionViewSettings::new(
        Point2::new(0.0, 0.0),
        Point2::new(1.0, 0.0),
        bounds.0,
        bounds.1,
    )
    .validate()?;
    Ok(bounds)
}

fn opening_placement_preview(
    model: &Model,
    drawing: &PlanDrawing,
    context: PlanContext,
    camera: PlanCamera,
    viewport: [f64; 2],
    pointer: Point2,
    draft: OpeningPlacementDraft,
) -> Result<Option<OpeningPlacementPreview>> {
    let kind = draft.kind;
    let type_id = draft.type_id;
    let point = camera.unproject(pointer, viewport)?;
    if !point_in_plan_crop(context, point) {
        return Ok(None);
    }
    let view = model
        .views
        .get(&context.view_id)
        .ok_or_else(|| Error::Invalid("opening plan no longer exists".into()))?;
    let level = view
        .parameters
        .level
        .ok_or_else(|| Error::Invalid("opening placement requires a floor plan".into()))?;
    let visible: std::collections::BTreeSet<_> = drawing
        .items(context)?
        .iter()
        .map(|item| item.entity)
        .collect();

    let local_hit = drawing.pick_screen(context, camera, viewport, pointer, 7.0)?;
    let picked_host = local_hit.and_then(|id| {
        model.walls.contains_key(&id).then_some(id).or_else(|| {
            model
                .openings
                .get(&id)
                .map(|opening| opening.parameters.host)
        })
    });
    let host_candidate = if let Some(host) = picked_host {
        model.walls.get(&host).and_then(|wall| {
            if wall.parameters.level != level || !visible.contains(&host) {
                return None;
            }
            let start = context.basis.world_to_plane(wall.parameters.start).ok()?;
            let end = context.basis.world_to_plane(wall.parameters.end).ok()?;
            let (t, distance) = project_to_segment(point, start, end);
            let distance_pixels = distance * camera.pixels_per_metre;
            (distance_pixels <= wall.parameters.thickness * camera.pixels_per_metre * 0.5 + 8.0)
                .then_some((host, t, distance_pixels))
        })
    } else if local_hit.is_some() {
        // A dimension, room, grid or provider line has precedence over a wall
        // that might happen to lie underneath it.
        None
    } else {
        model
            .walls
            .iter()
            .filter(|(id, wall)| wall.parameters.level == level && visible.contains(id))
            .filter_map(|(id, wall)| {
                let start = context.basis.world_to_plane(wall.parameters.start).ok()?;
                let end = context.basis.world_to_plane(wall.parameters.end).ok()?;
                let (t, distance) = project_to_segment(point, start, end);
                let distance_pixels = distance * camera.pixels_per_metre;
                (distance_pixels <= wall.parameters.thickness * camera.pixels_per_metre * 0.5 + 8.0)
                    .then_some((*id, t, distance_pixels))
            })
            .min_by(|a, b| a.2.total_cmp(&b.2))
    };
    let Some((host, t, _)) = host_candidate else {
        return Ok(None);
    };
    let wall = &model.walls[&host].parameters;
    let (definition, width) = if let Some(type_id) = type_id {
        let ty = model
            .opening_types
            .get(&type_id)
            .ok_or_else(|| Error::Invalid("selected opening type no longer exists".into()))?;
        os_core::ensure(
            ty.parameters.kind == kind,
            "selected opening type has the wrong kind",
        )?;
        (OpeningDefinition::Typed { type_id }, ty.parameters.width)
    } else {
        let width = if kind == OpeningKind::Door { 0.9 } else { 1.2 };
        (
            OpeningDefinition::Legacy {
                kind,
                width,
                height: if kind == OpeningKind::Door { 2.1 } else { 1.2 },
                sill: if kind == OpeningKind::Door { 0.0 } else { 0.9 },
            },
            width,
        )
    };
    let station = t * wall.length();
    let parameters = OpeningParams {
        hinge: Default::default(),
        swing: Default::default(),
        name: format!("{kind:?}"),
        host,
        offset: station - width * 0.5,
        definition,
    };
    let resolved = model.resolve_opening(&parameters)?;
    let message = resolved
        .validate_host(wall)
        .and_then(|()| {
            for opening in model
                .openings
                .values()
                .filter(|opening| opening.parameters.host == host)
            {
                let existing = model.resolve_opening(&opening.parameters)?;
                os_core::ensure(
                    parameters.offset + resolved.width + 0.001 <= existing.offset
                        || existing.offset + existing.width + 0.001 <= parameters.offset,
                    "openings require at least 1 mm separation along host",
                )?;
            }
            Ok(())
        })
        .err()
        .map(|error| error.to_string());
    Ok(Some(OpeningPlacementPreview {
        host,
        parameters,
        resolved,
        message,
    }))
}
impl PlanWorkspace {
    pub(super) fn sheet_pdf_pending(&self) -> bool {
        self.pending_pdf.is_some()
    }
    #[cfg(test)]
    pub(super) fn ready(&self) -> bool {
        self.drawing.is_some() && self.pending.is_none()
    }
    #[cfg(test)]
    pub(super) fn test_screen_point(&self, view: Id, point: Point2) -> Option<egui::Pos2> {
        let rect = self.canvas_rect?;
        let camera = self.cameras.get(&view).copied().unwrap_or_default();
        let screen = camera
            .project(point, [f64::from(rect.width()), f64::from(rect.height())])
            .ok()?;
        Some(rect.min + egui::vec2(screen.x as f32, screen.y as f32))
    }
    fn poll(&mut self, editor: &Editor) {
        let session = editor.document.session_id();
        if self.session != Some(session) {
            self.session = Some(session);
            self.active = None;
            self.active_sheet = None;
            self.cameras.clear();
        }
        if self
            .active_sheet
            .is_some_and(|id| !editor.document.model().sheets.contains_key(&id))
        {
            self.active_sheet = None;
        }
        if self
            .active
            .is_some_and(|id| !editor.document.model().views.contains_key(&id))
        {
            self.active = None;
        }
        let requested = self.active.map(|id| editor.native_view_context(id));
        let current = requested.as_ref().and_then(|r| r.as_ref().ok()).copied();
        if self.desired != current {
            self.desired = current;
            self.drawing = None;
            self.error = None;
            self.attempted = false;
            if let Some((_, valid)) = &mut self.pending {
                *valid = false;
            }
        }
        if let Some(Err(error)) = requested {
            self.error = Some(error.to_string());
        }
        if self
            .pending
            .as_ref()
            .is_some_and(|(job, _)| job.is_finished())
        {
            let (job, valid) = self.pending.take().expect("finished job");
            let result = job
                .join()
                .unwrap_or_else(|_| Err(Error::Invalid("plan worker failed".into())));
            if valid {
                match result {
                    Ok(drawing) => self.drawing = Some(drawing),
                    Err(error) => self.error = Some(error.to_string()),
                }
            }
        }
        if self.pending.is_none()
            && !self.attempted
            && let Some(context) = self.desired
        {
            self.attempted = true;
            match editor
                .native_view_snapshot(context.view_id)
                .and_then(|snapshot| {
                    std::thread::Builder::new()
                        .name("native-plan".into())
                        .spawn(move || snapshot.derive())
                        .map_err(|e| Error::Invalid(format!("cannot start plan worker: {e}")))
                }) {
                Ok(job) => self.pending = Some((job, true)),
                Err(error) => self.error = Some(error.to_string()),
            }
        }
    }
}

impl DesktopApp {
    fn create_sheet_from_active_view(&mut self) {
        let result = (|| {
            let view_id = self.plans.active.ok_or_else(|| {
                Error::Invalid("Open a Plan or Section view before creating a sheet".into())
            })?;
            let context = self.editor.native_view_context(view_id)?;
            let model = self.editor.document.model();
            let view = model
                .views
                .get(&view_id)
                .ok_or_else(|| Error::Invalid("source drawing view is missing".into()))?;
            let view_name = view.parameters.name.clone();
            os_core::ensure(
                matches!(
                    view.parameters.kind,
                    os_model::ViewKind::Plan | os_model::ViewKind::Section
                ) && view.parameters.validate_edit().is_ok(),
                "sheet source must be a configured Plan or Section view",
            )?;
            let number = (101..=9_999)
                .map(|index| format!("A{index:03}"))
                .find(|number| {
                    !model
                        .sheets
                        .values()
                        .any(|sheet| sheet.parameters.number.eq_ignore_ascii_case(number))
                })
                .ok_or_else(|| Error::Invalid("no available A-series sheet number".into()))?;
            let center = if view.parameters.kind == os_model::ViewKind::Section {
                let crop = context
                    .crop
                    .ok_or_else(|| Error::Invalid("section view has no drawing bounds".into()))?;
                Point2::new(
                    (crop.min.x + crop.max.x) * 0.5,
                    (crop.min.y + crop.max.y) * 0.5,
                )
            } else {
                self.plans
                    .cameras
                    .get(&view_id)
                    .copied()
                    .unwrap_or_default()
                    .center
            };
            let mut parameters = SheetParams::new(
                number,
                if view_name.len() <= 248 {
                    format!("{view_name} - Sheet")
                } else {
                    "View Sheet".into()
                },
            );
            parameters.viewports.push(SheetViewport {
                id: Id::new(),
                view: view_id,
                model_center_m: center,
                paper_center_mm: SHEET_VIEWPORT_CENTER_MM,
                width_mm: SHEET_VIEWPORT_SIZE_MM.x,
                height_mm: SHEET_VIEWPORT_SIZE_MM.y,
                scale_denominator: context.scale_denominator,
                title_override: None,
            });
            let sheet = Sheet::new("core.sheet", parameters);
            let id = sheet.id();
            self.editor
                .command("Create sheet", Command::AddSheet(sheet))?;
            Ok(id)
        })();
        match result {
            Ok(id) => {
                self.plans.active_sheet = Some(id);
                self.report(Ok(()), "Created A3 sheet with one linked view viewport.");
            }
            Err(error) => self.report(Err(error), ""),
        }
    }

    fn open_sheet(&mut self, id: Id) {
        let source_view = self
            .editor
            .document
            .model()
            .sheets
            .get(&id)
            .and_then(|sheet| sheet.parameters.viewports.first())
            .map(|viewport| viewport.view);
        if let Some(view) = source_view {
            self.focus_plan(Some(view));
        }
        self.plans.active_sheet = Some(id);
    }

    fn sheet_page(&self) -> Result<SheetPage> {
        let sheet_id = self
            .plans
            .active_sheet
            .ok_or_else(|| Error::Invalid("no sheet is open".into()))?;
        let model = self.editor.document.model();
        let sheet = model
            .sheets
            .get(&sheet_id)
            .ok_or_else(|| Error::Invalid("sheet is missing".into()))?;
        self.sheet_page_for(&sheet.parameters)
    }

    fn sheet_page_for(&self, parameters: &SheetParams) -> Result<SheetPage> {
        let model = self.editor.document.model();
        parameters.validate(model)?;
        os_core::ensure(
            parameters.schedule_placements.len() <= 1,
            "this preview supports one schedule table per sheet",
        )?;
        os_core::ensure(
            parameters.viewports.len() == 1,
            "this sheet preview supports one drawing viewport; multiple viewports are not rendered yet",
        )?;
        let viewport = &parameters.viewports[0];
        os_core::ensure(
            self.plans.active == Some(viewport.view),
            "switch to the sheet's source view before previewing",
        )?;
        let context = self.editor.native_view_context(viewport.view)?;
        os_core::ensure(
            self.plans.desired == Some(context),
            "source view is refreshing; wait for current graphics",
        )?;
        let native_drawing =
            self.plans.drawing.as_ref().ok_or_else(|| {
                Error::Invalid("source view graphics are still generating".into())
            })?;
        #[cfg(feature = "external-plugins")]
        let is_plan = model
            .views
            .get(&viewport.view)
            .is_some_and(|view| view.parameters.kind == os_model::ViewKind::Plan);
        #[cfg(feature = "external-plugins")]
        let drawing = {
            if is_plan {
                os_core::ensure(
                    !self.plans.providers.busy(),
                    "plan provider graphics are still generating",
                )?;
                os_core::ensure(
                    self.plans.providers.error.is_none()
                        && self.plans.providers.diagnostics.is_empty(),
                    "plan provider graphics are incomplete; resolve provider diagnostics before export",
                )?;
                self.plans
                    .providers
                    .drawing(&self.editor, viewport.view)
                    .unwrap_or(native_drawing)
            } else {
                native_drawing
            }
        };
        #[cfg(not(feature = "external-plugins"))]
        let drawing = native_drawing;
        let view_name = model
            .views
            .get(&viewport.view)
            .ok_or_else(|| Error::Invalid("source view is missing".into()))?
            .parameters
            .name
            .as_str();
        let page_size = parameters.paper_size.dimensions_mm();
        let mut page = compose_view_sheet(
            PaperSheetInfo {
                width_mm: page_size.x,
                height_mm: page_size.y,
                number: &parameters.number,
                name: &parameters.name,
            },
            viewport.title_override.as_deref().unwrap_or(view_name),
            PaperViewport {
                center_mm: viewport.paper_center_mm,
                width_mm: viewport.width_mm,
                height_mm: viewport.height_mm,
                model_center_m: viewport.model_center_m,
                scale_denominator: viewport.scale_denominator,
            },
            context,
            drawing,
        )?;
        for placement in &parameters.schedule_placements {
            let table = crate::opening_schedule::paper_table(model, placement.schedule)?;
            page = os_render::sheet::append_schedule_table(
                page,
                os_render::sheet::PaperRect {
                    min_mm: placement.paper_rect_mm.min_mm,
                    max_mm: placement.paper_rect_mm.max_mm,
                },
                &table,
            )
            .map_err(|e| Error::Invalid(format!("Schedule '{}': {e}", table.heading)))?;
        }
        page.to_pdf()?;
        Ok(page)
    }

    fn place_sheet_schedule(&mut self, sheet: Id, schedule: Option<Id>) {
        let result = (|| {
            let mut parameters = self
                .editor
                .document
                .model()
                .sheets
                .get(&sheet)
                .ok_or_else(|| Error::Invalid("sheet is missing".into()))?
                .parameters
                .clone();
            os_core::ensure(self.plans.active_sheet == Some(sheet), "sheet changed")?;
            os_core::ensure(parameters.viewports.len() == 1, "sheet needs one viewport")?;
            if let Some(schedule) = schedule {
                os_core::ensure(
                    parameters.schedule_placements.is_empty(),
                    "remove the existing table first",
                )?;
                let viewport = &mut parameters.viewports[0];
                viewport.paper_center_mm = Point2::new(210.0, 80.0);
                viewport.width_mm = 384.0;
                viewport.height_mm = 124.0;
                parameters
                    .schedule_placements
                    .push(os_model::SheetSchedulePlacement {
                        id: Id::new(),
                        schedule,
                        paper_rect_mm: os_model::SheetPaperRect {
                            min_mm: Point2::new(18.0, 154.0),
                            max_mm: Point2::new(402.0, 238.0),
                        },
                    });
                // Fit-check current rows and current drawing before any model/history mutation.
                self.sheet_page_for(&parameters)?;
            } else {
                parameters.schedule_placements.clear();
            }
            self.editor.command(
                "Place schedule on sheet",
                Command::UpdateSheet {
                    id: sheet,
                    parameters,
                },
            )
        })();
        self.report(result, "Sheet table placement updated.");
    }

    fn apply_sheet_layout(&mut self, id: Id, scale_denominator: f64, center_mm: Point2) {
        let result = (|| {
            let mut parameters = self
                .editor
                .document
                .model()
                .sheets
                .get(&id)
                .ok_or_else(|| Error::Invalid("sheet is missing".into()))?
                .parameters
                .clone();
            os_core::ensure(parameters.viewports.len() == 1, "sheet needs one viewport")?;
            parameters.viewports[0].scale_denominator = scale_denominator;
            parameters.viewports[0].paper_center_mm = center_mm;
            self.editor.command(
                "Place view viewport",
                Command::UpdateSheet { id, parameters },
            )
        })();
        self.report(result, "Sheet viewport layout updated.");
    }

    fn prepare_sheet_pdf(&mut self, page: &SheetPage) {
        let result = (|| {
            let path = sheet_pdf_path(&self.plans.pdf_path)?;
            let bytes = page.to_pdf()?;
            Ok(PendingSheetPdf {
                replace: path.exists(),
                path,
                bytes,
                session: self.editor.document.session_id(),
                revision: self.editor.document.revision(),
            })
        })();
        match result {
            Ok(pending) => self.plans.pending_pdf = Some(pending),
            Err(error) => self.report(Err(error), ""),
        }
    }

    pub(super) fn sheet_pdf_confirmation(&mut self, ctx: &egui::Context) {
        let Some(pending) = self.plans.pending_pdf.take() else {
            return;
        };
        if pending.session != self.editor.document.session_id()
            || pending.revision != self.editor.document.revision()
        {
            self.report(
                Err(Error::Invalid(
                    "PDF export cancelled: document changed; prepare a current preview.".into(),
                )),
                "",
            );
            return;
        }
        let mut accept = false;
        let mut cancel = false;
        egui::Modal::new(egui::Id::new("confirm_sheet_pdf_export")).show(ctx, |ui| {
            ui.set_width(460.0);
            ui.heading("Export one-page vector PDF?");
            ui.label(pending.path.display().to_string());
            ui.label("The file contains the current committed view graphics at the sheet scale.");
            ui.label("PDF text uses built-in Helvetica/WinAnsi; fonts are not embedded.");
            ui.label("This does not save the native project or change its dirty state.");
            if pending.replace {
                ui.colored_label(theme::ERROR, "The existing PDF will be replaced.");
            }
            ui.horizontal(|ui| {
                cancel = ui.button("Cancel").clicked();
                accept = ui
                    .button(if pending.replace {
                        "Replace PDF"
                    } else {
                        "Export PDF"
                    })
                    .clicked();
            });
        });
        if accept {
            if !pending.replace && pending.path.exists() {
                self.plans.pending_pdf = Some(PendingSheetPdf {
                    replace: true,
                    ..pending
                });
            } else {
                let result = write_sheet_pdf(&pending.path, &pending.bytes, pending.replace);
                self.report(
                    result,
                    &format!("Exported vector sheet to {}.", pending.path.display()),
                );
            }
        } else if !cancel {
            self.plans.pending_pdf = Some(pending);
        }
    }

    fn sheet_workspace(&mut self, ui: &mut egui::Ui) {
        let Some(id) = self.plans.active_sheet else {
            return;
        };
        let sheet = self.editor.document.model().sheets.get(&id).cloned();
        let Some(sheet) = sheet else {
            self.plans.active_sheet = None;
            return;
        };
        ui.horizontal_wrapped(|ui| {
            ui.label(format!(
                "{} - {}",
                sheet.parameters.number, sheet.parameters.name
            ));
            let source_kind = sheet
                .parameters
                .viewports
                .first()
                .and_then(|viewport| self.editor.document.model().views.get(&viewport.view))
                .map(|view| view.parameters.kind);
            if ui
                .button(if source_kind == Some(os_model::ViewKind::Section) {
                    "Edit source section"
                } else {
                    "Edit source plan"
                })
                .clicked()
            {
                self.plans.active_sheet = None;
            }
            if sheet.parameters.viewports.len() == 1 {
                let viewport = &sheet.parameters.viewports[0];
                let mut scale = viewport.scale_denominator;
                let mut center = viewport.paper_center_mm;
                ui.label("Scale 1:");
                ui.add(
                    egui::DragValue::new(&mut scale)
                        .range(0.001..=1_000_000.0)
                        .speed(1.0)
                        .max_decimals(3),
                );
                ui.label("Center mm X/Y:");
                ui.add(
                    egui::DragValue::new(&mut center.x)
                        .speed(0.5)
                        .max_decimals(1),
                );
                ui.add(
                    egui::DragValue::new(&mut center.y)
                        .speed(0.5)
                        .max_decimals(1),
                );
                if ui.button("Apply layout").clicked() {
                    self.apply_sheet_layout(id, scale, center);
                }
            }
            ui.label("PDF path");
            ui.add(
                egui::TextEdit::singleline(&mut self.plans.pdf_path)
                    .desired_width(220.0)
                    .char_limit(1024),
            );
        });
        ui.horizontal_wrapped(|ui| {
            if sheet.parameters.schedule_placements.is_empty() {
                let schedules = &self.editor.document.model().schedules;
                if self
                    .plans
                    .sheet_schedule
                    .is_none_or(|id| !schedules.contains_key(&id))
                {
                    self.plans.sheet_schedule = schedules.keys().next().copied();
                }
                egui::ComboBox::from_id_salt("sheet_schedule_picker")
                    .selected_text(
                        self.plans
                            .sheet_schedule
                            .and_then(|id| schedules.get(&id))
                            .map(|s| s.parameters.name.as_str())
                            .unwrap_or("Create a saved schedule in View"),
                    )
                    .show_ui(ui, |ui| {
                        for schedule in schedules.values() {
                            ui.selectable_value(
                                &mut self.plans.sheet_schedule,
                                Some(schedule.id()),
                                &schedule.parameters.name,
                            );
                        }
                    });
                if ui
                    .add_enabled(
                        self.plans.sheet_schedule.is_some(),
                        egui::Button::new("Add saved schedule to sheet"),
                    )
                    .clicked()
                {
                    self.place_sheet_schedule(id, self.plans.sheet_schedule);
                }
            } else if ui.button("Remove schedule table").clicked() {
                self.place_sheet_schedule(id, None);
            }
        });
        ui.label("Table layout: view 384 × 124 mm at (210,80); table 384 × 84 mm at top-left (18,154). One page; all rows must fit. Undo restores the prior layout.");
        let page = self.sheet_page();
        if let Err(error) = &page {
            ui.colored_label(theme::ERROR, error.to_string());
        }
        if ui
            .add_enabled(page.is_ok(), egui::Button::new("Export vector PDF"))
            .clicked()
            && let Ok(page) = &page
        {
            self.prepare_sheet_pdf(page);
        }
        let (response, painter) = ui.allocate_painter(ui.available_size(), egui::Sense::hover());
        painter.rect_filled(response.rect, 0.0, theme::CANVAS);
        if let Ok(page) = page
            && let Err(error) = paint_sheet_page(&painter, response.rect, &page)
        {
            ui.colored_label(theme::ERROR, error.to_string());
        }
    }

    pub(super) fn cancel_plan_wall(&mut self) {
        self.plans.column_placement = None;
        self.plans.endpoint_drag = None;
        self.wall_gesture = None;
    }

    pub(super) fn begin_aligned_dimension(&mut self, view: Id) {
        self.begin_dimension(view, os_model::DimensionLayout::Aligned);
    }

    pub(super) fn begin_dimension(&mut self, view: Id, layout: os_model::DimensionLayout) {
        let result = self.editor.native_plan_context(view);
        match result {
            Ok(context) => {
                let drawing_identity = self
                    .plans
                    .drawing
                    .as_ref()
                    .filter(|drawing| drawing.items(context).is_ok())
                    .map(PlanDrawing::identity);
                self.cancel_plan_wall();
                self.cancel_opening_placement();
                self.plans.floor_sketch = None;
                self.plans.room_placement_active = false;
                self.plans.dimension_draft = Some(DimensionDraft {
                    layout,
                    additional: Vec::new(),
                    placing: false,
                    provider_signature: plan_provider_signature(&self.editor),
                    drawing_identity,
                    context,
                    first: None,
                    second: None,
                });
                let guidance = match layout {
                    os_model::DimensionLayout::Aligned => {
                        "Aligned dimension · select two wall endpoints, then place the line offset. Escape cancels."
                    }
                    os_model::DimensionLayout::Chain | os_model::DimensionLayout::Baseline => {
                        "Dimension · select wall endpoints in order. Finish anchors, then place offset. Backspace removes an anchor; Escape cancels."
                    }
                    os_model::DimensionLayout::Angular => {
                        "Angular dimension · select two nonparallel walls near the ends that set the rays, then click inside the angle to set its radius. Backspace removes an anchor; Escape cancels."
                    }
                };
                self.report(Ok(()), guidance);
            }
            Err(error) => self.report(Err(error), ""),
        }
    }

    pub(super) fn cancel_aligned_dimension(&mut self) {
        self.plans.detail_line_draft = None;
        self.plans.room_separation_line_draft = None;
        self.plans.dimension_draft = None;
        self.plans.room_tag_draft = None;
    }

    pub(super) fn begin_opening_placement(&mut self, kind: OpeningKind) {
        let Some(view) = self.plans.active else {
            self.report(Err(Error::Invalid("Open a floor plan first".into())), "");
            return;
        };
        match self.editor.native_plan_context(view) {
            Ok(context) => {
                self.cancel_plan_wall();
                self.cancel_aligned_dimension();
                self.plans.floor_sketch = None;
                self.plans.room_placement_active = false;
                self.opening_draft = None;
                let model = self.editor.document.model();
                let type_id = self
                    .preferred_type(kind)
                    .filter(|id| {
                        model
                            .opening_types
                            .get(id)
                            .is_some_and(|t| t.parameters.kind == kind)
                    })
                    .or_else(|| {
                        model
                            .opening_types
                            .iter()
                            .find(|(_, ty)| ty.parameters.kind == kind)
                            .map(|(id, _)| *id)
                    });
                self.plans.opening_placement = Some(OpeningPlacementDraft {
                    context,
                    activation: self.editor.host.activation_id(os_walls::PLUGIN_ID),
                    session: self.editor.document.session_id(),
                    revision: self.editor.document.revision(),
                    kind,
                    type_id,
                });
                self.report(
                    Ok(()),
                    match kind {
                        OpeningKind::Door => {
                            "Place door · hover over a visible wall, click to place, Escape to exit."
                        }
                        OpeningKind::Window => "Place window · hover over a visible wall, click to place, Escape to exit.",
                    },
                );
            }
            Err(error) => self.report(Err(error), ""),
        }
    }

    pub(super) fn cancel_opening_placement(&mut self) {
        self.plans.opening_placement = None;
    }

    fn begin_section_placement(&mut self, view: Id) {
        let result = (|| {
            let context = self.editor.native_plan_context(view)?;
            let marker_level = self
                .editor
                .document
                .model()
                .views
                .get(&view)
                .and_then(|view| view.parameters.level)
                .ok_or_else(|| Error::Invalid("floor plan has no marker level".into()))?;
            let (bottom_elevation, top_elevation) =
                section_elevation_bounds(self.editor.document.model(), marker_level)?;
            Ok((context, bottom_elevation, top_elevation))
        })();
        match result {
            Ok((context, bottom_elevation, top_elevation)) => {
                self.cancel_plan_wall();
                self.cancel_aligned_dimension();
                self.cancel_opening_placement();
                self.plans.crop.cancel();
                self.plans.floor_sketch = None;
                self.plans.room_placement_active = false;
                self.plans.section_placement = Some(SectionPlacementDraft {
                    context,
                    session: self.editor.document.session_id(),
                    revision: self.editor.document.revision(),
                    provider_signature: plan_provider_signature(&self.editor),
                    first: None,
                    bottom_elevation,
                    top_elevation,
                });
                self.status = "Section marker · click two points in plan · Escape cancels".into();
                self.status_error = false;
            }
            Err(error) => self.report(Err(error), ""),
        }
    }

    fn commit_section_placement(&mut self, start: Point2, end: Point2) -> Result<Id> {
        let draft = self
            .plans
            .section_placement
            .as_ref()
            .ok_or_else(|| Error::Invalid("section placement is no longer active".into()))?;
        let marker_level = self
            .editor
            .document
            .model()
            .views
            .get(&draft.context.view_id)
            .and_then(|view| view.parameters.level)
            .ok_or_else(|| Error::Invalid("floor plan marker level is missing".into()))?;
        let settings = os_model::SectionViewSettings::new(
            start,
            end,
            draft.bottom_elevation,
            draft.top_elevation,
        );
        settings.validate()?;
        let model = self.editor.document.model();
        let suffix = (1..=9_999)
            .map(|number| format!("Section {number}"))
            .find(|name| {
                !model
                    .views
                    .values()
                    .any(|view| view.parameters.name.eq_ignore_ascii_case(name))
            })
            .ok_or_else(|| Error::Invalid("no available section view name".into()))?;
        self.editor
            .create_section_view(&suffix, marker_level, settings)
    }

    fn begin_floor_sketch(&mut self, view: Id) {
        match self.editor.native_plan_context(view) {
            Ok(context) => {
                self.cancel_plan_wall();
                self.cancel_aligned_dimension();
                self.cancel_opening_placement();
                self.plans.room_placement_active = false;
                self.plans.floor_sketch = Some(FloorSketchDraft {
                    context,
                    session: self.editor.document.session_id(),
                    revision: self.editor.document.revision(),
                    provider_signature: plan_provider_signature(&self.editor),
                    points: Vec::new(),
                    thickness: "0.2".into(),
                    top_offset: "0".into(),
                });
                self.status = "Floor boundary · click vertices, then Finish floor or close to the first point. Escape cancels.".into();
                self.status_error = false;
            }
            Err(error) => self.report(Err(error), ""),
        }
    }

    fn finish_floor_sketch(&mut self) {
        let result = (|| {
            let draft = self
                .plans
                .floor_sketch
                .as_ref()
                .ok_or_else(|| Error::Invalid("floor sketch is not active".into()))?;
            let context = draft.context;
            os_core::ensure(
                self.plans.active == Some(context.view_id)
                    && self.editor.native_plan_context(context.view_id)? == context
                    && self.editor.document.session_id() == draft.session
                    && self.editor.document.revision() == draft.revision,
                "floor sketch context is stale",
            )?;
            let view = self
                .editor
                .document
                .model()
                .views
                .get(&context.view_id)
                .ok_or_else(|| Error::Invalid("floor plan no longer exists".into()))?;
            let level = view
                .parameters
                .level
                .ok_or_else(|| Error::Invalid("floor needs a plan level".into()))?;
            let parameters = os_model::FloorParams {
                name: format!("Floor {}", self.editor.document.model().floors.len() + 1),
                level,
                material: None,
                boundary: draft.points.clone(),
                thickness: draft.thickness.trim().parse().map_err(|_| {
                    Error::Invalid("floor thickness must be a number in metres".into())
                })?,
                top_offset: draft.top_offset.trim().parse().map_err(|_| {
                    Error::Invalid("floor top offset must be a number in metres".into())
                })?,
            };
            parameters.validate()?;
            let floor = os_model::Floor::new("core.floor", parameters);
            let id = floor.id();
            self.editor
                .command("Create floor", Command::AddFloor(floor))?;
            self.plans.floor_sketch = None;
            self.select(Some(id));
            Ok(id)
        })();
        self.report(result.map(|_| ()), "Floor created.");
    }

    fn commit_opening_placement(&mut self, preview: OpeningPlacementPreview) -> Result<Id> {
        let draft = self
            .plans
            .opening_placement
            .as_ref()
            .ok_or_else(|| Error::Invalid("opening placement tool is not active".into()))?;
        let context = draft.context;
        let selected_type = draft.type_id;
        os_core::ensure(
            self.plans.active == Some(context.view_id)
                && self.editor.native_plan_context(context.view_id)? == context
                && self.editor.host.activation_id(os_walls::PLUGIN_ID) == draft.activation
                && self.editor.document.session_id() == draft.session
                && self.editor.document.revision() == draft.revision,
            "opening placement context is stale",
        )?;
        if let Some(message) = preview.message {
            return Err(Error::Invalid(message));
        }
        let kind = preview.resolved.kind;
        let mut parameters = preview.parameters;
        let mut commands = Vec::new();
        let type_id = if let Some(type_id) = selected_type {
            type_id
        } else {
            let resolved = preview.resolved;
            let opening_type = OpeningType::new(
                "core.opening_type",
                OpeningTypeParams {
                    family: Default::default(),
                    name: match resolved.kind {
                        OpeningKind::Door => "Basic Door 900 × 2100".into(),
                        OpeningKind::Window => "Basic Window 1200 × 1200".into(),
                    },
                    kind: resolved.kind,
                    width: resolved.width,
                    height: resolved.height,
                    sill: resolved.sill,
                    pane_position: resolved.pane_position,
                },
            );
            let id = opening_type.id();
            commands.push(Command::AddOpeningType(opening_type));
            id
        };
        parameters.definition = OpeningDefinition::Typed { type_id };
        let opening = Opening::new("core.opening", parameters);
        let id = opening.id();
        commands.push(Command::AddOpening(opening));
        self.editor
            .document
            .execute("Place door or window", commands)?;
        self.editor.regenerate()?;
        self.remember_type(kind, Some(type_id));
        self.select(Some(id));
        if let Some(draft) = self.plans.opening_placement.as_mut() {
            draft.context = self.editor.native_plan_context(context.view_id)?;
            draft.session = self.editor.document.session_id();
            draft.revision = self.editor.document.revision();
            draft.type_id = Some(type_id);
        }
        Ok(id)
    }

    fn commit_aligned_dimension(&mut self, placement: Point2) -> Result<Id> {
        let draft = self
            .plans
            .dimension_draft
            .as_ref()
            .ok_or_else(|| Error::Invalid("aligned dimension tool is not active".into()))?;
        let context = draft.context;
        os_core::ensure(
            self.plans.active == Some(context.view_id)
                && self.editor.native_plan_context(context.view_id)? == context
                && draft.provider_signature == plan_provider_signature(&self.editor)
                && self.plans.drawing.as_ref().is_some_and(|drawing| {
                    drawing.items(context).is_ok()
                        && draft
                            .drawing_identity
                            .is_none_or(|identity| drawing.identity() == identity)
                }),
            "dimension draft is stale",
        )?;
        let first = draft
            .first
            .as_ref()
            .ok_or_else(|| Error::Invalid("first dimension endpoint is missing".into()))?;
        let second = draft
            .second
            .as_ref()
            .ok_or_else(|| Error::Invalid("second dimension endpoint is missing".into()))?;
        if draft.layout == os_model::DimensionLayout::Angular {
            let parameters =
                Self::angular_parameters(draft, self.editor.document.model(), placement)?;
            let dimension = os_model::Dimension::new("core.dimension", parameters);
            let id = dimension.id();
            self.editor
                .command("Add angular dimension", Command::AddDimension(dimension))?;
            self.cancel_aligned_dimension();
            self.select(Some(id));
            return Ok(id);
        }
        let dx = second.point.x - first.point.x;
        let dy = second.point.y - first.point.y;
        let length = dx.hypot(dy);
        os_core::ensure(
            length.is_finite() && length > 1e-6,
            "dimension endpoints must be distinct",
        )?;
        os_core::ensure(
            point_in_plan_crop(context, placement),
            "dimension line placement is outside the plan crop",
        )?;
        let normal = Point2::new(-dy / length, dx / length);
        let offset_m =
            (placement.x - first.point.x) * normal.x + (placement.y - first.point.y) * normal.y;
        os_core::ensure(offset_m.is_finite(), "dimension offset is not finite")?;
        let parameters = DimensionParams {
            layout: draft.layout,
            additional: draft.additional.iter().map(|a| a.reference).collect(),
            baseline_spacing_m: 0.25,
            view: context.view_id,
            first: first.reference,
            second: second.reference,
            offset_m,
            orphan_hint: context.basis.plane_to_world(placement)?,
        };
        let dimension = os_model::Dimension::new("core.dimension", parameters);
        let id = dimension.id();
        self.editor
            .command("Add dimension", Command::AddDimension(dimension))?;
        self.cancel_aligned_dimension();
        self.select(Some(id));
        Ok(id)
    }

    fn angular_parameters(
        draft: &DimensionDraft,
        model: &os_model::Model,
        placement: Point2,
    ) -> Result<DimensionParams> {
        os_core::ensure(
            point_in_plan_crop(draft.context, placement),
            "angular placement outside crop",
        )?;
        let mut params = DimensionParams {
            layout: os_model::DimensionLayout::Angular,
            additional: Vec::new(),
            baseline_spacing_m: 0.25,
            view: draft.context.view_id,
            first: draft
                .first
                .as_ref()
                .ok_or_else(|| Error::Invalid("first wall missing".into()))?
                .reference,
            second: draft
                .second
                .as_ref()
                .ok_or_else(|| Error::Invalid("second wall missing".into()))?
                .reference,
            offset_m: 1.0,
            orphan_hint: draft.context.basis.plane_to_world(placement)?,
        };
        params.place_angular(model, params.orphan_hint)?;
        Ok(params)
    }

    fn dimension_anchor_at(
        &self,
        drawing: &PlanDrawing,
        context: PlanContext,
        camera: PlanCamera,
        rect: egui::Rect,
        pointer: egui::Pos2,
    ) -> Result<DimensionAnchorDraft> {
        let view = self
            .editor
            .document
            .model()
            .views
            .get(&context.view_id)
            .ok_or_else(|| Error::Invalid("dimension plan no longer exists".into()))?;
        let level = view
            .parameters
            .level
            .ok_or_else(|| Error::Invalid("dimension plan has no level".into()))?;
        let visible: std::collections::BTreeSet<_> = drawing
            .items(context)?
            .iter()
            .map(|item| item.entity)
            .collect();
        let local = Point2::new(
            f64::from(pointer.x - rect.left()),
            f64::from(pointer.y - rect.top()),
        );
        if self
            .plans
            .dimension_draft
            .as_ref()
            .is_some_and(|d| d.layout == os_model::DimensionLayout::Angular)
        {
            let plane =
                camera.unproject(local, [f64::from(rect.width()), f64::from(rect.height())])?;
            os_core::ensure(point_in_plan_crop(context, plane), "wall pick outside crop")?;
            let wall = drawing
                .items(context)?
                .iter()
                .rev()
                .find(|item| {
                    item.footprint.contains(plane)
                        && self
                            .editor
                            .document
                            .model()
                            .walls
                            .get(&item.entity)
                            .is_some_and(|w| w.parameters.level == level)
                })
                .ok_or_else(|| Error::Invalid("Choose a visible native wall body".into()))?;
            return Ok(DimensionAnchorDraft {
                reference: DimensionReference {
                    wall: wall.entity,
                    endpoint: DimensionEndpoint::End,
                },
                point: plane,
            });
        }
        let mut candidates = Vec::new();
        for (id, wall) in &self.editor.document.model().walls {
            if wall.parameters.level != level || !visible.contains(id) {
                continue;
            }
            for (endpoint, world) in [
                (DimensionEndpoint::Start, wall.parameters.start),
                (DimensionEndpoint::End, wall.parameters.end),
            ] {
                let point = context.basis.world_to_plane(world)?;
                if !point_in_plan_crop(context, point) {
                    continue;
                }
                let screen =
                    camera.project(point, [f64::from(rect.width()), f64::from(rect.height())])?;
                let distance = local.distance(screen);
                if distance <= 12.0 {
                    candidates.push((distance, *id, endpoint, point));
                }
            }
        }
        let (_, wall, endpoint, point) = candidates
            .into_iter()
            .min_by(|a, b| {
                a.0.total_cmp(&b.0)
                    .then_with(|| a.1.cmp(&b.1))
                    .then_with(|| {
                        (a.2 == DimensionEndpoint::End).cmp(&(b.2 == DimensionEndpoint::End))
                    })
            })
            .ok_or_else(|| {
                Error::Invalid("Choose a visible native wall endpoint within 12 px".into())
            })?;
        Ok(DimensionAnchorDraft {
            reference: DimensionReference { wall, endpoint },
            point,
        })
    }

    pub(super) fn finish_endpoint_input(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.key_pressed(egui::Key::Escape))
            || self.plans.active_sheet.is_some()
            || self
                .plans
                .opening_move
                .as_ref()
                .is_some_and(|d| !d.current(&self.editor, self.plans.active, self.selected))
        {
            self.plans.opening_move = None;
        }
        if !ctx.input(|i| i.pointer.primary_down()) {
            self.plans.opening_move = None;
            self.plans.opening_move_claimed = false;
            self.plans.crop.claimed = false;
            self.plans.room_tag_pointer_claimed = false;
            self.plans.detail_line_pointer_claimed = false;
            self.plans.room_separation_line_pointer_claimed = false;
            if self.plans.endpoint_drag.is_some() {
                self.cancel_plan_wall();
            }
            self.plans.endpoint_pointer_claimed = false;
        }
    }

    fn begin_plan_wall(&mut self, view: Id) {
        self.plans.column_placement = None;
        self.cancel_aligned_dimension();
        self.cancel_opening_placement();
        self.plans.room_placement_active = false;
        #[cfg(feature = "external-plugins")]
        if self.editor.plugin_work_pending() {
            self.report(
                Err(Error::Invalid(
                    "Finish or cancel pending plugin work before starting a wall".into(),
                )),
                "",
            );
            return;
        }
        #[cfg(feature = "external-plugins")]
        let installed = self
            .editor
            .host
            .worker_supported(os_plugin_api::wall::OWNER);
        #[cfg(feature = "external-plugins")]
        let result = if installed {
            crate::plan_gesture::WallGesture::begin_installed(
                &self.editor,
                view,
                self.draft.clone(),
            )
        } else {
            crate::plan_gesture::WallGesture::begin(&self.editor, view, self.draft.clone())
        };
        #[cfg(not(feature = "external-plugins"))]
        let result =
            crate::plan_gesture::WallGesture::begin(&self.editor, view, self.draft.clone());
        match result {
            Ok(gesture) => self.wall_gesture = Some(gesture),
            Err(error) => self.report(Err(error), ""),
        }
    }

    fn begin_plan_wall_edit(&mut self, view: Id, id: Id, mode: crate::plan_gesture::WallEdit) {
        self.cancel_aligned_dimension();
        self.cancel_opening_placement();
        self.plans.room_placement_active = false;
        #[cfg(feature = "external-plugins")]
        if self.editor.plugin_work_pending() {
            self.report(
                Err(Error::Invalid(
                    "Finish or cancel pending plugin work before editing a wall".into(),
                )),
                "",
            );
            return;
        }
        #[cfg(feature = "external-plugins")]
        let result = if self
            .editor
            .host
            .worker_supported(os_plugin_api::wall::OWNER)
        {
            crate::plan_gesture::WallGesture::begin_edit_installed(&self.editor, view, id, mode)
        } else {
            crate::plan_gesture::WallGesture::begin_edit(&self.editor, view, id, mode)
        };
        #[cfg(not(feature = "external-plugins"))]
        let result = crate::plan_gesture::WallGesture::begin_edit(&self.editor, view, id, mode);
        match result {
            Ok(gesture) => self.wall_gesture = Some(gesture),
            Err(error) => self.report(Err(error), ""),
        }
    }

    fn commit_plan_wall(&mut self, point: Point2) {
        let Some(gesture) = &self.wall_gesture else {
            return;
        };
        #[cfg(feature = "external-plugins")]
        if gesture.is_installed() {
            let prepared = gesture.installed_command(&self.editor, self.plans.active, point);
            let result = prepared.and_then(|(draft, context, view)| {
                self.submit_installed_wall(draft, context, view)
            });
            self.report(
                result,
                "Wall command pending… Escape or Cancel plugin command revokes it.",
            );
            return;
        }
        let result = gesture.commit(&mut self.editor, self.plans.active, point);
        if result.is_ok() {
            self.cancel_plan_wall();
            self.select(self.selected);
        }
        self.report(result, "Wall gesture applied.");
    }
    pub(super) fn focus_plan(&mut self, id: Option<Id>) {
        if self.plans.active_sheet.is_some_and(|sheet_id| {
            self.editor
                .document
                .model()
                .sheets
                .get(&sheet_id)
                .and_then(|sheet| sheet.parameters.viewports.first())
                .is_none_or(|viewport| Some(viewport.view) != id)
        }) {
            self.plans.active_sheet = None;
        }
        if self.plans.active != id {
            self.cancel_plan_wall();
            self.plans.room_placement_active = false;
            self.cancel_aligned_dimension();
            self.cancel_opening_placement();
            self.plans.section_placement = None;
            self.plans.floor_sketch = None;
        }
        self.plans.active = id;
        if let Some(level) = id
            .and_then(|id| self.editor.document.model().views.get(&id))
            .and_then(|v| v.parameters.level)
        {
            self.set_active_level(level);
        }
    }
    pub(super) fn plan_workspace(&mut self, ctx: &egui::Context) {
        self.plans.poll(&self.editor);
        if self.plans.drawing.is_none() {
            self.plans.crop.cancel();
        }
        self.plans.crop.validate(
            &self.editor,
            self.plans.active,
            None,
            ctx.input(|i| i.key_pressed(egui::Key::Escape)) || self.plan_draft.is_some(),
        );
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(theme::CANVAS))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .selectable_label(self.plans.active.is_none(), "3D")
                        .clicked()
                    {
                        self.focus_plan(None);
                    }
                    let views: Vec<_> = self
                        .editor
                        .document
                        .model()
                        .views
                        .values()
                        .filter(|v| {
                            matches!(
                                v.parameters.kind,
                                os_model::ViewKind::Plan | os_model::ViewKind::Section
                            )
                        })
                        .map(|v| {
                            let name = match v.parameters.kind {
                                os_model::ViewKind::Plan => v.parameters.name.clone(),
                                os_model::ViewKind::Section => {
                                    format!("Section · {}", v.parameters.name)
                                }
                                os_model::ViewKind::Perspective => unreachable!(),
                            };
                            (v.id(), name)
                        })
                        .collect();
                    egui::ComboBox::from_id_salt("named_plan_picker")
                        .selected_text(
                            views
                                .iter()
                                .find(|(id, _)| Some(*id) == self.plans.active)
                                .map_or("Choose view", |(_, name)| name.as_str()),
                        )
                        .show_ui(ui, |ui| {
                            for (id, name) in views {
                                if ui
                                    .selectable_label(self.plans.active == Some(id), name)
                                    .clicked()
                                {
                                    self.focus_plan(Some(id));
                                }
                            }
                        });
                    if ui
                        .button("New floor plan")
                        .on_hover_text("Create a named plan on the active level.")
                        .clicked()
                    {
                        let name =
                            format!("Floor plan {}", self.editor.document.model().views.len());
                        match self.editor.create_floor_plan(&name, self.active_level) {
                            Ok(id) => {
                                self.focus_plan(Some(id));
                                self.report(Ok(()), "Floor plan created.");
                            }
                            Err(error) => self.report(Err(error), ""),
                        }
                    }
                    let can_place_section = self
                        .plans
                        .active
                        .and_then(|id| self.editor.document.model().views.get(&id))
                        .is_some_and(|view| view.parameters.kind == os_model::ViewKind::Plan);
                    if ui
                        .add_enabled(
                            can_place_section,
                            egui::Button::new("Place section"),
                        )
                        .on_hover_text(
                            "Click two points in a floor plan to create a linked building section.",
                        )
                        .clicked()
                        && let Some(view) = self.plans.active
                    {
                        self.begin_section_placement(view);
                    }
                    let sheets: Vec<_> = self
                        .editor
                        .document
                        .model()
                        .sheets
                        .values()
                        .map(|sheet| {
                            (
                                sheet.id(),
                                format!(
                                    "{} - {}",
                                    sheet.parameters.number, sheet.parameters.name
                                ),
                            )
                        })
                        .collect();
                    let sheets_label = self
                        .plans
                        .active_sheet
                        .and_then(|id| sheets.iter().find(|(sheet_id, _)| *sheet_id == id))
                        .map_or_else(|| "Sheets".to_owned(), |(_, label)| label.clone());
                    ui.menu_button(sheets_label, |ui| {
                        for (id, label) in sheets {
                            if ui
                                .selectable_label(self.plans.active_sheet == Some(id), label)
                                .clicked()
                            {
                                self.open_sheet(id);
                            }
                        }
                        ui.separator();
                        if ui
                            .add_enabled(
                                self.plans.active.and_then(|id| self.editor.document.model().views.get(&id))
                                    .is_some_and(|view| matches!(view.parameters.kind,
                                        os_model::ViewKind::Plan | os_model::ViewKind::Section)
                                        && view.parameters.validate_edit().is_ok()),
                                egui::Button::new("New sheet from view"),
                            )
                            .clicked()
                        {
                            self.create_sheet_from_active_view();
                        }
                    });
                    ui.checkbox(&mut self.plans.split, "Split 2D / 3D");
                    let crop_enabled = self.plans.active.and_then(|id| self.editor.document.model().views.get(&id))
                        .is_some_and(|view| view.parameters.kind == os_model::ViewKind::Plan
                            && view.parameters.plan.is_some_and(|settings| settings.crop.is_some()));
                    if ui.add_enabled(crop_enabled, egui::Button::new("Edit Crop").selected(self.plans.crop.mode.is_some())).clicked() {
                        self.toggle_crop_edit();
                    }
                    if ui
                        .add_enabled(
                            self.plans.active.and_then(|id| self.editor.document.model().views.get(&id))
                                .is_some_and(|view| view.parameters.kind == os_model::ViewKind::Plan),
                            egui::Button::new("Plan settings"),
                        )
                        .clicked()
                        && let Some(id) = self.plans.active
                    {
                        match crate::plan_settings::PlanDraft::begin(&self.editor, id) {
                            Ok(draft) => self.plan_draft = Some(draft),
                            Err(error) => self.report(Err(error), ""),
                        }
                    }
                });
                // UI view changes invalidate work before either drawing or picking.
                if self.plan_draft.is_some() { self.cancel_plan_wall(); }
                let toolbar = ui.horizontal_wrapped(|ui| {
                    if self.plans.active_sheet.is_some() {
                        ui.label("Paper preview - edit scale and viewport placement below.");
                    } else if self.plans.active
                        .and_then(|id| self.editor.document.model().views.get(&id))
                        .is_some_and(|view| view.parameters.kind == os_model::ViewKind::Plan)
                    {
                    ui.menu_button(if self.plans.snaps.enabled { "Snaps" } else { "Snaps off" }, |ui| {
                        let snaps = &mut self.plans.snaps;
                        ui.checkbox(&mut snaps.enabled, "Enable snapping");
                        ui.separator();
                        ui.checkbox(&mut snaps.endpoints, "Endpoints");
                        ui.checkbox(&mut snaps.intersections, "Intersections");
                        ui.checkbox(&mut snaps.perpendicular, "Perpendicular from anchor");
                        ui.checkbox(&mut snaps.midpoints, "Midpoints");
                        ui.checkbox(&mut snaps.nearest, "Nearest lines and grid axes");
                        ui.checkbox(&mut snaps.axis_extensions, "Line axis extensions");
                        ui.label("Priority: endpoint, intersection, perpendicular, midpoint, grid axis, nearest.");
                        ui.label("Axis extensions are last priority; they do not extend model geometry.");
                        ui.label("Exact input overrides snapping. Session preferences only.");
                    });
                    if let Some(draft) = self.plans.section_placement.as_ref() {
                        ui.label(if draft.first.is_some() {
                            "Section marker · click the second point"
                        } else {
                            "Section marker · click the first point"
                        });
                        if ui.button("Cancel section").clicked() {
                            self.plans.section_placement = None;
                            self.status = "Section placement canceled.".into();
                            self.status_error = false;
                        }
                    } else if self.plans.column_placement.is_some() {
                        ui.label("Column · click center · Escape cancels");
                        if ui.button("Cancel column").clicked() { self.plans.column_placement = None; }
                    } else if let Some(draft) = self.plans.floor_sketch.as_mut() {
                        ui.label(format!("Floor boundary · {} vertices · metres", draft.points.len()));
                        ui.label("Thickness");
                        ui.add(egui::TextEdit::singleline(&mut draft.thickness).desired_width(56.0).char_limit(32));
                        ui.label("Top offset");
                        ui.add(egui::TextEdit::singleline(&mut draft.top_offset).desired_width(56.0).char_limit(32));
                        let can_finish = draft.points.len() >= 3;
                        if ui.add_enabled(can_finish, egui::Button::new("Finish floor")).clicked() {
                            self.finish_floor_sketch();
                        }
                        if ui.button("Cancel floor").clicked() {
                            self.plans.floor_sketch = None;
                            self.status = "Floor sketch canceled.".into();
                            self.status_error = false;
                        }
                    } else if self.wall_gesture.is_none() {
                        if self.plans.dimension_draft.is_some() {
                            let draft = self.plans.dimension_draft.as_mut().unwrap();
                            ui.label(format!("{:?} · {} anchors · {}", draft.layout,
                                usize::from(draft.first.is_some()) + usize::from(draft.second.is_some()) + draft.additional.len(),
                                if draft.placing { "Place offset" } else { "Select ordered endpoints" }));
                            if matches!(draft.layout, os_model::DimensionLayout::Chain | os_model::DimensionLayout::Baseline) && !draft.placing
                                && ui.add_enabled(!draft.additional.is_empty(), egui::Button::new("Finish anchors")).clicked() {
                                draft.placing = true;
                            }
                            if ui.button("Cancel dimension").clicked() {
                                self.cancel_aligned_dimension();
                            }
                        } else {
                        if let Some(view) = self.plans.active
                            && ui.button("Place column").clicked() {
                            self.begin_column(view);
                        }
                        if ui.add_enabled(self.plans.active.is_some(),egui::Button::new("New grid")).clicked() {
                            self.begin_grid_form(None);
                        }
                        if let Some(view) = self.plans.active
                            && ui.button("Draw floor").on_hover_text("Sketch a straight-edged floor boundary in this plan. Click vertices, then finish.").clicked() {
                            self.begin_floor_sketch(view);
                        }
                        if ui.add_enabled(self.plans.active.is_some(),egui::Button::new("Draw wall in plan")).on_hover_text("Two clicks. Uses Properties height/thickness. Installed Wall gestures submit bounded commands; edit the source in its level plan.").clicked()
                            && let Some(view)=self.plans.active {
                            self.begin_plan_wall(view);
                        }
                        use crate::plan_gesture::WallEdit;
                        if let (Some(view),Some(id))=(self.plans.active,self.selected)
                            && self.editor.document.model().walls.contains_key(&id) {
                            for (label,mode) in [("Move wall",WallEdit::Move),("Resize start",WallEdit::ResizeStart),("Resize end",WallEdit::ResizeEnd),("Offset wall",WallEdit::OffsetCopy)] {
                                if ui.button(label).clicked() {
                                    self.begin_plan_wall_edit(view,id,mode);
                                }
                            }
                        }
                        }
                    } else {
                        ui.horizontal(|ui| {
                            let gesture = self.wall_gesture.as_mut().unwrap();
                            ui.label(gesture.prompt());
                            let offset = gesture.edit_mode()
                                == Some(crate::plan_gesture::WallEdit::OffsetCopy);
                            ui.label(if offset {
                                "Offset (m)"
                            } else if gesture.edit_mode()
                                == Some(crate::plan_gesture::WallEdit::Move)
                            {
                                "Distance (m)"
                            } else {
                                "Length (m)"
                            });
                            ui.add(
                                egui::TextEdit::singleline(&mut gesture.length)
                                    .desired_width(70.0)
                                    .char_limit(64),
                            );
                            if !offset {
                                ui.label("Angle (deg)");
                                ui.add(
                                    egui::TextEdit::singleline(&mut gesture.angle_degrees)
                                        .desired_width(70.0)
                                        .char_limit(64),
                                );
                            }
                            if ui.button("Cancel wall").clicked() {
                                self.cancel_plan_wall();
                            }
                        });
                    }
                    } else {
                        ui.label("Section view · native wall and slab cut contours · drag to pan, scroll to zoom, click a source to select.");
                    }
                });
                let toolbar_height = toolbar.response.rect.height();
                self.plans.toolbar_height = self
                    .plans
                    .toolbar_height
                    .max(toolbar_height)
                    .max(52.0);
                ui.add_space(self.plans.toolbar_height - toolbar_height);
                if self.wall_gesture.as_ref().is_some_and(|g| !g.current(&self.editor,self.plans.active)) { self.cancel_plan_wall(); }
                self.plans.poll(&self.editor);
                #[cfg(feature = "external-plugins")]
                {
                    let provider_view = self.plans.active.filter(|id| {
                        self.editor
                            .document
                            .model()
                            .views
                            .get(id)
                            .is_some_and(|view| view.parameters.kind == os_model::ViewKind::Plan)
                    });
                    self.plans.providers.poll(&mut self.editor, provider_view);
                }
                if self.plans.active_sheet.is_some() {
                    self.sheet_workspace(ui);
                } else if self.plans.active.is_some() {
                    if self.plans.split {
                        ui.columns(2, |columns| {
                            self.plan_canvas(&mut columns[0], ctx);
                            self.model_view(&mut columns[1], ctx);
                        });
                    } else {
                        self.plan_canvas(ui, ctx);
                    }
                } else {
                    self.model_view(ui, ctx);
                }
            });
        if !ctx.input(|i| i.pointer.primary_down()) {
            self.plans.crop.claimed = false;
            self.plans.column_pointer_claimed = false;
        }
        if self.plans.pending.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
        #[cfg(feature = "external-plugins")]
        if self.plans.providers.busy() {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
    }

    fn plan_canvas(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let escape_pressed = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        let stale_move = self
            .plans
            .opening_move
            .as_ref()
            .is_some_and(|d| !d.current(&self.editor, self.plans.active, self.selected));
        if escape_pressed || stale_move {
            self.plans.opening_move = None;
        }
        let stale_column = self
            .plans
            .column_placement
            .as_ref()
            .is_some_and(|d| d.stale(&self.editor, self.plans.active));
        if stale_column || escape_pressed {
            self.plans.column_placement = None;
        }
        if stale_column {
            self.status =
                "Column placement canceled because its plan, document, or providers changed."
                    .into();
        }
        if !ctx.wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::Delete)) {
            self.delete_column();
        }
        let stale_wall = self
            .wall_gesture
            .as_ref()
            .is_some_and(|g| !g.current(&self.editor, self.plans.active));
        let stale_opening = self.plans.opening_placement.as_ref().is_some_and(|draft| {
            self.plans.active != Some(draft.context.view_id)
                || self.editor.native_plan_context(draft.context.view_id).ok()
                    != Some(draft.context)
                || self.editor.host.activation_id(os_walls::PLUGIN_ID) != draft.activation
                || self.editor.document.session_id() != draft.session
                || self.editor.document.revision() != draft.revision
                || draft.type_id.is_some_and(|type_id| {
                    self.editor
                        .document
                        .model()
                        .opening_types
                        .get(&type_id)
                        .is_none_or(|ty| ty.parameters.kind != draft.kind)
                })
        });
        let stale_floor = self.plans.floor_sketch.as_ref().is_some_and(|draft| {
            self.plans.active != Some(draft.context.view_id)
                || self.editor.native_plan_context(draft.context.view_id).ok()
                    != Some(draft.context)
                || self.editor.document.session_id() != draft.session
                || self.editor.document.revision() != draft.revision
                || plan_provider_signature(&self.editor) != draft.provider_signature
        });
        let stale_section = self.plans.section_placement.as_ref().is_some_and(|draft| {
            self.plans.active != Some(draft.context.view_id)
                || self.editor.native_plan_context(draft.context.view_id).ok()
                    != Some(draft.context)
                || self.editor.document.session_id() != draft.session
                || self.editor.document.revision() != draft.revision
                || plan_provider_signature(&self.editor) != draft.provider_signature
        });
        let stale_tag = self
            .plans
            .room_tag_draft
            .as_ref()
            .is_some_and(|d| d.stale(&self.editor, self.plans.active));
        if stale_tag {
            self.plans.room_tag_draft = None;
        }
        let stale_detail = self
            .plans
            .detail_line_draft
            .as_ref()
            .is_some_and(|d| d.stale(&self.editor, self.plans.active));
        if stale_detail {
            self.plans.detail_line_draft = None;
        }
        let stale_separator = self
            .plans
            .room_separation_line_draft
            .as_ref()
            .is_some_and(|d| d.stale(&self.editor, self.plans.active));
        if stale_separator {
            self.plans.room_separation_line_draft = None;
        }
        let cancelled = stale_separator
            || stale_move
            || stale_column
            || escape_pressed
            || stale_detail
            || stale_wall
            || stale_opening
            || stale_floor
            || stale_section
            || stale_tag;
        if escape_pressed {
            let had_tool = self.wall_gesture.is_some()
                || self.plans.room_separation_line_draft.is_some()
                || self.plans.detail_line_draft.is_some()
                || self.plans.room_tag_draft.is_some()
                || self.plans.room_placement_active
                || self.plans.dimension_draft.is_some()
                || self.plans.opening_placement.is_some()
                || self.plans.section_placement.is_some()
                || self.plans.floor_sketch.is_some();
            self.cancel_plan_wall();
            // Cancel the draft but retain pointer ownership until release.
            self.plans.detail_line_draft = None;
            self.plans.room_placement_active = false;
            self.cancel_aligned_dimension();
            self.cancel_opening_placement();
            self.plans.section_placement = None;
            self.plans.floor_sketch = None;
            if had_tool {
                self.status = "Plan tool canceled.".into();
                self.status_error = false;
            }
        } else if stale_wall {
            self.cancel_plan_wall();
        }
        if stale_opening {
            self.cancel_opening_placement();
            self.status =
                "Door/window placement canceled because its plan or provider changed.".into();
            self.status_error = true;
        }
        if stale_floor {
            self.plans.floor_sketch = None;
            self.status = "Floor sketch canceled because its plan or document changed.".into();
            self.status_error = true;
        }
        if stale_section {
            self.plans.section_placement = None;
            self.status = "Section placement canceled because its plan or document changed.".into();
            self.status_error = true;
        }
        if ctx.input(|input| input.key_pressed(egui::Key::Backspace))
            && !ctx.wants_keyboard_input()
            && let Some(draft) = self.plans.dimension_draft.as_mut()
        {
            draft.placing = false;
            if draft.additional.pop().is_none() && draft.second.take().is_none() {
                draft.first = None;
            }
        }
        let stale_dimension = self.plans.dimension_draft.as_ref().is_some_and(|draft| {
            if plan_provider_signature(&self.editor) != draft.provider_signature {
                return true;
            }
            let drawing_changed = draft
                .drawing_identity
                .zip(
                    self.plans
                        .drawing
                        .as_ref()
                        .filter(|drawing| drawing.items(draft.context).is_ok())
                        .map(PlanDrawing::identity),
                )
                .is_some_and(|(expected, current)| expected != current);
            drawing_changed
                || self.plans.active != Some(draft.context.view_id)
                || self.editor.native_plan_context(draft.context.view_id).ok()
                    != Some(draft.context)
        });
        if stale_dimension {
            self.cancel_aligned_dimension();
            self.status = "Aligned dimension canceled because its plan context changed.".into();
            self.status_error = true;
        } else if let (Some(draft), Some(drawing)) = (
            self.plans.dimension_draft.as_mut(),
            self.plans.drawing.as_ref(),
        ) && draft.drawing_identity.is_none()
            && drawing.items(draft.context).is_ok()
        {
            draft.drawing_identity = Some(drawing.identity());
        }
        let Some(id) = self.plans.active else {
            return;
        };
        let camera = self.plans.cameras.entry(id).or_default();
        let fit = ui
            .horizontal(|ui| {
                let fit = ui
                    .button(if self.editor.document.model().views[&id].parameters.kind
                        == os_model::ViewKind::Section
                    {
                        "Fit section"
                    } else {
                        "Fit plan"
                    })
                    .clicked();
                ui.label(if self.plans.crop.mode.is_some() {
                    "Crop: boundary-only preview; geometry updates on release · Escape exits"
                } else if self.plans.floor_sketch.is_some() {
                    "Floor: click boundary vertices · close near the first point or finish · Escape cancels"
                } else if self.plans.room_separation_line_draft.is_some() {
                    "Room Separator · click start and end · Escape exits"
                } else if self.plans.room_tag_draft.is_some() {
                    "Room Tag · choose room, then position · Escape cancels"
                } else if self.plans.room_placement_active {
                    "Room: click inside an enclosed space · Escape to exit"
                } else if let Some(draft) = &self.plans.opening_placement {
                    match draft.kind {
                        OpeningKind::Door => {
                            "Door: hover a visible wall · click to place · Escape exits"
                        }
                        OpeningKind::Window => {
                            "Window: hover a visible wall · click to place · Escape exits"
                        }
                    }
                } else if self.plans.dimension_draft.is_some() {
                    "Dimension: two wall endpoints, then line position · Escape to cancel"
                } else {
                    "Drag to pan · Scroll to zoom · Click to select"
                });
                fit
            })
            .inner;
        if let Some(error) = &self.plans.error {
            ui.colored_label(theme::ERROR, error);
        }
        let Some(context) = self.plans.desired else {
            return;
        };
        #[cfg(feature = "external-plugins")]
        {
            if self.plans.providers.busy() {
                ui.label("Generating plugin plan graphics…");
            }
            if let Some(error) = &self.plans.providers.error {
                ui.colored_label(theme::ERROR, error);
            }
            let diagnostics = &self.plans.providers.diagnostics;
            if !diagnostics.is_empty() {
                ui.collapsing(
                    format!("Plan provider diagnostics ({})", diagnostics.len()),
                    |ui| {
                        egui::ScrollArea::vertical().max_height(100.0).show_rows(
                            ui,
                            18.0,
                            diagnostics.len(),
                            |ui, range| {
                                for (id, error) in
                                    diagnostics.iter().skip(range.start).take(range.len())
                                {
                                    ui.label(format!("{id}: {error}"));
                                }
                            },
                        );
                    },
                );
            }
        }
        let Some(drawing) = &self.plans.drawing else {
            if self.plans.error.is_none() {
                ui.label("Generating plan…");
            }
            return;
        };
        #[cfg(feature = "external-plugins")]
        let drawing = if self.editor.document.model().views[&id].parameters.kind
            == os_model::ViewKind::Plan
        {
            self.plans
                .providers
                .drawing(&self.editor, id)
                .unwrap_or(drawing)
        } else {
            drawing
        };
        if self.editor.native_view_context(id).ok() != Some(context) {
            return;
        }
        let Ok(items) = drawing.items(context) else {
            return;
        };
        let Ok(grids) = drawing.grids(context) else {
            return;
        };
        let Ok(provider_lines) = drawing.provider_lines(context) else {
            return;
        };
        let Ok(rooms) = drawing.rooms(context) else {
            return;
        };
        let Ok(room_faces) = drawing.room_faces(context) else {
            return;
        };
        let Ok(dimensions) = drawing.dimensions(context) else {
            return;
        };
        let Ok(angular_dimensions) = drawing.angular_dimensions(context) else {
            return;
        };
        let Ok(floors) = drawing.floors(context) else {
            return;
        };
        let Ok(columns) = drawing.columns(context) else {
            return;
        };
        let room_boundary_diagnostic = drawing.room_boundary_diagnostic(context).ok().flatten();
        if let Some(message) = room_boundary_diagnostic {
            ui.colored_label(theme::ERROR, message);
        }
        let unavailable = drawing.unavailable(context).map_or(0, |ids| ids.len());
        if unavailable > 0 {
            ui.colored_label(
                theme::ERROR,
                format!("Incomplete plan: {unavailable} plugin elements unavailable."),
            );
        }
        let (response, painter) =
            ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
        let rect = response.rect;
        #[cfg(test)]
        {
            self.plans.canvas_rect = Some(rect);
        }
        if rect.width() < 1.0 || rect.height() < 1.0 {
            return;
        }
        let size = [f64::from(rect.width()), f64::from(rect.height())];
        if fit && !self.plans.crop.claimed {
            let mut dimension_fit_points = Vec::new();
            for angle in angular_dimensions {
                if let Ok(segments) = angle.segments(context) {
                    for (a, b) in segments {
                        dimension_fit_points.extend([a, b]);
                    }
                }
            }
            for dimension in dimensions.iter().flat_map(PlanDimensionItem::graphics) {
                if dimension.value_m.is_some() {
                    for (start, end) in dimension.lines() {
                        if let Some((start, end)) = clip_plan_segment(start, end, context.crop) {
                            dimension_fit_points.extend([start, end]);
                        }
                    }
                    let midpoint = Point2::new(
                        (dimension.line_start.x + dimension.line_end.x) * 0.5,
                        (dimension.line_start.y + dimension.line_end.y) * 0.5,
                    );
                    if point_in_plan_crop(context, midpoint) {
                        dimension_fit_points.push(midpoint);
                    }
                } else if point_in_plan_crop(context, dimension.orphan_hint) {
                    dimension_fit_points.push(dimension.orphan_hint);
                }
            }
            let points: Vec<_> = items
                .iter()
                .chain(columns.iter())
                .flat_map(|item| item.footprint.vertices().iter().copied())
                .chain(grids.iter().flat_map(|grid| [grid.start, grid.end]))
                .chain(
                    provider_lines
                        .iter()
                        .flat_map(|line| [line.start, line.end]),
                )
                .chain(
                    drawing
                        .room_separation_lines(context)
                        .unwrap_or_default()
                        .iter()
                        .flat_map(|line| [line.start, line.end]),
                )
                .chain(rooms.iter().flat_map(|room| room.boundary.iter().copied()))
                .chain(
                    floors
                        .iter()
                        .flat_map(|floor| floor.boundary.iter().copied()),
                )
                .chain(dimension_fit_points)
                .chain(
                    room_faces
                        .iter()
                        .flat_map(|face| face.boundary.iter().copied()),
                )
                .collect();
            if !points.is_empty() {
                let min = points
                    .iter()
                    .fold(Point2::new(f64::INFINITY, f64::INFINITY), |p, q| {
                        Point2::new(p.x.min(q.x), p.y.min(q.y))
                    });
                let max = points
                    .iter()
                    .fold(Point2::new(f64::NEG_INFINITY, f64::NEG_INFINITY), |p, q| {
                        Point2::new(p.x.max(q.x), p.y.max(q.y))
                    });
                let candidate = PlanCamera {
                    center: Point2::new(
                        min.x + (max.x - min.x) * 0.5,
                        min.y + (max.y - min.y) * 0.5,
                    ),
                    pixels_per_metre: (0.85
                        * (size[0] / (max.x - min.x).max(0.001))
                            .min(size[1] / (max.y - min.y).max(0.001)))
                    .clamp(0.001, 10000.0),
                };
                if candidate.project(candidate.center, size).is_ok() {
                    *camera = candidate;
                }
            } else {
                *camera = PlanCamera::default();
            }
        }
        self.plans.crop.validate(
            &self.editor,
            self.plans.active,
            Some(drawing.identity()),
            false,
        );
        if self.plans.column_placement.is_some()
            && response.contains_pointer()
            && ctx.input(|i| i.pointer.primary_pressed())
        {
            self.plans.column_pointer_claimed = true;
        }
        let column_input =
            self.plans.column_placement.is_some() || self.plans.column_pointer_claimed;
        let crop_action = if column_input {
            None
        } else {
            self.plans.crop.input(
                &self.editor,
                context,
                drawing.identity(),
                *camera,
                &response,
                ctx,
            )
        };
        let crop_input = self.plans.crop.claimed;
        let detail_action = detail_lines::input(
            &self.editor,
            &mut self.plans.detail_line_draft,
            &mut self.plans.detail_line_pointer_claimed,
            drawing,
            context,
            *camera,
            &response,
            ctx,
            self.selected,
            self.plans.snaps,
            !crop_input
                && self.plans.room_separation_line_draft.is_none()
                && !self.plans.room_separation_line_pointer_claimed
                && !cancelled
                && self.wall_gesture.is_none()
                && self.grid_draft.is_none()
                && self.opening_draft.is_none()
                && self.plan_draft.is_none()
                && !self.plans.room_placement_active
                && self.plans.room_tag_draft.is_none()
                && self.plans.opening_placement.is_none()
                && self.plans.dimension_draft.is_none()
                && self.plans.floor_sketch.is_none()
                && !column_input
                && self.plans.section_placement.is_none(),
        );
        let detail_input = self.plans.detail_line_draft.is_some()
            || self.plans.detail_line_pointer_claimed
            || detail_action.is_some();
        let room_tag_action = room_tags::input(
            &self.editor,
            &mut self.plans.room_tag_draft,
            &mut self.plans.room_tag_pointer_claimed,
            drawing,
            context,
            *camera,
            &response,
            ctx,
            !crop_input
                && self.plans.room_separation_line_draft.is_none()
                && !self.plans.room_separation_line_pointer_claimed
                && !detail_input
                && !cancelled
                && self.wall_gesture.is_none()
                && self.grid_draft.is_none()
                && self.opening_draft.is_none()
                && self.plan_draft.is_none()
                && !self.plans.room_placement_active
                && self.plans.opening_placement.is_none()
                && self.plans.dimension_draft.is_none()
                && self.plans.floor_sketch.is_none()
                && !column_input
                && self.plans.section_placement.is_none(),
        );
        let separator_action = room_separation_lines::input(
            &self.editor,
            &mut self.plans.room_separation_line_draft,
            &mut self.plans.room_separation_line_pointer_claimed,
            drawing,
            context,
            *camera,
            &response,
            ctx,
            self.selected,
            self.plans.snaps,
            !crop_input
                && !detail_input
                && !self.plans.room_tag_pointer_claimed
                && room_tag_action.is_none()
                && !cancelled
                && self.wall_gesture.is_none()
                && self.grid_draft.is_none()
                && self.opening_draft.is_none()
                && self.plan_draft.is_none()
                && !self.plans.room_placement_active
                && self.plans.room_tag_draft.is_none()
                && self.plans.opening_placement.is_none()
                && self.plans.dimension_draft.is_none()
                && self.plans.floor_sketch.is_none()
                && !column_input
                && self.plans.section_placement.is_none(),
        );
        let separator_input = self.plans.room_separation_line_draft.is_some()
            || self.plans.room_separation_line_pointer_claimed
            || separator_action.is_some();
        let tag_input = separator_input
            || crop_input
            || detail_input
            || self.plans.room_tag_draft.is_some()
            || self.plans.room_tag_pointer_claimed
            || room_tag_action.is_some();
        let visible_wall = self
            .selected
            .filter(|id| items.iter().any(|item| item.entity == *id))
            .and_then(|id| self.editor.document.model().walls.get(&id));
        let handles = visible_wall.map_or_else(Vec::new, |wall| {
            endpoint_handles(&wall.parameters, context, *camera, rect)
        });
        let active_handle = self.plans.endpoint_drag.as_ref().map(|drag| drag.mode);
        let show_handles = active_handle.is_some()
            || (!tag_input
                && self.wall_gesture.is_none()
                && self.grid_draft.is_none()
                && self.opening_draft.is_none()
                && self.plan_draft.is_none()
                && self.plans.opening_placement.is_none()
                && self.plans.dimension_draft.is_none()
                && self.plans.floor_sketch.is_none()
                && !column_input
                && self.plans.section_placement.is_none());
        let hovered_handle =
            if show_handles && self.wall_gesture.is_none() && response.contains_pointer() {
                response
                    .hover_pos()
                    .and_then(|pointer| hit_endpoint(&handles, pointer))
            } else {
                None
            };
        let new_handle = if !cancelled
            && !tag_input
            && !self.plans.endpoint_pointer_claimed
            && self.wall_gesture.is_none()
            && self.grid_draft.is_none()
            && self.opening_draft.is_none()
            && self.plan_draft.is_none()
            && self.plans.opening_placement.is_none()
            && self.plans.dimension_draft.is_none()
            && self.plans.floor_sketch.is_none()
            && !column_input
            && self.plans.section_placement.is_none()
            && response.contains_pointer()
            && ctx.input(|i| i.pointer.primary_pressed())
        {
            ctx.input(|i| i.pointer.press_origin())
                .filter(|pos| rect.contains(*pos))
                .and_then(|pos| hit_endpoint(&handles, pos).map(|mode| (mode, pos)))
        } else {
            None
        };
        if new_handle.is_some() {
            self.plans.endpoint_pointer_claimed = true;
        }
        let allow_opening_move = !cancelled
            && !tag_input
            && !column_input
            && self.plans.crop.mode.is_none()
            && !self.plans.endpoint_pointer_claimed
            && self.wall_gesture.is_none()
            && self.grid_draft.is_none()
            && self.opening_draft.is_none()
            && self.plan_draft.is_none()
            && !self.plans.room_placement_active
            && self.plans.opening_placement.is_none()
            && self.plans.dimension_draft.is_none()
            && self.plans.floor_sketch.is_none()
            && self.plans.section_placement.is_none();
        let opening_move_release = crate::opening_tools::move_input(
            &self.editor,
            &mut self.plans.opening_move,
            &mut self.plans.opening_move_claimed,
            drawing,
            context,
            *camera,
            &response,
            ctx,
            self.selected,
            allow_opening_move,
        );
        let opening_move_input = self.plans.opening_move_claimed;
        let opening_move_preview = self
            .plans
            .opening_move
            .as_ref()
            .map(|d| d.preview(&self.editor));
        let opening_move_error = opening_move_preview
            .as_ref()
            .and_then(|p| p.as_ref().err())
            .map(ToString::to_string);
        let replacement = opening_move_preview.as_ref().and_then(|p| p.as_ref().ok());
        // Replace only the selected symbol and host cells in the paint stream.
        // Cached drawing, picking, model, history and the 3D scene stay untouched.
        let preview_items;
        let preview_lines;
        let (items, provider_lines) = if let Some(preview) = replacement {
            let d = self.plans.opening_move.as_ref().unwrap();
            preview_items = items
                .iter()
                .filter(|i| i.entity != d.host())
                .cloned()
                .chain(preview.items(context).unwrap().iter().cloned())
                .collect::<Vec<_>>();
            preview_lines = provider_lines
                .iter()
                .filter(|l| l.entity != d.id)
                .cloned()
                .chain(preview.provider_lines(context).unwrap().iter().cloned())
                .collect::<Vec<_>>();
            (preview_items.as_slice(), preview_lines.as_slice())
        } else {
            (items, provider_lines)
        };
        if response.dragged()
            && !opening_move_input
            && !tag_input
            && !self.plans.endpoint_pointer_claimed
            && !self.plans.room_placement_active
            && self.plans.opening_placement.is_none()
            && self.plans.dimension_draft.is_none()
            && self.plans.floor_sketch.is_none()
            && !column_input
            && self.plans.section_placement.is_none()
        {
            let delta = ctx.input(|i| i.pointer.delta());
            let _ = camera.pan(Point2::new(f64::from(delta.x), f64::from(delta.y)), size);
        }
        if !crop_input
            && !opening_move_input
            && !detail_input
            && !separator_input
            && response.hovered()
            && let Some(pointer) = response.hover_pos()
        {
            let scroll = ctx.input(|i| i.smooth_scroll_delta.y);
            let _ = camera.zoom_at(
                Point2::new(
                    f64::from(pointer.x - rect.left()),
                    f64::from(pointer.y - rect.top()),
                ),
                (f64::from(scroll) * 0.002).exp(),
                size,
            );
        }
        painter.rect_filled(rect, 0.0, theme::CANVAS);
        // Architectural datum lines are a background layer, not thin wall solids.
        for grid in grids {
            let screen = |p: Point2| -> Result<egui::Pos2> {
                let p = camera.project(p, size)?;
                os_core::ensure(
                    p.x.abs() < 1e8 && p.y.abs() < 1e8,
                    "grid exceeds screen range",
                )?;
                Ok(egui::pos2(
                    rect.left() + p.x as f32,
                    rect.top() + p.y as f32,
                ))
            };
            let (Ok(a), Ok(b)) = (screen(grid.start), screen(grid.end)) else {
                ui.colored_label(
                    theme::ERROR,
                    "Grid cannot be displayed at this navigation scale.",
                );
                return;
            };
            let color = if self.selected == Some(grid.entity) {
                theme::ACCENT
            } else {
                theme::MUTED
            };
            painter.line_segment([a, b], egui::Stroke::new(1.0_f32, color));
            painter.text(
                a + egui::vec2(4.0, -4.0),
                egui::Align2::LEFT_BOTTOM,
                &grid.name,
                egui::FontId::proportional(12.0),
                color,
            );
        }
        for line in provider_lines {
            if drawing.is_native_line(line.entity) {
                continue;
            }
            let screen = |point| -> Result<egui::Pos2> {
                let p = camera.project(point, size)?;
                os_core::ensure(
                    p.x.abs() < 1e8 && p.y.abs() < 1e8,
                    "provider line exceeds screen range",
                )?;
                Ok(egui::pos2(
                    rect.left() + p.x as f32,
                    rect.top() + p.y as f32,
                ))
            };
            let (Ok(a), Ok(b)) = (screen(line.start), screen(line.end)) else {
                ui.colored_label(
                    theme::ERROR,
                    "Provider graphics cannot be displayed at this navigation scale.",
                );
                return;
            };
            let color = if self.selected == Some(line.entity) {
                theme::ACCENT
            } else {
                drawing
                    .line_surface(line.entity, line.feature)
                    .color()
                    .map(|[r, g, b]| egui::Color32::from_rgb(r, g, b))
                    .unwrap_or(theme::TEXT)
            };
            let width = if line.role == os_geometry::plan::PlanRole::Cut {
                2.0_f32
            } else {
                1.0_f32
            };
            painter.line_segment([a, b], egui::Stroke::new(width, color));
        }
        for floor in floors {
            if let Err(error) = paint_floor_graphic(
                &painter,
                context,
                *camera,
                rect,
                floor,
                self.selected == Some(floor.entity),
                drawing
                    .appearance(
                        context,
                        floor.entity,
                        os_geometry::plan::PlanRole::Projected,
                    )
                    .ok()
                    .flatten(),
            ) {
                ui.colored_label(theme::ERROR, error.to_string());
            }
        }
        for item in columns.iter().chain(items.iter()) {
            let points: Result<Vec<_>> = item
                .footprint
                .vertices()
                .iter()
                .map(|p| {
                    camera.project(*p, size).and_then(|p| {
                        os_core::ensure(
                            p.x.abs() < f64::from(f32::MAX) / 2.0
                                && p.y.abs() < f64::from(f32::MAX) / 2.0,
                            "plan exceeds screen coordinate range",
                        )?;
                        Ok(egui::pos2(
                            rect.left() + p.x as f32,
                            rect.top() + p.y as f32,
                        ))
                    })
                })
                .collect();
            let Ok(points) = points else {
                ui.colored_label(
                    theme::ERROR,
                    "Plan cannot be displayed at this navigation scale.",
                );
                return;
            };
            let selected = self.selected == Some(item.entity);
            let fill = if selected {
                theme::SELECTED
            } else {
                item.surface
                    .color()
                    .map(|[r, g, b]| egui::Color32::from_rgb(r, g, b))
                    .unwrap_or(theme::SURFACE)
            };
            let color = if selected { theme::ACCENT } else { theme::TEXT };
            let width = if item.footprint.role == os_geometry::plan::PlanRole::Cut {
                2.0_f32
            } else {
                1.0_f32
            };
            let appearance = drawing
                .appearance(context, item.entity, item.footprint.role)
                .ok()
                .flatten();
            let styled_outline = appearance.is_some() || !item.hidden_edges.is_empty();
            painter.add(egui::Shape::convex_polygon(
                points.clone(),
                fill,
                if !styled_outline {
                    egui::Stroke::new(width, color)
                } else {
                    egui::Stroke::NONE
                },
            ));
            if styled_outline {
                let mut phase_mm = 0.0;
                let px_per_paper_mm =
                    (camera.pixels_per_metre * context.scale_denominator / 1000.0) as f32;
                for (a, b) in item.outline() {
                    let (Ok(a), Ok(b)) = (camera.project(a, size), camera.project(b, size)) else {
                        continue;
                    };
                    let a = rect.min + egui::vec2(a.x as f32, a.y as f32);
                    let b = rect.min + egui::vec2(b.x as f32, b.y as f32);
                    if let Err(error) = paint_plan_line(
                        &painter,
                        a,
                        b,
                        (!selected).then_some(appearance).flatten(),
                        egui::Stroke::new(width, color),
                        px_per_paper_mm,
                        &mut phase_mm,
                    ) {
                        ui.colored_label(theme::ERROR, error.to_string());
                        break;
                    }
                }
            }
        }
        if self.plans.room_placement_active {
            for face in room_faces {
                if let Err(error) = paint_room_outline(
                    &painter,
                    &face.boundary,
                    context,
                    *camera,
                    rect,
                    size,
                    egui::Stroke::new(1.0, theme::ACCENT.gamma_multiply(0.55)),
                ) {
                    ui.colored_label(theme::ERROR, error.to_string());
                }
            }
        }
        for room in rooms {
            let color = if self.selected == Some(room.entity) {
                theme::ACCENT
            } else if room.diagnostic.is_some() {
                theme::ERROR
            } else {
                theme::MUTED
            };
            if room.boundary.len() >= 3 {
                if let Err(error) = paint_room_outline(
                    &painter,
                    &room.boundary,
                    context,
                    *camera,
                    rect,
                    size,
                    egui::Stroke::new(
                        if self.selected == Some(room.entity) {
                            2.0
                        } else {
                            1.25
                        },
                        color,
                    ),
                ) {
                    ui.colored_label(theme::ERROR, error.to_string());
                }
                if point_in_plan_crop(context, room.seed)
                    && let Ok(seed) = camera.project(room.seed, size)
                {
                    let position =
                        egui::pos2(rect.left() + seed.x as f32, rect.top() + seed.y as f32);
                    if rect.contains(position) {
                        painter.text(
                            position,
                            egui::Align2::CENTER_CENTER,
                            if drawing
                                .room_tags(context)
                                .is_ok_and(|tags| tags.iter().any(|t| t.room == room.entity))
                            {
                                format!("{:.2} m²", room.area_m2)
                            } else {
                                format!("{} · {}\n{:.2} m²", room.number, room.name, room.area_m2)
                            },
                            egui::FontId::proportional(11.0),
                            color,
                        );
                    }
                }
            } else if point_in_plan_crop(context, room.seed)
                && let Ok(seed) = camera.project(room.seed, size)
            {
                let position = egui::pos2(rect.left() + seed.x as f32, rect.top() + seed.y as f32);
                if rect.contains(position) {
                    painter.text(
                        position,
                        egui::Align2::CENTER_CENTER,
                        if drawing
                            .room_tags(context)
                            .is_ok_and(|tags| tags.iter().any(|t| t.room == room.entity))
                        {
                            room.diagnostic
                                .as_deref()
                                .unwrap_or("Room boundary unavailable")
                                .to_owned()
                        } else {
                            format!(
                                "{} · {}\n{}",
                                room.number,
                                room.name,
                                room.diagnostic
                                    .as_deref()
                                    .unwrap_or("Room boundary unavailable")
                            )
                        },
                        egui::FontId::proportional(11.0),
                        color,
                    );
                }
            }
        }
        for line in provider_lines
            .iter()
            .filter(|line| drawing.is_native_line(line.entity))
        {
            let (Ok(a), Ok(b)) = (
                camera.project(line.start, size),
                camera.project(line.end, size),
            ) else {
                continue;
            };
            if [a.x, a.y, b.x, b.y].iter().any(|n| n.abs() >= 1e8) {
                continue;
            }
            let color = if self.selected == Some(line.entity) {
                theme::ACCENT
            } else {
                theme::TEXT
            };
            let a = egui::pos2(rect.left() + a.x as f32, rect.top() + a.y as f32);
            let b = egui::pos2(rect.left() + b.x as f32, rect.top() + b.y as f32);
            let appearance = drawing
                .appearance(context, line.entity, line.role)
                .ok()
                .flatten();
            let px_per_paper_mm =
                (camera.pixels_per_metre * context.scale_denominator / 1000.0) as f32;
            let mut phase_mm = 0.0;
            if let Err(error) = paint_plan_line(
                &painter,
                a,
                b,
                self.selected
                    .is_none_or(|id| id != line.entity)
                    .then_some(appearance)
                    .flatten(),
                egui::Stroke::new(1.5, color),
                px_per_paper_mm,
                &mut phase_mm,
            ) {
                ui.colored_label(theme::ERROR, error.to_string());
            }
        }
        for dimension in dimensions.iter().flat_map(PlanDimensionItem::graphics) {
            if let Err(error) = paint_dimension_graphic(
                &painter,
                context,
                *camera,
                rect,
                size,
                dimension,
                self.selected == Some(dimension.entity),
            ) {
                ui.colored_label(theme::ERROR, error.to_string());
            }
        }
        for graphic in angular_dimensions {
            if let Err(error) = paint_angular_graphic(
                &painter,
                context,
                *camera,
                rect,
                graphic,
                self.selected == Some(graphic.entity),
            ) {
                ui.colored_label(theme::ERROR, error.to_string());
            }
        }
        if let Some(draft) = &self.plans.dimension_draft {
            let anchors: Vec<_> = draft
                .first
                .iter()
                .chain(draft.second.iter())
                .chain(draft.additional.iter())
                .collect();
            for (index, anchor) in anchors.iter().enumerate() {
                if let Ok(screen) = camera.project(anchor.point, size) {
                    let pos = rect.min + egui::vec2(screen.x as f32, screen.y as f32);
                    painter.circle_filled(pos, 5.0, theme::ACCENT);
                    painter.text(
                        pos + egui::vec2(8.0, -8.0),
                        egui::Align2::LEFT_BOTTOM,
                        format!("{}", index + 1),
                        egui::FontId::proportional(12.0),
                        theme::TEXT,
                    );
                }
            }
            if anchors.len() >= 2
                && let Some(pointer) = response.hover_pos()
            {
                let screen = Point2::new(
                    f64::from(pointer.x - rect.left()),
                    f64::from(pointer.y - rect.top()),
                );
                if draft.layout == os_model::DimensionLayout::Angular {
                    if let Ok(params) = camera.unproject(screen, size).and_then(|placement| {
                        Self::angular_parameters(draft, self.editor.document.model(), placement)
                    }) && let Ok(graphic) = crate::plan::angular_graphic(
                        context,
                        Id::new(),
                        params.orphan_hint,
                        params.resolve_angular(self.editor.document.model()),
                    ) {
                        let _ =
                            paint_angular_graphic(&painter, context, *camera, rect, &graphic, true);
                    }
                } else if let Ok(placement) = camera.unproject(screen, size) {
                    let length = anchors[0].point.distance(anchors[1].point);
                    let normal = Point2::new(
                        (anchors[0].point.y - anchors[1].point.y) / length,
                        (anchors[1].point.x - anchors[0].point.x) / length,
                    );
                    let offset = (placement.x - anchors[0].point.x) * normal.x
                        + (placement.y - anchors[0].point.y) * normal.y;
                    for i in 1..anchors.len() {
                        let first = if draft.layout == os_model::DimensionLayout::Chain {
                            anchors[i - 1]
                        } else {
                            anchors[0]
                        };
                        let spacing = if draft.layout == os_model::DimensionLayout::Baseline {
                            (if offset < 0.0 { -1.0 } else { 1.0 }) * (i - 1) as f64 * 0.25
                        } else {
                            0.0
                        };
                        let placement = Point2::new(
                            placement.x + normal.x * spacing,
                            placement.y + normal.y * spacing,
                        );
                        if let Some(mut preview) =
                            preview_dimension(first.point, anchors[i].point, placement, Id::new())
                        {
                            preview.shared_start_witness =
                                draft.layout == os_model::DimensionLayout::Chain && i > 1;
                            if let Err(error) = paint_dimension_graphic(
                                &painter, context, *camera, rect, size, &preview, true,
                            ) {
                                ui.colored_label(theme::ERROR, error.to_string());
                            }
                        }
                    }
                }
            }
        }
        if items.is_empty()
            && grids.is_empty()
            && drawing
                .room_separation_lines(context)
                .is_ok_and(|lines| lines.is_empty())
            && provider_lines.is_empty()
            && floors.is_empty()
            && columns.is_empty()
            && rooms.is_empty()
            && dimensions.is_empty()
            && angular_dimensions.is_empty()
        {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "No visible plan geometry with these settings",
                egui::FontId::proportional(13.0),
                theme::MUTED,
            );
        }
        if show_handles {
            let stroke_width = |mode: WallEdit| {
                if active_handle == Some(mode) || hovered_handle == Some(mode) {
                    2.0
                } else {
                    1.0
                }
            };
            if let Some(wall) = visible_wall {
                for (mode, pos) in endpoint_handles(&wall.parameters, context, *camera, rect) {
                    painter.circle_filled(pos, ENDPOINT_RADIUS, theme::ACCENT);
                    painter.circle_stroke(
                        pos,
                        ENDPOINT_RADIUS,
                        egui::Stroke::new(stroke_width(mode), theme::ACCENT),
                    );
                }
            }
        }
        room_tags::paint(
            &painter,
            drawing.room_tags(context).unwrap_or_default(),
            self.plans.room_tag_draft.as_ref(),
            &self.editor,
            context,
            *camera,
            rect,
            self.selected,
            response.hover_pos(),
        );
        let camera = *camera;
        let column_preview = self.plans.column_placement.as_ref().and_then(|draft| {
            response
                .hover_pos()
                .filter(|p| rect.contains(*p))
                .map(|pointer| {
                    columns::preview(
                        &self.editor,
                        draft,
                        drawing,
                        context,
                        camera,
                        rect,
                        pointer,
                        self.plans.snaps,
                    )
                })
        });
        if let Some(Ok((_, footprint))) = &column_preview {
            columns::paint(&painter, footprint, camera, rect);
        }
        if let Some(draft) = &self.plans.floor_sketch {
            let mut boundary = draft
                .points
                .iter()
                .map(|point| context.basis.world_to_plane(*point))
                .collect::<Result<Vec<_>>>();
            let mut pointer_target = None;
            let mut pointer_screen = None;
            if response.hovered()
                && let Some(pointer) = response.hover_pos()
            {
                let local = Point2::new(
                    f64::from(pointer.x - rect.left()),
                    f64::from(pointer.y - rect.top()),
                );
                pointer_screen = Some(local);
                pointer_target =
                    floor_snap_target(drawing, context, camera, size, local, self.plans.snaps)
                        .ok()
                        .filter(|point| point_in_plan_crop(context, *point));
            }
            if let Ok(points) = &mut boundary {
                if let Some(target) = pointer_target {
                    let closes = points.len() >= 3
                        && points
                            .first()
                            .and_then(|first| camera.project(*first, size).ok())
                            .zip(pointer_screen)
                            .is_some_and(|(first, pointer)| {
                                first.distance(pointer) <= f64::from(ENDPOINT_HIT_RADIUS)
                            });
                    if !closes
                        && points
                            .last()
                            .is_none_or(|last| last.distance(target) > 1e-6)
                    {
                        points.push(target);
                    }
                }
                if points.len() >= 3
                    && let Ok(triangles) = os_geometry::floors::triangulate_floor(points)
                {
                    let preview = PlanFloorItem {
                        entity: Id::new(),
                        area_m2: os_geometry::floors::signed_area(points).abs(),
                        boundary: points.clone(),
                        triangles,
                    };
                    if let Err(error) =
                        paint_floor_graphic(&painter, context, camera, rect, &preview, true, None)
                    {
                        ui.colored_label(theme::ERROR, error.to_string());
                    }
                } else {
                    for segment in points.windows(2) {
                        if let (Ok(a), Ok(b)) = (
                            camera.project(segment[0], size),
                            camera.project(segment[1], size),
                        ) {
                            painter.line_segment(
                                [
                                    rect.min + egui::vec2(a.x as f32, a.y as f32),
                                    rect.min + egui::vec2(b.x as f32, b.y as f32),
                                ],
                                egui::Stroke::new(2.0, theme::ERROR),
                            );
                        }
                    }
                }
            }
        }
        let opening_preview = self
            .plans
            .opening_placement
            .as_ref()
            .filter(|_| !cancelled && response.hovered())
            .and_then(|draft| {
                let pointer = response.hover_pos()?;
                let local = Point2::new(
                    f64::from(pointer.x - rect.left()),
                    f64::from(pointer.y - rect.top()),
                );
                opening_placement_preview(
                    self.editor.document.model(),
                    drawing,
                    context,
                    camera,
                    size,
                    local,
                    *draft,
                )
                .ok()
                .flatten()
            });
        if let Some(preview) = &opening_preview {
            let model = self.editor.document.model();
            if let Some(wall) = model.walls.get(&preview.host)
                && let Some(level) = model.levels.get(&wall.parameters.level)
            {
                match crate::opening_tools::plan_symbol(
                    Id::new(),
                    &preview.resolved,
                    &wall.parameters,
                    level.parameters.elevation,
                    context,
                ) {
                    Ok(lines) => {
                        let color = if preview.message.is_some() {
                            theme::ERROR
                        } else {
                            theme::ACCENT
                        };
                        for line in lines {
                            if let (Ok(a), Ok(b)) = (
                                camera.project(line.start, size),
                                camera.project(line.end, size),
                            ) && [a.x, a.y, b.x, b.y].iter().all(|coordinate| {
                                coordinate.is_finite()
                                    && coordinate.abs() < f64::from(f32::MAX) / 2.0
                            }) {
                                painter.line_segment(
                                    [
                                        rect.min + egui::vec2(a.x as f32, a.y as f32),
                                        rect.min + egui::vec2(b.x as f32, b.y as f32),
                                    ],
                                    egui::Stroke::new(2.0, color),
                                );
                            }
                        }
                        let text = preview.message.as_deref().map_or_else(
                            || {
                                format!(
                                    "{:?} · {:.2} m wide · offset {:.3} m",
                                    preview.resolved.kind,
                                    preview.resolved.width,
                                    preview.parameters.offset
                                )
                            },
                            |message| format!("Cannot place: {message}"),
                        );
                        painter.text(
                            rect.left_top() + egui::vec2(8.0, 8.0),
                            egui::Align2::LEFT_TOP,
                            text,
                            egui::FontId::proportional(12.0),
                            color,
                        );
                    }
                    Err(error) => {
                        ui.colored_label(theme::ERROR, error.to_string());
                    }
                }
            }
        }
        let section_preview_target = self
            .plans
            .section_placement
            .as_ref()
            .filter(|_| !cancelled && response.hovered())
            .and_then(|_| {
                let pointer = response.hover_pos()?;
                let local = Point2::new(
                    f64::from(pointer.x - rect.left()),
                    f64::from(pointer.y - rect.top()),
                );
                floor_snap_target(drawing, context, camera, size, local, self.plans.snaps)
                    .ok()
                    .filter(|point| point_in_plan_crop(context, *point))
                    .and_then(|point| context.basis.plane_to_world(point).ok())
            });
        if let Some(draft) = &self.plans.section_placement {
            let screen = |world: Point2| -> Result<egui::Pos2> {
                let plane = context.basis.world_to_plane(world)?;
                let point = camera.project(plane, size)?;
                os_core::ensure(
                    point.x.is_finite()
                        && point.y.is_finite()
                        && point.x.abs() < f64::from(f32::MAX) / 2.0
                        && point.y.abs() < f64::from(f32::MAX) / 2.0,
                    "section marker preview exceeds screen range",
                )?;
                Ok(rect.min + egui::vec2(point.x as f32, point.y as f32))
            };
            if let Some(first) = draft.first {
                if let Ok(start) = screen(first) {
                    painter.circle_filled(start, 3.5, theme::ACCENT);
                    if let Some(end) = section_preview_target.and_then(|point| screen(point).ok()) {
                        let delta = end - start;
                        let length = delta.length();
                        if length > 8.0 {
                            painter
                                .line_segment([start, end], egui::Stroke::new(2.0, theme::ACCENT));
                            let direction = delta / length;
                            let perpendicular = egui::vec2(-direction.y, direction.x);
                            let base = end - direction * 12.0;
                            painter.line_segment(
                                [end, base + perpendicular * 5.0],
                                egui::Stroke::new(2.0, theme::ACCENT),
                            );
                            painter.line_segment(
                                [end, base - perpendicular * 5.0],
                                egui::Stroke::new(2.0, theme::ACCENT),
                            );
                        } else {
                            painter
                                .line_segment([start, end], egui::Stroke::new(2.0, theme::ERROR));
                        }
                    }
                }
            } else if let Some(target) = section_preview_target.and_then(|point| screen(point).ok())
            {
                painter.circle_stroke(target, 5.0, egui::Stroke::new(1.5, theme::ACCENT));
            }
        }
        self.plans
            .crop
            .paint(&self.editor, context, camera, &response, ctx, &painter);
        detail_lines::paint(
            &painter,
            drawing,
            self.plans.detail_line_draft.as_ref(),
            &self.editor,
            context,
            camera,
            rect,
            self.selected,
        );
        room_separation_lines::paint(
            &painter,
            drawing,
            self.plans.room_separation_line_draft.as_ref(),
            &self.editor,
            context,
            camera,
            rect,
            self.selected,
        );
        crate::opening_tools::paint_move_grip(
            &self.editor,
            self.plans.opening_move.as_ref(),
            self.selected,
            drawing,
            context,
            camera,
            &response,
            &painter,
            allow_opening_move,
            opening_move_error.as_deref(),
        );
        if opening_move_input {
            if opening_move_release && let Some(draft) = self.plans.opening_move.take() {
                let result = draft.commit(&mut self.editor, self.plans.active, self.selected);
                self.report(result, "Opening move finished.");
                ctx.request_repaint();
            }
            self.finish_endpoint_input(ctx);
            return;
        }
        if column_input {
            if !cancelled && response.clicked() && self.plans.column_placement.is_some() {
                match column_preview {
                    Some(Ok((parameters, _))) => self.commit_column(parameters),
                    Some(Err(error)) => self.report(Err(error), ""),
                    None => {}
                }
            }
            self.finish_endpoint_input(ctx);
            return;
        }
        if tag_input {
            if let Some(action) = separator_action {
                self.finish_room_separation_line_action(action);
            }
            if let Some(action) = detail_action {
                self.finish_detail_line_action(action);
            }
            if let Some(crop) = crop_action {
                let view = self.editor.document.model().views[&id].parameters.clone();
                let mut settings = view.plan.expect("plan crop");
                settings.crop = Some(crop);
                let result = self.editor.update_floor_plan(
                    id,
                    &view.name,
                    view.level.expect("plan level"),
                    settings,
                );
                self.report(result, "Plan crop updated.");
            }
            if let Some(action) = room_tag_action {
                self.finish_room_tag_action(action);
            }
            self.finish_endpoint_input(ctx);
            return;
        }
        if let Some((mode, origin)) = new_handle {
            self.begin_plan_wall_edit(id, self.selected.expect("visible selected wall"), mode);
            if self.wall_gesture.is_some() {
                self.plans.endpoint_drag = Some(EndpointDrag {
                    mode,
                    origin,
                    moved: false,
                });
            }
        }
        let pointer = ctx.input(|i| i.pointer.interact_pos());
        if let Some(drag) = &mut self.plans.endpoint_drag {
            // A stationary long press is still a click-without-drag.
            drag.moved |= pointer.is_some_and(|p| {
                p.distance(drag.origin) > ctx.options(|o| o.input_options.max_click_dist)
            });
        }
        let endpoint_release = self
            .plans
            .endpoint_drag
            .as_ref()
            .is_some_and(|drag| drag.moved)
            && ctx.input(|i| i.pointer.primary_released());
        let pointer = if self.plans.endpoint_drag.is_some() {
            pointer.filter(|p| rect.contains(*p) && response.contains_pointer())
        } else {
            response.hover_pos().filter(|_| response.hovered())
        };
        // Beginning an edit mutates UI state; reacquire the same checked drawing
        // for the shared snap/preview path after that borrow has ended.
        let drawing = self.plans.drawing.as_ref().expect("checked drawing");
        #[cfg(feature = "external-plugins")]
        let drawing = self
            .plans
            .providers
            .drawing(&self.editor, id)
            .unwrap_or(drawing);
        if self.wall_gesture.is_some()
            && !cancelled
            && let Some(pointer) = pointer
        {
            let screen = Point2::new(
                f64::from(pointer.x - rect.left()),
                f64::from(pointer.y - rect.top()),
            );
            let query = os_render::snapping::SnapQuery {
                camera,
                viewport: size,
                pointer: screen,
                radius_pixels: 12.0,
                endpoints: self.plans.snaps.enabled && self.plans.snaps.endpoints,
                midpoints: self.plans.snaps.enabled && self.plans.snaps.midpoints,
                intersections: self.plans.snaps.enabled && self.plans.snaps.intersections,
                perpendicular_from: if self.plans.snaps.enabled && self.plans.snaps.perpendicular {
                    self.wall_gesture.as_ref().and_then(|g| g.start)
                } else {
                    None
                },
                nearest: self.plans.snaps.enabled && self.plans.snaps.nearest,
                axis_extensions: self.plans.snaps.enabled && self.plans.snaps.axis_extensions,
                exclude_entity: self.wall_gesture.as_ref().and_then(|g| g.snap_exclusion()),
            };
            let acquired = if self
                .wall_gesture
                .as_ref()
                .is_some_and(|g| g.has_exact_destination())
            {
                Ok(None)
            } else {
                drawing
                    .snap(context, query)
                    .and_then(|result| result.candidate(context, query))
            };
            let target = acquired.and_then(|candidate| {
                let point = if let Some(hit) = candidate {
                    hit.point
                } else {
                    camera.unproject(screen, size)?
                };
                Ok((point, candidate))
            });
            match target {
                Ok((point, candidate)) => {
                    let project = |point: Point2| -> Result<egui::Pos2> {
                        let p = camera.project(point, size)?;
                        os_core::ensure(
                            p.x.abs() < 1e8 && p.y.abs() < 1e8,
                            "Preview exceeds screen range",
                        )?;
                        Ok(egui::pos2(
                            rect.left() + p.x as f32,
                            rect.top() + p.y as f32,
                        ))
                    };
                    let exact = self.wall_gesture.as_ref().is_some_and(|g| {
                        g.start.is_some()
                            && (!g.length.trim().is_empty() || !g.angle_degrees.trim().is_empty())
                    });
                    if !exact
                        && let Some(hit) = candidate
                        && let Ok(pos) = project(hit.point)
                    {
                        painter.circle_stroke(pos, 5.0, egui::Stroke::new(1.5_f32, theme::ACCENT));
                        painter.text(
                            pos + egui::vec2(9.0, -9.0),
                            egui::Align2::LEFT_BOTTOM,
                            format!("{:?}", hit.kind),
                            egui::FontId::proportional(12.0),
                            theme::TEXT,
                        );
                    }
                    let gesture = self.wall_gesture.as_mut().unwrap();
                    let preview = if gesture.start.is_some() {
                        Some(gesture.parameters(point))
                    } else {
                        None
                    };
                    if let Some(Ok(parameters)) = &preview {
                        if let (Ok(a), Ok(b)) = (
                            context
                                .basis
                                .world_to_plane(parameters.start)
                                .and_then(project),
                            context
                                .basis
                                .world_to_plane(parameters.end)
                                .and_then(project),
                        ) {
                            painter.line_segment([a, b], egui::Stroke::new(2.0_f32, theme::ACCENT));
                            if let Some(distance) = gesture.offset_measurement(parameters) {
                                painter.text(
                                    a + egui::vec2(9.0, -9.0),
                                    egui::Align2::LEFT_BOTTOM,
                                    format!("Offset {distance:+.3} m"),
                                    egui::FontId::proportional(12.0),
                                    theme::TEXT,
                                );
                            }
                            if exact {
                                painter.text(
                                    (if gesture.edit_mode()
                                        == Some(crate::plan_gesture::WallEdit::ResizeStart)
                                    {
                                        a
                                    } else {
                                        b
                                    }) + egui::vec2(9.0, -9.0),
                                    egui::Align2::LEFT_BOTTOM,
                                    "Exact input",
                                    egui::FontId::proportional(12.0),
                                    theme::TEXT,
                                );
                            }
                        }
                    } else if let Some(Err(error)) = &preview {
                        let waiting = gesture.length.trim().is_empty()
                            && gesture.angle_degrees.trim().is_empty()
                            && gesture
                                .start
                                .is_some_and(|start| start.distance(point) <= 1e-9);
                        painter.text(
                            rect.left_top() + egui::vec2(8.0, 8.0),
                            egui::Align2::LEFT_TOP,
                            if waiting {
                                "Choose the other endpoint or enter exact dimensions".into()
                            } else {
                                error.to_string()
                            },
                            egui::FontId::proportional(12.0),
                            if waiting { theme::MUTED } else { theme::ERROR },
                        );
                    }
                    if endpoint_release
                        || (!self.plans.endpoint_pointer_claimed && response.clicked())
                    {
                        if gesture.start.is_none() {
                            gesture.start = Some(point);
                        } else {
                            self.commit_plan_wall(point);
                        }
                        ctx.request_repaint();
                    }
                }
                Err(error) => {
                    painter.text(
                        rect.left_top() + egui::vec2(8.0, 8.0),
                        egui::Align2::LEFT_TOP,
                        error.to_string(),
                        egui::FontId::proportional(12.0),
                        theme::ERROR,
                    );
                }
            }
        } else if self.plans.section_placement.is_some()
            && self.wall_gesture.is_none()
            && !self.plans.endpoint_pointer_claimed
            && !cancelled
            && response.clicked()
            && let Some(pointer) = response.interact_pointer_pos()
        {
            let local = Point2::new(
                f64::from(pointer.x - rect.left()),
                f64::from(pointer.y - rect.top()),
            );
            let target = floor_snap_target(drawing, context, camera, size, local, self.plans.snaps)
                .and_then(|point| {
                    os_core::ensure(
                        point_in_plan_crop(context, point),
                        "Section marker is outside the plan crop",
                    )?;
                    context.basis.plane_to_world(point)
                });
            match target {
                Ok(point) => {
                    let first = self
                        .plans
                        .section_placement
                        .as_ref()
                        .expect("checked section placement")
                        .first;
                    if let Some(first) = first {
                        match self.commit_section_placement(first, point) {
                            Ok(section) => {
                                self.plans.section_placement = None;
                                self.focus_plan(Some(section));
                                self.report(
                                    Ok(()),
                                    "Section created · linked building cut is ready.",
                                );
                            }
                            Err(error) => self.report(Err(error), ""),
                        }
                    } else if self.plans.section_placement.as_mut().is_some_and(|draft| {
                        draft.first = Some(point);
                        true
                    }) {
                        self.status = "Section marker · click the second point".into();
                        self.status_error = false;
                    }
                    ctx.request_repaint();
                }
                Err(error) => self.report(Err(error), ""),
            }
        } else if self.plans.floor_sketch.is_some()
            && self.wall_gesture.is_none()
            && !self.plans.endpoint_pointer_claimed
            && !cancelled
            && response.clicked()
            && let Some(pointer) = response.interact_pointer_pos()
        {
            let local = Point2::new(
                f64::from(pointer.x - rect.left()),
                f64::from(pointer.y - rect.top()),
            );
            let draft = self
                .plans
                .floor_sketch
                .as_ref()
                .expect("checked floor sketch");
            let target = floor_snap_target(drawing, context, camera, size, local, self.plans.snaps)
                .and_then(|point| {
                    os_core::ensure(
                        point_in_plan_crop(context, point),
                        "Floor vertex is outside the plan crop",
                    )?;
                    Ok(point)
                });
            match target {
                Ok(point) => {
                    let closes = draft.points.len() >= 3
                        && draft
                            .points
                            .first()
                            .and_then(|first| context.basis.world_to_plane(*first).ok())
                            .and_then(|first| camera.project(first, size).ok())
                            .is_some_and(|first| {
                                first.distance(local) <= f64::from(ENDPOINT_HIT_RADIUS)
                            });
                    if closes {
                        self.finish_floor_sketch();
                    } else {
                        match context.basis.plane_to_world(point) {
                            Ok(world) => {
                                let draft = self.plans.floor_sketch.as_mut().unwrap();
                                if draft.points.len() >= 256 {
                                    self.report(
                                        Err(Error::Invalid(
                                            "floor boundary is limited to 256 vertices".into(),
                                        )),
                                        "",
                                    );
                                } else if draft
                                    .points
                                    .last()
                                    .is_some_and(|last| last.distance(world) <= 1e-6)
                                {
                                    self.report(
                                        Err(Error::Invalid(
                                            "floor boundary vertices must be distinct".into(),
                                        )),
                                        "",
                                    );
                                } else {
                                    draft.points.push(world);
                                }
                            }
                            Err(error) => self.report(Err(error), ""),
                        }
                    }
                    ctx.request_repaint();
                }
                Err(error) => self.report(Err(error), ""),
            }
        } else if self.plans.opening_placement.is_some()
            && self.wall_gesture.is_none()
            && !self.plans.endpoint_pointer_claimed
            && !cancelled
            && response.clicked()
            && let Some(pointer) = response.interact_pointer_pos()
        {
            let local = Point2::new(
                f64::from(pointer.x - rect.left()),
                f64::from(pointer.y - rect.top()),
            );
            let placement = *self
                .plans
                .opening_placement
                .as_ref()
                .expect("checked opening placement");
            let result = opening_placement_preview(
                self.editor.document.model(),
                drawing,
                context,
                camera,
                size,
                local,
                placement,
            )
            .and_then(|preview| {
                preview.ok_or_else(|| {
                    Error::Invalid("Hover over a visible wall to place the opening".into())
                })
            })
            .and_then(|preview| self.commit_opening_placement(preview));
            self.report(
                result.map(|_| ()),
                match placement.kind {
                    OpeningKind::Door => "Door placed · click another wall or Escape to finish.",
                    OpeningKind::Window => {
                        "Window placed · click another wall or Escape to finish."
                    }
                },
            );
            ctx.request_repaint();
        } else if self.plans.dimension_draft.is_some()
            && self.wall_gesture.is_none()
            && !self.plans.endpoint_pointer_claimed
            && !cancelled
            && response.clicked()
            && let Some(pointer) = response.interact_pointer_pos()
        {
            let needs_anchor = self
                .plans
                .dimension_draft
                .as_ref()
                .is_some_and(|draft| !draft.placing);
            if needs_anchor {
                match self.dimension_anchor_at(drawing, context, camera, rect, pointer) {
                    Ok(anchor) => {
                        let draft = self.plans.dimension_draft.as_mut().unwrap();
                        if let Some(second) = &draft.second {
                            let first = draft.first.as_ref().unwrap();
                            let mut additional: Vec<_> =
                                draft.additional.iter().map(|a| a.reference).collect();
                            additional.push(anchor.reference);
                            let candidate = DimensionParams {
                                layout: draft.layout,
                                first: first.reference,
                                second: second.reference,
                                additional,
                                baseline_spacing_m: 0.25,
                                view: draft.context.view_id,
                                offset_m: 0.0,
                                orphan_hint: Point2::new(0.0, 0.0),
                            };
                            match candidate.validate_creation(self.editor.document.model()) {
                                Ok(()) => draft.additional.push(anchor),
                                Err(error) => self.report(Err(error), ""),
                            }
                        } else if let Some(first) = &draft.first {
                            if (draft.layout == os_model::DimensionLayout::Angular
                                && first.reference.wall == anchor.reference.wall)
                                || first.reference == anchor.reference
                                || first.point.distance(anchor.point) <= 1e-6
                            {
                                self.report(
                                    Err(Error::Invalid(
                                        "Choose a distinct endpoint more than one micrometre away"
                                            .into(),
                                    )),
                                    "",
                                );
                            } else {
                                draft.second = Some(anchor);
                                draft.placing = matches!(
                                    draft.layout,
                                    os_model::DimensionLayout::Aligned
                                        | os_model::DimensionLayout::Angular
                                );
                            }
                        } else {
                            draft.first = Some(anchor);
                        }
                    }
                    Err(error) => self.report(Err(error), ""),
                }
            } else {
                let local = Point2::new(
                    f64::from(pointer.x - rect.left()),
                    f64::from(pointer.y - rect.top()),
                );
                match camera
                    .unproject(local, size)
                    .and_then(|placement| self.commit_aligned_dimension(placement))
                {
                    Ok(_) => self.report(Ok(()), "Dimension added."),
                    Err(error) => self.report(Err(error), ""),
                }
            }
            ctx.request_repaint();
        } else if self.plans.room_placement_active
            && self.wall_gesture.is_none()
            && !self.plans.endpoint_pointer_claimed
            && !cancelled
            && response.clicked()
            && let Some(pointer) = response.interact_pointer_pos()
        {
            let screen = Point2::new(
                f64::from(pointer.x - rect.left()),
                f64::from(pointer.y - rect.top()),
            );
            let placement = camera.unproject(screen, size).and_then(|plane| {
                os_core::ensure(
                    point_in_plan_crop(context, plane),
                    "Room placement is outside the plan crop",
                )?;
                let face = drawing.room_face_at(context, plane)?.ok_or_else(|| {
                    Error::Invalid("Click inside one enclosed space, away from its boundary".into())
                })?;
                Ok((
                    context.basis.plane_to_world(plane)?,
                    face.boundary_signature.clone(),
                ))
            });
            match placement {
                Ok((seed, signature)) => {
                    let level = self.editor.document.model().views[&id]
                        .parameters
                        .level
                        .expect("checked floor plan level");
                    let existing = self
                        .editor
                        .document
                        .model()
                        .rooms
                        .values()
                        .find(|room| {
                            room.parameters.level == level
                                && room.parameters.boundary_signature == signature
                        })
                        .map(|room| room.id());
                    if let Some(existing) = existing {
                        self.select(Some(existing));
                        self.report(Ok(()), "This space already has a room.");
                    } else {
                        self.create_room_at(level, seed, signature);
                    }
                }
                Err(error) => self.report(Err(error), ""),
            }
            ctx.request_repaint();
        } else if self.wall_gesture.is_none()
            && !self.plans.endpoint_pointer_claimed
            && !self.plans.room_placement_active
            && self.plans.opening_placement.is_none()
            && self.plans.floor_sketch.is_none()
            && !column_input
            && !cancelled
            && response.clicked()
            && let Some(pointer) = response.interact_pointer_pos()
        {
            let point = Point2::new(
                f64::from(pointer.x - rect.left()),
                f64::from(pointer.y - rect.top()),
            );
            let selected_model = drawing
                .pick_screen(context, camera, size, point, 6.0)
                .ok()
                .flatten()
                .or_else(|| {
                    camera
                        .unproject(point, size)
                        .and_then(|plane| drawing.pick_column(context, plane))
                        .ok()
                        .flatten()
                });
            let selected_room = if selected_model.is_none() {
                camera
                    .unproject(point, size)
                    .and_then(|plane| {
                        if point_in_plan_crop(context, plane) {
                            drawing.pick_room(context, plane)
                        } else {
                            Ok(None)
                        }
                    })
                    .ok()
                    .flatten()
            } else {
                None
            };
            let selected_floor = if selected_model.is_none() && selected_room.is_none() {
                camera
                    .unproject(point, size)
                    .and_then(|plane| {
                        if point_in_plan_crop(context, plane) {
                            drawing.pick_floor(context, plane)
                        } else {
                            Ok(None)
                        }
                    })
                    .ok()
                    .flatten()
            } else {
                None
            };
            let selected = selected_model.or(selected_room).or(selected_floor);
            if selected.is_some_and(|selected| {
                self.editor
                    .document
                    .model()
                    .views
                    .get(&selected)
                    .is_some_and(|view| view.parameters.kind == os_model::ViewKind::Section)
            }) {
                self.focus_plan(selected);
                self.select(None);
            } else {
                self.select(selected);
            }
            ctx.request_repaint();
        }
        self.finish_endpoint_input(ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::mpsc,
        time::{Duration, Instant},
    };

    #[test]
    fn provider_only_canvas_renders_lines_without_empty_message() {
        let mut app = DesktopApp::new().unwrap();
        let level = *app.editor.document.model().levels.keys().next().unwrap();
        let view = app.editor.create_floor_plan("Plan", level).unwrap();
        let context = app.editor.native_plan_context(view).unwrap();
        let entity = Id::new();
        let line = os_render::plan::PlanLine {
            entity,
            feature: 1,
            start: Point2::new(-1.0, 0.0),
            end: Point2::new(1.0, 0.0),
            role: os_geometry::plan::PlanRole::Cut,
        };
        app.plans.active = Some(view);
        app.plans.desired = Some(context);
        app.plans.drawing = Some(
            PlanDrawing::from_prisms(context, &BTreeMap::new(), vec![entity])
                .unwrap()
                .with_provider_lines([(entity, vec![line])].into())
                .unwrap(),
        );
        let ctx = egui::Context::default();
        let output = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| app.plan_canvas(ui, ctx));
        });
        assert!(output.shapes.iter().any(|shape| matches!(
            &shape.shape, egui::Shape::LineSegment { stroke, .. }
            if stroke.width == 2.0 && stroke.color == theme::TEXT
        )));
        assert!(!output.shapes.iter().any(|shape| matches!(
            &shape.shape, egui::Shape::Text(text)
            if text.galley.job.text.starts_with("No visible")
        )));
        app.plans.drawing =
            Some(PlanDrawing::from_prisms(context, &BTreeMap::new(), vec![]).unwrap());
        let output = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| app.plan_canvas(ui, ctx));
        });
        assert!(output.shapes.iter().any(|shape| matches!(
            &shape.shape, egui::Shape::Text(text)
            if text.galley.job.text == "No visible plan geometry with these settings"
        )));
    }

    fn settle(workspace: &mut PlanWorkspace, editor: &Editor) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while workspace.pending.is_some() {
            assert!(Instant::now() < deadline, "plan worker did not settle");
            std::thread::yield_now();
            workspace.poll(editor);
        }
    }

    #[test]
    fn retired_view_work_drains_and_cannot_publish_when_view_returns() {
        let mut editor = Editor::new().unwrap();
        let level = *editor.document.model().levels.keys().next().unwrap();
        let view = editor.create_floor_plan("Plan", level).unwrap();
        let context = editor.native_plan_context(view).unwrap();
        let mut workspace = PlanWorkspace {
            session: Some(editor.document.session_id()),
            active: Some(view),
            desired: Some(context),
            attempted: true,
            ..Default::default()
        };
        let (tx, rx) = mpsc::channel();
        let snapshot = editor.plan_snapshot(view).unwrap();
        workspace.pending = Some((
            std::thread::spawn(move || {
                rx.recv().unwrap();
                snapshot.derive()
            }),
            true,
        ));
        workspace.active = None;
        workspace.poll(&editor);
        assert!(!workspace.pending.as_ref().unwrap().1);
        workspace.active = Some(view);
        workspace.poll(&editor);
        assert!(
            !workspace.pending.as_ref().unwrap().1,
            "reactivation revived retired work"
        );
        assert!(workspace.drawing.is_none());
        tx.send(()).unwrap();
        settle(&mut workspace, &editor);
        assert!(workspace.drawing.as_ref().unwrap().items(context).is_ok());
        editor
            .command("Rename", Command::RenameProject("Changed".into()))
            .unwrap();
        workspace.poll(&editor);
        assert!(workspace.drawing.is_none());
        settle(&mut workspace, &editor);
        assert!(workspace.drawing.as_ref().unwrap().items(context).is_err());
        editor.document = Document::new("Replacement").unwrap();
        workspace.poll(&editor);
        assert!(workspace.active.is_none());
        assert!(workspace.drawing.is_none());
        assert!(workspace.cameras.is_empty());
    }

    #[test]
    fn failed_worker_is_visible_and_not_retried_every_frame() {
        let mut editor = Editor::new().unwrap();
        let level = *editor.document.model().levels.keys().next().unwrap();
        let view = editor.create_floor_plan("Plan", level).unwrap();
        let context = editor.native_plan_context(view).unwrap();
        let mut workspace = PlanWorkspace {
            session: Some(editor.document.session_id()),
            active: Some(view),
            desired: Some(context),
            attempted: true,
            ..Default::default()
        };
        workspace.pending = Some((
            std::thread::spawn(|| Err(Error::Invalid("test failure".into()))),
            true,
        ));
        settle(&mut workspace, &editor);
        for _ in 0..5 {
            workspace.poll(&editor);
        }
        assert!(workspace.pending.is_none());
        assert!(workspace.drawing.is_none());
        assert!(workspace.error.as_ref().unwrap().contains("test failure"));
    }
}
