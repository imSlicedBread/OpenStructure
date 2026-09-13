//! Generate an independently inspectable two-storey rotated-wall fixture.
use os_core::Point2;
use os_model::{Level, LevelParams, Model, Wall, WallParams};
use std::io::Write;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("Provide a new output .ifc path")?;
    let mut m = Model::new("Rotated walls — étage");
    let ground = *m.levels.keys().next().unwrap();
    let building = *m.buildings.keys().next().unwrap();
    let upper = Level::new(
        "core.level",
        LevelParams {
            name: "Upper".into(),
            elevation: 4.2,
            building,
        },
    );
    let up = upper.id();
    m.levels.insert(up, upper);
    for (level, start, end) in [
        (ground, Point2::new(-2., 1.), Point2::new(2., 4.)),
        (up, Point2::new(8., -3.), Point2::new(5., -7.)),
    ] {
        let wall = Wall::new(
            "org.openstructure.walls.wall",
            WallParams {
                name: "Rotated wall".into(),
                start,
                end,
                thickness: 0.3,
                height: 3.5,
                level,
                material: None,
            },
        );
        m.walls.insert(wall.id(), wall);
    }
    let report = os_ifc::WallIfc.export_report(&m)?;
    for warning in report.warnings {
        eprintln!("{warning}");
    }
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?
        .write_all(&report.value)?;
    Ok(())
}
