//! Bounded paper-space vector pages and a deliberately limited single-page PDF adapter.
//!
//! Coordinates are millimetres from the upper-left paper corner. PDF text uses the
//! standard Helvetica/WinAnsi font; this adapter does not embed fonts or claim a
//! production typography, pagination, hyperlink, or publishing workflow.
use crate::plan::{
    PlanAngularDimension, PlanContext, PlanDimensionItem, PlanDrawing, PlanFloorItem, PlanGrid,
    PlanItem, PlanLine, PlanRoomItem,
};
use os_core::{Error, Point2, Result, ensure};
use os_geometry::plan::{PlanCrop, PlanRole};

pub const MAX_SHEET_MARKS: usize = 100_000;
pub const MAX_SHEET_POINTS: usize = 1_000_000;
const MAX_PDF_BYTES: usize = 64 * 1024 * 1024;
const MM_TO_PT: f64 = 72.0 / 25.4;
mod tables;
pub use tables::{PaperTable, append_schedule_table};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PaperRect {
    pub min_mm: Point2,
    pub max_mm: Point2,
}

impl PaperRect {
    pub fn validate(self) -> Result<()> {
        ensure(
            self.min_mm.is_finite()
                && self.max_mm.is_finite()
                && self.min_mm.x >= 0.0
                && self.min_mm.y >= 0.0
                && self.max_mm.x > self.min_mm.x
                && self.max_mm.y > self.min_mm.y
                && self.max_mm.x <= 5_000.0
                && self.max_mm.y <= 5_000.0,
            "invalid paper rectangle",
        )
    }
    pub fn contains(self, point: Point2) -> bool {
        point.is_finite()
            && point.x >= self.min_mm.x
            && point.y >= self.min_mm.y
            && point.x <= self.max_mm.x
            && point.y <= self.max_mm.y
    }
}

/// A 2D drawing viewport mapping. `model_center_m` uses source-view plane
/// coordinates; source-world transforms are resolved by the drawing context.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PaperViewport {
    pub center_mm: Point2,
    pub width_mm: f64,
    pub height_mm: f64,
    pub model_center_m: Point2,
    pub scale_denominator: f64,
}

