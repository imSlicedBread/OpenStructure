//! Reproducible visual fixture. Run with a new native output path.
use os_core::Point2;
use os_document::Command;
use os_model::WallParams;
use os_plugin_api::Request;
use os_ui::Editor;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("Provide a new .osb output path")?;
    if Path::new(&path).exists() {
        return Err("Output exists; choose a new path".into());
    }
    let mut editor = Editor::new()?;
    editor.command(
        "Name fixture",
        Command::RenameProject("Depth-buffer crossing walls".into()),
    )?;
    let level = *editor.document.model().levels.keys().next().unwrap();
    for (name, start, end, height) in [
        (
            "East-west wall",
            Point2::new(-4., 0.),
            Point2::new(4., 0.),
            3.,
        ),
        (
            "North-south wall",
            Point2::new(0., -4.),
            Point2::new(0., 4.),
            4.,
        ),
        (
            "Diagonal wall",
            Point2::new(-3., -3.),
            Point2::new(3., 3.),
            2.,
        ),
    ] {
        editor.wall_command(
            "Create crossing wall",
            Request::CreateWall(WallParams {
                name: name.into(),
                start,
                end,
                height,
                thickness: 0.5,
                level,
                material: None,
            }),
        )?;
    }
    editor.save(Path::new(&path))?;
    println!("Created {path}: three intersecting walls, no boolean joins.");
    Ok(())
}
