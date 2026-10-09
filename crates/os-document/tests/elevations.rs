use os_core::{Id, Point2};
use os_document::{Command, Document};
use os_model::*;

fn fixture() -> (Document, Id, View) {
    let mut doc = Document::new("Elevations").unwrap();
    let level = *doc.model().levels.keys().next().unwrap();
    let wall = Wall::new(
        "org.openstructure.walls.wall",
        WallParams {
            name: "Host".into(),
            path: WallPath::Straight {
                start: Point2::new(0.0, 0.0),
                end: Point2::new(8.0, 0.0),
            },
            thickness: 0.2,
            height: 3.0,
            level,
            material: None,
        },
    );
    let host = wall.id();
    doc.execute("Host", vec![Command::AddWall(wall)]).unwrap();
    let view = View::new(
        "core.view",
        ViewParams {
            name: "Left elevation".into(),
            kind: ViewKind::Elevation,
            level: None,
            settings_revision: 0,
            plan: None,
            section: None,
            facade: None,
            elevation: Some(ElevationViewSettings::new(
                host,
                ElevationSide::Left,
                -0.5,
                9.0,
                -1.0,
                5.0,
            )),
        },
    );
    (doc, host, view)
}

#[test]
fn elevation_dimensions_are_live_atomic_orphan_preserving_and_repairable() {
    use ElevationDimensionFeature as F;
    let (mut doc, host, view) = fixture();
    let view_id = view.id();
    doc.execute("View", vec![Command::AddView(view)]).unwrap();
    let p = DimensionParams {
        layout: DimensionLayout::ElevationHorizontal,
        additional: vec![],
        baseline_spacing_m: 0.25,
        view: view_id,
        first: DimensionReference::ElevationWall {
            wall: host,
            feature: F::Start,
        },
        second: DimensionReference::ElevationWall {
            wall: host,
            feature: F::End,
        },
        offset_m: 0.5,
        orphan_hint: Point2::new(4., 2.),
    };
    let mut d = Dimension::new("core.dimension", p.clone());
    d.header
        .properties
        .insert("opaque".into(), "preserve me".into());
    let id = d.id();
    let before = doc.model().clone();
    doc.execute("Dimension", vec![Command::AddDimension(d.clone())])
        .unwrap();
    let created = doc.model().clone();
    assert!(doc.undo());
    assert_eq!(doc.model(), &before);
    assert!(doc.redo());
    assert_eq!(doc.model(), &created);
    assert_eq!(p.resolve(doc.model()).unwrap().length_metres, 8.);
    for side in [ElevationSide::Left, ElevationSide::Right] {
        let mut vp = doc.model().views[&view_id].parameters.clone();
        vp.elevation.as_mut().unwrap().side = side;
        doc.execute(
            "Side",
            vec![Command::UpdateView {
                id: view_id,
                parameters: vp,
            }],
        )
        .unwrap();
        for (start, end) in [
            (Point2::new(3., 4.), Point2::new(9., 12.)),
            (Point2::new(9., 12.), Point2::new(3., 4.)),
        ] {
            let mut w = doc.model().walls[&host].parameters.clone();
            w.path = WallPath::Straight { start, end };
            doc.execute(
                "Host",
                vec![Command::UpdateWall {
                    id: host,
                    parameters: w,
                }],
            )
            .unwrap();
            assert_eq!(p.resolve(doc.model()).unwrap().length_metres, 10.);
        }
    }
    let mut p2 = p.clone();
    p2.offset_m = -0.5;
    doc.execute(
        "Offset",
        vec![Command::UpdateDimension {
            id,
            parameters: p2.clone(),
        }],
    )
    .unwrap();
    assert_eq!(doc.model().dimensions[&id].header, d.header);
    let original = doc.model().clone();
    let history = doc.history_stats();
    for case in 0..4 {
        let mut bad = p2.clone();
        match case {
            0 => {
                bad.first = DimensionReference::ElevationWall {
                    wall: Id::new(),
                    feature: F::Start,
                }
            }
            1 => bad.layout = DimensionLayout::ElevationVertical,
            2 => bad.second = bad.first,
            _ => {
                bad.first = DimensionReference::WallEndpoint {
                    wall: host,
                    endpoint: DimensionEndpoint::Start,
                }
            }
        }
        assert!(
            doc.execute(
                "Bad",
                vec![Command::UpdateDimension {
                    id,
                    parameters: bad
                }]
            )
            .is_err()
        );
        assert_eq!(doc.model(), &original);
        assert_eq!(doc.history_stats(), history);
    }
    doc.execute("Delete host", vec![Command::RemoveWall(host)])
        .unwrap();
    assert_eq!(doc.model().dimensions[&id].parameters, p2);
    assert_eq!(
        p2.resolve(doc.model()),
        Err(DimensionDiagnostic::MissingWall)
    );
    let mut p3 = p2.clone();
    p3.offset_m = 0.8;
    doc.execute(
        "Orphan offset",
        vec![Command::UpdateDimension { id, parameters: p3 }],
    )
    .unwrap();
    assert_eq!(doc.model().dimensions[&id].header, d.header);
    doc.execute("Delete dimension", vec![Command::RemoveDimension(id)])
        .unwrap();
    assert!(!doc.model().dimensions.contains_key(&id));
    assert!(doc.undo());
    assert_eq!(doc.model().dimensions[&id].header, d.header);
}

