#[path = "support/plan_graphics_rules.rs"]
mod support;
use os_core::Id;
use os_model::*;
use support::*;

fn number(field: PlanRuleNumberField, op: PlanRuleNumberOp, value: f64) -> PlanGraphicsPredicate {
    PlanGraphicsPredicate::Number { field, op, value }
}

#[test]
fn rules_resolve_compounds_formulas_pins_and_live_properties() {
    let (mut model, view, [wall, door, window, slab]) = fixture();
    let categories = [
        PlanRuleCategory::Walls,
        PlanRuleCategory::Doors,
        PlanRuleCategory::Windows,
        PlanRuleCategory::Slabs,
    ];
    let fields = [
        PlanRuleNumberField::WallThickness,
        PlanRuleNumberField::OpeningWidth,
        PlanRuleNumberField::OpeningWidth,
        PlanRuleNumberField::SlabThickness,
    ];
    let values = [0.4, 1.2, 1.2, 0.25];
    let rules: Vec<_> = categories
        .into_iter()
        .zip(fields)
        .zip(values)
        .map(|((category, field), value)| {
            let mut rule = rule(category);
            rule.predicates = vec![number(field, PlanRuleNumberOp::Equals, value)];
            rule
        })
        .collect();
    model.plan_graphics_rules.insert(view, rules.clone());
    model.validate().unwrap();
    assert_eq!(model.resolve_plan_graphics_rules(view).unwrap().len(), 4);
    model
        .openings
        .get_mut(&door)
        .unwrap()
        .parameters
        .width_override = Some(0.9);
    assert!(
        !model
            .resolve_plan_graphics_rules(view)
            .unwrap()
            .contains_key(&door)
    );
    model
        .openings
        .get_mut(&door)
        .unwrap()
        .parameters
        .width_override = None;
    let parameter = model.opening_type_length_bindings[&model
        .resolve_opening(&model.openings[&window].parameters)
        .unwrap()
        .type_id
        .unwrap()]
        .width
        .unwrap();
    model
        .length_parameters
        .get_mut(&parameter)
        .unwrap()
        .parameters
        .formula = Some("1m + 0.5m".into());
    assert!(
        !model
            .resolve_plan_graphics_rules(view)
            .unwrap()
            .contains_key(&window)
    );
    let ty = model.wall_type_assignments[&wall].type_id;
    model.wall_types.get_mut(&ty).unwrap().parameters.layers[0].thickness = 0.5;
    model.floors.get_mut(&slab).unwrap().parameters.thickness = 0.3;
    assert_eq!(
        model
            .resolve_plan_graphics_rules(view)
            .unwrap()
            .keys()
            .copied()
            .collect::<Vec<_>>(),
        vec![door]
    );
    assert_eq!(
        model.plan_graphics_rules[&view], rules,
        "evaluating never rewrites definitions"
    );
    let mut added = model.openings[&door].clone();
    added.header.id = Id::new();
    added.parameters.offset = 7.;
    let added_id = added.id();
    model.openings.insert(added_id, added);
    assert!(
        model
            .resolve_plan_graphics_rules(view)
            .unwrap()
            .contains_key(&added_id)
    );
}

