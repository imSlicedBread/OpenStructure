#[path = "support/drafting.rs"]
mod support;
use os_core::Point2;
use os_model::*;
#[test]
fn drafting_owners_coordinates_text_limits_and_strict_wire() {
    for section in [false, true] {
        let (mut m, view) = support::fixture(section);
        let line = DetailLine::new(
            "core.detail_line",
            DetailLineParams {
                view,
                start: Point2::new(1., 2.),
                end: Point2::new(3., 4.),
            },
        );
        m.detail_lines.insert(line.id(), line);
        let mut p = TextNoteParams::new(view);
        p.text = "First line\nSecond line".into();
        p.leader = Some(Point2::new(-1., -1.));
        p.validate(&m).unwrap();
        for text in ["", "bad\ttext", "中文", "bad\rtext"] {
            let mut bad = p.clone();
            bad.text = text.into();
            assert!(bad.validate(&m).is_err());
        }
        let mut bad = p.clone();
        bad.width_mm = 1.;
        bad.text_height_mm = 3.;
        assert!(bad.validate(&m).is_err());
        bad = p.clone();
        bad.text = "x\n".repeat(129);
        assert!(bad.validate(&m).is_err());
        let mut wire = serde_json::to_value(&p).unwrap();
        wire["position"]["z"] = 1.into();
        assert!(serde_json::from_value::<TextNoteParams>(wire).is_err());
        let note = TextNote::new("core.text_note", p);
        m.text_notes.insert(note.id(), note);
        m.validate().unwrap();
        m.views.remove(&view);
        assert!(m.validate().is_err());
    }
}
#[test]
fn note_wrapping_preserves_every_character_and_blank_row() {
    let (_, view) = support::fixture(false);
    let mut p = TextNoteParams::new(view);
    p.width_mm = 10.;
    p.text_height_mm = 2.;
    p.text = "AB CD E\n\nF".into();
    assert_eq!(p.rows(), ["AB CD", " E", "", "F"]);
}
