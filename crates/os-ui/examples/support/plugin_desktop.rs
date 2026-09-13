//! Developer-owned native acceptance harness, not automatic project loading.
use os_plugin_api::Permission;
use std::path::PathBuf;
pub fn run(wall: bool) -> Result<(), Box<dyn std::error::Error>> {
    let mut app = os_ui::DesktopApp::new()?;
    let path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(if wall {
                "../../outputs/rust-wall-install"
            } else {
                "../../outputs/rust-column-geometry-install"
            })
        });
    // Explicitly replace the bundled provider only in this acceptance harness.
    app.editor.host.unload(os_plugin_api::wall::OWNER)?;
    app.editor.host.load_wasm_directory(
        &path,
        [
            Permission::ModelRead,
            Permission::ModelWrite,
            Permission::UiTool,
        ]
        .into(),
    )?;
    eframe::run_native(
        if wall {
            "OpenStructure — Independent Wall acceptance"
        } else {
            "OpenStructure — Plugin form acceptance"
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
