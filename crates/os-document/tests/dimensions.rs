use os_core::{Id, Point2};
use os_document::{Command, Document};
use os_model::*;

fn setup() -> (Document, Dimension, Wall) {
    let mut model = Model::new("Dimensions");
    let level = *model.levels.keys().next().unwrap();
    let view = View::new("core.view", ViewParams::floor_plan("Plan", level));
    let wall = Wall::new(
        "org.openstructure.walls.wall",
        WallParams {
            name: "Wall".into(),
            start: Point2::new(0., 0.),
            end: Point2::new(4., 0.),
            height: 3.,
            thickness: 0.2,
            level,
            material: None,
        },
    );
    let dimension = Dimension::new(
        "core.dimension",
        DimensionParams {
            layout: os_model::DimensionLayout::Aligned,
            additional: Vec::new(),
            baseline_spacing_m: 0.25,
            view: view.id(),
            first: DimensionReference {
                wall: wall.id(),
                endpoint: DimensionEndpoint::Start,
            },
            second: DimensionReference {
                wall: wall.id(),
                endpoint: DimensionEndpoint::End,
            },
            offset_m: -1.,
            orphan_hint: Point2::new(2., -1.),
        },
    );
    model.views.insert(view.id(), view);
    model.walls.insert(wall.id(), wall.clone());
    (Document::from_model(model).unwrap(), dimension, wall)
}

#[test]
fn atomic_dimension_commands_preserve_identity_and_one_step_history() {
    let (mut doc, dimension, _) = setup();
    let before = doc.model().clone();
    doc.execute("Place", vec![Command::AddDimension(dimension.clone())])
        .unwrap();
    assert_eq!(doc.history_stats().undo_entries, 1);
    let placed = doc.model().clone();
    let event = doc.drain_events().pop().unwrap();
    assert!(event.invalidated.contains(&dimension.id()));
    assert!(event.invalidated.contains(&dimension.parameters.view));
    assert!(doc.undo());
    assert_eq!(doc.model(), &before);
    assert!(doc.redo());
    assert_eq!(doc.model(), &placed);

    let mut parameters = dimension.parameters.clone();
    parameters.offset_m = 2.;
    doc.execute(
        "Offset",
        vec![Command::UpdateDimension {
            id: dimension.id(),
            parameters,
        }],
    )
    .unwrap();
    assert_eq!(
        doc.model().dimensions[&dimension.id()].header,
        dimension.header
    );
    let edited = doc.model().clone();
    assert!(doc.undo());
    assert_eq!(doc.model(), &placed);
    assert!(doc.redo());
    assert_eq!(doc.model(), &edited);
    doc.drain_events();
    let revision = doc.revision();
    let history = doc.history_stats();
    let mut bad = dimension.parameters.clone();
    bad.offset_m = f64::INFINITY;
    assert!(
        doc.execute(
            "Bad batch",
            vec![
                Command::RenameProject("Bad".into()),
                Command::UpdateDimension {
                    id: dimension.id(),
                    parameters: bad
                }
            ]
        )
        .is_err()
    );
    assert_eq!(doc.model(), &edited);
    assert_eq!(doc.revision(), revision);
    assert_eq!(doc.history_stats(), history);
    assert!(doc.drain_events().is_empty());
    assert!(
        doc.execute("Duplicate", vec![Command::AddDimension(dimension.clone())])
            .is_err()
    );
    assert!(
        doc.execute(
            "Remove view",
            vec![Command::RemoveView(dimension.parameters.view)]
        )
        .is_err()
    );
    doc.execute(
        "Delete annotation and view",
        vec![
            Command::RemoveDimension(dimension.id()),
            Command::RemoveView(dimension.parameters.view),
        ],
    )
    .unwrap();
    assert!(doc.model().dimensions.is_empty());
    assert!(doc.undo());
    assert_eq!(doc.model(), &edited);
    assert!(doc.redo());
    assert!(!doc.model().views.contains_key(&dimension.parameters.view));
}

