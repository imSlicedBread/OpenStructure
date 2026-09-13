//! Explicit developer-owned migration UI harness; no production auto-loading.
use os_plugin_api::Permission;
use os_storage::StorageBackend;
use std::path::PathBuf;
pub fn run(mixed: bool) -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut args = std::env::args().skip(1);
    let plugin = args.next().map(PathBuf::from).unwrap_or_else(|| {
        root.join(if mixed {
            "outputs/rust-column-mixed-install"
        } else {
            "outputs/rust-column-v2-install"
        })
    });
    let source = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("outputs/native-column-acceptance.osb"));
    let mut app = os_ui::DesktopApp::new()?;
    app.editor.host.load_wasm_directory(
        &plugin,
        [
            Permission::ModelRead,
            Permission::ModelWrite,
            Permission::UiTool,
        ]
        .into(),
    )?;
    // Import only into this disposable harness; never overwrite the source file.
    let mut model = os_storage::ZipJsonStorage.open(&source)?.model().clone();
    let template = model
        .extensions
        .values()
        .next()
        .ok_or("source must contain a v1 column")?
        .clone();
    os_core::ensure(
        template.owner == "org.example.columns" && template.payload_schema_version == 1,
        "source must contain a v1 column",
    )?;
    if mixed {
        os_core::ensure(
            model.extensions.len() <= 8,
            "mixed source must have at most eight columns",
        )?;
    }
    while model.extensions.len() < if mixed { 8 } else { 33 } {
        let mut entity = template.clone();
        entity.id = os_core::Id::new();
        model.extensions.insert(entity.id, entity);
    }
    if mixed {
        for _ in 0..5 {
            let mut entity = template.clone();
            entity.id = os_core::Id::new();
            entity.type_id = "org.example.columns.test-block".into();
            entity.name = "Migration test block".into();
            model.extensions.insert(entity.id, entity);
        }
    }
    app.editor.document = os_document::Document::from_model(model)?;
    app.editor.regenerate()?;
    eframe::run_native(
        if mixed {
            "OpenStructure — Mixed migration acceptance"
        } else {
            "OpenStructure — Migration acceptance"
        },
        eframe::NativeOptions {
            viewport: eframe::egui::ViewportBuilder::default().with_inner_size([1280.0, 800.0]),
            ..Default::default()
        },
        Box::new(|cc| {
            os_ui::theme::apply(&cc.egui_ctx);
            Ok(Box::new(app))
        }),
    )?;
    Ok(())
}
