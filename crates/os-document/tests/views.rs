use os_core::{Id, Point2};
use os_document::{Command, Document};
use os_model::{
    PlanGraphicsBinding, PlanGraphicsStyles, PlanGraphicsTemplate, PlanGraphicsTemplateParams,
    PlanViewCrop, View, ViewKind, ViewParams,
};

fn plan(document: &Document) -> View {
    View::new(
        "core.view",
        ViewParams::floor_plan(
            "Ground plan",
            *document.model().levels.keys().next().unwrap(),
        ),
    )
}

#[test]
fn named_view_commands_are_atomic_identity_preserving_and_undoable() {
    let mut document = Document::new("Plans").unwrap();
    let view = plan(&document);
    let id = view.id();
    document
        .execute("Create plan", vec![Command::AddView(view)])
        .unwrap();
    assert!(document.drain_events()[0].invalidated.contains(&id));
    let original = document.model().clone();
    let mut parameters = document.model().views[&id].parameters.clone();
    parameters.name = "Renamed plan".into();
    parameters.settings_revision = 9999; // callers cannot choose a revision
    let settings = parameters.plan.as_mut().unwrap();
    settings.basis.rotation = 0.25;
    settings.range.cut = 1.5;
    settings.crop = Some(PlanViewCrop {
        min: Point2::new(-1.0, -1.0),
        max: Point2::new(10.0, 10.0),
    });
    settings.scale_denominator = 50.0;
    document
        .execute("Settings", vec![Command::UpdateView { id, parameters }])
        .unwrap();
    assert_eq!(document.model().views[&id].parameters.settings_revision, 1);
    assert_eq!(
        document.model().views[&id].header,
        original.views[&id].header
    );
    let edited = document.model().clone();
    assert!(document.undo());
    assert_eq!(document.model(), &original);
    assert!(document.redo());
    assert_eq!(document.model(), &edited);
    document
        .execute("Remove plan", vec![Command::RemoveView(id)])
        .unwrap();
    assert!(!document.model().views.contains_key(&id));
    assert!(document.undo());
    assert_eq!(document.model(), &edited);
}

#[test]
fn invalid_batches_and_noop_updates_keep_model_history_revision_and_events() {
    let mut document = Document::new("Plans").unwrap();
    let view = plan(&document);
    let id = view.id();
    document
        .execute("Create", vec![Command::AddView(view.clone())])
        .unwrap();
    document
        .execute("Rename", vec![Command::RenameProject("Changed".into())])
        .unwrap();
    document.undo();
    document.drain_events();
    let original = document.model().clone();
    let stats = document.history_stats();
    let revision = document.revision();
    let mut invalid = view.parameters.clone();
    invalid.plan.as_mut().unwrap().range.cut = 99.0;
    for commands in [
        vec![Command::AddView(view.clone())],
        vec![Command::UpdateView {
            id,
            parameters: invalid,
        }],
        vec![
            Command::RenameProject("Partial".into()),
            Command::RemoveView(Id::new()),
        ],
        vec![Command::RemoveLevel(view.parameters.level.unwrap())],
    ] {
        assert!(document.execute("Invalid", commands).is_err());
        assert_eq!(document.model(), &original);
        assert_eq!(document.history_stats(), stats);
        assert_eq!(document.revision(), revision);
        assert!(document.drain_events().is_empty());
    }
    let mut no_op = view.parameters;
    no_op.settings_revision = 123;
    document
        .execute(
            "No-op",
            vec![Command::UpdateView {
                id,
                parameters: no_op,
            }],
        )
        .unwrap();
    assert_eq!(document.history_stats(), stats);
    assert_eq!(document.revision(), revision);
    assert!(document.can_redo());
    assert!(document.drain_events().is_empty());
}

#[test]
fn view_replacement_checks_the_final_graph_not_an_intermediate_empty_view_list() {
    let mut document = Document::new("Plans").unwrap();
    let initial = *document.model().views.keys().next().unwrap();
    assert!(
        document
            .execute("Remove last", vec![Command::RemoveView(initial)])
            .is_err()
    );
    let replacement = plan(&document);
    let id = replacement.id();
    document
        .execute(
            "Replace view",
            vec![Command::RemoveView(initial), Command::AddView(replacement)],
        )
        .unwrap();
    assert_eq!(document.model().views.len(), 1);
    assert!(document.model().views.contains_key(&id));
    assert!(document.undo());
    assert!(document.model().views.contains_key(&initial));
}

