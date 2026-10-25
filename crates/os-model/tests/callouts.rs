#[path = "support/callouts.rs"]
mod support;
use os_core::Id;
use os_model::*;
use support::*;

#[test]
fn callout_compatibility_requires_distinct_floor_plans_on_same_level() {
    let (model, parent, target, _) = fixture();
    assert!(model.compatible_callout_plans(parent, target));
    assert!(!model.compatible_callout_plans(parent, parent));
    assert!(!model.compatible_callout_plans(parent, Id::new()));
    let mut other = model.clone();
    let mut level = model.levels.values().next().unwrap().parameters.clone();
    level.elevation = 3.;
    let level = Level::new("core.level", level);
    other.views.get_mut(&target).unwrap().parameters.level = Some(level.id());
    other.levels.insert(level.id(), level);
    assert!(!other.compatible_callout_plans(parent, target));
    assert!(other.validate().is_err());
    for kind in ["rcp", "perspective", "unconfigured"] {
        let mut other = model.clone();
        let p = &mut other.views.get_mut(&target).unwrap().parameters;
        match kind {
            "rcp" => *p = ViewParams::reflected_ceiling_plan("RCP", p.level.unwrap()),
            "perspective" => {
                p.kind = ViewKind::Perspective;
                p.plan = None;
                p.level = None;
            }
            _ => p.plan = None,
        }
        assert!(!other.compatible_callout_plans(parent, target), "{kind}");
        assert!(other.validate().is_err(), "{kind}");
    }
}

#[test]
fn callout_ownership_and_explicit_destination_require_repair() {
    let (model, parent, target, marker) = fixture();
    let mut duplicate = model.clone();
    let second = PlanCallout::new(
        "core.plan_callout",
        model.plan_callouts[&marker].parameters.clone(),
    );
    let second_id = second.id();
    duplicate.plan_callouts.insert(second_id, second);
    assert!(duplicate.validate().is_err(), "only one owning marker");
    duplicate
        .plan_callouts
        .get_mut(&second_id)
        .unwrap()
        .parameters
        .owns_target = false;
    duplicate.validate().unwrap();
    let reverse = PlanCallout::new(
        "core.plan_callout",
        PlanCalloutParams {
            parent: target,
            target: parent,
            owns_target: true,
            boundary: crop(),
            destination: None,
        },
    );
    duplicate.plan_callouts.insert(reverse.id(), reverse);
    assert!(duplicate.validate().is_err(), "ownership cycle");
    for missing in [parent, target] {
        let mut broken = model.clone();
        broken.views.remove(&missing);
        assert!(broken.validate().is_err());
        broken.plan_callouts.remove(&marker);
        broken.validate().unwrap();
    }
    let mut chosen = model;
    let (s, vp) = sheet(&mut chosen, target, "A201", "7");
    chosen
        .plan_callouts
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
        .plan_callouts
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
        model.callout_reference(&model.plan_callouts[&marker].parameters),
        CalloutReference::Unplaced
    );
    assert!(model.validate_callout_issue_references(&[source]).is_err());
    let (dest, vp) = sheet(&mut model, target, "A201", "7");
    assert_eq!(
        model.callout_reference(&model.plan_callouts[&marker].parameters),
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
        model.callout_reference(&model.plan_callouts[&marker].parameters),
        CalloutReference::Ambiguous
    );
    assert!(
        model
            .validate_callout_issue_references(&[source, dest, other])
            .is_err()
    );
    model
        .plan_callouts
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
            .callout_reference(&model.plan_callouts[&marker].parameters)
            .label(),
        "9 / A202"
    );
}
