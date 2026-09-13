//! OpenStructure desktop entrypoint and deterministic integration smoke command.
use os_core::{Point2, Result};
use os_document::Command;
use os_model::{Level, LevelParams, WallParams};
use os_plugin_api::Request;
use os_ui::{DesktopApp, Editor};
use std::path::Path;

fn main() {
    if let Err(error) = run() {
        eprintln!("OpenStructure: {error}");
        std::process::exit(1);
    }
}
fn run() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args
        .first()
        .is_some_and(|a| a == "--export-ifc" || a == "--import-ifc")
    {
        return exchange(&args);
    }
    if args.first().is_some_and(|arg| arg == "--smoke") {
        let path = args
            .get(1)
            .ok_or("Usage: os-app --smoke <new-output.osb>")?;
        if Path::new(path).exists() {
            return Err("Smoke output already exists; choose a new path.".into());
        }
        smoke(Path::new(path))?;
        println!(
            "PASS: plugin load, wall edits, level reassignment, regeneration, undo/redo, save/reopen. Experimental IFC4 wall exchange available in File and CLI."
        );
        return Ok(());
    }
    if args.first().is_some_and(|arg| arg == "--help") {
        println!(
            "OpenStructure\n  os-app [project.osb]\n  os-app --smoke <new-output.osb>\n  os-app --export-ifc <input.osb> <new-output.ifc> [--allow-loss]\n  os-app --import-ifc <input.ifc> <new-output.osb>\nExperimental restricted IFC4 walls only. Outputs never overwrite existing files."
        );
        return Ok(());
    }
    if args.len() > 1 {
        return Err("Expected at most one project path. Use --help.".into());
    }
    let mut app = DesktopApp::new()?;
    if let Some(path) = args.first() {
        app.open_path(Path::new(path))?;
    }
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([1000.0, 650.0]),
        ..Default::default()
    };
    eframe::run_native(
        "OpenStructure",
        options,
        Box::new(|cc| {
            os_ui::theme::apply(&cc.egui_ctx);
            Ok(Box::new(app))
        }),
    )?;
    Ok(())
}

fn exchange(args: &[String]) -> std::result::Result<(), Box<dyn std::error::Error>> {
    use os_storage::{StorageBackend, ZipJsonStorage};
    use std::io::{Read, Write};
    let export = args[0] == "--export-ifc";
    let allow_loss = export && args.len() == 4 && args[3] == "--allow-loss";
    if args.len() != 3 && !allow_loss {
        return Err("Use --help for IFC exchange command syntax.".into());
    }
    let source = Path::new(&args[1]);
    let destination = Path::new(&args[2]);
    let expected = if export { "ifc" } else { "osb" };
    if !destination
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case(expected))
    {
        return Err(format!("Output must have .{expected} extension.").into());
    }
    if destination.exists() {
        return Err("Output already exists; choose a new path.".into());
    }
    let parent = destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let data = if export {
        let document = ZipJsonStorage.open(source)?;
        let report = os_ifc::WallIfc.export_report_with_files(
            document.model(),
            document.auxiliary_files().keys().map(String::as_str),
        )?;
        for warning in &report.warnings {
            eprintln!("IFC warning: {warning}");
        }
        if !report.warnings.is_empty() && !allow_loss {
            return Err("Export not written. Use --allow-loss to acknowledge the warnings; keep the native .osb original.".into());
        }
        report.value
    } else {
        let mut bytes = Vec::new();
        std::fs::File::open(source)?
            .take(16 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        let report = os_ifc::WallIfc.import_report(&bytes)?;
        for warning in &report.warnings {
            eprintln!("IFC warning: {warning}");
        }
        let document = os_document::Document::from_model(report.value)?;
        // Verify native geometry before emitting an editable project.
        for wall in document.model().walls.values() {
            use os_geometry::GeometryKernel;
            let elevation = document.model().levels[&wall.parameters.level]
                .parameters
                .elevation;
            os_geometry::PrismKernel
                .tessellate(&os_walls::wall_solid(&wall.parameters, elevation)?)?;
        }
        let temporary = tempfile::tempdir_in(parent)?;
        let path = temporary.path().join("import.osb");
        ZipJsonStorage.save(&document, &path)?;
        std::fs::read(path)?
    };
    let mut output = tempfile::NamedTempFile::new_in(parent)?;
    output.write_all(&data)?;
    output.as_file_mut().sync_all()?;
    output.persist_noclobber(destination)?;
    println!(
        "Wrote {} (experimental IFC4 wall subset).",
        destination.display()
    );
    Ok(())
}
fn smoke(path: &Path) -> Result<()> {
    let mut editor = Editor::new()?;
    let level = *editor.document.model().levels.keys().next().unwrap();
    let wall = WallParams {
        name: "Example wall".into(),
        start: Point2::new(0.0, 0.0),
        end: Point2::new(5.0, 0.0),
        thickness: 0.2,
        height: 3.0,
        level,
        material: None,
    };
    editor.wall_command("Create wall", Request::CreateWall(wall))?;
    let id = *editor.document.model().walls.keys().next().unwrap();
    let building = *editor.document.model().buildings.keys().next().unwrap();
    let upper = Level::new(
        "core.level",
        LevelParams {
            name: "Upper".into(),
            elevation: 3.0,
            building,
        },
    );
    let upper_id = upper.id();
    editor.command("Add upper level", Command::AddLevel(upper))?;
    let mut parameters = editor.document.model().walls[&id]
        .parameters
        .with_length(7.0)?;
    parameters.thickness = 0.3;
    parameters.height = 3.5;
    parameters.level = upper_id;
    editor.wall_command("Edit wall", Request::EditWall { id, parameters })?;
    let edited = editor.document.model().clone();
    let mesh = editor.scene[&id].clone();
    os_core::ensure(
        (mesh.signed_volume() - 7.35).abs() < 1e-8,
        "wrong regenerated volume",
    )?;
    editor.undo()?;
    os_core::ensure(
        editor.document.model().walls[&id].parameters.length() == 5.0,
        "undo failed",
    )?;
    editor.redo()?;
    os_core::ensure(
        editor.document.model() == &edited && editor.scene[&id] == mesh,
        "redo failed",
    )?;
    editor.save(path)?;
    let mut reopened = Editor::new()?;
    reopened.open(path)?;
    os_core::ensure(
        reopened.document.model() == &edited && reopened.scene[&id] == mesh,
        "save/reopen changed model or geometry",
    )?;
    Ok(())
}
