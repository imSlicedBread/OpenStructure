use super::*;
use os_document::Document;
use os_geometry::section::VerticalSectionPlane;
use os_model::{
    Building, BuildingParams, CurtainGrid, Level, LevelParams, Material, MaterialParams,
    SectionViewSettings, ViewKind, ViewParams,
};

#[path = "../../../os-model/tests/support/curtains.rs"]
mod curtain_fixture;

fn context() -> PlanContext {
    PlanContext {
        session_id: Id::new(),
        model_revision: 1,
        view_id: Id::new(),
        settings_revision: 1,
        basis: HorizontalBasis::default(),
        range: PlanRange {
            top: 4.,
            cut: 2.,
            bottom: -1.,
            depth: -1.,
        },
        crop: Some(PlanCrop {
            min: Point2::new(0., -1.),
            max: Point2::new(4., 4.),
        }),
        scale_denominator: 100.,
        show_walls: true,
        show_extensions: false,
    }
}

fn fixture() -> (os_model::Model, os_model::CurtainSystem) {
    let (mut model, mut curtain) = curtain_fixture::fixture();
    let materials = ["Panel finish", "Frame finish"].map(|name| {
        Material::new(
            "core.material",
            MaterialParams {
                name: name.into(),
                density_kg_m3: 1800.,
                color: [90, 100, 110],
            },
        )
    });
    let panel_material = materials[0].id();
    let mullion_material = materials[1].id();
    for material in materials {
        model.materials.insert(material.id(), material);
    }
    model
        .curtain_panel_types
        .get_mut(&curtain.parameters.panel_type)
        .unwrap()
        .parameters
        .material = Some(panel_material);
    model
        .curtain_mullion_types
        .get_mut(&curtain.parameters.mullion_type)
        .unwrap()
        .parameters
        .material = Some(mullion_material);
    curtain.parameters.start = Point2::new(0., 0.);
    curtain.parameters.end = Point2::new(4., 0.);
    curtain.parameters.base_offset = 0.3;
    model
        .levels
        .get_mut(&curtain.parameters.level)
        .unwrap()
        .parameters
        .elevation = 0.2;
    (model, curtain)
}

// Give test snapshots a deterministic context so drawing access validates.
fn derive_with_context(
    plane: VerticalSectionPlane,
    assembly: Id,
    components: Vec<os_geometry::curtain_systems::CurtainComponentMesh>,
    ctx: PlanContext,
) -> PlanDrawing {
    SectionSnapshot {
        roofs: Vec::new(),
        context: ctx,
        interfaces: Vec::new(),
        plane,
        walls: Vec::new(),
        floors: Vec::new(),
        ceilings: Vec::new(),
        stairs: Vec::new(),
        ramps: Vec::new(),
        curtains: vec![(assembly, components)],
    }
    .derive()
    .unwrap()
}

#[test]
fn transverse_cut_keeps_panel_and_mullion_materials_and_parent_identity() {
    let (model, curtain) = fixture();
    let assembly = curtain.id();
    let ctx = context();
    let components = os_geometry::curtain_systems::curtain_geometry(&curtain.parameters, &model)
        .unwrap()
        .components;
    let drawing = derive_with_context(
        VerticalSectionPlane {
            origin: Point2::new(1., -2.),
            direction: Point2::new(0., 1.),
        },
        assembly,
        components,
        ctx,
    );
    let lines = drawing.provider_lines(ctx).unwrap();
    assert_eq!(lines.len(), 20);
    assert!(drawing.is_native_line(assembly));
    assert!(lines.iter().all(|line| line.entity == assembly));
    let materials: std::collections::BTreeSet<_> = lines
        .iter()
        .map(|line| drawing.line_surface(assembly, line.feature).material)
        .collect();
    assert_eq!(
        materials,
        std::collections::BTreeSet::from([
            Some(
                model.curtain_panel_types[&curtain.parameters.panel_type]
                    .parameters
                    .material
                    .unwrap()
            ),
            Some(
                model.curtain_mullion_types[&curtain.parameters.mullion_type]
                    .parameters
                    .material
                    .unwrap()
            ),
        ])
    );
}

#[test]
fn curtain_section_miss_tangent_and_longitudinal_cuts_are_well_formed() {
    let (model, curtain) = fixture();
    let assembly = curtain.id();
    let ctx = context();
    let components = os_geometry::curtain_systems::curtain_geometry(&curtain.parameters, &model)
        .unwrap()
        .components;
    let miss = derive_with_context(
        VerticalSectionPlane {
            origin: Point2::new(-0.2, -2.),
            direction: Point2::new(0., 1.),
        },
        assembly,
        components.clone(),
        ctx,
    );
    assert!(miss.provider_lines(ctx).unwrap().is_empty());
    assert!(!miss.is_native_line(assembly));

    // A supporting vertical plane touches the exterior at one vertical edge;
    // the section kernel correctly treats this zero-area tangent as empty.
    let half_depth = model.curtain_mullion_types[&curtain.parameters.mullion_type]
        .parameters
        .depth
        / 2.0;
    let tangent = derive_with_context(
        VerticalSectionPlane {
            origin: Point2::new(0., half_depth),
            direction: Point2::new(1., 1.),
        },
        assembly,
        components.clone(),
        ctx,
    );
    assert!(tangent.provider_lines(ctx).unwrap().is_empty());
    assert!(!tangent.is_native_line(assembly));

    // A transverse cut through the perimeter mullion interior is nonempty,
    // distinct from the zero-area tangent above.
    let perimeter_cut = derive_with_context(
        VerticalSectionPlane {
            origin: Point2::new(half_depth / 4.0, -2.),
            direction: Point2::new(0., 1.),
        },
        assembly,
        components,
        ctx,
    );
    assert!(!perimeter_cut.provider_lines(ctx).unwrap().is_empty());

    let ctx = context();
    let components = os_geometry::curtain_systems::curtain_geometry(&curtain.parameters, &model)
        .unwrap()
        .components;
    let drawing = derive_with_context(
        VerticalSectionPlane {
            origin: Point2::new(0., 0.),
            direction: Point2::new(1., 0.),
        },
        assembly,
        components,
        ctx,
    );
    let lines = drawing.provider_lines(ctx).unwrap();
    assert!(!lines.is_empty());
    assert!(lines.iter().all(|line| line.entity == assembly));
    assert!(lines.iter().any(|line| {
        drawing
            .line_surface(assembly, line.feature)
            .material
            .is_some()
    }));
}

