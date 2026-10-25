//! One paper-unit annotation layout for enabled Plan annotation crops.
use super::*;
use os_core::Id;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnnotationKind {
    Dimension,
    OpeningTag,
    DetailLine,
    Note,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AnnotationGraphic {
    pub entity: Id,
    pub kind: AnnotationKind,
    pub marks: Vec<PaperMark>,
}

/// Bounds are in the owning view plane. The caller may intersect them with a
/// sheet viewport; native navigation does not change the paper-unit layout.
pub fn annotation_graphics(
    context: PlanContext,
    drawing: &PlanDrawing,
    viewport: PaperViewport,
    bounds: PlanCrop,
) -> Result<Vec<AnnotationGraphic>> {
    drawing.items(context)?;
    bounds.validate()?;
    let clip = PaperRect {
        min_mm: viewport.model_to_paper(Point2::new(bounds.min.x, bounds.max.y))?,
        max_mm: viewport.model_to_paper(Point2::new(bounds.max.x, bounds.min.y))?,
    };
    let stroke = |width_mm| PaperStroke {
        color: PaperColor::BLACK,
        width_mm,
        dashed: false,
    };
    let mut graphics = Vec::new();
    let mut count = 0usize;
    let mut append = |graphic: AnnotationGraphic| -> Result<()> {
        count = count.saturating_add(graphic.marks.len());
        ensure(count <= MAX_SHEET_MARKS, "annotation mark budget exceeded")?;
        graphics.push(graphic);
        Ok(())
    };
    let line = |marks: &mut Vec<PaperMark>, a, b, width| -> Result<()> {
        if let Some((a, b)) = clip_segment(a, b, bounds) {
            if a.distance(b) > 1e-9 {
                push_line(marks, a, b, stroke(width), viewport, Some(clip))?;
            }
        }
        Ok(())
    };
    // Fixed cells retain placement at crop edges rather than re-centering a
    // truncated label. PDF, native paint and picking consume these same cells.
    let text = |marks: &mut Vec<PaperMark>, anchor: Point2, label: &str, size: f64| -> Result<()> {
        let cell = size * viewport.scale_denominator / 1000.;
        for (i, ch) in label.chars().enumerate() {
            let p = Point2::new(anchor.x + i as f64 * cell, anchor.y);
            if p.x + cell <= bounds.min.x
                || p.x >= bounds.max.x
                || p.y + cell <= bounds.min.y
                || p.y >= bounds.max.y
            {
                continue;
            }
            marks.push(PaperMark {
                clip: Some(clip),
                kind: PaperMarkKind::Text {
                    baseline_mm: viewport.model_to_paper(p)?,
                    text: ch.to_string(),
                    size_mm: size,
                    color: PaperColor::BLACK,
                },
            });
        }
        Ok(())
    };
    for root in drawing.dimensions(context)? {
        let mut marks = Vec::new();
        for d in root.graphics() {
            for (a, b) in d.lines() {
                line(&mut marks, a, b, 0.15)?;
            }
            let center = if d.value_m.is_some() {
                Point2::new(
                    (d.line_start.x + d.line_end.x) / 2.,
                    (d.line_start.y + d.line_end.y) / 2.,
                )
            } else {
                d.orphan_hint
            };
            let label = d.label();
            let unit = viewport.scale_denominator / 1000.;
            text(
                &mut marks,
                Point2::new(
                    center.x - label.len() as f64 * 2.2 * unit / 2.,
                    center.y + 1.0 * unit,
                ),
                &label,
                2.2,
            )?;
        }
        append(AnnotationGraphic {
            entity: root.entity,
            kind: AnnotationKind::Dimension,
            marks,
        })?;
    }
    let annotation_context = PlanContext {
        crop: context.annotation_bounds(),
        ..context
    };
    for d in drawing.angular_dimensions(context)? {
        let mut marks = Vec::new();
        for (a, b) in d.segments(annotation_context)? {
            line(&mut marks, a, b, 0.15)?;
        }
        text(
            &mut marks,
            if d.diagnostic.is_some() {
                d.orphan_hint
            } else {
                d.point(0.5)
            },
            &d.label(),
            2.2,
        )?;
        append(AnnotationGraphic {
            entity: d.entity,
            kind: AnnotationKind::Dimension,
            marks,
        })?;
    }
    for d in drawing.arc_length_dimensions(context)? {
        let mut marks = Vec::new();
        for (a, b) in d.segments(annotation_context)? {
            line(&mut marks, a, b, 0.15)?;
        }
        text(
            &mut marks,
            if d.diagnostic.is_some() {
                d.orphan_hint
            } else {
                d.point(0.5)
            },
            &d.label(),
            2.2,
        )?;
        append(AnnotationGraphic {
            entity: d.entity,
            kind: AnnotationKind::Dimension,
            marks,
        })?;
    }
    for tag in drawing.opening_tags(context)? {
        let mut marks = Vec::new();
        let unit = viewport.scale_denominator / 1000.;
        let half = (tag.label.len() as f64 * 2.5 / 2. + 1.).max(5.) * unit;
        let box_bounds = PlanCrop {
            min: Point2::new(tag.anchor.x - half, tag.anchor.y - 2.5 * unit),
            max: Point2::new(tag.anchor.x + half, tag.anchor.y + 2.5 * unit),
        };
        let corners = [
            box_bounds.min,
            Point2::new(box_bounds.max.x, box_bounds.min.y),
            box_bounds.max,
            Point2::new(box_bounds.min.x, box_bounds.max.y),
        ];
        for i in 0..4 {
            line(&mut marks, corners[i], corners[(i + 1) % 4], 0.18)?;
        }
        if let Some(source) = tag.leader_source.filter(|_| tag.diagnostic.is_none())
            && let Some((a, b)) = crate::plan::PlanOpeningTag::leader_to_box(
                source,
                [box_bounds.min, box_bounds.max],
                0.1 * unit,
            )
        {
            line(&mut marks, a, b, 0.18)?;
        }
        text(
            &mut marks,
            Point2::new(
                tag.anchor.x - tag.label.len() as f64 * 2.5 * unit / 2.,
                tag.anchor.y - 1.25 * unit,
            ),
            &tag.label,
            2.5,
        )?;
        append(AnnotationGraphic {
            entity: tag.entity,
            kind: AnnotationKind::OpeningTag,
            marks,
        })?;
    }
    for d in drawing.detail_lines(context)? {
        let mut marks = Vec::new();
        line(
            &mut marks,
            d.start,
            d.end,
            crate::plan::DETAIL_LINE_WEIGHT_MM,
        )?;
        append(AnnotationGraphic {
            entity: d.entity,
            kind: AnnotationKind::DetailLine,
            marks,
        })?;
    }
    for note in drawing.text_notes(context)? {
        let mut marks = Vec::new();
        let layout = note.layout(viewport.scale_denominator)?;
        for (p, ch) in layout.glyphs {
            text(
                &mut marks,
                p,
                &ch.to_string(),
                note.parameters.text_height_mm,
            )?;
        }
        for (a, b) in layout.leader {
            line(&mut marks, a, b, 0.25)?;
        }
        append(AnnotationGraphic {
            entity: note.entity,
            kind: AnnotationKind::Note,
            marks,
        })?;
    }
    Ok(graphics)
}
