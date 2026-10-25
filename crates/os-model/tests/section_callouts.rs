#[path = "support/section_callouts.rs"]
mod support;
use os_core::Id;
use os_model::*;
use support::*;

#[test]
fn section_callout_requires_same_authored_plane_not_same_crop() {
    let (model, parent, target, _) = fixture();
    assert!(model.compatible_callout_sections(parent, target));
    assert!(!model.compatible_callout_sections(parent, parent));
    assert!(!model.compatible_callout_sections(parent, Id::new()));
    let mut other = model.clone();
    let mut level = model.levels.values().next().unwrap().parameters.clone();
    level.elevation = 3.;
    let level = Level::new("core.level", level);
    other.views.get_mut(&target).unwrap().parameters.level = Some(level.id());
    other.levels.insert(level.id(), level);
    assert!(!other.compatible_callout_sections(parent, target));
    assert!(other.validate().is_err());
    for field in ["start", "reverse", "side", "mode", "depth", "plan"] {
        let mut other = model.clone();
        let p = &mut other.views.get_mut(&target).unwrap().parameters;
        let s = p.section.as_mut().unwrap();
        match field {
            "start" => s.start.x += 1.,
            "reverse" => std::mem::swap(&mut s.start, &mut s.end),
            "side" => s.side = ElevationSide::Right,
            "mode" => s.mode = SectionDisplayMode::CutAndProjection,
            "depth" => s.far_depth += 1.,
            _ => *p = ViewParams::floor_plan("Plan", p.level.unwrap()),
        }
        assert!(
            !other.compatible_callout_sections(parent, target),
            "{field}"
        );
        assert_eq!(other.validate().is_err(), field == "plan");
    }
    let mut independent = model;
    let s = independent
        .views
        .get_mut(&target)
        .unwrap()
        .parameters
        .section
        .as_mut()
        .unwrap();
    s.min_station = -20.;
    s.max_station = 50.;
    s.bottom_elevation = -10.;
    s.top_elevation = 25.;
    s.scale_denominator = 20.;
    independent.validate().unwrap();
}

#[test]
fn callout_ownership_and_explicit_destination_require_repair() {
    let (model, parent, target, marker) = fixture();
    let mut duplicate = model.clone();
    let second = SectionCallout::new(
        "core.section_callout",
        model.section_callouts[&marker].parameters.clone(),
    );
    let second_id = second.id();
    duplicate.section_callouts.insert(second_id, second);
    assert!(duplicate.validate().is_err(), "only one owning marker");
    duplicate
        .section_callouts
        .get_mut(&second_id)
        .unwrap()
        .parameters
        .owns_target = false;
    duplicate.validate().unwrap();
    let reverse = SectionCallout::new(
        "core.section_callout",
        SectionCalloutParams {
            parent: target,
            target: parent,
            owns_target: true,
            boundary: crop(),
            destination: None,
        },
    );
    duplicate.section_callouts.insert(reverse.id(), reverse);
    assert!(duplicate.validate().is_err(), "ownership cycle");
    for missing in [parent, target] {
        let mut broken = model.clone();
        broken.views.remove(&missing);
        assert!(broken.validate().is_err());
        broken.section_callouts.remove(&marker);
        broken.validate().unwrap();
    }
    let mut chosen = model;
    let (s, vp) = sheet(&mut chosen, target, "A201", "7");
    chosen
        .section_callouts
        .get_mut(&marker)
        .unwrap()
        .parameters
        .destination = Some(vp);
    chosen.validate().unwrap();
    chosen.sheets.remove(&s);
    assert!(
        chosen.validate().is_err(),
        "chosen placement cannot silently disappear"
    );
    chosen
        .section_callouts
        .get_mut(&marker)
        .unwrap()
        .parameters
        .destination = None;
    chosen.validate().unwrap();
}

#[test]
fn callout_references_resolve_unplaced_unique_ambiguous_and_issue_membership() {
    let (mut model, parent, target, marker) = fixture();
    let (source, _) = sheet(&mut model, parent, "A101", "1");
    assert_eq!(
        model.section_callout_reference(&model.section_callouts[&marker].parameters),
        CalloutReference::Unplaced
    );
    assert!(model.validate_callout_issue_references(&[source]).is_err());
    let (dest, vp) = sheet(&mut model, target, "A201", "7");
    assert_eq!(
        model.section_callout_reference(&model.section_callouts[&marker].parameters),
        CalloutReference::Placed {
            sheet: dest,
            viewport: vp,
            label: "7 / A201".into(),
        }
    );
    let error = model
        .validate_callout_issue_references(&[source])
        .unwrap_err()
        .to_string();
    assert!(error.contains("omitted"), "{error}");
    model
        .validate_callout_issue_references(&[source, dest])
        .unwrap();
    let (other, _) = sheet(&mut model, target, "A301", "8");
    assert_eq!(
        model.section_callout_reference(&model.section_callouts[&marker].parameters),
        CalloutReference::Ambiguous
    );
    assert!(
        model
            .validate_callout_issue_references(&[source, dest, other])
            .is_err()
    );
    model
        .section_callouts
        .get_mut(&marker)
        .unwrap()
        .parameters
        .destination = Some(vp);
    model
        .validate_callout_issue_references(&[source, dest])
        .unwrap();
    assert!(
        model
            .validate_callout_issue_references(&[source, other])
            .is_err()
    );
    let p = &mut model.sheets.get_mut(&dest).unwrap().parameters;
    p.number = "A202".into();
    p.viewports[0].detail_number = "9".into();
    assert_eq!(
        model
            .section_callout_reference(&model.section_callouts[&marker].parameters)
            .label(),
        "9 / A202"
    );
}

#[test]
fn section_callout_nested_bounds_reject_unknown_coordinates_and_invalid_extents() {
    for text in [
        r#"{"min":{"x":0,"y":0,"z":2},"max":{"x":1,"y":1}}"#,
        r#"{"min":{"x":0,"y":0},"max":{"x":1,"y":1,"z":2}}"#,
    ] {
        assert!(serde_json::from_str::<SectionCalloutBounds>(text).is_err());
    }
    for (x, y) in [(0., 1.), (1., 0.), (f64::NAN, 1.), (1e7, 1.)] {
        assert!(
            SectionCalloutBounds {
                min: os_core::Point2::default(),
                max: os_core::Point2::new(x, y)
            }
            .validate()
            .is_err()
        );
    }
}
