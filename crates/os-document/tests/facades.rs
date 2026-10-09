use os_core::Point2;
use os_document::{Command, Document};
use os_model::*;
#[test]
fn facade_view_marker_identity_one_transaction_history_settings_and_wall_independence() {
    let mut doc = Document::new("Facade").unwrap();
    let level = *doc.model().levels.keys().next().unwrap();
    let s = FacadeViewSettings::new(Point2::new(0., 5.), Point2::new(10., 5.), -1., 10.).unwrap();
    let view = View::new(
        "core.view",
        ViewParams {
            name: "Facade".into(),
            kind: ViewKind::Facade,
            level: Some(level),
            settings_revision: 0,
            plan: None,
            section: None,
            elevation: None,
            facade: Some(s),
        },
    );
    let id = view.id();
    let before = doc.model().clone();
    let history = doc.history_stats();
    doc.execute("Create facade and marker", vec![Command::AddView(view)])
        .unwrap();
    assert_eq!(doc.history_stats().undo_entries, history.undo_entries + 1);
    let added = doc.model().clone();
    assert!(doc.undo());
    assert_eq!(doc.model(), &before);
    assert!(doc.redo());
    assert_eq!(doc.model(), &added);
    let mut p = doc.model().views[&id].parameters.clone();
    p.facade.as_mut().unwrap().far_depth = 50.;
    doc.execute("Depth", vec![Command::UpdateView { id, parameters: p }])
        .unwrap();
    assert_eq!(doc.model().views[&id].id(), id);
    assert_eq!(doc.model().views[&id].parameters.settings_revision, 1);
    let current = doc.model().clone();
    let mut bad = current.views[&id].parameters.clone();
    bad.facade.as_mut().unwrap().near_depth = 60.;
    assert!(
        doc.execute(
            "Bad depth",
            vec![Command::UpdateView {
                id,
                parameters: bad
            }]
        )
        .is_err()
    );
    assert_eq!(doc.model(), &current);
    let wall = Wall::new(
        "org.openstructure.walls.wall",
        WallParams {
            name: "Host".into(),
            path: WallPath::Straight {
                start: Point2::new(8., 0.),
                end: Point2::new(0., 0.),
            },
            height: 3.,
            thickness: 0.2,
            material: None,
            level,
        },
    );
    let host = wall.id();
    doc.execute("Wall", vec![Command::AddWall(wall)]).unwrap();
    doc.execute("Remove wall", vec![Command::RemoveWall(host)])
        .unwrap();
    assert_eq!(doc.model().views[&id], current.views[&id]);
}