impl PaperViewport {
    pub fn validate(self) -> Result<()> {
        ensure(
            self.center_mm.is_finite()
                && self.model_center_m.is_finite()
                && (10.0..=5_000.0).contains(&self.width_mm)
                && (10.0..=5_000.0).contains(&self.height_mm)
                && (0.001..=1_000_000.0).contains(&self.scale_denominator)
                && self.width_mm.is_finite()
                && self.height_mm.is_finite()
                && self.scale_denominator.is_finite(),
            "invalid sheet viewport mapping",
        )
    }
    pub fn paper_rect(self) -> Result<PaperRect> {
        self.validate()?;
        let rect = PaperRect {
            min_mm: Point2::new(
                self.center_mm.x - self.width_mm * 0.5,
                self.center_mm.y - self.height_mm * 0.5,
            ),
            max_mm: Point2::new(
                self.center_mm.x + self.width_mm * 0.5,
                self.center_mm.y + self.height_mm * 0.5,
            ),
        };
        rect.validate()?;
        Ok(rect)
    }
    /// Convert source-view-plane metres to paper millimetres at the persisted scale.
    pub fn model_to_paper(self, point_m: Point2) -> Result<Point2> {
        self.validate()?;
        ensure(point_m.is_finite(), "invalid sheet model coordinate")?;
        let millimetres_per_metre = 1_000.0 / self.scale_denominator;
        let point = Point2::new(
            self.center_mm.x + (point_m.x - self.model_center_m.x) * millimetres_per_metre,
            self.center_mm.y - (point_m.y - self.model_center_m.y) * millimetres_per_metre,
        );
        ensure(point.is_finite(), "sheet projection overflow")?;
        Ok(point)
    }
    /// The source-plane rectangle visible through this viewport.
    pub fn model_bounds(self) -> Result<os_geometry::plan::PlanCrop> {
        self.validate()?;
        let half_width = self.width_mm * self.scale_denominator / 2_000.0;
        let half_height = self.height_mm * self.scale_denominator / 2_000.0;
        let bounds = os_geometry::plan::PlanCrop {
            min: Point2::new(
                self.model_center_m.x - half_width,
                self.model_center_m.y - half_height,
            ),
            max: Point2::new(
                self.model_center_m.x + half_width,
                self.model_center_m.y + half_height,
            ),
        };
        bounds.validate()?;
        Ok(bounds)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PaperColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl PaperColor {
    pub const BLACK: Self = Self {
        red: 0,
        green: 0,
        blue: 0,
    };
    pub const WHITE: Self = Self {
        red: 255,
        green: 255,
        blue: 255,
    };
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PaperStroke {
    pub color: PaperColor,
    pub width_mm: f64,
    pub dashed: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PaperMarkKind {
    Path {
        points_mm: Vec<Point2>,
        closed: bool,
        stroke: Option<PaperStroke>,
        fill: Option<PaperColor>,
    },
    Text {
        baseline_mm: Point2,
        text: String,
        size_mm: f64,
        color: PaperColor,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct PaperMark {
    /// When present, clip the mark to this page-space rectangle.
    pub clip: Option<PaperRect>,
    pub kind: PaperMarkKind,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SheetPage {
    pub width_mm: f64,
    pub height_mm: f64,
    marks: Vec<PaperMark>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PaperSheetInfo<'a> {
    pub width_mm: f64,
    pub height_mm: f64,
    pub number: &'a str,
    pub name: &'a str,
}

impl SheetPage {
    pub fn new(width_mm: f64, height_mm: f64, marks: Vec<PaperMark>) -> Result<Self> {
        let page = Self {
            width_mm,
            height_mm,
            marks,
        };
        page.validate()?;
        Ok(page)
    }
    pub fn marks(&self) -> &[PaperMark] {
        &self.marks
    }
    pub fn validate(&self) -> Result<()> {
        ensure(
            self.width_mm.is_finite()
                && self.height_mm.is_finite()
                && (10.0..=5_000.0).contains(&self.width_mm)
                && (10.0..=5_000.0).contains(&self.height_mm)
                && self.marks.len() <= MAX_SHEET_MARKS,
            "invalid sheet page or mark count",
        )?;
        let page_bounds = PaperRect {
            min_mm: Point2::new(0.0, 0.0),
            max_mm: Point2::new(self.width_mm, self.height_mm),
        };
        let mut points = 0usize;
        let mut estimated_pdf_bytes = 1_024usize;
        for mark in &self.marks {
            if let Some(clip) = mark.clip {
                clip.validate()?;
                ensure(
                    clip.max_mm.x <= self.width_mm && clip.max_mm.y <= self.height_mm,
                    "sheet mark clip exceeds page",
                )?;
            }
            match &mark.kind {
                PaperMarkKind::Path {
                    points_mm,
                    closed,
                    stroke,
                    fill,
                } => {
                    points = points.saturating_add(points_mm.len());
                    estimated_pdf_bytes = estimated_pdf_bytes
                        .saturating_add(points_mm.len().saturating_mul(48))
                        .saturating_add(160);
                    ensure(
                        points_mm.len() >= if *closed { 3 } else { 2 }
                            && points_mm.len() <= MAX_SHEET_POINTS
                            && points_mm.iter().all(|p| {
                                p.is_finite()
                                    && p.x.abs() <= 1_000_000.0
                                    && p.y.abs() <= 1_000_000.0
                            })
                            && (stroke.is_some() || fill.is_some())
                            && stroke.is_none_or(|s| {
                                s.width_mm.is_finite() && (0.0..=100.0).contains(&s.width_mm)
                            }),
                        "invalid sheet path",
                    )?;
                    if *closed && fill.is_none() && stroke.is_none() {
                        return Err(Error::Invalid("empty sheet path style".into()));
                    }
                }
                PaperMarkKind::Text {
                    baseline_mm,
                    text,
                    size_mm,
                    ..
                } => {
                    estimated_pdf_bytes = estimated_pdf_bytes
                        .saturating_add(text.len().saturating_mul(2))
                        .saturating_add(160);
                    ensure(
                        baseline_mm.is_finite()
                            && baseline_mm.x.abs() <= 1_000_000.0
                            && baseline_mm.y.abs() <= 1_000_000.0
                            && !text.is_empty()
                            && text.len() <= 4096
                            && !text.chars().any(char::is_control)
                            && size_mm.is_finite()
                            && (0.1..=100.0).contains(size_mm),
                        "invalid sheet text",
                    )?;
                }
            }
            ensure(
                points <= MAX_SHEET_POINTS && estimated_pdf_bytes <= MAX_PDF_BYTES,
                "sheet exceeds vector output budget",
            )?;
        }
        ensure(page_bounds.max_mm.is_finite(), "invalid sheet page bounds")
    }

    /// Write a single vector PDF page using the PDF-standard Helvetica font.
    /// Non-WinAnsi text is rejected rather than silently transliterated.
    pub fn to_pdf(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let content = self.content_stream()?;
        ensure(
            content.len() <= MAX_PDF_BYTES,
            "sheet PDF exceeds output limit",
        )?;
        let width = self.width_mm * MM_TO_PT;
        let height = self.height_mm * MM_TO_PT;
        let objects = [
            b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
            b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {:.6} {:.6}] /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>",
                width, height
            )
            .into_bytes(),
            {
                let mut stream = format!("<< /Length {} >>\nstream\n", content.len()).into_bytes();
                stream.extend_from_slice(&content);
                stream.extend_from_slice(b"endstream");
                stream
            },
            b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>"
                .to_vec(),
        ];
        let mut output = b"%PDF-1.4\n%OpenStructure\n".to_vec();
        let mut offsets = Vec::with_capacity(objects.len() + 1);
        offsets.push(0usize);
        for (index, object) in objects.iter().enumerate() {
            offsets.push(output.len());
            output.extend_from_slice(format!("{} 0 obj\n", index + 1).as_bytes());
            output.extend_from_slice(object);
            output.extend_from_slice(b"\nendobj\n");
            ensure(
                output.len() <= MAX_PDF_BYTES,
                "sheet PDF exceeds output limit",
            )?;
        }
        let xref = output.len();
        output.extend_from_slice(format!("xref\n0 {}\n", offsets.len()).as_bytes());
        output.extend_from_slice(b"0000000000 65535 f \n");
        for offset in offsets.into_iter().skip(1) {
            ensure(
                offset <= 9_999_999_999,
                "sheet PDF offset exceeds xref limit",
            )?;
            output.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
        }
        output.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
                objects.len() + 1
            )
            .as_bytes(),
        );
        ensure(
            output.len() <= MAX_PDF_BYTES,
            "sheet PDF exceeds output limit",
        )?;
        Ok(output)
    }

    fn content_stream(&self) -> Result<Vec<u8>> {
        let mut content = Vec::new();
        content.extend_from_slice(b"q\n1 J\n1 j\n");
        for mark in &self.marks {
            if let Some(clip) = mark.clip {
                write_clip(&mut content, clip, self.height_mm);
            } else {
                content.extend_from_slice(b"q\n");
            }
            match &mark.kind {
                PaperMarkKind::Path {
                    points_mm,
                    closed,
                    stroke,
                    fill,
                } => write_path(
                    &mut content,
                    points_mm,
                    *closed,
                    *stroke,
                    *fill,
                    self.height_mm,
                ),
                PaperMarkKind::Text {
                    baseline_mm,
                    text,
                    size_mm,
                    color,
                } => write_text(
                    &mut content,
                    *baseline_mm,
                    text,
                    *size_mm,
                    *color,
                    self.height_mm,
                )?,
            }
            content.extend_from_slice(b"Q\n");
        }
        content.extend_from_slice(b"Q\n");
        Ok(content)
    }
}

/// Compose currently supported native 2D-view graphics into one paper viewport.
/// This deliberately uses the same checked `PlanDrawing` consumed by the editor;
/// stale or unresolved provider geometry is rejected instead of silently omitted.
pub fn compose_view_sheet(
    paper: PaperSheetInfo<'_>,
    viewport_title: &str,
    viewport: PaperViewport,
    context: PlanContext,
    drawing: &PlanDrawing,
) -> Result<SheetPage> {
    ensure(
        !paper.number.trim().is_empty()
            && paper.number.len() <= 64
            && !paper.number.chars().any(char::is_control)
            && !paper.name.trim().is_empty()
            && paper.name.len() <= 256
            && !paper.name.chars().any(char::is_control)
            && !viewport_title.trim().is_empty()
            && viewport_title.len() <= 256
            && !viewport_title.chars().any(char::is_control),
        "invalid sheet title-block text",
    )?;
    let viewport_rect = viewport.paper_rect()?;
    ensure(
        viewport_rect.max_mm.x <= paper.width_mm && viewport_rect.max_mm.y <= paper.height_mm,
        "sheet viewport lies outside paper",
    )?;
    let unavailable = drawing.unavailable(context)?;
    ensure(
        unavailable.is_empty(),
        format!(
            "sheet export blocked: {} view element(s) have unavailable graphics",
            unavailable.len()
        ),
    )?;
    let model_bounds = intersect_crop(context.crop, Some(viewport.model_bounds()?));
    let mut marks = Vec::new();
    let clip = Some(viewport_rect);
    if let Some(bounds) = model_bounds {
        let viewport_color = PaperColor::BLACK;
        for item in drawing.items(context)? {
            append_item(
                &mut marks,
                item,
                bounds,
                viewport,
                clip,
                drawing.appearance(context, item.entity, item.footprint.role)?,
            )?;
        }
        for grid in drawing.grids(context)? {
            append_grid(&mut marks, grid, bounds, viewport, clip)?;
        }
        for line in drawing.provider_lines(context)? {
            append_line(
                &mut marks,
                line,
                bounds,
                viewport,
                clip,
                drawing
                    .is_native_line(line.entity)
                    .then(|| drawing.appearance(context, line.entity, line.role))
                    .transpose()?
                    .flatten(),
            )?;
        }
        for floor in drawing.floors(context)? {
            append_floor(
                &mut marks,
                floor,
                bounds,
                viewport,
                clip,
                drawing.appearance(context, floor.entity, PlanRole::Projected)?,
            )?;
        }
        for room in drawing.rooms(context)? {
            append_room(&mut marks, room, bounds, viewport, clip)?;
        }
        for tag in drawing.room_tags(context)? {
            if bounds_contains(bounds, tag.anchor) {
                push_text(
                    &mut marks,
                    tag.anchor,
                    &tag.label,
                    2.5,
                    viewport,
                    clip,
                    viewport_color,
                )?;
            }
        }
        for dimension in drawing.dimensions(context)? {
            append_dimension(&mut marks, dimension, bounds, viewport, clip)?;
        }
        for dimension in drawing.angular_dimensions(context)? {
            append_angular_dimension(&mut marks, dimension, context, bounds, viewport, clip)?;
        }
        for line in drawing.detail_lines(context)? {
            if let Some((start, end)) = clip_segment(line.start, line.end, bounds) {
                push_line(
                    &mut marks,
                    start,
                    end,
                    PaperStroke {
                        color: PaperColor::BLACK,
                        width_mm: crate::plan::DETAIL_LINE_WEIGHT_MM,
                        dashed: false,
                    },
                    viewport,
                    clip,
                )?;
            }
        }
    }
    // The viewport frame and title block are paper annotations, not model geometry.
    let frame = PaperRect {
        min_mm: Point2::new(8.0, 8.0),
        max_mm: Point2::new(paper.width_mm - 8.0, paper.height_mm - 8.0),
    };
    marks.push(path_mark(
        vec![
            frame.min_mm,
            Point2::new(frame.max_mm.x, frame.min_mm.y),
            frame.max_mm,
            Point2::new(frame.min_mm.x, frame.max_mm.y),
        ],
        true,
        Some(PaperStroke {
            color: PaperColor::BLACK,
            width_mm: 0.25,
            dashed: false,
        }),
        None,
        None,
    ));
    marks.push(path_mark(
        vec![
            Point2::new(frame.min_mm.x, paper.height_mm - 48.0),
            Point2::new(frame.max_mm.x, paper.height_mm - 48.0),
        ],
        false,
        Some(PaperStroke {
            color: PaperColor::BLACK,
            width_mm: 0.25,
            dashed: false,
        }),
        None,
        None,
    ));
    marks.push(path_mark(
        vec![
            Point2::new(paper.width_mm - 145.0, paper.height_mm - 48.0),
            Point2::new(paper.width_mm - 145.0, frame.max_mm.y),
        ],
        false,
        Some(PaperStroke {
            color: PaperColor::BLACK,
            width_mm: 0.18,
            dashed: false,
        }),
        None,
        None,
    ));
    push_page_text(
        &mut marks,
        Point2::new(frame.min_mm.x + 4.0, paper.height_mm - 30.0),
        paper.name,
        4.0,
        PaperColor::BLACK,
    )?;
    push_page_text(
        &mut marks,
        Point2::new(paper.width_mm - 137.0, paper.height_mm - 30.0),
        paper.number,
        4.0,
        PaperColor::BLACK,
    )?;
    push_page_text(
        &mut marks,
        Point2::new(frame.min_mm.x + 4.0, paper.height_mm - 39.0),
        &format!("{viewport_title} - 1:{:.0}", viewport.scale_denominator),
        2.5,
        PaperColor::BLACK,
    )?;
    let viewport_frame = [
        viewport_rect.min_mm,
        Point2::new(viewport_rect.max_mm.x, viewport_rect.min_mm.y),
        viewport_rect.max_mm,
        Point2::new(viewport_rect.min_mm.x, viewport_rect.max_mm.y),
    ];
    marks.push(path_mark(
        viewport_frame.to_vec(),
        true,
        Some(PaperStroke {
            color: PaperColor {
                red: 128,
                green: 128,
                blue: 128,
            },
            width_mm: 0.15,
            dashed: false,
        }),
        None,
        None,
    ));
    SheetPage::new(paper.width_mm, paper.height_mm, marks)
}

fn intersect_crop(first: Option<PlanCrop>, second: Option<PlanCrop>) -> Option<PlanCrop> {
    match (first, second) {
        (Some(a), Some(b)) => {
            let crop = PlanCrop {
                min: Point2::new(a.min.x.max(b.min.x), a.min.y.max(b.min.y)),
                max: Point2::new(a.max.x.min(b.max.x), a.max.y.min(b.max.y)),
            };
            (crop.min.x < crop.max.x && crop.min.y < crop.max.y).then_some(crop)
        }
        (Some(crop), None) | (None, Some(crop)) => Some(crop),
        (None, None) => None,
    }
}

fn bounds_contains(bounds: PlanCrop, point: Point2) -> bool {
    point.x >= bounds.min.x
        && point.x <= bounds.max.x
        && point.y >= bounds.min.y
        && point.y <= bounds.max.y
}

fn clip_segment(start: Point2, end: Point2, bounds: PlanCrop) -> Option<(Point2, Point2)> {
    if !start.is_finite() || !end.is_finite() {
        return None;
    }
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let mut enter: f64 = 0.0;
    let mut leave: f64 = 1.0;
    for (p, q) in [
        (-dx, start.x - bounds.min.x),
        (dx, bounds.max.x - start.x),
        (-dy, start.y - bounds.min.y),
        (dy, bounds.max.y - start.y),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
        } else {
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
    }
    Some((
        Point2::new(start.x + dx * enter, start.y + dy * enter),
        Point2::new(start.x + dx * leave, start.y + dy * leave),
    ))
}

fn clip_polygon(points: &[Point2], bounds: PlanCrop) -> Vec<Point2> {
    let mut output = points.to_vec();
    for edge in 0..4 {
        let input = std::mem::take(&mut output);
        if input.is_empty() {
            break;
        }
        let inside = |point: Point2| match edge {
            0 => point.x >= bounds.min.x,
            1 => point.x <= bounds.max.x,
            2 => point.y >= bounds.min.y,
            _ => point.y <= bounds.max.y,
        };
        let intersect = |a: Point2, b: Point2| match edge {
            0 | 1 => {
                let x = if edge == 0 {
                    bounds.min.x
                } else {
                    bounds.max.x
                };
                let t = (x - a.x) / (b.x - a.x);
                Point2::new(x, a.y + t * (b.y - a.y))
            }
            2 | 3 => {
                let y = if edge == 2 {
                    bounds.min.y
                } else {
                    bounds.max.y
                };
                let t = (y - a.y) / (b.y - a.y);
                Point2::new(a.x + t * (b.x - a.x), y)
            }
            _ => unreachable!(),
        };
        let mut previous = *input.last().expect("nonempty polygon");
        for current in input {
            let previous_inside = inside(previous);
            let current_inside = inside(current);
            match (previous_inside, current_inside) {
                (true, true) => output.push(current),
                (true, false) => output.push(intersect(previous, current)),
                (false, true) => {
                    output.push(intersect(previous, current));
                    output.push(current);
                }
                (false, false) => {}
            }
            previous = current;
        }
    }
    output
}

fn style_for_role(role: PlanRole) -> PaperStroke {
    PaperStroke {
        color: match role {
            PlanRole::Depth => PaperColor {
                red: 128,
                green: 128,
                blue: 128,
            },
            _ => PaperColor::BLACK,
        },
        width_mm: match role {
            PlanRole::Cut => 0.35,
            PlanRole::Projected => 0.18,
            PlanRole::Depth => 0.12,
        },
        dashed: false,
    }
}

fn paper_stroke(stroke: crate::plan::PlanStroke) -> PaperStroke {
    PaperStroke {
        color: PaperColor {
            red: stroke.color[0],
            green: stroke.color[1],
            blue: stroke.color[2],
        },
        width_mm: stroke.weight_mm,
        dashed: stroke.dashed,
    }
}

fn path_mark(
    points_mm: Vec<Point2>,
    closed: bool,
    stroke: Option<PaperStroke>,
    fill: Option<PaperColor>,
    clip: Option<PaperRect>,
) -> PaperMark {
    PaperMark {
        clip,
        kind: PaperMarkKind::Path {
            points_mm,
            closed,
            stroke,
            fill,
        },
    }
}

fn push_line(
    marks: &mut Vec<PaperMark>,
    start: Point2,
    end: Point2,
    stroke: PaperStroke,
    viewport: PaperViewport,
    clip: Option<PaperRect>,
) -> Result<()> {
    let start = viewport.model_to_paper(start)?;
    let end = viewport.model_to_paper(end)?;
    if clip.is_none_or(|rect| rect.contains(start) || rect.contains(end)) {
        marks.push(path_mark(vec![start, end], false, Some(stroke), None, clip));
    } else if let Some(rect) = clip
        && (start.x.min(end.x) <= rect.max_mm.x
            && start.x.max(end.x) >= rect.min_mm.x
            && start.y.min(end.y) <= rect.max_mm.y
            && start.y.max(end.y) >= rect.min_mm.y)
    {
        marks.push(path_mark(vec![start, end], false, Some(stroke), None, clip));
    }
    Ok(())
}

fn push_text(
    marks: &mut Vec<PaperMark>,
    anchor_m: Point2,
    text: &str,
    size_mm: f64,
    viewport: PaperViewport,
    clip: Option<PaperRect>,
    color: PaperColor,
) -> Result<()> {
    let baseline_mm = viewport.model_to_paper(anchor_m)?;
    if clip.is_none_or(|rect| rect.contains(baseline_mm)) {
        marks.push(PaperMark {
            clip,
            kind: PaperMarkKind::Text {
                baseline_mm,
                text: text.to_owned(),
                size_mm,
                color,
            },
        });
    }
    Ok(())
}

fn append_item(
    marks: &mut Vec<PaperMark>,
    item: &PlanItem,
    bounds: PlanCrop,
    viewport: PaperViewport,
    clip: Option<PaperRect>,
    appearance: Option<crate::plan::PlanStroke>,
) -> Result<()> {
    let polygon = clip_polygon(item.footprint.vertices(), bounds);
    if polygon.len() < 3 {
        return Ok(());
    }
    let points_mm = polygon
        .into_iter()
        .map(|point| viewport.model_to_paper(point))
        .collect::<Result<Vec<_>>>()?;
    let [red, green, blue] = item.surface.color().unwrap_or([232, 232, 232]);
    let fill = (item.footprint.role == PlanRole::Cut).then_some(PaperColor { red, green, blue });
    if let Some(style) = appearance {
        marks.push(path_mark(points_mm, true, None, fill, clip));
        for (start, end) in item.outline() {
            if let Some((start, end)) = clip_segment(start, end, bounds) {
                push_line(marks, start, end, paper_stroke(style), viewport, clip)?;
            }
        }
        return Ok(());
    }
    marks.push(path_mark(
        points_mm,
        true,
        item.hidden_edges
            .is_empty()
            .then(|| style_for_role(item.footprint.role)),
        fill,
        clip,
    ));
    if !item.hidden_edges.is_empty() {
        for (a, b) in item.outline() {
            if let Some((a, b)) = clip_segment(a, b, bounds) {
                push_line(
                    marks,
                    a,
                    b,
                    style_for_role(item.footprint.role),
                    viewport,
                    clip,
                )?;
            }
        }
    }
    Ok(())
}

fn append_grid(
    marks: &mut Vec<PaperMark>,
    grid: &PlanGrid,
    bounds: PlanCrop,
    viewport: PaperViewport,
    clip: Option<PaperRect>,
) -> Result<()> {
    if let Some((start, end)) = clip_segment(grid.start, grid.end, bounds) {
        let stroke = PaperStroke {
            color: PaperColor {
                red: 90,
                green: 90,
                blue: 90,
            },
            width_mm: 0.18,
            dashed: false,
        };
        push_line(marks, start, end, stroke, viewport, clip)?;
        push_text(marks, start, &grid.name, 2.2, viewport, clip, stroke.color)?;
        push_text(marks, end, &grid.name, 2.2, viewport, clip, stroke.color)?;
    }
    Ok(())
}

fn append_line(
    marks: &mut Vec<PaperMark>,
    line: &PlanLine,
    bounds: PlanCrop,
    viewport: PaperViewport,
    clip: Option<PaperRect>,
    appearance: Option<crate::plan::PlanStroke>,
) -> Result<()> {
    if let Some((start, end)) = clip_segment(line.start, line.end, bounds) {
        push_line(
            marks,
            start,
            end,
            appearance.map_or_else(|| style_for_role(line.role), paper_stroke),
            viewport,
            clip,
        )?;
    }
    Ok(())
}

fn append_floor(
    marks: &mut Vec<PaperMark>,
    floor: &PlanFloorItem,
    bounds: PlanCrop,
    viewport: PaperViewport,
    clip: Option<PaperRect>,
    appearance: Option<crate::plan::PlanStroke>,
) -> Result<()> {
    let fill = PaperColor {
        red: 245,
        green: 245,
        blue: 245,
    };
    for triangle in &floor.triangles {
        let points = triangle.map(|index| floor.boundary[index as usize]);
        let clipped = clip_polygon(&points, bounds);
        if clipped.len() >= 3 {
            marks.push(path_mark(
                clipped
                    .into_iter()
                    .map(|point| viewport.model_to_paper(point))
                    .collect::<Result<Vec<_>>>()?,
                true,
                None,
                Some(fill),
                clip,
            ));
        }
    }
    let stroke = appearance.map_or(
        PaperStroke {
            color: PaperColor {
                red: 100,
                green: 100,
                blue: 100,
            },
            width_mm: 0.15,
            dashed: false,
        },
        paper_stroke,
    );
    if appearance.is_some() && boundary_inside_crop(bounds, &floor.boundary) {
        let points = floor
            .boundary
            .iter()
            .map(|point| viewport.model_to_paper(*point))
            .collect::<Result<Vec<_>>>()?;
        marks.push(path_mark(points, true, Some(stroke), None, clip));
        return Ok(());
    }
    for (start, end) in floor
        .boundary
        .iter()
        .copied()
        .zip(floor.boundary.iter().copied().cycle().skip(1))
        .take(floor.boundary.len())
    {
        if let Some((a, b)) = clip_segment(start, end, bounds) {
            push_line(marks, a, b, stroke, viewport, clip)?;
        }
    }
    Ok(())
}

fn boundary_inside_crop(bounds: PlanCrop, boundary: &[Point2]) -> bool {
    boundary.iter().all(|p| {
        p.x >= bounds.min.x && p.x <= bounds.max.x && p.y >= bounds.min.y && p.y <= bounds.max.y
    })
}

fn append_room(
    marks: &mut Vec<PaperMark>,
    room: &PlanRoomItem,
    bounds: PlanCrop,
    viewport: PaperViewport,
    clip: Option<PaperRect>,
) -> Result<()> {
    let stroke = PaperStroke {
        color: PaperColor {
            red: 110,
            green: 110,
            blue: 110,
        },
        width_mm: 0.15,
        dashed: false,
    };
    for (start, end) in room
        .boundary
        .iter()
        .copied()
        .zip(room.boundary.iter().copied().cycle().skip(1))
        .take(room.boundary.len())
    {
        if let Some((a, b)) = clip_segment(start, end, bounds) {
            push_line(marks, a, b, stroke, viewport, clip)?;
        }
    }
    let label = if room.diagnostic.is_some() {
        format!("{} - boundary unavailable", room.number)
    } else {
        format!("{} - {} - {:.2} m²", room.number, room.name, room.area_m2)
    };
    if bounds_contains(bounds, room.seed) {
        push_text(
            marks,
            room.seed,
            &label,
            2.5,
            viewport,
            clip,
            PaperColor::BLACK,
        )?;
    }
    Ok(())
}

fn append_dimension(
    marks: &mut Vec<PaperMark>,
    root: &PlanDimensionItem,
    bounds: PlanCrop,
    viewport: PaperViewport,
    clip: Option<PaperRect>,
) -> Result<()> {
    let stroke = PaperStroke {
        color: if root.diagnostic.is_some() {
            PaperColor {
                red: 170,
                green: 0,
                blue: 0,
            }
        } else {
            PaperColor::BLACK
        },
        width_mm: 0.15,
        dashed: false,
    };
    for dimension in root.graphics() {
        for (start, end) in dimension.lines() {
            if let Some((a, b)) = clip_segment(start, end, bounds) {
                push_line(marks, a, b, stroke, viewport, clip)?;
            }
        }
        let (anchor, text) = if let Some(value) = dimension.value_m {
            (
                Point2::new(
                    (dimension.line_start.x + dimension.line_end.x) * 0.5,
                    (dimension.line_start.y + dimension.line_end.y) * 0.5,
                ),
                format!("{value:.3} m"),
            )
        } else {
            (dimension.orphan_hint, dimension.label())
        };
        if bounds_contains(bounds, anchor) {
            push_text(marks, anchor, &text, 2.2, viewport, clip, stroke.color)?;
        }
    }
    Ok(())
}

fn append_angular_dimension(
    marks: &mut Vec<PaperMark>,
    dimension: &PlanAngularDimension,
    context: PlanContext,
    bounds: PlanCrop,
    viewport: PaperViewport,
    clip: Option<PaperRect>,
) -> Result<()> {
    let stroke = PaperStroke {
        color: if dimension.diagnostic.is_some() {
            PaperColor {
                red: 170,
                green: 0,
                blue: 0,
            }
        } else {
            PaperColor::BLACK
        },
        width_mm: 0.15,
        dashed: false,
    };
    for (start, end) in dimension.segments(context)? {
        if let Some((a, b)) = clip_segment(start, end, bounds) {
            push_line(marks, a, b, stroke, viewport, clip)?;
        }
    }
    let anchor = if dimension.diagnostic.is_some() {
        dimension.orphan_hint
    } else {
        dimension.point(0.5)
    };
    if bounds_contains(bounds, anchor) {
        push_text(
            marks,
            anchor,
            &dimension.label(),
            2.2,
            viewport,
            clip,
            stroke.color,
        )?;
    }
    Ok(())
}

fn push_page_text(
    marks: &mut Vec<PaperMark>,
    baseline_mm: Point2,
    text: &str,
    size_mm: f64,
    color: PaperColor,
) -> Result<()> {
    ensure(
        !text.is_empty()
            && text.len() <= 4096
            && !text.chars().any(char::is_control)
            && baseline_mm.is_finite()
            && (0.1..=100.0).contains(&size_mm),
        "invalid sheet title-block text",
    )?;
    marks.push(PaperMark {
        clip: None,
        kind: PaperMarkKind::Text {
            baseline_mm,
            text: text.to_owned(),
            size_mm,
            color,
        },
    });
    Ok(())
}

fn write_clip(output: &mut Vec<u8>, rect: PaperRect, page_height_mm: f64) {
    let x = rect.min_mm.x * MM_TO_PT;
    let y = (page_height_mm - rect.max_mm.y) * MM_TO_PT;
    let width = (rect.max_mm.x - rect.min_mm.x) * MM_TO_PT;
    let height = (rect.max_mm.y - rect.min_mm.y) * MM_TO_PT;
    output
        .extend_from_slice(format!("q\n{x:.6} {y:.6} {width:.6} {height:.6} re W n\n").as_bytes());
}

fn write_path(
    output: &mut Vec<u8>,
    points: &[Point2],
    closed: bool,
    stroke: Option<PaperStroke>,
    fill: Option<PaperColor>,
    page_height_mm: f64,
) {
    if let Some(stroke) = stroke {
        write_color(output, stroke.color, true);
        output.extend_from_slice(format!("{:.6} w\n", stroke.width_mm * MM_TO_PT).as_bytes());
        if stroke.dashed {
            output.extend_from_slice(
                format!("[{} {}] 0 d\n", 3.0 * MM_TO_PT, 1.5 * MM_TO_PT).as_bytes(),
            );
        } else {
            output.extend_from_slice(b"[] 0 d\n");
        }
    }
    if let Some(fill) = fill {
        write_color(output, fill, false);
    }
    for (index, point) in points.iter().enumerate() {
        let x = point.x * MM_TO_PT;
        let y = (page_height_mm - point.y) * MM_TO_PT;
        output.extend_from_slice(
            format!("{x:.6} {y:.6} {}\n", if index == 0 { "m" } else { "l" }).as_bytes(),
        );
    }
    if closed {
        output.extend_from_slice(b"h\n");
    }
    output.extend_from_slice(match (fill.is_some(), stroke.is_some()) {
        (true, true) => b"B\n",
        (true, false) => b"f\n",
        (false, true) => b"S\n",
        (false, false) => b"n\n",
    });
}

fn write_color(output: &mut Vec<u8>, color: PaperColor, stroke: bool) {
    let operator = if stroke { "RG" } else { "rg" };
    output.extend_from_slice(
        format!(
            "{:.6} {:.6} {:.6} {operator}\n",
            f64::from(color.red) / 255.0,
            f64::from(color.green) / 255.0,
            f64::from(color.blue) / 255.0
        )
        .as_bytes(),
    );
}

fn winansi_byte(character: char) -> Option<u8> {
    let code = u32::from(character);
    if (0x20..=0x7e).contains(&code) || (0xa0..=0xff).contains(&code) {
        return Some(code as u8);
    }
    Some(match character {
        '\u{20ac}' => 0x80,
        '\u{201a}' => 0x82,
        '\u{0192}' => 0x83,
        '\u{201e}' => 0x84,
        '\u{2026}' => 0x85,
        '\u{2020}' => 0x86,
        '\u{2021}' => 0x87,
        '\u{02c6}' => 0x88,
        '\u{2030}' => 0x89,
        '\u{0160}' => 0x8a,
        '\u{2039}' => 0x8b,
        '\u{0152}' => 0x8c,
        '\u{017d}' => 0x8e,
        '\u{2018}' => 0x91,
        '\u{2019}' => 0x92,
        '\u{201c}' => 0x93,
        '\u{201d}' => 0x94,
        '\u{2022}' => 0x95,
        '\u{2013}' => 0x96,
        '\u{2014}' => 0x97,
        '\u{02dc}' => 0x98,
        '\u{2122}' => 0x99,
        '\u{0161}' => 0x9a,
        '\u{203a}' => 0x9b,
        '\u{0153}' => 0x9c,
        '\u{017e}' => 0x9e,
        '\u{0178}' => 0x9f,
        _ => return None,
    })
}

fn write_text(
    output: &mut Vec<u8>,
    baseline: Point2,
    text: &str,
    size_mm: f64,
    color: PaperColor,
    page_height_mm: f64,
) -> Result<()> {
    let mut encoded = Vec::with_capacity(text.len());
    for character in text.chars() {
        let Some(byte) = winansi_byte(character) else {
            return Err(Error::Unsupported(format!(
                "PDF Helvetica output does not support U+{:04X}",
                u32::from(character)
            )));
        };
        encoded.push(byte);
    }
    let x = baseline.x * MM_TO_PT;
    let y = (page_height_mm - baseline.y) * MM_TO_PT;
    write_color(output, color, false);
    output.extend_from_slice(
        format!(
            "BT /F1 {:.6} Tf 1 0 0 1 {x:.6} {y:.6} Tm <",
            size_mm * MM_TO_PT
        )
        .as_bytes(),
    );
    for byte in encoded {
        output.extend_from_slice(format!("{byte:02X}").as_bytes());
    }
    output.extend_from_slice(b"> Tj ET\n");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::PlanGrid;
    use os_core::Id;
    use os_geometry::plan::{HorizontalBasis, PlanRange};
    use std::collections::BTreeMap;

    #[test]
    fn scale_conversion_page_bounds_and_pdf_media_box_are_measurable() {
        let viewport = PaperViewport {
            center_mm: Point2::new(210.0, 148.5),
            width_mm: 390.0,
            height_mm: 240.0,
            model_center_m: Point2::new(8.0, -2.0),
            scale_denominator: 100.0,
        };
        let origin = viewport.model_to_paper(viewport.model_center_m).unwrap();
        let one_metre = viewport.model_to_paper(Point2::new(9.0, -2.0)).unwrap();
        assert_eq!(origin, Point2::new(210.0, 148.5));
        assert_eq!(one_metre.x - origin.x, 10.0);
        let bounds = viewport.model_bounds().unwrap();
        assert_eq!(bounds.min, Point2::new(-11.5, -14.0));
        assert_eq!(bounds.max, Point2::new(27.5, 10.0));

        let page = SheetPage::new(
            420.0,
            297.0,
            vec![PaperMark {
                clip: Some(viewport.paper_rect().unwrap()),
                kind: PaperMarkKind::Path {
                    points_mm: vec![origin, one_metre],
                    closed: false,
                    stroke: Some(PaperStroke {
                        color: PaperColor::BLACK,
                        width_mm: 0.25,
                        dashed: false,
                    }),
                    fill: None,
                },
            }],
        )
        .unwrap();
        let pdf = String::from_utf8(page.to_pdf().unwrap()).unwrap();
        assert!(pdf.contains("/MediaBox [0 0 1190.551181 841.889764]"));
        assert!(pdf.contains("595.275591 420.944882 m\n623.622047 420.944882 l"));
        assert!(pdf.contains("/BaseFont /Helvetica /Encoding /WinAnsiEncoding"));
        assert!(pdf.contains(" re W n"));
        assert!(pdf.ends_with("%%EOF\n"));
    }

    #[test]
    fn pdf_text_is_searchable_and_unsupported_scripts_fail_explicitly() {
        let page = SheetPage::new(
            420.0,
            297.0,
            vec![PaperMark {
                clip: None,
                kind: PaperMarkKind::Text {
                    baseline_mm: Point2::new(20.0, 30.0),
                    text: "Door - Café".into(),
                    size_mm: 2.5,
                    color: PaperColor::BLACK,
                },
            }],
        )
        .unwrap();
        let pdf = String::from_utf8(page.to_pdf().unwrap()).unwrap();
        assert!(pdf.contains("<446F6F72202D20436166E9> Tj"));
        let unsupported = SheetPage::new(
            420.0,
            297.0,
            vec![PaperMark {
                clip: None,
                kind: PaperMarkKind::Text {
                    baseline_mm: Point2::new(20.0, 30.0),
                    text: "東京".into(),
                    size_mm: 2.5,
                    color: PaperColor::BLACK,
                },
            }],
        )
        .unwrap();
        assert!(unsupported.to_pdf().is_err());
    }

    #[test]
    fn page_and_viewport_validation_reject_nonfinite_and_out_of_page_bounds() {
        assert!(
            PaperViewport {
                center_mm: Point2::new(20.0, 20.0),
                width_mm: 20.0,
                height_mm: 20.0,
                model_center_m: Point2::default(),
                scale_denominator: f64::NAN,
            }
            .validate()
            .is_err()
        );
        assert!(SheetPage::new(f64::INFINITY, 297.0, vec![]).is_err());
        assert!(
            SheetPage::new(
                420.0,
                297.0,
                vec![PaperMark {
                    clip: Some(PaperRect {
                        min_mm: Point2::new(400.0, 10.0),
                        max_mm: Point2::new(440.0, 100.0),
                    }),
                    kind: PaperMarkKind::Path {
                        points_mm: vec![Point2::new(400.0, 10.0), Point2::new(410.0, 20.0)],
                        closed: false,
                        stroke: Some(PaperStroke {
                            color: PaperColor::BLACK,
                            width_mm: 0.2,
                            dashed: false,
                        }),
                        fill: None,
                    },
                }]
            )
            .is_err()
        );
    }

    #[test]
    fn plan_composition_reuses_checked_drawings_clips_to_viewport_and_rejects_stale_or_missing() {
        let context = PlanContext {
            session_id: Id::new(),
            model_revision: 3,
            view_id: Id::new(),
            settings_revision: 2,
            basis: HorizontalBasis {
                origin: Point2::new(12.0, -7.0),
                rotation: 0.63,
            },
            range: PlanRange::default(),
            crop: Some(PlanCrop {
                min: Point2::new(-5.0, -2.0),
                max: Point2::new(5.0, 2.0),
            }),
            scale_denominator: 100.0,
            show_walls: true,
            show_extensions: true,
        };
        let grid = PlanGrid {
            entity: Id::new(),
            name: "A".into(),
            start: Point2::new(-10.0, 0.0),
            end: Point2::new(10.0, 0.0),
        };
        let drawing = PlanDrawing::from_prisms(context, &BTreeMap::new(), vec![])
            .unwrap()
            .with_grids(vec![grid])
            .unwrap();
        let viewport = PaperViewport {
            center_mm: Point2::new(210.0, 130.0),
            width_mm: 100.0,
            height_mm: 40.0,
            model_center_m: Point2::default(),
            scale_denominator: 100.0,
        };
        let page = compose_view_sheet(
            PaperSheetInfo {
                width_mm: 420.0,
                height_mm: 297.0,
                number: "A101",
                name: "Level 1 Plan",
            },
            "Level 1",
            viewport,
            context,
            &drawing,
        )
        .unwrap();
        let first = &page.marks()[0];
        let PaperMarkKind::Path { points_mm, .. } = &first.kind else {
            panic!("the first plan mark is the clipped grid line")
        };
        assert_eq!(points_mm[0], Point2::new(160.0, 130.0));
        assert_eq!(points_mm[1], Point2::new(260.0, 130.0));
        assert_eq!(first.clip, Some(viewport.paper_rect().unwrap()));
        let pdf = String::from_utf8(page.to_pdf().unwrap()).unwrap();
        assert!(pdf.contains("453.543307 473.385827 m\n737.007874 473.385827 l"));

        let mut stale = context;
        stale.model_revision += 1;
        assert!(
            compose_view_sheet(
                PaperSheetInfo {
                    width_mm: 420.0,
                    height_mm: 297.0,
                    number: "A101",
                    name: "Level 1 Plan",
                },
                "Level 1",
                viewport,
                stale,
                &drawing,
            )
            .is_err()
        );

        let unavailable =
            PlanDrawing::from_prisms(context, &BTreeMap::new(), vec![Id::new()]).unwrap();
        assert!(
            compose_view_sheet(
                PaperSheetInfo {
                    width_mm: 420.0,
                    height_mm: 297.0,
                    number: "A101",
                    name: "Level 1 Plan",
                },
                "Level 1",
                viewport,
                context,
                &unavailable,
            )
            .is_err()
        );
    }
}
