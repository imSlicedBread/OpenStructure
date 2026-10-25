use super::*;
use crate::sheet::{AnnotationGraphic, PaperMarkKind, PaperViewport, annotation_graphics};

impl PlanDrawing {
    /// A navigation-independent paper mapping, also used by native hit testing.
    pub fn annotation_viewport(context: PlanContext, center: Point2) -> PaperViewport {
        PaperViewport {
            center_mm: Point2::new(2500., 2500.),
            width_mm: 5000.,
            height_mm: 5000.,
            model_center_m: center,
            scale_denominator: context.scale_denominator,
        }
    }
    pub fn native_annotation_graphics(
        &self,
        context: PlanContext,
        center: Point2,
    ) -> Result<Vec<AnnotationGraphic>> {
        let bounds = context
            .annotation_crop
            .ok_or_else(|| os_core::Error::Invalid("annotation crop is disabled".into()))?;
        annotation_graphics(
            context,
            self,
            Self::annotation_viewport(context, center),
            bounds,
        )
    }
    pub fn annotation_hits(
        &self,
        context: PlanContext,
        camera: PlanCamera,
        size: [f64; 2],
        pointer: Point2,
        radius: f64,
        accept: impl Fn(Id) -> bool,
    ) -> Result<Vec<Id>> {
        ensure(
            pointer.is_finite() && radius.is_finite() && (0.0..=64.).contains(&radius),
            "invalid annotation pick",
        )?;
        let plane = camera.unproject(pointer, size)?;
        let viewport = Self::annotation_viewport(context, camera.center);
        let paper = viewport.model_to_paper(plane)?;
        let tolerance = radius / (camera.pixels_per_metre * context.scale_denominator / 1000.);
        let mut hits = Vec::new();
        for graphic in self
            .native_annotation_graphics(context, camera.center)?
            .iter()
            .rev()
            .filter(|g| accept(g.entity))
        {
            if graphic.marks.iter().any(|mark| {
                if mark.clip.is_some_and(|c| !c.contains(paper)) {
                    return false;
                }
                match &mark.kind {
                    PaperMarkKind::Path { points_mm, .. } => points_mm
                        .windows(2)
                        .any(|p| point_segment_distance(paper, p[0], p[1]) <= tolerance),
                    PaperMarkKind::Text {
                        baseline_mm,
                        text,
                        size_mm,
                        ..
                    } => {
                        paper.x >= baseline_mm.x
                            && paper.x <= baseline_mm.x + text.len() as f64 * size_mm
                            && paper.y >= baseline_mm.y - size_mm
                            && paper.y <= baseline_mm.y
                    }
                }
            }) {
                hits.push(graphic.entity);
            }
        }
        Ok(hits)
    }
}