#[test]
fn elevation_commands_preserve_identity_and_revision_through_undo_redo() {
    let (mut doc, host, view) = fixture();
    let id = view.id();
    let before = doc.model().clone();
    doc.execute("Create", vec![Command::AddView(view.clone())])
        .unwrap();
    let created = doc.model().clone();
    assert!(doc.undo());
    assert_eq!(doc.model(), &before);
    assert!(doc.redo());
    assert_eq!(doc.model(), &created);
    let mut parameters = view.parameters.clone();
    parameters.name = "Right elevation".into();
    parameters.settings_revision = 999;
    parameters.elevation.as_mut().unwrap().side = ElevationSide::Right;
    doc.execute("Edit", vec![Command::UpdateView { id, parameters }])
        .unwrap();
    assert_eq!(doc.model().views[&id].header, view.header);
    assert_eq!(doc.model().views[&id].parameters.settings_revision, 1);
    let edited = doc.model().clone();
    assert!(doc.undo());
    assert_eq!(doc.model(), &created);
    assert!(doc.redo());
    assert_eq!(doc.model(), &edited);
    doc.execute("Delete host", vec![Command::RemoveWall(host)])
        .unwrap();
    assert_eq!(doc.model().views[&id], edited.views[&id]);
    assert!(
        doc.model().views[&id]
            .parameters
            .elevation
            .unwrap()
            .resolve_host(doc.model())
            .is_err()
    );
    let orphan = doc.model().clone();
    assert!(doc.undo());
    assert_eq!(doc.model(), &edited);
    assert!(doc.redo());
    assert_eq!(doc.model(), &orphan);
    doc.execute("Remove orphan view", vec![Command::RemoveView(id)])
        .unwrap();
    assert!(doc.undo());
    assert_eq!(doc.model(), &orphan);
}