#[test]
fn creation_requires_resolved_noncoincident_same_level_native_anchors() {
    for case in 0..5 {
        let (mut doc, mut dimension, wall) = setup();
        match case {
            0 => dimension.parameters.first.wall = Id::new(),
            1 => dimension.parameters.second = dimension.parameters.first,
            2 => {
                let mut other = wall.clone();
                other.header.id = Id::new();
                dimension.parameters.second = DimensionReference {
                    wall: other.id(),
                    endpoint: DimensionEndpoint::Start,
                };
                doc.execute("Other", vec![Command::AddWall(other)]).unwrap();
            }
            3 => {
                let mut other_level = doc.model().levels[&wall.parameters.level].clone();
                other_level.header.id = Id::new();
                let mut parameters = wall.parameters.clone();
                parameters.level = other_level.id();
                doc.execute(
                    "Relevel",
                    vec![
                        Command::AddLevel(other_level),
                        Command::UpdateWall {
                            id: wall.id(),
                            parameters,
                        },
                    ],
                )
                .unwrap();
            }
            _ => {
                dimension.parameters.view = *doc
                    .model()
                    .views
                    .iter()
                    .find(|(_, v)| v.parameters.kind == ViewKind::Perspective)
                    .unwrap()
                    .0
            }
        }
        let before = doc.model().clone();
        let revision = doc.revision();
        let history = doc.history_stats();
        assert!(
            doc.execute("Invalid", vec![Command::AddDimension(dimension)])
                .is_err()
        );
        assert_eq!(doc.model(), &before);
        assert_eq!(doc.revision(), revision);
        assert_eq!(doc.history_stats(), history);
    }
}

#[test]
fn live_measurements_and_orphans_follow_edits_and_undo_without_blocking_walls() {
    let (mut doc, mut dimension, wall) = setup();
    let mut second_wall = wall.clone();
    second_wall.header.id = Id::new();
    second_wall.parameters.start = Point2::new(4., 0.);
    second_wall.parameters.end = Point2::new(8., 0.);
    dimension.parameters.second = DimensionReference {
        wall: second_wall.id(),
        endpoint: DimensionEndpoint::Start,
    };
    doc.execute(
        "Setup",
        vec![
            Command::AddWall(second_wall.clone()),
            Command::AddDimension(dimension.clone()),
        ],
    )
    .unwrap();
    assert_eq!(
        dimension
            .parameters
            .resolve(doc.model())
            .unwrap()
            .length_metres,
        4.
    );
    let mut resized = wall.parameters.clone();
    resized.start.x = -2.;
    doc.execute(
        "Resize",
        vec![Command::UpdateWall {
            id: wall.id(),
            parameters: resized,
        }],
    )
    .unwrap();
    assert_eq!(
        dimension
            .parameters
            .resolve(doc.model())
            .unwrap()
            .length_metres,
        6.
    );
    assert!(doc.undo());
    assert_eq!(
        dimension
            .parameters
            .resolve(doc.model())
            .unwrap()
            .length_metres,
        4.
    );
    assert!(doc.redo());
    assert_eq!(
        dimension
            .parameters
            .resolve(doc.model())
            .unwrap()
            .length_metres,
        6.
    );
    assert!(doc.undo());

    for case in 0..4 {
        let before = doc.model().clone();
        let expected = match case {
            0 => {
                let mut parameters = second_wall.parameters.clone();
                parameters.start = wall.parameters.start;
                doc.execute(
                    "Coincide",
                    vec![Command::UpdateWall {
                        id: second_wall.id(),
                        parameters,
                    }],
                )
                .unwrap();
                DimensionDiagnostic::CoincidentAnchors
            }
            1 => {
                doc.execute("Delete anchor wall", vec![Command::RemoveWall(wall.id())])
                    .unwrap();
                DimensionDiagnostic::MissingWall
            }
            _ => {
                let mut level = doc.model().levels[&wall.parameters.level].clone();
                level.header.id = Id::new();
                let change = if case == 2 {
                    let mut parameters = wall.parameters.clone();
                    parameters.level = level.id();
                    Command::UpdateWall {
                        id: wall.id(),
                        parameters,
                    }
                } else {
                    let mut parameters = doc.model().views[&dimension.parameters.view]
                        .parameters
                        .clone();
                    parameters.level = Some(level.id());
                    Command::UpdateView {
                        id: dimension.parameters.view,
                        parameters,
                    }
                };
                doc.execute("Relevel", vec![Command::AddLevel(level), change])
                    .unwrap();
                DimensionDiagnostic::WrongLevel
            }
        };
        assert_eq!(dimension.parameters.resolve(doc.model()), Err(expected));
        assert_eq!(doc.model().dimensions[&dimension.id()], dimension);
        assert!(
            doc.drain_events()
                .last()
                .unwrap()
                .invalidated
                .contains(&dimension.id())
        );
        assert!(doc.undo());
        assert_eq!(doc.model(), &before);
        assert!(dimension.parameters.resolve(doc.model()).is_ok());
        assert!(doc.redo());
        assert_eq!(dimension.parameters.resolve(doc.model()), Err(expected));
        assert!(doc.undo());
    }
}
