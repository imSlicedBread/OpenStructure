use super::*;
use os_model::{ElevationSide, Wall, WallPath};

fn fixture() -> (Editor, Id, Id) {
    let mut e = Editor::new().unwrap();
    let level = *e.document.model().levels.keys().next().unwrap();
    let mut wall = default_wall(level);
    wall.path = WallPath::Straight {
        start: Point2::new(2., 3.),
        end: Point2::new(8., 11.),
    };
    wall.height = 4.;
    let wall = Wall::new(os_walls::WALL_TYPE, wall);
    let host = wall.id();
    e.command("Host", Command::AddWall(wall)).unwrap();
    let view = e
        .create_wall_elevation("Elevation", host, ElevationSide::Left)
        .unwrap();
    (e, host, view)
}

#[test]
fn elevation_exact_place_edit_preview_history_and_reopen() {
    let (mut e, host, view) = fixture();
    for (kind, point) in [
        (OpeningKind::Door, Point2::new(2., 0.)),
        (OpeningKind::Window, Point2::new(6., 1.)),
    ] {
        let mut draft = OpeningDraft::begin(&e, Some(view), Some(host), Some(kind), None).unwrap();
        draft.elevation_point(e.document.model(), point).unwrap();
        let before = e.document.model().clone();
        let history = e.document.history_stats();
        let scene = e.scene.clone();
        assert!(
            !draft
                .preview(&e, Some(view), Some(host))
                .unwrap()
                .is_empty()
        );
        assert_eq!(e.document.model(), &before);
        assert_eq!(e.document.history_stats(), history);
        assert_eq!(e.scene, scene);
        let id = draft.apply(&mut e, Some(view), Some(host), false).unwrap();
        assert_eq!(
            e.document.history_stats().undo_entries,
            history.undo_entries + 1
        );
        let original = e.document.model().openings[&id].clone();
        let ty = original.parameters.type_id().unwrap();
        let before = e.document.model().clone();
        let before_scene = e.scene.clone();
        let mut edit = OpeningDraft::begin(&e, Some(view), Some(id), None, None).unwrap();
        edit.values[0] = (original.parameters.offset + 0.2).to_string();
        edit.dimension_overrides = [true, true];
        edit.values[1] = "1.0".into();
        edit.values[2] = "1.8".into();
        if kind == OpeningKind::Window {
            edit.override_sill = true;
            edit.values[3] = "1.1".into();
        }
        edit.preview(&e, Some(view), Some(id)).unwrap();
        assert_eq!(e.document.model(), &before);
        assert_eq!(e.scene, before_scene);
        let n = e.document.history_stats().undo_entries;
        edit.apply(&mut e, Some(view), Some(id), false).unwrap();
        assert_eq!(e.document.history_stats().undo_entries, n + 1);
        let after = e.document.model().clone();
        let changed = &after.openings[&id];
        assert_eq!(changed.header, original.header);
        assert_eq!(changed.parameters.type_id(), Some(ty));
        assert_eq!(changed.parameters.hinge, original.parameters.hinge);
        assert_eq!(changed.parameters.swing, original.parameters.swing);
        assert_eq!(
            changed.parameters.open_state,
            original.parameters.open_state
        );
        assert_eq!(
            changed.parameters.pane_position_override,
            original.parameters.pane_position_override
        );
        assert_eq!(after.opening_types, before.opening_types);
        assert!(e.document.undo());
        e.regenerate().unwrap();
        assert_eq!(e.document.model(), &before);
        assert!(e.document.redo());
        e.regenerate().unwrap();
        assert_eq!(e.document.model(), &after);
        let dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let path = dir.path().join("elevation.osb");
        e.save(&path).unwrap();
        e.open(&path).unwrap();
        assert_eq!(e.document.model(), &after);
        assert!(
            !e.native_drawing(view)
                .unwrap()
                .provider_lines(e.native_elevation_context(view).unwrap())
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn elevation_invalid_overlap_stale_drafts_leave_everything_unchanged() {
    let (mut e, host, view) = fixture();
    let mut first =
        OpeningDraft::begin(&e, Some(view), Some(host), Some(OpeningKind::Window), None).unwrap();
    first
        .elevation_point(e.document.model(), Point2::new(4., 1.))
        .unwrap();
    first.apply(&mut e, Some(view), Some(host), false).unwrap();
    let mut draft =
        OpeningDraft::begin(&e, Some(view), Some(host), Some(OpeningKind::Window), None).unwrap();
    let before = e.document.model().clone();
    let scene = e.scene.clone();
    let stats = e.document.history_stats();
    for point in [
        Point2::new(4., 1.),
        Point2::new(-1., 1.),
        Point2::new(9.9, 1.),
        Point2::new(7., 3.8),
        Point2::new(7., -1.),
    ] {
        draft.elevation_point(e.document.model(), point).unwrap();
        assert!(draft.apply(&mut e, Some(view), Some(host), false).is_err());
        assert_eq!(e.document.model(), &before);
        assert_eq!(e.scene, scene);
        assert_eq!(e.document.history_stats(), stats);
    }
    draft
        .elevation_point(e.document.model(), Point2::new(7., 1.))
        .unwrap();
    let frozen = e.elevation_snapshot(view).unwrap();
    let old = e.native_elevation_context(view).unwrap();
    let mut p = e.document.model().walls[&host].parameters.clone();
    p.path = WallPath::Straight {
        start: Point2::new(10., 20.),
        end: Point2::new(16., 28.),
    };
    e.command(
        "Move host",
        Command::UpdateWall {
            id: host,
            parameters: p,
        },
    )
    .unwrap();
    let after = e.document.model().clone();
    let stats = e.document.history_stats();
    assert!(!draft.current(&e, Some(view), Some(host)));
    assert!(draft.apply(&mut e, Some(view), Some(host), false).is_err());
    assert_eq!(e.document.model(), &after);
    assert_eq!(e.document.history_stats(), stats);
    let old_drawing = frozen.derive().unwrap();
    let new = e.native_elevation_context(view).unwrap();
    assert!(old_drawing.provider_lines(new).is_err());
    let signature = |lines: &[os_render::plan::PlanLine]| {
        let mut result: Vec<_> = lines
            .iter()
            .map(|line| {
                let quantize = |value: f64| (value * 1e8).round() as i64;
                let a = (quantize(line.start.x), quantize(line.start.y));
                let b = (quantize(line.end.x), quantize(line.end.y));
                let (a, b) = if a <= b { (a, b) } else { (b, a) };
                (line.entity, a.0, a.1, b.0, b.1)
            })
            .collect();
        result.sort_unstable();
        result
    };
    assert_eq!(
        signature(old_drawing.provider_lines(old).unwrap()),
        signature(e.native_drawing(view).unwrap().provider_lines(new).unwrap())
    );
}

#[test]
fn elevation_crop_picking_materials_sheet_pdf_and_regeneration() {
    use os_render::sheet::{PaperMarkKind, PaperSheetInfo, PaperViewport, compose_view_sheet};
    let (mut e, host, view) = fixture();
    let mut draft =
        OpeningDraft::begin(&e, Some(view), Some(host), Some(OpeningKind::Window), None).unwrap();
    draft
        .elevation_point(e.document.model(), Point2::new(5., 1.))
        .unwrap();
    let id = draft.apply(&mut e, Some(view), Some(host), false).unwrap();
    let ty = e.document.model().openings[&id]
        .parameters
        .type_id()
        .unwrap();
    let m = os_model::Material::new(
        "core.material",
        os_model::MaterialParams {
            cut_pattern: None,
            name: "Panel".into(),
            density_kg_m3: 800.,
            color: [80, 100, 120],
        },
    );
    let material = m.id();
    e.command("Material", Command::AddMaterial(m)).unwrap();
    let mut p = e.document.model().opening_types[&ty].parameters.clone();
    p.family.panel_material = Some(material);
    p.family.frame_material = Some(material);
    p.family.frame_width = 0.05;
    e.command(
        "Material",
        Command::UpdateOpeningType {
            id: ty,
            parameters: p.clone(),
        },
    )
    .unwrap();
    let before = e.native_drawing(view).unwrap();
    let c = e.native_elevation_context(view).unwrap();
    assert!(
        before.provider_lines(c).unwrap().iter().any(
            |l| l.entity == id && before.line_surface(id, l.feature).material == Some(material)
        )
    );
    p.height = 1.5;
    e.command(
        "Type height",
        Command::UpdateOpeningType {
            id: ty,
            parameters: p,
        },
    )
    .unwrap();
    let after = e.native_drawing(view).unwrap();
    let c = e.native_elevation_context(view).unwrap();
    assert!(before.provider_lines(c).is_err());
    let mut vp = e.document.model().views[&view].parameters.clone();
    let s = vp.elevation.as_mut().unwrap();
    s.min_station = 4.5;
    s.max_station = 5.5;
    s.bottom_elevation = 0.8;
    s.top_elevation = 2.8;
    e.command(
        "Crop",
        Command::UpdateView {
            id: view,
            parameters: vp,
        },
    )
    .unwrap();
    let drawing = e.native_drawing(view).unwrap();
    let c = e.native_elevation_context(view).unwrap();
    assert!(after.provider_lines(c).is_err());
    let lines = drawing.provider_lines(c).unwrap();
    assert!(!lines.is_empty());
    for l in lines {
        for p in [l.start, l.end] {
            assert!(
                p.x >= 4.5 - 1e-8 && p.x <= 5.5 + 1e-8 && p.y >= 0.8 - 1e-8 && p.y <= 2.8 + 1e-8
            );
        }
    }
    let camera = os_render::plan::PlanCamera {
        center: Point2::new(5., 1.8),
        pixels_per_metre: 100.,
    };
    let line = lines.iter().find(|l| l.entity == id).unwrap();
    let point = Point2::new(
        (line.start.x + line.end.x) * 0.5,
        (line.start.y + line.end.y) * 0.5,
    );
    assert!(
        drawing
            .hits_screen(
                c,
                camera,
                [800., 600.],
                camera.project(point, [800., 600.]).unwrap(),
                6.
            )
            .unwrap()
            .contains(&id)
    );
    assert!(
        drawing
            .hits_screen(
                c,
                camera,
                [800., 600.],
                camera.project(Point2::new(3., 1.), [800., 600.]).unwrap(),
                6.
            )
            .unwrap()
            .is_empty()
    );
    let viewport = PaperViewport {
        model_center_m: Point2::new(5., 1.8),
        center_mm: Point2::new(210., 148.),
        width_mm: 100.,
        height_mm: 100.,
        scale_denominator: 100.,
    };
    let page = compose_view_sheet(
        PaperSheetInfo {
            width_mm: 420.,
            height_mm: 297.,
            number: "A101",
            name: "Elevation",
        },
        "Elevation",
        viewport,
        c,
        &drawing,
    )
    .unwrap();
    let pdf = String::from_utf8(page.to_pdf().unwrap()).unwrap();
    assert!(pdf.starts_with("%PDF") && pdf.contains(" re W n") && !pdf.contains("/Subtype /Image"));
    for line in lines {
        let points = [
            viewport.model_to_paper(line.start).unwrap(),
            viewport.model_to_paper(line.end).unwrap(),
        ];
        assert!(page.marks().iter().any(|m|matches!(&m.kind,PaperMarkKind::Path{points_mm,closed:false,..} if points_mm.as_slice()==points)));
        let pt = 72. / 25.4;
        assert!(pdf.contains(&format!(
            "{:.6} {:.6} m\n{:.6} {:.6} l",
            points[0].x * pt,
            (297. - points[0].y) * pt,
            points[1].x * pt,
            (297. - points[1].y) * pt
        )));
    }
}