#[test]
fn elevation_create_edit_and_final_transaction_graph_reject_bad_hosts_atomically() {
    let (mut doc, host, view) = fixture();
    doc.execute("Create", vec![Command::AddView(view.clone())])
        .unwrap();
    doc.drain_events();
    let before = doc.model().clone();
    let history = doc.history_stats();
    let revision = doc.revision();
    for case in 0..6 {
        let mut parameters = view.parameters.clone();
        let commands = match case {
            0 => {
                parameters.elevation.as_mut().unwrap().host = Id::new();
                vec![Command::UpdateView {
                    id: view.id(),
                    parameters,
                }]
            }
            1 => {
                parameters.elevation.as_mut().unwrap().min_station = f64::NAN;
                vec![Command::UpdateView {
                    id: view.id(),
                    parameters,
                }]
            }
            2 => {
                parameters.elevation = None;
                vec![Command::UpdateView {
                    id: view.id(),
                    parameters,
                }]
            }
            3 => {
                let mut added = view.clone();
                added.header.id = Id::new();
                vec![Command::AddView(added), Command::RemoveWall(host)]
            }
            4 => vec![
                Command::UpdateView {
                    id: view.id(),
                    parameters,
                },
                Command::RemoveWall(host),
            ],
            _ => {
                let mut added = view.clone();
                added.header.id = Id::new();
                added.parameters.elevation.as_mut().unwrap().host = Id::new();
                vec![Command::AddView(added)]
            }
        };
        assert!(doc.execute("Invalid", commands).is_err(), "{case}");
        assert_eq!(doc.model(), &before);
        assert_eq!(doc.history_stats(), history);
        assert_eq!(doc.revision(), revision);
        assert!(doc.drain_events().is_empty());
    }
    let mut wall = doc.model().walls[&host].parameters.clone();
    wall.path = WallPath::CircularArc {
        center: Point2::new(0.0, 0.0),
        radius: 4.0,
        start_angle_rad: 0.0,
        signed_sweep_rad: 1.0,
    };
    doc.execute(
        "Curve host",
        vec![Command::UpdateWall {
            id: host,
            parameters: wall,
        }],
    )
    .unwrap();
    let curved = doc.model().clone();
    let mut parameters = view.parameters.clone();
    parameters.name = "Rename orphan".into();
    assert!(
        doc.execute(
            "Invalid orphan edit",
            vec![Command::UpdateView {
                id: view.id(),
                parameters
            }]
        )
        .is_err()
    );
    let mut added = view;
    added.header.id = Id::new();
    assert!(
        doc.execute("Invalid curved create", vec![Command::AddView(added)])
            .is_err()
    );
    assert_eq!(doc.model(), &curved);
    assert!(doc.undo());
    assert_eq!(doc.model(), &before);
}

#[test]
fn endpoint_reversal_flips_physical_face_and_rebases_station_without_changing_authored_settings() {
    let (mut doc, host, mut left) = fixture();
    left.parameters.elevation.as_mut().unwrap().min_station = 1.0;
    left.parameters.elevation.as_mut().unwrap().max_station = 3.0;
    let mut right = left.clone();
    right.header.id = Id::new();
    right.parameters.name = "Right".into();
    right.parameters.elevation.as_mut().unwrap().side = ElevationSide::Right;
    doc.execute(
        "Both sides",
        vec![
            Command::AddView(left.clone()),
            Command::AddView(right.clone()),
        ],
    )
    .unwrap();
    let before = doc.model().clone();
    let mut parameters = doc.model().walls[&host].parameters.clone();
    parameters.path = WallPath::Straight {
        start: parameters.end(),
        end: parameters.start(),
    };
    doc.execute(
        "Reverse host",
        vec![Command::UpdateWall {
            id: host,
            parameters,
        }],
    )
    .unwrap();
    let reversed = doc.model().clone();
    for (view, sign) in [(&left, 1.0), (&right, -1.0)] {
        let settings = view.parameters.elevation.unwrap();
        let old = settings.resolve_host(&before).unwrap();
        let current = settings.resolve_host(doc.model()).unwrap();
        assert_eq!(old.path.point(0.0), Point2::new(0.0, 0.0));
        assert_eq!(current.path.point(0.0), Point2::new(8.0, 0.0));
        assert_eq!(old.path.point(settings.min_station), Point2::new(1.0, 0.0));
        assert_eq!(
            current.path.point(settings.min_station),
            Point2::new(7.0, 0.0)
        );
        assert_eq!(
            old.path.offset_point(1.0, sign * old.thickness / 2.0),
            Point2::new(1.0, sign * 0.1)
        );
        assert_eq!(
            current
                .path
                .offset_point(1.0, sign * current.thickness / 2.0),
            Point2::new(7.0, -sign * 0.1)
        );
        assert_eq!(doc.model().views[&view.id()], *view);
    }
    assert!(doc.undo());
    assert_eq!(doc.model(), &before);
    assert!(doc.redo());
    assert_eq!(doc.model(), &reversed);
}