#[test]
fn rules_order_role_inheritance_disabling_text_phase_and_exact_comparisons() {
    let (mut model, view, [wall, ..]) = fixture();
    let mut high = rule(PlanRuleCategory::Walls);
    high.cut = Some(stroke([1, 2, 3]));
    high.projected = None;
    high.predicates = vec![PlanGraphicsPredicate::Text {
        field: PlanRuleTextField::TypeName,
        op: PlanRuleTextOp::Equals,
        value: "Compound".into(),
    }];
    let mut low = rule(PlanRuleCategory::Walls);
    low.cut = Some(stroke([4, 5, 6]));
    model
        .plan_graphics_rules
        .insert(view, vec![high.clone(), low.clone()]);
    let value = model.resolve_plan_graphics_rules(view).unwrap()[&wall];
    assert_eq!(value.cut, high.cut);
    assert_eq!(value.projected, low.projected);
    model.plan_graphics_rules.get_mut(&view).unwrap().swap(0, 1);
    assert_eq!(
        model.resolve_plan_graphics_rules(view).unwrap()[&wall].cut,
        low.cut
    );
    model.plan_graphics_rules.get_mut(&view).unwrap()[0].enabled = false;
    assert_eq!(
        model.resolve_plan_graphics_rules(view).unwrap()[&wall].projected,
        None
    );
    model.wall_type_assignments.remove(&wall);
    high.predicates = vec![PlanGraphicsPredicate::Text {
        field: PlanRuleTextField::TypeName,
        op: PlanRuleTextOp::NotEquals,
        value: "Unknown".into(),
    }];
    model.plan_graphics_rules.insert(view, vec![high.clone()]);
    assert!(model.resolve_plan_graphics_rules(view).unwrap().is_empty());
    for (op, operand, matches) in [
        (PlanRuleNumberOp::Equals, 10., true),
        (PlanRuleNumberOp::NotEquals, 10., false),
        (PlanRuleNumberOp::Less, 10., false),
        (PlanRuleNumberOp::LessOrEqual, 10., true),
        (PlanRuleNumberOp::Greater, 10., false),
        (PlanRuleNumberOp::GreaterOrEqual, 10., true),
        (PlanRuleNumberOp::Equals, 10.0000000001, false),
    ] {
        high.predicates = vec![number(PlanRuleNumberField::WallLength, op, operand)];
        model.plan_graphics_rules.insert(view, vec![high.clone()]);
        assert_eq!(
            model
                .resolve_plan_graphics_rules(view)
                .unwrap()
                .contains_key(&wall),
            matches
        );
    }
    high.predicates = vec![
        PlanGraphicsPredicate::Phase {
            op: PlanRulePhaseOp::Equals,
            value: PhaseStatus::Existing,
        },
        PlanGraphicsPredicate::Text {
            field: PlanRuleTextField::InstanceName,
            op: PlanRuleTextOp::Contains,
            value: "Ext".into(),
        },
    ];
    model.plan_graphics_rules.insert(view, vec![high.clone()]);
    assert!(
        model
            .resolve_plan_graphics_rules(view)
            .unwrap()
            .contains_key(&wall)
    );
    model.walls.get_mut(&wall).unwrap().parameters.name = "exterior".into();
    assert!(model.resolve_plan_graphics_rules(view).unwrap().is_empty());
    model.walls.get_mut(&wall).unwrap().parameters.name = "Exterior".into();
    let creation = model.latest_phase().unwrap();
    model.element_lifecycles.insert(
        wall,
        ElementLifecycle {
            created_in: creation,
            demolished_in: None,
        },
    );
    assert!(
        model.resolve_plan_graphics_rules(view).unwrap().is_empty(),
        "explicit creation in a nonzero target phase is New"
    );
    let later = new_phase("Later", 2);
    model.phases.insert(later.id(), later);
    assert!(
        model
            .resolve_plan_graphics_rules(view)
            .unwrap()
            .contains_key(&wall),
        "New becomes Existing in a later target phase"
    );
    model
        .views
        .get_mut(&view)
        .unwrap()
        .parameters
        .plan
        .as_mut()
        .unwrap()
        .target_phase = Some(creation);
    assert!(model.resolve_plan_graphics_rules(view).unwrap().is_empty());
}

#[test]
fn rules_validate_disabled_rows_category_values_ids_and_bounds() {
    let (mut model, view, _) = fixture();
    let base = rule(PlanRuleCategory::Walls);
    for change in 0..10 {
        let mut bad = base.clone();
        bad.enabled = false;
        match change {
            0 => bad.name = "é".repeat(129),
            1 => bad.name = "bad\nname".into(),
            2 => bad.predicates.clear(),
            3 => bad.predicates = vec![bad.predicates[0].clone(); 9],
            4 => {
                bad.cut = None;
                bad.projected = None;
            }
            5 => {
                bad.predicates = vec![number(
                    PlanRuleNumberField::OpeningWidth,
                    PlanRuleNumberOp::Equals,
                    1.,
                )]
            }
            6 => {
                bad.predicates = vec![number(
                    PlanRuleNumberField::WallHeight,
                    PlanRuleNumberOp::Equals,
                    f64::NAN,
                )]
            }
            7 => {
                bad.category = PlanRuleCategory::Slabs;
                bad.cut = Some(stroke([0, 0, 0]));
            }
            8 => bad.projected.as_mut().unwrap().weight_mm = 2.01,
            _ => bad.id = serde_json::from_str("\"00000000-0000-0000-0000-000000000000\"").unwrap(),
        }
        assert!(
            model
                .validate_plan_graphics_rules_replacement(view, &[bad])
                .is_err(),
            "case {change}"
        );
    }
    assert!(
        model
            .validate_plan_graphics_rules_replacement(view, &[base.clone(), base.clone()])
            .is_err()
    );
    let rules: Vec<_> = (0..32).map(|_| rule(PlanRuleCategory::Walls)).collect();
    model
        .validate_plan_graphics_rules_replacement(view, &rules)
        .unwrap();
    model.plan_graphics_rules.insert(view, rules);
    for _ in 0..7 {
        let mut copy = model.views[&view].clone();
        copy.header.id = Id::new();
        let id = copy.id();
        model.views.insert(id, copy);
        model
            .plan_graphics_rules
            .insert(id, model.copied_plan_graphics_rules(view));
    }
    model.validate().unwrap();
    let mut copy = model.views[&view].clone();
    copy.header.id = Id::new();
    let id = copy.id();
    model.views.insert(id, copy);
    assert!(
        model
            .validate_plan_graphics_rules_replacement(id, &[base])
            .is_err()
    );
}
