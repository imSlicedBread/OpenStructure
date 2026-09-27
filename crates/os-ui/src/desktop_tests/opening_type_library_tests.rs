use super::*;
use os_model::{
    Model, OpeningFamily, OpeningKind, OpeningType, OpeningTypeParams, WindowPanePosition,
};

const PROFILES: [(egui::Vec2, f32); 2] = [
    (egui::vec2(1280.0, 800.0), 1.0),
    (egui::vec2(1000.0, 650.0), 1.5),
];

fn write_package(directory: &std::path::Path, name: &str, kind: OpeningKind) {
    write_package_with_width(
        directory,
        name,
        kind,
        if kind == OpeningKind::Door { 0.9 } else { 1.2 },
    );
}

fn write_package_with_width(
    directory: &std::path::Path,
    name: &str,
    kind: OpeningKind,
    width: f64,
) {
    let mut model = Model::new("Local library fixture");
    let ty = OpeningType::new(
        "core.opening_type",
        OpeningTypeParams {
            family: OpeningFamily::default(),
            name: name.into(),
            kind,
            width,
            height: if kind == OpeningKind::Door { 2.1 } else { 1.2 },
            sill: if kind == OpeningKind::Door { 0.0 } else { 0.8 },
            pane_position: WindowPanePosition::Center,
        },
    );
    let id = ty.id();
    model.opening_types.insert(id, ty);
    os_storage::write_opening_type_package(&model, id, &directory.join(format!("{name}.osot")))
        .unwrap();
}

fn rendered_contains(h: &Harness, value: &str) -> bool {
    h.output.shapes.iter().any(|shape| match &shape.shape {
        egui::Shape::Text(text) => text.galley.job.text.contains(value),
        _ => false,
    })
}

fn wait_for_scan(h: &mut Harness) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !rendered_contains(h, "2 valid package(s)") {
        assert!(
            std::time::Instant::now() < deadline,
            "local package scan did not finish"
        );
        std::thread::yield_now();
        h.frame(vec![]);
    }
}

#[test]
fn local_library_search_preview_cancel_and_import_reuse_the_safe_package_flow() {
    for (size, scale) in PROFILES {
        for (wanted, unwanted, kind) in [
            ("Oak Door 900", "Picture Window", OpeningKind::Door),
            ("Picture Window", "Oak Door 900", OpeningKind::Window),
        ] {
            let directory = tempfile::tempdir().unwrap();
            write_package(directory.path(), "Oak Door 900", OpeningKind::Door);
            write_package(directory.path(), "Picture Window", OpeningKind::Window);
            std::fs::write(directory.path().join("broken.osot"), b"not json").unwrap();
            std::fs::write(directory.path().join("ignore.json"), b"not a package").unwrap();
            std::fs::create_dir(directory.path().join("nested")).unwrap();

            let mut h = Harness::at_size(size, scale);
            let before = h.app.editor.document.model().clone();
            let history = h.app.editor.document.history_stats();
            h.app.begin_opening_type_library_dialog();
            h.app.opening_type_library_dialog.as_mut().unwrap().path =
                directory.path().display().to_string();
            h.frame(vec![]);
            h.frame(vec![]);
            h.click("Scan library");
            wait_for_scan(&mut h);
            assert!(rendered_contains(&h, "1 issue(s)"));
            assert!(rendered_contains(&h, "broken.osot"));
            assert!(rendered_contains(&h, wanted));
            assert!(rendered_contains(&h, unwanted));

            h.click("name, kind, file, material");
            h.replace_focused(wanted);
            h.frame(vec![]);
            assert!(rendered_contains(&h, wanted));
            assert!(!rendered_contains(&h, unwanted));
            assert!(h.visible_text_rect("1 matching package(s)").is_some());
            write_package_with_width(directory.path(), wanted, kind, 1.3);
            h.click("Preview import");
            assert!(h.app.opening_type_library_dialog.is_some());
            assert!(h.app.opening_type_package_dialog.is_some());
            assert!(
                h.visible_text_rect(&format!(
                    "Package: {wanted} · {kind:?} · {:.3} × {:.3} m",
                    1.3,
                    if kind == OpeningKind::Door { 2.1 } else { 1.2 }
                ))
                .is_some()
            );
            assert_eq!(h.app.editor.document.model(), &before);
            assert_eq!(h.app.editor.document.history_stats(), history);

            h.click("Cancel");
            assert!(h.app.opening_type_package_dialog.is_none());
            assert!(h.app.opening_type_library_dialog.is_some());
            assert_eq!(h.app.editor.document.model(), &before);
            h.click("Preview import");
            h.click("Import type");
            let id = h.app.selected.expect("new catalog type should be selected");
            assert_eq!(
                h.app.editor.document.model().opening_types[&id]
                    .parameters
                    .name,
                wanted
            );
            assert_eq!(
                h.app.editor.document.model().opening_types[&id]
                    .parameters
                    .kind,
                kind
            );
            assert_eq!(
                h.app.editor.document.model().opening_types[&id]
                    .parameters
                    .width,
                1.3
            );
            assert_eq!(
                h.app.editor.document.history_stats().undo_entries,
                history.undo_entries + 1
            );
            assert!(h.app.opening_type_library_dialog.is_none());
            h.app.history(false);
            assert_eq!(h.app.editor.document.model(), &before);
            h.app.history(true);
            assert_eq!(
                h.app.editor.document.model().opening_types[&id]
                    .parameters
                    .kind,
                kind
            );
            assert_eq!(h.ctx.pixels_per_point(), scale);
        }
    }
}
