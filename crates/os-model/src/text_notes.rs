//! Independent notes. Plan anchors use world XY; Section anchors use station/elevation.
use crate::{Entity, Model, ViewKind};
use os_core::{Id, Point2, Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NoteAlignment {
    Left,
    Center,
    Right,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextNoteParams {
    pub view: Id,
    /// Top-left of the wrapping box in the owning view's coordinate convention.
    #[serde(deserialize_with = "crate::sections::strict_point")]
    pub position: Point2,
    pub text: String,
    pub width_mm: f64,
    pub text_height_mm: f64,
    pub alignment: NoteAlignment,
    /// Optional arrow tip in the same coordinate convention as position.
    #[serde(deserialize_with = "strict_tip")]
    pub leader: Option<Point2>,
}
pub type TextNote = Entity<TextNoteParams>;

fn strict_tip<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<Option<Point2>, D::Error> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Point {
        x: f64,
        y: f64,
    }
    Ok(Option::<Point>::deserialize(d)?.map(|p| Point2::new(p.x, p.y)))
}
impl TextNoteParams {
    pub fn new(view: Id) -> Self {
        Self {
            view,
            position: Point2::default(),
            text: "Note".into(),
            width_mm: 80.,
            text_height_mm: 2.5,
            alignment: NoteAlignment::Left,
            leader: None,
        }
    }
    pub fn validate_content(&self) -> Result<()> {
        ensure(
            !self.text.trim().is_empty()
                && self.text.len() <= 2048
                && self
                    .text
                    .bytes()
                    .all(|b| b == b'\n' || (32..=126).contains(&b)),
            "notes support 1-2048 printable ASCII characters and newlines; unsupported text must be replaced",
        )?;
        ensure(
            self.width_mm.is_finite()
                && self.text_height_mm.is_finite()
                && (1.0..=20.0).contains(&self.text_height_mm)
                && self.width_mm >= self.text_height_mm
                && self.width_mm <= 500.,
            "note width must fit a character, up to 500 mm; text height must be 1-20 mm",
        )?;
        ensure(
            std::iter::once(self.position)
                .chain(self.leader)
                .all(|p| p.is_finite() && p.x.abs() <= 1e6 && p.y.abs() <= 1e6),
            "invalid note coordinates",
        )?;
        ensure(
            self.leader.is_none_or(|p| p.distance(self.position) > 1e-6),
            "leader tip must differ from note anchor",
        )?;
        ensure(
            self.rows().len() <= 128,
            "note exceeds 128 wrapped rows; increase width or shorten text",
        )
    }
    /// One em per printable ASCII character. Hard wraps retain every space/character.
    pub fn rows(&self) -> Vec<String> {
        let columns = (self.width_mm / self.text_height_mm)
            .floor()
            .clamp(1., 2048.) as usize;
        self.text
            .split('\n')
            .flat_map(|line| {
                if line.is_empty() {
                    vec![String::new()]
                } else {
                    line.as_bytes()
                        .chunks(columns)
                        .map(|s| String::from_utf8_lossy(s).into_owned())
                        .collect()
                }
            })
            .collect()
    }
    pub fn validate(&self, model: &Model) -> Result<()> {
        self.validate_content()?;
        ensure(
            model.views.get(&self.view).is_some_and(|v| {
                (v.parameters.kind == ViewKind::Drafting && v.parameters.validate().is_ok())
                    || ((v.parameters.kind == ViewKind::Plan && v.parameters.plan.is_some())
                        || (v.parameters.kind == ViewKind::Section
                            && v.parameters.section.is_some()))
                        && v.parameters
                            .level
                            .is_some_and(|id| model.levels.contains_key(&id))
            }),
            "text note requires a configured Plan or Section",
        )
    }
}
pub(crate) fn validate(model: &Model) -> Result<()> {
    ensure(
        model.text_notes.len() <= 1000
            && model
                .text_notes
                .values()
                .map(|n| n.parameters.text.len())
                .sum::<usize>()
                <= 100_000,
        "note count/text budget exceeded",
    )?;
    for note in model.text_notes.values() {
        note.parameters.validate(model)?;
    }
    Ok(())
}
