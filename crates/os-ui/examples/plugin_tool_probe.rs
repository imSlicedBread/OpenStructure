//! Exercise descriptor drafts with installed Wasm, without desktop widget glue.
use os_core::ensure;
use os_document::Document;
use os_plugin_api::{Permission, generic::Mode};
use os_plugin_host::{PluginHost, worker::JobOutcome};
use os_ui::plugin_tools::{ToolContext, ToolDraft};
use serde_json::json;
use std::{path::Path, time::Duration};

fn apply(
    host: &PluginHost,
    document: &mut Document,
    draft: &ToolDraft,
    context: ToolContext,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut job = draft.start(host, document, context, None, Duration::from_secs(5))?;
    loop {
        if let Some(result) = host.poll_job(&mut job, document, None, "Descriptor tool")? {
            ensure(
                matches!(result, JobOutcome::Committed),
                "expected command commit",
            )?;
            return Ok(());
        }
        std::thread::park_timeout(Duration::from_millis(1));
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    ensure(
        args.len() == 1,
        "usage: plugin_tool_probe <installed-plugin-directory>",
    )?;
    let mut host = PluginHost::default();
    host.load_wasm_directory(
        Path::new(&args[0]),
        [
            Permission::ModelRead,
            Permission::ModelWrite,
            Permission::UiTool,
        ]
        .into(),
    )?;
    let owner = host
        .manifests()
        .next()
        .ok_or("missing manifest")?
        .id
        .clone();
    let catalog = host.catalog(&owner).ok_or("missing catalog")?;
    let create = catalog
        .commands
        .iter()
        .find(|c| c.mode == Mode::Create)
        .ok_or("missing create tool")?
        .id
        .clone();
    let edit = catalog
        .commands
        .iter()
        .find(|c| c.mode == Mode::Edit)
        .ok_or("missing edit tool")?
        .id
        .clone();
    let mut doc = Document::new("Descriptor draft")?;
    let context = ToolContext {
        level: *doc.model().levels.keys().next().unwrap(),
        selection: None,
    };
    let draft = ToolDraft::begin(&host, &doc, &owner, &create, context)?;
    apply(&host, &mut doc, &draft, context)?;
    let id = *doc
        .model()
        .walls
        .keys()
        .chain(doc.model().extensions.keys())
        .next()
        .ok_or("tool created no element")?;
    let context = ToolContext {
        selection: Some(id),
        ..context
    };
    let mut draft = ToolDraft::begin(&host, &doc, &owner, &edit, context)?;
    ensure(
        draft.values().get("height") == Some(&json!(3.0)),
        "edit draft did not read committed height",
    )?;
    draft.set("height", json!(-1.0))?;
    ensure(
        draft
            .start(&host, &doc, context, None, Duration::from_secs(5))
            .is_err(),
        "invalid draft dispatched",
    )?;
    ensure(doc.revision() == 1, "draft mutated history")?;
    draft.set("height", json!(4.0))?;
    apply(&host, &mut doc, &draft, context)?;
    let refreshed = ToolDraft::begin(&host, &doc, &owner, &edit, context)?;
    ensure(
        refreshed.values().get("height") == Some(&json!(4.0)),
        "edit was not committed",
    )?;
    ensure(doc.undo(), "missing undo")?;
    ensure(
        refreshed
            .start(&host, &doc, context, None, Duration::from_secs(5))
            .is_err(),
        "stale draft dispatched",
    )?;
    println!(
        "PASS: descriptor-driven {owner} create/edit drafts, typed rejection, worker commit, selected-value prefill and undo invalidation. No desktop widgets activated."
    );
    Ok(())
}
