//! Synthetic snapshot cost probe; timings are observations, not pass/fail budgets.
use os_core::Point2;
use os_document::{Command, Document, HistoryLimits};
use os_model::{Model, Wall, WallParams};
use std::{hint::black_box, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    let reference = arguments.as_slice() == ["--unbounded-reference"];
    if !arguments.is_empty() && !reference {
        return Err("usage: history_cost [--unbounded-reference]".into());
    }
    eprintln!(
        "Retention: {}",
        if reference {
            "unbounded reference (20 edits only)"
        } else {
            "default bounded policy"
        }
    );
    println!(
        "walls,edits,model_estimated_bytes,retained_history_estimate,undo_entries,evicted_entries,clone_us,edit_total_us"
    );
    for count in [100, 1_000, 10_000] {
        let mut model = Model::new("History cost");
        let level = *model.levels.keys().next().unwrap();
        for index in 0..count {
            let x = (index % 100) as f64 * 6.0;
            let y = (index / 100) as f64 * 4.0;
            let wall = Wall::new(
                "org.openstructure.walls.wall",
                WallParams {
                    name: "Wall".into(),
                    start: Point2::new(x, y),
                    end: Point2::new(x + 5.0, y),
                    thickness: 0.2,
                    height: 3.0,
                    level,
                    material: None,
                },
            );
            model.walls.insert(wall.id(), wall);
        }
        let bytes = model.estimated_memory_bytes();
        let start = Instant::now();
        for _ in 0..20 {
            black_box(model.clone());
        }
        let clone_us = start.elapsed().as_micros() / 20;
        let mut document = Document::from_model(model)?;
        if reference {
            document.set_history_limits(HistoryLimits {
                max_entries: usize::MAX,
                max_estimated_bytes: usize::MAX,
            })?;
        }
        let start = Instant::now();
        for index in 0..20 {
            document.execute(
                "Rename",
                vec![Command::RenameProject(format!("Rename {index}"))],
            )?;
            document.drain_events();
        }
        let elapsed = start.elapsed().as_micros();
        let stats = document.history_stats();
        println!(
            "{count},20,{bytes},{},{},{},{clone_us},{elapsed}",
            stats.estimated_bytes, stats.undo_entries, stats.evicted_entries
        );
    }
    Ok(())
}
