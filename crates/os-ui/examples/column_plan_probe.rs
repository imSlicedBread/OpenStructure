//! Build this host before compiling/installing the independent column plan guest.
use os_core::{Id, ensure};
use os_plugin_host::{
    generic::{Invocation, Scope},
    plan_graphics::{PlanGraphics, PlanRequest},
};
use os_storage::{StorageBackend, ZipJsonStorage};
use os_ui::{Editor, plan_providers::PlanProviderBatch};
use std::time::{Duration, Instant};
const OWNER: &str = "org.example.columns";

fn graphics(
    editor: &mut Editor,
    view: Id,
    element: Id,
) -> Result<PlanGraphics, Box<dyn std::error::Error>> {
    let level = editor.document.model().views[&view]
        .parameters
        .level
        .unwrap();
    let mut batch = PlanProviderBatch::begin(
        editor,
        view,
        vec![(
            OWNER.into(),
            PlanRequest {
                provider: format!("{OWNER}.plan"),
                element,
                view,
                read: [element, level].into(),
                timeout: Duration::from_secs(5),
            },
        )],
        Duration::from_secs(5),
    )?;
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        if batch.poll(editor, Some(view))? {
            let mut output = batch.into_results(editor, Some(view))?;
            ensure(
                output.failures.is_empty() && output.graphics.len() == 1,
                "plan batch failed",
            )?;
            return Ok(output.graphics.pop().unwrap());
        }
        ensure(Instant::now() < until, "plan worker failed to settle")?;
        std::thread::yield_now();
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    ensure(
        args.len() == 1,
        "usage: column_plan_probe <installed-directory>",
    )?;
    let mut editor = Editor::new()?;
    editor.host.load_wasm_directory(
        std::path::Path::new(&args[0]),
        [
            os_plugin_api::Permission::ModelRead,
            os_plugin_api::Permission::ModelWrite,
            os_plugin_api::Permission::UiTool,
        ]
        .into(),
    )?;
    let level = *editor.document.model().levels.keys().next().unwrap();
    let view = editor.create_floor_plan("Independent column plan", level)?;
    editor.host.execute_generic(
        OWNER,
        &mut editor.document,
        "Create independent column",
        Invocation {
            command_id: format!("{OWNER}.create"),
            inputs: [
                ("width".into(), serde_json::json!(0.4)),
                ("depth".into(), serde_json::json!(0.6)),
                ("height".into(), serde_json::json!(3.0)),
            ]
            .into(),
            scope: Scope {
                read: [level].into(),
                write: Default::default(),
                create_count: 1,
            },
            timeout: Duration::from_secs(5),
        },
    )?;
    let element = *editor.document.model().extensions.keys().next().unwrap();
    let before = editor.document.model().clone();
    let revision = editor.document.revision();
    let raw = graphics(&mut editor, view, element)?;
    let prepared = editor.prepare_provider_plan(view, vec![raw])?;
    let composed = std::thread::spawn(move || prepared.derive())
        .join()
        .map_err(|_| "plan drawing worker panicked")??;
    let context = editor.native_plan_context(view)?;
    let drawing = composed.drawing(&editor, view)?;
    let lines = drawing.provider_lines(context)?;
    ensure(
        lines.len() == 4
            && lines
                .iter()
                .all(|l| l.role == os_geometry::plan::PlanRole::Cut),
        "column cut outline incorrect",
    )?;
    ensure(
        lines[0].start == os_core::Point2::new(0.0, 0.0)
            && lines[0].end == os_core::Point2::new(0.4, 0.0),
        "column outline coordinates incorrect",
    )?;
    ensure(
        drawing.unavailable(context)?.is_empty(),
        "column stayed unavailable",
    )?;
    ensure(
        editor.document.model() == &before && editor.document.revision() == revision,
        "plan query changed model",
    )?;
    let mut settings = editor.document.model().views[&view]
        .parameters
        .plan
        .unwrap();
    settings.range = os_model::PlanViewRange {
        top: 5.0,
        cut: 4.0,
        bottom: 2.0,
        depth: 0.0,
    };
    editor.update_floor_plan(view, "Projected column", level, settings)?;
    ensure(
        composed.drawing(&editor, view).is_err(),
        "stale drawing survived view change",
    )?;
    let raw = graphics(&mut editor, view, element)?;
    ensure(
        raw.segments(&editor.host, &editor.document, view)?
            .iter()
            .all(|l| l.role == os_plugin_api::plan::Role::Projected),
        "projected role incorrect",
    )?;
    let temp = tempfile::tempdir()?;
    let request = || PlanRequest {
        provider: format!("{OWNER}.plan"),
        element,
        view,
        read: [element, level].into(),
        timeout: Duration::from_secs(5),
    };
    let mut cancelled = PlanProviderBatch::begin(
        &editor,
        view,
        vec![(OWNER.into(), request())],
        Duration::from_secs(5),
    )?;
    ensure(
        !cancelled.poll(&mut editor, Some(view))?,
        "job unexpectedly completed on admission",
    )?;
    cancelled.cancel();
    ensure(
        cancelled.poll(&mut editor, Some(view)).is_err(),
        "cancelled batch revived",
    )?;
    let until = Instant::now() + Duration::from_secs(5);
    while editor.host.active_jobs() > 0 {
        ensure(Instant::now() < until, "cancelled batch did not drain")?;
        std::thread::yield_now();
    }
    let bad = Id::new();
    let mut batch = PlanProviderBatch::begin(
        &editor,
        view,
        vec![
            (OWNER.into(), request()),
            (
                OWNER.into(),
                PlanRequest {
                    provider: format!("{OWNER}.plan"),
                    element: bad,
                    view,
                    read: [bad, level].into(),
                    timeout: Duration::from_secs(5),
                },
            ),
        ],
        Duration::from_secs(5),
    )?;
    let until = Instant::now() + Duration::from_secs(5);
    while !batch.poll(&mut editor, Some(view))? {
        ensure(Instant::now() < until, "batch did not settle")?;
        std::thread::yield_now();
    }
    let output = batch.into_results(&editor, Some(view))?;
    ensure(
        output.graphics.len() == 1 && output.failures.contains_key(&bad),
        "partial batch lost its failure diagnostic",
    )?;
    let path = temp.path().join("column-plan.osb");
    ZipJsonStorage.save(&editor.document, &path)?;
    editor.host.unload(OWNER)?;
    ensure(
        raw.segments(&editor.host, &editor.document, view).is_err(),
        "unloaded graphics survived",
    )?;
    ensure(
        ZipJsonStorage.open(&path)?.model() == editor.document.model(),
        "missing-provider save/reopen changed data",
    )?;
    println!(
        "PASS: independently installed column create -> bounded plan batch -> Wasm worker -> checked drawing; cut/projected outline, cancellation/drain, partial failure diagnostics, context invalidation, read-only query, unload and native round-trip."
    );
    Ok(())
}
