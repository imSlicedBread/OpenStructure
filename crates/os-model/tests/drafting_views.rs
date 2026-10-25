use os_core::Point2;
use os_model::*;
#[test]
fn drafting_view_has_only_valid_annotation_settings() {
    let mut model = Model::new("Drafting");
    let view = View::new("core.view", ViewParams::drafting("Detail"));
    let id = view.id();
    model.views.insert(id, view);
    model.validate().unwrap();
    let good = model.views[&id].parameters.clone();
    for change in 0..7 {
        let mut bad = good.clone();
        match change {
            0 => bad.level = Some(*model.levels.keys().next().unwrap()),
            1 => bad.plan = Some(Default::default()),
            2 => bad.drafting = None,
            3 => bad.drafting.as_mut().unwrap().crop.max.x = -5.,
            4 => bad.drafting.as_mut().unwrap().scale_denominator = f64::INFINITY,
            5 => bad.name = "".into(),
            _ => bad.kind = ViewKind::Perspective,
        }
        assert!(bad.validate_edit().is_err());
    }
    let line = DetailLine::new(
        "core.detail_line",
        DetailLineParams {
            view: id,
            start: Point2::default(),
            end: Point2::new(1., 0.),
        },
    );
    model.detail_lines.insert(line.id(), line);
    let note = TextNote::new("core.text_note", TextNoteParams::new(id));
    model.text_notes.insert(note.id(), note);
    model.validate().unwrap();
    let mut wire = serde_json::to_value(&good).unwrap();
    wire["drafting"]["crop"]["min"]["z"] = 0.into();
    assert!(serde_json::from_value::<ViewParams>(wire).is_err());
}