#[test]
fn creation_validation_kind_changes_revision_overflow_and_level_invalidation() {
    let mut document = Document::new("Plans").unwrap();
    for case in ["name", "level", "revision", "settings"] {
        let mut view = plan(&document);
        match case {
            "name" => view.parameters.name = " ".into(),
            "level" => view.parameters.level = None,
            "revision" => view.parameters.settings_revision = 1,
            "settings" => view.parameters.plan = None,
            _ => unreachable!(),
        }
        assert!(
            document
                .execute("Invalid", vec![Command::AddView(view)])
                .is_err()
        );
        assert_eq!(document.revision(), 0);
    }
    let view = plan(&document);
    let id = view.id();
    let level = view.parameters.level.unwrap();
    document
        .execute("Create", vec![Command::AddView(view)])
        .unwrap();
    let mut parameters = document.model().views[&id].parameters.clone();
    parameters.kind = ViewKind::Perspective;
    parameters.plan = None;
    assert!(
        document
            .execute("Change kind", vec![Command::UpdateView { id, parameters }])
            .is_err()
    );
    document.drain_events();
    let mut parameters = document.model().levels[&level].parameters.clone();
    parameters.elevation = 4.0;
    document
        .execute(
            "Level",
            vec![Command::UpdateLevel {
                id: level,
                parameters,
            }],
        )
        .unwrap();
    assert!(document.drain_events()[0].invalidated.contains(&id));
    let mut model = document.model().clone();
    let valid = model.clone();
    let mut invalid_level = model.levels[&level].parameters.clone();
    invalid_level.elevation = f64::MAX;
    assert!(
        document
            .execute(
                "Unrepresentable plan range",
                vec![Command::UpdateLevel {
                    id: level,
                    parameters: invalid_level
                }]
            )
            .is_err()
    );
    assert_eq!(document.model(), &valid);
    model
        .views
        .get_mut(&id)
        .unwrap()
        .parameters
        .settings_revision = u64::MAX;
    let mut document = Document::from_model(model).unwrap();
    let original = document.model().clone();
    let mut parameters = original.views[&id].parameters.clone();
    parameters.name = "Overflow".into();
    assert!(
        document
            .execute("Overflow", vec![Command::UpdateView { id, parameters }])
            .is_err()
    );
    assert_eq!(document.model(), &original);
    assert_eq!(document.revision(), 0);
    assert!(!document.can_undo());
}

#[test]
fn graphics_template_link_guard_and_removal_are_atomic_and_undoable() {
    let mut document = Document::new("Graphics templates").unwrap();
    let first = plan(&document);
    let first_id = first.id();
    let second = plan(&document);
    let second_id = second.id();
    document
        .execute(
            "Create plans",
            vec![Command::AddView(first), Command::AddView(second)],
        )
        .unwrap();
    let template = PlanGraphicsTemplate::new(
        "core.plan_graphics_template",
        PlanGraphicsTemplateParams {
            name: "Office standards".into(),
            styles: PlanGraphicsStyles::default(),
        },
    );
    let template_id = template.id();
    document
        .execute(
            "Create and link standards",
            vec![
                Command::AddPlanGraphicsTemplate(template),
                Command::SetPlanGraphics {
                    view: first_id,
                    binding: Some(PlanGraphicsBinding {
                        template: Some(template_id),
                        local: PlanGraphicsStyles::default(),
                    }),
                },
                Command::SetPlanGraphics {
                    view: second_id,
                    binding: Some(PlanGraphicsBinding {
                        template: Some(template_id),
                        local: PlanGraphicsStyles::default(),
                    }),
                },
            ],
        )
        .unwrap();
    let original = document.model().clone();
    let mut edited_template = document.model().plan_graphics_templates[&template_id]
        .parameters
        .clone();
    edited_template.styles.walls.cut.weight_mm = 0.75;
    document.drain_events();
    document
        .execute(
            "Edit linked standards",
            vec![Command::UpdatePlanGraphicsTemplate {
                id: template_id,
                parameters: edited_template,
            }],
        )
        .unwrap();
    let linked = document.model().clone();
    let events = document.drain_events();
    assert!(events.iter().any(|event| {
        event.invalidated.contains(&first_id) && event.invalidated.contains(&second_id)
    }));
    assert_eq!(
        document
            .model()
            .plan_graphics_styles(second_id)
            .unwrap()
            .walls
            .cut
            .weight_mm,
        0.75
    );
    assert!(document.undo());
    assert_eq!(document.model(), &original);
    assert!(document.redo());
    assert_eq!(document.model(), &linked);

    let linked = document.model().clone();
    let history = document.history_stats();
    let revision = document.revision();
    document.drain_events();
    assert!(
        document
            .execute(
                "Delete linked template",
                vec![Command::RemovePlanGraphicsTemplate(template_id)]
            )
            .is_err()
    );
    assert_eq!(document.model(), &linked);
    assert_eq!(document.history_stats(), history);
    assert_eq!(document.revision(), revision);
    assert!(document.drain_events().is_empty());

    document
        .execute(
            "Unlink and remove standards",
            vec![
                Command::SetPlanGraphics {
                    view: first_id,
                    binding: None,
                },
                Command::SetPlanGraphics {
                    view: second_id,
                    binding: None,
                },
                Command::RemovePlanGraphicsTemplate(template_id),
            ],
        )
        .unwrap();
    assert!(
        !document
            .model()
            .plan_graphics_templates
            .contains_key(&template_id)
    );
    assert!(document.undo());
    assert_eq!(document.model(), &linked);
}
