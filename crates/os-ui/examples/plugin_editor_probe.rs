//! Installed plugin -> descriptor -> Editor -> worker -> scene/history/save.
use os_core::ensure;
use os_plugin_api::{Permission, generic::Mode};
use os_storage::{StorageBackend, ZipJsonStorage};
use os_ui::{
    Editor,
    plugin_tools::{ToolContext, ToolDraft},
};
use serde_json::json;
use std::{
    path::Path,
    time::{Duration, Instant},
};

fn drain(editor: &mut Editor, context: ToolContext) -> Result<(), Box<dyn std::error::Error>> {
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        let report = editor.poll_plugin_work(context, None)?;
        ensure(
            report.errors.is_empty(),
            format!("plugin errors: {:?}", report.errors),
        )?;
        if !report.busy {
            return Ok(());
        }
        ensure(Instant::now() < until, "editor jobs failed to settle")?;
        std::thread::park_timeout(Duration::from_millis(1));
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    ensure(
        args.len() == 1,
        "usage: plugin_editor_probe <installed-plugin-directory>",
    )?;
    let mut editor = Editor::new()?;
    // Explicit test-owned replacement, never automatic plugin execution from files.
    editor.host.unload(os_plugin_api::wall::OWNER)?;
    let mut loading = editor.host.start_wasm_load(
        Path::new(&args[0]).into(),
        [
            Permission::ModelRead,
            Permission::ModelWrite,
            Permission::UiTool,
        ]
        .into(),
        Duration::from_secs(10),
    )?;
    let until = Instant::now() + Duration::from_secs(10);
    while editor.host.poll_wasm_load(&mut loading)?.is_none() {
        ensure(
            Instant::now() < until,
            "background installation did not settle",
        )?;
        std::thread::park_timeout(Duration::from_millis(1));
    }
    let owner = editor
        .host
        .manifests()
        .next()
        .ok_or("missing plugin")?
        .id
        .clone();
    let catalog = editor.host.catalog(&owner).ok_or("missing catalog")?;
    let create = catalog
        .commands
        .iter()
        .find(|c| c.mode == Mode::Create)
        .unwrap()
        .id
        .clone();
    let edit = catalog
        .commands
        .iter()
        .find(|c| c.mode == Mode::Edit)
        .unwrap()
        .id
        .clone();
    let context = ToolContext {
        level: *editor.document.model().levels.keys().next().unwrap(),
        selection: None,
    };
    let draft = ToolDraft::begin(&editor.host, &editor.document, &owner, &create, context)?;
    editor.start_plugin_tool(&draft, context, None)?;
    ensure(editor.plugin_work_pending(), "no pending tool")?;
    let temporary = tempfile::tempdir()?;
    let path = temporary.path().join("pending.osb");
    ensure(
        editor.save(&path).is_err() && !path.exists(),
        "save accepted a pending tool",
    )?;
    drain(&mut editor, context)?;
    ensure(
        editor.scene.len() == 1,
        "editor did not regenerate created element",
    )?;
    let id = *editor.scene.keys().next().unwrap();
    let original = editor.document.model().clone();
    let volume = editor.scene[&id].signed_volume();
    let context = ToolContext {
        selection: Some(id),
        ..context
    };
    let mut draft = ToolDraft::begin(&editor.host, &editor.document, &owner, &edit, context)?;
    draft.set("height", json!(6.0))?;
    editor.start_plugin_tool(&draft, context, None)?;
    drain(&mut editor, context)?;
    ensure(
        (editor.scene[&id].signed_volume() - volume * 2.0).abs() < 1e-8,
        "scene did not track edited height",
    )?;
    editor.undo()?;
    ensure(
        !editor.scene.contains_key(&id),
        "undo left stale plugin mesh visible",
    )?;
    drain(&mut editor, context)?;
    ensure(
        editor.document.model() == &original,
        "undo failed to restore model",
    )?;
    ensure(
        (editor.scene[&id].signed_volume() - volume).abs() < 1e-8,
        "undo geometry not restored",
    )?;
    let draft = ToolDraft::begin(&editor.host, &editor.document, &owner, &edit, context)?;
    editor.start_plugin_tool(&draft, context, None)?;
    editor.cancel_plugin_tool();
    let report = editor.poll_plugin_work(context, None)?;
    ensure(
        !report.committed && !report.errors.is_empty(),
        "cancelled tool accepted",
    )?;
    ensure(
        editor.document.model() == &original,
        "cancel changed document",
    )?;
    drain(&mut editor, context)?;
    editor.save(&path)?;
    ensure(!editor.is_dirty(), "save did not update dirty state")?;
    ensure(
        ZipJsonStorage.open(&path)?.model() == editor.document.model(),
        "saved model mismatch",
    )?;
    let session = editor.document.session_id();
    editor.start_plugin_open(&path, Duration::from_secs(10))?;
    ensure(
        editor.poll_plugin_open()?.is_none(),
        "external open skipped workers",
    )?;
    ensure(
        editor.document.session_id() == session && editor.scene.contains_key(&id),
        "staging replaced current work",
    )?;
    ensure(
        editor.cancel_plugin_open(),
        "cancel did not discard candidate",
    )?;
    ensure(
        !editor.cancel_plugin_open() && editor.document.session_id() == session,
        "cancel changed current work",
    )?;
    editor.start_plugin_open(&path, Duration::from_secs(10))?;
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(opened) = editor.poll_plugin_open()? {
            ensure(
                opened.path == path && opened.unavailable.is_empty(),
                "incorrect open result",
            )?;
            break;
        }
        ensure(Instant::now() < until, "staged open did not settle")?;
        ensure(
            editor.document.session_id() == session,
            "candidate replaced document early",
        )?;
        std::thread::park_timeout(Duration::from_millis(1));
    }
    ensure(
        editor.document.session_id() != session && !editor.is_dirty(),
        "open did not establish fresh saved session",
    )?;
    ensure(
        editor.document.model() == &original,
        "staged reopen changed model",
    )?;
    let report = editor.poll_plugin_work(context, None)?;
    ensure(
        !report.busy && report.errors.is_empty(),
        "opened geometry was immediately invalidated",
    )?;
    ensure(
        (editor.scene[&id].signed_volume() - volume).abs() < 1e-8,
        "staged geometry mismatch",
    )?;
    editor.host.unload(&owner)?;
    editor.poll_plugin_work(context, None)?;
    ensure(
        !editor.scene.contains_key(&id),
        "unload retained managed mesh",
    )?;
    ensure(editor.document.model() == &original, "unload changed model")?;
    println!(
        "PASS: {owner} Editor tool commit, asynchronous scene regeneration, height edit/undo, pending-save protection, cancellation, staged worker reopen and unload invalidation. Desktop controls not activated."
    );
    Ok(())
}