#[test]
fn maximum_curtain_grid_topology_stays_within_section_budgets() {
    let (model, mut curtain) = fixture();
    curtain.parameters.vertical = (0..os_model::MAX_CURTAIN_GRIDS)
        .map(|i| CurtainGrid {
            id: Id::new(),
            position: i as f64 * 4. / 17.,
        })
        .collect();
    curtain.parameters.horizontal = (0..os_model::MAX_CURTAIN_GRIDS)
        .map(|i| CurtainGrid {
            id: Id::new(),
            position: i as f64 * 3. / 17.,
        })
        .collect();
    curtain.parameters.reconcile().unwrap();
    let ctx = context();
    let components = os_geometry::curtain_systems::curtain_geometry(&curtain.parameters, &model)
        .unwrap()
        .components;
    assert_eq!(components.len(), 18 + 17 * 18 + 17 * 17);
    let drawing = derive_with_context(
        VerticalSectionPlane {
            origin: Point2::new(2., -2.),
            direction: Point2::new(0., 1.),
        },
        curtain.id(),
        components,
        ctx,
    );
    let lines = drawing.provider_lines(ctx).unwrap();
    assert!(lines.len() <= MAX_PLAN_ELEMENTS);
    assert!(lines.iter().all(|line| line.entity == curtain.id()));
    assert!(
        lines
            .windows(2)
            .all(|pair| pair[0].feature != pair[1].feature)
    );
}

#[test]
fn editor_section_snapshot_captures_same_building_curtains_only() {
    let (source, mut curtain) = fixture();
    let mut editor = Editor::new().unwrap();
    let model = editor.document.model();
    let level = *model.levels.keys().next().unwrap();
    let building = model.levels[&level].parameters.building;
    let site = model.buildings[&building].parameters.site;
    let mut model = editor.document.model().clone();
    model.materials = source.materials.clone();
    model.curtain_panel_types = source.curtain_panel_types.clone();
    model.curtain_mullion_types = source.curtain_mullion_types.clone();
    curtain.parameters.level = level;
    let same_id = curtain.id();
    let mut foreign = curtain.clone();
    let other_building = Building::new(
        "core.building",
        BuildingParams {
            name: "Other".into(),
            site,
        },
    );
    let other_building_id = other_building.id();
    let other_level = Level::new(
        "core.level",
        LevelParams {
            name: "Other level".into(),
            elevation: 0.,
            building: other_building_id,
        },
    );
    let other_level_id = other_level.id();
    model.buildings.insert(other_building_id, other_building);
    model.levels.insert(other_level_id, other_level);
    model.curtain_systems.insert(same_id, curtain);
    foreign.parameters.level = other_level_id;
    foreign.header.id = Id::new();
    for grid in foreign
        .parameters
        .vertical
        .iter_mut()
        .chain(&mut foreign.parameters.horizontal)
    {
        grid.id = Id::new();
    }
    foreign.parameters.panels.clear();
    foreign.parameters.mullions.clear();
    foreign.parameters.reconcile().unwrap();
    let foreign_id = foreign.id();
    model.curtain_systems.insert(foreign_id, foreign);
    editor.document = Document::from_model(model).unwrap();

    let view = View::new(
        "core.view",
        ViewParams {
            name: "Curtain section".into(),
            kind: ViewKind::Section,
            level: Some(level),
            settings_revision: 0,
            plan: None,
            section: Some(SectionViewSettings::new(
                Point2::new(1., -2.),
                Point2::new(1., 2.),
                -1.,
                4.,
            )),
        },
    );
    let view_id = view.id();
    editor
        .document
        .execute("Section view", vec![Command::AddView(view)])
        .unwrap();
    let snapshot = editor.section_snapshot(view_id).unwrap();
    assert_eq!(
        snapshot
            .curtains
            .iter()
            .map(|(id, _)| *id)
            .collect::<Vec<_>>(),
        vec![same_id]
    );
    assert_ne!(same_id, foreign_id);
    let drawing = snapshot.derive().unwrap();
    let context = editor.native_section_context(view_id).unwrap();
    assert!(!drawing.provider_lines(context).unwrap().is_empty());
}
