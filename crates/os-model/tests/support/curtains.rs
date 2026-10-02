use os_core::{Id, Point2};
use os_model::*;

pub fn fixture() -> (Model, CurtainSystem) {
    let mut model = Model::new("Curtain acceptance");
    let level = *model.levels.keys().next().unwrap();
    let panel = CurtainPanelType::new(
        "core.curtain_panel_type",
        CurtainPanelTypeParams {
            name: "Glass".into(),
            kind: CurtainPanelKind::Glazing,
            thickness: 0.02,
            material: None,
        },
    );
    let mullion = CurtainMullionType::new(
        "core.curtain_mullion_type",
        CurtainMullionTypeParams {
            name: "Frame".into(),
            width: 0.1,
            depth: 0.2,
            material: None,
        },
    );
    let mut p = CurtainSystemParams {
        name: "C1".into(),
        level,
        start: Point2::new(0., 0.),
        end: Point2::new(4., 0.),
        base_offset: 0.3,
        height: 3.,
        normal_flip: false,
        panel_type: panel.id(),
        mullion_type: mullion.id(),
        vertical: [0., 2., 4.]
            .map(|position| CurtainGrid {
                id: Id::new(),
                position,
            })
            .to_vec(),
        horizontal: [0., 1.5, 3.]
            .map(|position| CurtainGrid {
                id: Id::new(),
                position,
            })
            .to_vec(),
        panels: vec![],
        mullions: vec![],
    };
    p.reconcile().unwrap();
    model.curtain_panel_types.insert(panel.id(), panel);
    model.curtain_mullion_types.insert(mullion.id(), mullion);
    (model, CurtainSystem::new("core.curtain_system", p))
}
