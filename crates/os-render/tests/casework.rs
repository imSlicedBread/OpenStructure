use os_core::{Id, Point2};
use os_geometry::{
    SurfaceIdentity,
    casework::casework_solid,
    plan::{HorizontalBasis, PlanCrop, PlanFootprint, PlanRange, PlanRole},
};
use os_model::{CaseworkParams, CaseworkTypeParams};
use os_render::plan::*;
use std::collections::BTreeMap;

fn context() -> PlanContext {
    PlanContext {
        session_id: Id::new(),
        model_revision: 0,
        view_id: Id::new(),
        settings_revision: 0,
        basis: HorizontalBasis::default(),
        range: PlanRange::default(),
        crop: None,
        scale_denominator: 100.0,
        show_walls: true,
        show_extensions: true,
    }
}

#[test]
fn casework_uses_shared_crop_rotation_cut_projection_and_picking() {
    let id = Id::new();
    let parameters = CaseworkParams {
        name: "Island".into(),
        type_id: Id::new(),
        level: Id::new(),
        center: Point2::new(2.0, 3.0),
        yaw: std::f64::consts::FRAC_PI_4,
        base_offset: 0.15,
    };
    let cabinet_type = CaseworkTypeParams {
        name: "Cabinet".into(),
        width: 1.2,
        depth: 0.6,
        height: 0.9,
        material: Some(Id::new()),
    };
    let solid = casework_solid(&parameters, &cabinet_type, 0.0).unwrap();
    let surface = SurfaceIdentity {
        layer: None,
        material: cabinet_type.material,
    };
    let context = PlanContext {
        range: PlanRange {
            top: 4.0,
            cut: 0.6,
            bottom: -1.0,
            depth: -1.0,
        },
        ..context()
    };
    let drawing = PlanDrawing::from_prisms(context, &BTreeMap::new(), vec![])
        .unwrap()
        .with_casework(&[(id, solid.clone(), surface)])
        .unwrap();
    let item = &drawing.casework(context).unwrap()[0];
    assert_eq!(item.entity, id);
    assert_eq!(item.surface, surface);
    assert_eq!(item.footprint.role, PlanRole::Cut);
    assert_eq!(
        drawing
            .pick_casework(context, Point2::new(2.0, 3.0))
            .unwrap(),
        Some(id)
    );
    assert_eq!(
        drawing
            .pick_casework(context, Point2::new(4.0, 3.0))
            .unwrap(),
        None
    );

    let projected_context = PlanContext {
        range: PlanRange {
            top: 2.5,
            cut: 1.2,
            bottom: 0.0,
            depth: -1.0,
        },
        ..context
    };
    let mut projected = solid;
    projected.height = 0.8;
    projected.transform.translation.z = 0.0;
    let drawing = PlanDrawing::from_prisms(projected_context, &BTreeMap::new(), vec![])
        .unwrap()
        .with_casework(&[(id, projected, surface)])
        .unwrap();
    assert_eq!(
        drawing.casework(projected_context).unwrap()[0]
            .footprint
            .role,
        PlanRole::Projected
    );

    let clipped_context = PlanContext {
        crop: Some(PlanCrop {
            min: Point2::new(1.5, 2.5),
            max: Point2::new(2.0, 3.5),
        }),
        ..context
    };
    let drawing = PlanDrawing::from_prisms(clipped_context, &BTreeMap::new(), vec![])
        .unwrap()
        .with_casework(&[(
            id,
            casework_solid(&parameters, &cabinet_type, 0.0).unwrap(),
            surface,
        )])
        .unwrap();
    let footprint: &PlanFootprint = &drawing.casework(clipped_context).unwrap()[0].footprint;
    assert!(footprint.vertices().len() >= 3);
    assert!(footprint.vertices().iter().all(|point| {
        point.x >= 1.5 - 1e-9
            && point.x <= 2.0 + 1e-9
            && point.y >= 2.5 - 1e-9
            && point.y <= 3.5 + 1e-9
    }));
}

#[test]
fn casework_above_cut_is_hidden_and_duplicate_ids_are_rejected() {
    let id = Id::new();
    let parameters = CaseworkParams {
        name: "Wall cabinet".into(),
        type_id: Id::new(),
        level: Id::new(),
        center: Point2::new(0.0, 0.0),
        yaw: 0.0,
        base_offset: 4.0,
    };
    let cabinet_type = CaseworkTypeParams {
        name: "Wall cabinet type".into(),
        width: 0.6,
        depth: 0.4,
        height: 0.7,
        material: None,
    };
    let solid = casework_solid(&parameters, &cabinet_type, 0.0).unwrap();
    let surface = SurfaceIdentity::default();
    let context = context();
    let drawing = PlanDrawing::from_prisms(context, &BTreeMap::new(), vec![])
        .unwrap()
        .with_casework(&[(id, solid.clone(), surface)])
        .unwrap();
    assert!(drawing.casework(context).unwrap().is_empty());
    assert!(
        PlanDrawing::from_prisms(context, &BTreeMap::new(), vec![])
            .unwrap()
            .with_casework(&[(id, solid.clone(), surface), (id, solid, surface)])
            .is_err()
    );
}
