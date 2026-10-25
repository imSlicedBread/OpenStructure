use os_model::*;
use std::collections::BTreeMap;
fn parameter(unit: LengthUnit, value: f64, formula: Option<String>) -> LengthParameter {
    LengthParameter::new(
        "core.length_parameter",
        LengthParameterParams {
            name: "Driver".into(),
            unit,
            value,
            formula,
        },
    )
}
#[test]
fn typed_formulas_arithmetic_dimensions_dependencies_and_rename() {
    let a = parameter(LengthUnit::Metres, 0.02, None);
    let b = parameter(LengthUnit::Scalar, 3., None);
    let c = parameter(
        LengthUnit::Metres,
        0.,
        Some(format!("(@{{{}}} * @{{{}}}) + 0.01m", a.id(), b.id())),
    );
    let ids = [a.id(), b.id(), c.id()];
    let mut ps: BTreeMap<_, _> = [a, b, c].into_iter().map(|p| (p.id(), p)).collect();
    assert!((evaluate_parameters(&ps).unwrap()[&ids[2]].value - 0.07).abs() < 1e-12);
    ps.get_mut(&ids[0]).unwrap().parameters.name = "Renamed".into();
    assert_eq!(
        parameter_dependencies(&ps, [ids[2]].into()).unwrap().len(),
        3
    );
    assert!((evaluate_parameters(&ps).unwrap()[&ids[2]].value - 0.07).abs() < 1e-12);
    for formula in [
        "1m + 2",
        "1m * 2m",
        "2 / 1m",
        "1m / 0",
        "1000000m * 2",
        "unknown(1)",
        "NaN",
        "1e99m",
    ] {
        ps.get_mut(&ids[2]).unwrap().parameters.formula = Some(formula.into());
        assert!(evaluate_parameters(&ps).is_err(), "{formula}");
    }
    ps.get_mut(&ids[2]).unwrap().parameters.formula = Some("1m / 2m".into());
    ps.get_mut(&ids[2]).unwrap().parameters.unit = LengthUnit::Scalar;
    assert_eq!(evaluate_parameters(&ps).unwrap()[&ids[2]].value, 0.5);
}
#[test]
fn typed_formulas_cycles_missing_references_and_hard_budgets() {
    let a = parameter(LengthUnit::Metres, 1., None);
    let id = a.id();
    let mut ps = BTreeMap::from([(id, a)]);
    for formula in [
        format!("@{{{id}}}"),
        format!("@{{{}}}", os_core::Id::new()),
        format!("{}1m{}", "(".repeat(33), ")".repeat(33)),
        "1m+".repeat(129),
        " ".repeat(2049),
    ] {
        ps.get_mut(&id).unwrap().parameters.formula = Some(formula);
        assert!(evaluate_parameters(&ps).is_err());
    }
    ps.get_mut(&id).unwrap().parameters.formula = None;
    let mut previous = id;
    for _ in 0..32 {
        let p = parameter(LengthUnit::Metres, 0., Some(format!("@{{{previous}}}")));
        previous = p.id();
        ps.insert(p.id(), p);
    }
    assert!(evaluate_parameters(&ps).is_err());
    for p in ps.values_mut() {
        p.parameters.formula = Some(
            std::iter::repeat_n("1m", 30)
                .collect::<Vec<_>>()
                .join(" + "),
        );
    }
    // Independent formulas exceed the shared graph-work budget at the 64-definition cap.
    while ps.len() < 64 {
        let p = parameter(
            LengthUnit::Metres,
            0.,
            Some(
                std::iter::repeat_n("1m", 30)
                    .collect::<Vec<_>>()
                    .join(" + "),
            ),
        );
        ps.insert(p.id(), p);
    }
    // 64 * (59 AST nodes + parameter dispatch) is allowed, one extra definition is not.
    assert!(evaluate_parameters(&ps).is_ok());
    fn balanced(n: usize) -> String {
        if n == 1 {
            "1m".into()
        } else {
            format!("({}+{})", balanced(n / 2), balanced(n - n / 2))
        }
    }
    for p in ps.values_mut() {
        p.parameters.formula = Some(balanced(40));
    }
    assert!(evaluate_parameters(&ps).is_err());
    let p = parameter(LengthUnit::Metres, 0., None);
    ps.insert(p.id(), p);
    assert!(evaluate_parameters(&ps).is_err());
}

#[test]
fn typed_formula_order_independent_chains_and_near_budget_remapping() {
    for reverse in [false, true] {
        let ids: Vec<_> = (1..=24)
            .map(|n| {
                os_core::Id(
                    format!("{:032x}", if reverse { 25 - n } else { n })
                        .parse()
                        .unwrap(),
                )
            })
            .collect();
        let mut ps = BTreeMap::new();
        for (i, id) in ids.iter().enumerate() {
            let mut p = parameter(
                LengthUnit::Metres,
                1.,
                (i > 0).then(|| format!("@{{{}}} + 0.01m", ids[i - 1])),
            );
            p.header.id = *id;
            ps.insert(*id, p);
        }
        assert!((evaluate_parameters(&ps).unwrap()[&ids[23]].value - 1.23).abs() < 1e-12);
    }
    let formula = std::iter::repeat_n("0.001m", 30)
        .collect::<Vec<_>>()
        .join(" + ");
    let ast = ParameterExpression::parse(&formula).unwrap();
    let remapped = ast.remapped(&BTreeMap::new()).unwrap();
    assert_eq!(ParameterExpression::parse(&remapped).unwrap(), ast);
}
