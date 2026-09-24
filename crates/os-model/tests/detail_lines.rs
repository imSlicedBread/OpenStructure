use os_core::{Id, Point2};
use os_model::{DetailLine, DetailLineParams, Model, View, ViewParams};

fn fixture() -> (Model, DetailLine) {
    let mut model = Model::new("Details");
    let level = *model.levels.keys().next().unwrap();
    let view = View::new("core.view", ViewParams::floor_plan("Ground", level));
    let line = DetailLine::new(
        "core.detail_line",
        DetailLineParams {
            view: view.id(),
            start: Point2::new(0.0, 0.0),
            end: Point2::new(2.0, 0.0),
        },
    );
    model.views.insert(view.id(), view);
    (model, line)
}

#[test]
fn detail_line_requires_configured_plan_and_bounded_distinct_points() {
    let (mut model, line) = fixture();
    line.parameters.validate(&model).unwrap();
    for view in [
        Id::new(),
        *model
            .views
            .keys()
            .find(|id| **id != line.parameters.view)
            .unwrap(),
    ] {
        let mut invalid = line.parameters.clone();
        invalid.view = view;
        assert!(invalid.validate(&model).is_err());
    }
    for (start, end) in [
        (Point2::new(f64::NAN, 0.0), line.parameters.end),
        (Point2::new(1_000_001.0, 0.0), line.parameters.end),
        (line.parameters.start, line.parameters.start),
        (line.parameters.start, Point2::new(1e-10, 0.0)),
    ] {
        let mut invalid = line.parameters.clone();
        invalid.start = start;
        invalid.end = end;
        assert!(invalid.validate(&model).is_err());
    }
    model.detail_lines.insert(line.id(), line.clone());
    model.validate().unwrap();
    model.views.remove(&line.parameters.view);
    assert!(model.validate().is_err());
}

#[test]
fn detail_line_identity_is_global_and_map_key_must_match() {
    let (mut model, line) = fixture();
    model.detail_lines.insert(line.id(), line.clone());
    model.validate().unwrap();
    let mut duplicate = line.clone();
    duplicate.header.id = model.project.id();
    model.detail_lines.insert(duplicate.id(), duplicate);
    assert!(model.validate().is_err());
    model.detail_lines.remove(&model.project.id());
    model.detail_lines.insert(Id::new(), line);
    assert!(model.validate().is_err());
}
