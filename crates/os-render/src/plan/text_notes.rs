//! Shared fixed-cell ASCII layout in paper units; adapters consume identical anchors.
use super::*;
use os_model::{NoteAlignment, TextNoteParams};

#[derive(Clone, Debug, PartialEq)]
pub struct NoteGraphic {
    pub entity: Id,
    pub parameters: TextNoteParams,
}
#[derive(Clone, Debug, PartialEq)]
pub struct NoteLayout {
    pub bounds: PlanCrop,
    pub glyphs: Vec<(Point2, char)>,
    pub leader: Vec<(Point2, Point2)>,
}
impl NoteGraphic {
    pub fn layout(&self, scale: f64) -> Result<NoteLayout> {
        self.parameters.validate_content()?;
        ensure(
            scale.is_finite() && scale > 0. && scale <= 1e6,
            "invalid note scale",
        )?;
        let p = &self.parameters;
        let unit = scale / 1000.;
        let cell = p.text_height_mm * unit;
        let width = p.width_mm * unit;
        let rows = p.rows();
        let mut glyphs = Vec::new();
        for (row, text) in rows.iter().enumerate() {
            let spare = width - text.len() as f64 * cell;
            let offset = match p.alignment {
                NoteAlignment::Left => 0.,
                NoteAlignment::Center => spare / 2.,
                NoteAlignment::Right => spare,
            };
            for (column, character) in text.chars().enumerate() {
                glyphs.push((
                    Point2::new(
                        p.position.x + offset + column as f64 * cell,
                        p.position.y - cell - row as f64 * cell * 1.25,
                    ),
                    character,
                ));
            }
        }
        let mut leader = Vec::new();
        if let Some(tip) = p.leader {
            leader.push((p.position, tip));
            let length = p.position.distance(tip);
            let ux = (p.position.x - tip.x) / length;
            let uy = (p.position.y - tip.y) / length;
            let a = (2.0 * unit).min(length * 0.4);
            for sign in [-1., 1.] {
                leader.push((
                    tip,
                    Point2::new(
                        tip.x + a * ux - sign * a * 0.4 * uy,
                        tip.y + a * uy + sign * a * 0.4 * ux,
                    ),
                ));
            }
        }
        Ok(NoteLayout {
            bounds: PlanCrop {
                min: Point2::new(
                    p.position.x,
                    p.position.y - cell * (rows.len() as f64 * 1.25),
                ),
                max: Point2::new(p.position.x + width, p.position.y),
            },
            glyphs,
            leader,
        })
    }
}
impl PlanDrawing {
    pub fn with_text_notes(mut self, notes: Vec<NoteGraphic>) -> Result<Self> {
        ensure(
            notes.len() <= 1000
                && notes.iter().map(|n| n.parameters.text.len()).sum::<usize>() <= 100_000,
            "note graphic budget exceeded",
        )?;
        for note in &notes {
            ensure(
                !note.entity.0.is_nil()
                    && note.parameters.view == self.context.view_id
                    && self.source_ids.insert(note.entity),
                "invalid note identity/view",
            )?;
            note.parameters.validate_content()?;
        }
        self.text_notes = notes;
        Ok(self)
    }
    pub fn text_notes(&self, current: PlanContext) -> Result<&[NoteGraphic]> {
        ensure(self.context == current, "stale note drawing")?;
        Ok(&self.text_notes)
    }
    pub fn pick_text_note(
        &self,
        current: PlanContext,
        point: Point2,
        tolerance: f64,
    ) -> Result<Option<Id>> {
        ensure(
            point.is_finite() && tolerance.is_finite() && tolerance >= 0.,
            "invalid note pick",
        )?;
        if current.annotation_crop.is_some() {
            let ppm = if tolerance > 0. { 8. / tolerance } else { 100. };
            let camera = PlanCamera {
                center: point,
                pixels_per_metre: ppm,
            };
            return Ok(self
                .annotation_hits(
                    current,
                    camera,
                    [100., 100.],
                    Point2::new(50., 50.),
                    if tolerance > 0. { 8. } else { 0. },
                    |id| self.text_notes.iter().any(|n| n.entity == id),
                )?
                .into_iter()
                .next());
        }
        if current.crop.is_some_and(|c| !point_in_crop(c, point)) {
            return Ok(None);
        }
        for note in self.text_notes(current)?.iter().rev() {
            let layout = note.layout(current.scale_denominator)?;
            if point_in_crop(layout.bounds, point) {
                return Ok(Some(note.entity));
            }
            for (a, b) in layout.leader {
                let l = a.distance(b);
                if l <= 1e-9 {
                    continue;
                }
                let t = (((point.x - a.x) * (b.x - a.x) + (point.y - a.y) * (b.y - a.y)) / (l * l))
                    .clamp(0., 1.);
                if point.distance(Point2::new(a.x + t * (b.x - a.x), a.y + t * (b.y - a.y)))
                    <= tolerance
                {
                    return Ok(Some(note.entity));
                }
            }
        }
        Ok(None)
    }
}
