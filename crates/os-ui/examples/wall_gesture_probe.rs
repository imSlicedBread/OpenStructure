//! Installed guest acceptance for the controller adapter, not native UI evidence.
use os_core::{Point2, ensure};
use os_model::WallParams;
use os_plugin_host::worker::ViewContext;
use os_render::{
    plan::PlanCamera,
    snapping::{SnapKind, SnapQuery},
};
use os_storage::{StorageBackend, ZipJsonStorage};
use os_ui::{Editor, plan_gesture::WallGesture, plugin_tools::ToolContext};
use std::time::{Duration, Instant};

fn settle(
    editor: &mut Editor,
    context: ToolContext,
    view: Option<ViewContext>,
    failed: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let until = Instant::now() + Duration::from_secs(10);
    let mut errors = vec![];
    loop {
        let report = editor.poll_plugin_work(context, view)?;
        errors.extend(report.errors);
        if !report.busy && editor.host.worker_available(os_plugin_api::wall::OWNER) {
            break;
        }
        ensure(Instant::now() < until, "Wall worker did not drain")?;
        std::thread::yield_now();
    }
    ensure(
        failed != errors.is_empty(),
        format!("unexpected worker errors: {errors:?}"),
    )?;
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    ensure(
        args.len() == 1,
        "usage: wall_gesture_probe <installed-directory>",
    )?;
    let mut editor = Editor::new()?;
    editor.host.unload(os_plugin_api::wall::OWNER)?;
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
    let view = editor.create_floor_plan("Installed pointer plan", level)?;
    let mut settings = editor.document.model().views[&view]
        .parameters
        .plan
        .unwrap();
    settings.basis.origin = Point2::new(1000.0, -2000.0);
    settings.basis.rotation = 0.37;
    editor.update_floor_plan(view, "Installed pointer plan", level, settings)?;
    let p = WallParams {
        name: "Wall".into(),
        start: Point2::default(),
        end: Point2::new(5.0, 0.0),
        thickness: 0.2,
        height: 3.0,
        level,
        material: None,
    };
    let before = editor.document.model().clone();
    let mut gesture = WallGesture::begin_installed(&editor, view, p.clone())?;
    gesture.start = Some(Point2::default());
    gesture.length = "5".into();
    gesture.angle_degrees = "0".into();
    let expected = gesture.parameters(Point2::default())?;
    ensure(
        gesture
            .commit(&mut editor, Some(view), Point2::default())
            .is_err(),
        "synchronous commit allowed",
    )?;
    let (draft, context, ticket) =
        gesture.installed_command(&editor, Some(view), Point2::default())?;
    ensure(
        *editor.document.model() == before,
        "preview mutated document",
    )?;
    editor.start_plugin_tool(&draft, context, Some(ticket))?;
    settle(&mut editor, context, Some(ticket), false)?;
    let first = editor
        .document
        .model()
        .walls
        .values()
        .next()
        .unwrap()
        .clone();
    ensure(
        first.parameters.start == expected.start && first.parameters.end == expected.end,
        "guest changed exact preview coordinates",
    )?;
    ensure(
        !gesture.current(&editor, Some(view)),
        "committed gesture remained valid",
    )?;

    let c = editor.native_plan_context(view)?;
    let drawing = editor.native_wall_plan(view)?;
    let camera = PlanCamera::default();
    let q = SnapQuery {
        camera,
        viewport: [800.0, 600.0],
        pointer: camera.project(
            c.basis.world_to_plane(first.parameters.end)?,
            [800.0, 600.0],
        )?,
        radius_pixels: 8.0,
        endpoints: true,
        midpoints: false,
        intersections: false,
        perpendicular_from: None,
        nearest: false,
        axis_extensions: false,
        exclude_entity: None,
    };
    let snap = drawing.snap(c, q)?.candidate(c, q)?.unwrap();
    ensure(
        snap.kind == SnapKind::Endpoint && snap.entity == first.id(),
        "wrong semantic snap",
    )?;
    let mut second = WallGesture::begin_installed(&editor, view, p.clone())?;
    second.start = Some(snap.point);
    second.length = "3".into();
    second.angle_degrees = "90".into();
    let (draft, context, ticket) = second.installed_command(&editor, Some(view), snap.point)?;
    editor.start_plugin_tool(&draft, context, Some(ticket))?;
    settle(&mut editor, context, Some(ticket), false)?;
    ensure(
        editor.document.model().walls.len() == 2,
        "second gesture did not create one wall",
    )?;
    let two = editor.document.model().clone();
    editor.undo()?;
    ensure(
        editor.document.model().walls.len() == 1,
        "gesture was not one undo step",
    )?;
    editor.redo()?;
    ensure(
        *editor.document.model() == two,
        "redo changed wall identity",
    )?;
    settle(&mut editor, context, Some(ticket), false)?;

    for mode in [
        os_ui::plan_gesture::WallEdit::Move,
        os_ui::plan_gesture::WallEdit::ResizeStart,
        os_ui::plan_gesture::WallEdit::ResizeEnd,
        os_ui::plan_gesture::WallEdit::OffsetCopy,
    ] {
        let before = editor.document.model().clone();
        let mut edit = WallGesture::begin_edit_installed(&editor, view, first.id(), mode)?;
        if mode == os_ui::plan_gesture::WallEdit::Move {
            edit.start = Some(Point2::default());
        }
        edit.length = if mode == os_ui::plan_gesture::WallEdit::OffsetCopy {
            "-2"
        } else {
            "2"
        }
        .into();
        if mode != os_ui::plan_gesture::WallEdit::OffsetCopy {
            edit.angle_degrees = "35".into();
        }
        let expected = edit.parameters(Point2::default())?;
        let (draft, context, ticket) =
            edit.installed_command(&editor, Some(view), Point2::default())?;
        let invocation = draft.invocation(
            &editor.host,
            &editor.document,
            context,
            Duration::from_secs(5),
        )?;
        let copying = mode == os_ui::plan_gesture::WallEdit::OffsetCopy;
        ensure(
            invocation.scope.create_count == usize::from(copying)
                && invocation.scope.write
                    == if copying {
                        Default::default()
                    } else {
                        [first.id()].into()
                    },
            "wrong edit/copy scope",
        )?;
        editor.start_plugin_tool(&draft, context, Some(ticket))?;
        settle(&mut editor, context, Some(ticket), false)?;
        let actual = if copying {
            ensure(
                editor.document.model().walls[&first.id()] == before.walls[&first.id()],
                "copy changed its source",
            )?;
            editor
                .document
                .model()
                .walls
                .values()
                .find(|wall| !before.walls.contains_key(&wall.id()))
                .unwrap()
        } else {
            &editor.document.model().walls[&first.id()]
        };
        ensure(
            actual.parameters.start == expected.start
                && actual.parameters.end == expected.end
                && actual.parameters.level == expected.level
                && actual.parameters.height == expected.height
                && actual.parameters.thickness == expected.thickness,
            "installed edit changed preview parameters",
        )?;
        ensure(
            editor.document.model().walls.len() == before.walls.len() + usize::from(copying),
            "unexpected edit count",
        )?;
        editor.undo()?;
        settle(&mut editor, context, Some(ticket), false)?;
        ensure(
            *editor.document.model() == before,
            "edit/copy was not one undo step",
        )?;
    }

    let mut cancelled = WallGesture::begin_installed(&editor, view, p)?;
    cancelled.start = Some(Point2::default());
    cancelled.length = "NaN".into();
    cancelled.angle_degrees = "0".into();
    ensure(
        cancelled
            .installed_command(&editor, Some(view), Point2::default())
            .is_err(),
        "invalid draft admitted",
    )?;
    cancelled.length = "4".into();
    let (draft, context, ticket) =
        cancelled.installed_command(&editor, Some(view), Point2::default())?;
    editor.start_plugin_tool(&draft, context, Some(ticket))?;
    editor.cancel_plugin_tool();
    settle(&mut editor, context, Some(ticket), true)?;
    ensure(
        *editor.document.model() == two,
        "cancelled gesture committed",
    )?;
    editor.start_plugin_tool(&draft, context, Some(ticket))?;
    settle(&mut editor, context, None, true)?;
    ensure(
        *editor.document.model() == two,
        "inactive-view result committed",
    )?;
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("walls.osb");
    editor.save(&file)?;
    ensure(
        ZipJsonStorage.open(&file)?.model() == &two,
        "round-trip changed model",
    )?;
    editor.host.unload(os_plugin_api::wall::OWNER)?;
    ensure(
        !cancelled.current(&editor, Some(view)),
        "unloaded gesture remained current",
    )?;
    println!(
        "PASS: installed Wall exact creation, semantic snaps, move/resize/offset worker scopes and preview agreement; one-step history, invalid input, cancellation, inactive-view rejection, unload and native round-trip. Controller only."
    );
    Ok(())
}
