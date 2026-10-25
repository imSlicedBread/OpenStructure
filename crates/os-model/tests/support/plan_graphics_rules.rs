#![allow(dead_code)]
use os_core::{Id, Point2};
use os_model::*;

pub fn stroke(color: [u8; 3]) -> PlanStrokeStyle {
    PlanStrokeStyle {
        color,
        weight_mm: 0.71,
        pattern: PlanLinePattern::Dashed,
    }
}

pub fn rule(category: PlanRuleCategory) -> PlanGraphicsRule {
    PlanGraphicsRule {
        id: Id::new(),
        name: "Live graphics".into(),
        enabled: true,
        category,
        predicates: vec![PlanGraphicsPredicate::Text {
            field: PlanRuleTextField::InstanceName,
            op: PlanRuleTextOp::Contains,
            value: "".into(),
        }],
        cut: None,
        projected: Some(stroke([210, 15, 75])),
    }
}

pub fn fixture() -> (Model, Id, [Id; 4]) {
    let mut model = Model::new("Rule acceptance");
    let level = *model.levels.keys().next().unwrap();
    let view = View::new("core.view", ViewParams::floor_plan("Rule plan", level));
    let view_id = view.id();
    model.views.insert(view_id, view);
    let wall = Wall::new(
        "org.openstructure.walls.wall",
        WallParams {
            name: "Exterior".into(),
            path: WallPath::Straight {
                start: Point2::new(0., 0.),
                end: Point2::new(10., 0.),
            },
            thickness: 0.1,
            height: 3.,
            level,
            material: None,
        },
    );
    let wall_id = wall.id();
    model.walls.insert(wall_id, wall);
    let ty = WallType::new(
        "core.wall_type",
        WallTypeParams {
            name: "Compound".into(),
            layers: vec![WallLayer {
                id: Id::new(),
                name: "Core".into(),
                thickness: 0.4,
                function: LayerFunction::Structure,
                material: None,
            }],
        },
    );
    model.wall_type_assignments.insert(
        wall_id,
        WallTypeAssignment {
            type_id: ty.id(),
            flipped: false,
        },
    );
    model.wall_types.insert(ty.id(), ty);
    let mut openings = Vec::new();
    for (kind, offset) in [(OpeningKind::Door, 1.), (OpeningKind::Window, 5.)] {
        let ty = OpeningType::new(
            "core.opening_type",
            OpeningTypeParams {
                name: format!("Standard {kind:?}"),
                kind,
                width: 0.8,
                height: 1.8,
                sill: if kind == OpeningKind::Door { 0. } else { 0.8 },
                window_operation: Default::default(),
                family: Default::default(),
                pane_position: Default::default(),
            },
        );
        let type_id = ty.id();
        model.opening_types.insert(type_id, ty);
        let param = LengthParameter::new(
            "core.length_parameter",
            LengthParameterParams {
                name: format!("{kind:?} width"),
                unit: LengthUnit::Metres,
                value: 0.8,
                formula: Some("1m + 0.2m".into()),
            },
        );
        model.opening_type_length_bindings.insert(
            type_id,
            OpeningTypeLengthBindings {
                width: Some(param.id()),
                height: None,
                sill: None,
            },
        );
        model.length_parameters.insert(param.id(), param);
        let opening = Opening::new(
            "core.opening",
            OpeningParams {
                name: format!("Placed {kind:?}"),
                host: wall_id,
                offset,
                definition: OpeningDefinition::Typed { type_id },
                width_override: None,
                height_override: None,
                sill_override: None,
                pane_position_override: None,
                lite_side_override: None,
                open_state: Default::default(),
                hinge: Default::default(),
                swing: Default::default(),
            },
        );
        openings.push(opening.id());
        model.openings.insert(opening.id(), opening);
    }
    let floor = Floor::new(
        "core.floor",
        FloorParams {
            name: "Concrete slab".into(),
            level,
            material: None,
            boundary: vec![
                Point2::new(-1., -2.),
                Point2::new(11., -2.),
                Point2::new(11., 2.),
                Point2::new(-1., 2.),
            ],
            holes: vec![],
            thickness: 0.25,
            top_offset: 0.,
        },
    );
    let floor_id = floor.id();
    model.floors.insert(floor_id, floor);
    model.validate().unwrap();
    (
        model,
        view_id,
        [wall_id, openings[0], openings[1], floor_id],
    )
}
