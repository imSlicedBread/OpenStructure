//! Installed v1 -> separately compiled v2 -> migration workers -> history/storage.
use os_core::ensure;
use os_document::Command;
use os_plugin_api::Permission;
use os_storage::{StorageBackend, ZipJsonStorage};
use os_ui::{
    Editor,
    plugin_tools::{ToolContext, ToolDraft},
};
use std::{
    path::Path,
    time::{Duration, Instant},
};
const OWNER: &str = "org.example.columns";
fn load(editor: &mut Editor, path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut ticket = editor.host.start_wasm_load(
        Path::new(path).into(),
        [
            Permission::ModelRead,
            Permission::ModelWrite,
            Permission::UiTool,
        ]
        .into(),
        Duration::from_secs(10),
    )?;
    while editor.host.poll_wasm_load(&mut ticket)?.is_none() {
        std::thread::park_timeout(Duration::from_millis(1));
    }
    Ok(())
}
fn drain(editor: &mut Editor, context: ToolContext) -> Result<(), Box<dyn std::error::Error>> {
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        let report = editor.poll_plugin_work(context, None)?;
        ensure(report.errors.is_empty(), format!("{:?}", report.errors))?;
        if !report.busy {
            return Ok(());
        }
        ensure(Instant::now() < until, "migration workers did not settle")?;
        std::thread::park_timeout(Duration::from_millis(1));
    }
}
fn draft(editor: &Editor, context: ToolContext) -> os_core::Result<ToolDraft> {
    ToolDraft::begin(
        &editor.host,
        &editor.document,
        OWNER,
        "org.example.columns.migrate",
        context,
    )
}
fn batched(
    host: &mut os_plugin_host::PluginHost,
    source: &os_model::Model,
    installed: &str,
) -> os_core::Result<()> {
    use os_document::Document;
    use os_plugin_host::migration::MigrationCommand;
    let mut model = source.clone();
    let template = model.extensions.values().next().unwrap().clone();
    while model.extensions.len() < 33 {
        let mut entity = template.clone();
        entity.id = os_core::Id::new();
        model.extensions.insert(entity.id, entity);
    }
    let plan = || -> std::collections::BTreeMap<String, MigrationCommand> {
        [(
            template.type_id.clone(),
            MigrationCommand {
                command_id: "org.example.columns.migrate".into(),
                inputs: Default::default(),
            },
        )]
        .into()
    };
    let wait = || std::thread::park_timeout(Duration::from_millis(1));
    let untouched = Document::from_model(model.clone())?;
    ensure(
        host.start_migration(
            &untouched,
            OWNER,
            Default::default(),
            Duration::from_secs(10),
        )
        .is_err(),
        "empty reviewed plan accepted",
    )?;
    let mut wrong_command = plan();
    wrong_command.get_mut(&template.type_id).unwrap().command_id =
        "org.example.columns.edit".into();
    ensure(
        host.start_migration(&untouched, OWNER, wrong_command, Duration::from_secs(10))
            .is_err(),
        "ordinary edit accepted as migration plan",
    )?;
    for timeout in [Duration::ZERO, Duration::from_secs(301)] {
        ensure(
            host.start_migration(&untouched, OWNER, plan(), timeout)
                .is_err(),
            "invalid deadline accepted",
        )?;
    }
    for scenario in [
        "cancel",
        "late-reject",
        "stale",
        "reopen",
        "reload",
        "deadline",
        "commit",
    ] {
        let mut original = model.clone();
        if scenario == "late-reject" {
            original.extensions.last_entry().unwrap().get_mut().payload["height_m"] =
                serde_json::json!("preserve vendor value");
        }
        let mut doc = Document::from_model(original.clone())?;
        let revision = doc.revision();
        let timeout = Duration::from_secs(if scenario == "deadline" { 2 } else { 10 });
        let mut session = host.start_migration(&doc, OWNER, plan(), timeout)?;
        loop {
            let completed = session.progress().unwrap().0;
            let result = host.poll_migration(&mut session, &mut doc);
            if scenario == "late-reject" && result.is_err() {
                ensure(completed == 32, "failure occurred before the final batch")?;
                ensure(
                    doc.model() == &original && doc.revision() == revision,
                    "late failure changed live document",
                )?;
                ensure(!doc.undo(), "late failure left undo history")?;
                break;
            }
            if result? {
                ensure(scenario == "commit", "unexpected migration commit")?;
                ensure(
                    doc.revision() == revision + 1,
                    "migration was not one transaction",
                )?;
                ensure(
                    doc.model().plugin_requirements[OWNER].version == "2.0.0",
                    "batch requirement mismatch",
                )?;
                for (id, entity) in &doc.model().extensions {
                    ensure(
                        entity.payload_schema_version == 2
                            && entity.payload["height_m"]
                                == original.extensions[id].payload["height"],
                        "batch conversion mismatch",
                    )?;
                }
                let migrated = doc.model().clone();
                ensure(
                    doc.undo() && doc.model() == &original && !doc.undo(),
                    "batch undo was not atomic",
                )?;
                ensure(
                    doc.redo() && doc.model() == &migrated,
                    "batch redo mismatch",
                )?;
                break;
            }
            ensure(
                doc.model() == &original && doc.revision() == revision,
                "candidate batch leaked into live document",
            )?;
            if session.progress().unwrap().0 >= 16 {
                if ["reopen", "reload", "deadline"].contains(&scenario) {
                    let expected = match scenario {
                        "reopen" => {
                            doc = Document::from_model(original.clone())?;
                            "document changed"
                        }
                        "reload" => {
                            host.unload(OWNER)?;
                            host.load_wasm_directory(
                                Path::new(installed),
                                [
                                    Permission::ModelRead,
                                    Permission::ModelWrite,
                                    Permission::UiTool,
                                ]
                                .into(),
                            )?;
                            "provider changed"
                        }
                        _ => {
                            let expire = Instant::now() + Duration::from_millis(2100);
                            while Instant::now() < expire {
                                std::thread::park_timeout(
                                    expire.saturating_duration_since(Instant::now()),
                                );
                            }
                            "deadline expired"
                        }
                    };
                    let error = host
                        .poll_migration(&mut session, &mut doc)
                        .expect_err("invalidated session committed");
                    ensure(
                        error.to_string().contains(expected),
                        format!("wrong lifecycle rejection: {error}"),
                    )?;
                    ensure(
                        doc.model() == &original && doc.revision() == revision && !doc.undo(),
                        "lifecycle rejection changed live state",
                    )?;
                    ensure(
                        session.progress().is_none()
                            && host.poll_migration(&mut session, &mut doc).is_err(),
                        "failed session retained authority",
                    )?;
                    break;
                }
                if scenario == "cancel" {
                    session.cancel();
                    ensure(
                        host.poll_migration(&mut session, &mut doc).is_err() && !doc.undo(),
                        "cancel retained authority or history",
                    )?;
                    break;
                }
                if scenario == "stale" {
                    doc.execute(
                        "Concurrent edit",
                        vec![Command::RenameProject("Concurrent".into())],
                    )?;
                    ensure(
                        host.poll_migration(&mut session, &mut doc).is_err(),
                        "stale batch accepted",
                    )?;
                    ensure(
                        doc.model().extensions == original.extensions,
                        "stale batch changed extensions",
                    )?;
                    break;
                }
            }
            wait();
        }
        let deadline = Instant::now() + Duration::from_secs(10);
        while host.active_jobs() != 0 {
            ensure(Instant::now() < deadline, "batch worker did not drain")?;
            wait();
        }
    }
    println!(
        "PASS: 33-column staged migration, plan/deadline validation, isolated partial batches, cancellation, late rejection, stale/reopened document, same-version provider reload, expired deadline and single-transaction undo/redo."
    );
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    ensure(
        (2..=3).contains(&args.len()),
        "usage: plugin_migration_probe <installed-v1-column> <installed-v2-column> [installed-mixed-test-guest]",
    )?;
    let mut editor = Editor::new()?;
    load(&mut editor, &args[0])?;
    let context = ToolContext {
        level: *editor.document.model().levels.keys().next().unwrap(),
        selection: None,
    };
    for _ in 0..2 {
        let create = ToolDraft::begin(
            &editor.host,
            &editor.document,
            OWNER,
            "org.example.columns.create",
            context,
        )?;
        editor.start_plugin_tool(&create, context, None)?;
        drain(&mut editor, context)?;
    }
    let id = *editor.document.model().extensions.keys().next().unwrap();
    let volume: f64 = editor.scene.values().map(|m| m.signed_volume()).sum();
    let before = editor.document.model().clone();
    let directory = tempfile::tempdir()?;
    let old_path = directory.path().join("v1.osb");
    editor.save(&old_path)?;
    let original_bytes = std::fs::read(&old_path)?;
    editor.host.unload(OWNER)?;
    load(&mut editor, &args[1])?;
    editor.regenerate()?;
    editor.start_plugin_open(&old_path, Duration::from_secs(10))?;
    let opened = loop {
        if let Some(opened) = editor.poll_plugin_open()? {
            break opened;
        }
        std::thread::park_timeout(Duration::from_millis(1));
    };
    ensure(
        opened.unavailable.len() == 2 && editor.scene.is_empty(),
        "old payloads were not preserved unavailable",
    )?;
    let context = ToolContext {
        selection: Some(id),
        ..context
    };
    editor.start_plugin_tool(&draft(&editor, context)?, context, None)?;
    editor.cancel_plugin_tool();
    ensure(
        !editor.poll_plugin_work(context, None)?.errors.is_empty(),
        "cancel not reported",
    )?;
    ensure(
        editor.document.model() == &before,
        "cancel mutated old data",
    )?;
    let until = Instant::now() + Duration::from_secs(10);
    while editor.host.active_jobs() != 0 {
        ensure(Instant::now() < until, "cancelled worker did not drain")?;
        std::thread::park_timeout(Duration::from_millis(1));
    }
    let mut collision = editor.document.model().extensions[&id].clone();
    collision.payload["height_m"] = serde_json::json!("vendor data must not be overwritten");
    editor.command(
        "Seed conflicting vendor key",
        Command::ReplaceExtension {
            expected_schema_version: 1,
            entity: collision,
        },
    )?;
    let conflicting = editor.document.model().clone();
    editor.start_plugin_tool(&draft(&editor, context)?, context, None)?;
    loop {
        let report = editor.poll_plugin_work(context, None)?;
        if !report.errors.is_empty() {
            break;
        }
        ensure(
            report.busy && Instant::now() < until,
            "expected migration rejection",
        )?;
        std::thread::park_timeout(Duration::from_millis(1));
    }
    ensure(
        editor.document.model() == &conflicting,
        "rejected migration changed original",
    )?;
    editor.undo()?;
    // A document change revokes even a valid prepared migration reply.
    editor.start_plugin_tool(&draft(&editor, context)?, context, None)?;
    editor.command(
        "Concurrent edit",
        Command::RenameProject("Changed during migration".into()),
    )?;
    ensure(
        !editor.poll_plugin_work(context, None)?.errors.is_empty(),
        "stale migration was accepted",
    )?;
    ensure(
        editor.document.model().extensions == before.extensions,
        "stale migration changed payloads",
    )?;
    editor.undo()?;
    while editor.host.active_jobs() != 0 {
        ensure(Instant::now() < until, "stale worker did not drain")?;
        std::thread::park_timeout(Duration::from_millis(1));
    }
    editor.start_plugin_tool(&draft(&editor, context)?, context, None)?;
    drain(&mut editor, context)?;
    let after = editor.document.model().clone();
    ensure(
        after.plugin_requirements[OWNER].version == "2.0.0",
        "requirement not migrated",
    )?;
    for (id, entity) in &after.extensions {
        ensure(
            entity.payload_schema_version == 2
                && entity.payload.get("height").is_none()
                && entity.payload["height_m"] == before.extensions[id].payload["height"],
            "payload conversion failed",
        )?;
    }
    ensure(
        (editor
            .scene
            .values()
            .map(|m| m.signed_volume())
            .sum::<f64>()
            - volume)
            .abs()
            < 1e-9,
        "migration changed geometry",
    )?;
    editor.undo()?;
    ensure(
        editor.document.model() == &before && editor.scene.is_empty(),
        "undo did not restore unavailable v1 data",
    )?;
    editor.redo()?;
    drain(&mut editor, context)?;
    ensure(
        editor.document.model() == &after,
        "redo did not restore migration",
    )?;
    editor.save(&directory.path().join("v2.osb"))?;
    ensure(
        ZipJsonStorage
            .open(&directory.path().join("v2.osb"))?
            .model()
            == &after,
        "migrated reopen mismatch",
    )?;
    ensure(
        std::fs::read(old_path)? == original_bytes,
        "original file was modified",
    )?;
    batched(&mut editor.host, &before, &args[1])?;
    editor.host.unload(OWNER)?;
    editor.regenerate()?;
    editor.save(&directory.path().join("without-plugin.osb"))?;
    ensure(
        ZipJsonStorage
            .open(&directory.path().join("without-plugin.osb"))?
            .model()
            == &after,
        "absent-plugin round trip changed data",
    )?;
    load(&mut editor, &args[1])?;
    let mut many = before.clone();
    let template = many.extensions.values().next().unwrap().clone();
    while many.extensions.len() < 33 {
        let mut entity = template.clone();
        entity.id = os_core::Id::new();
        many.extensions.insert(entity.id, entity);
    }
    editor.document = os_document::Document::from_model(many.clone())?;
    editor.regenerate()?;
    editor.start_plugin_tool(&draft(&editor, context)?, context, None)?;
    loop {
        let report = editor.poll_plugin_work(context, None)?;
        ensure(
            report.errors.is_empty() && !report.committed,
            "editor migration settled prematurely",
        )?;
        ensure(
            editor.document.model() == &many,
            "editor leaked staged data",
        )?;
        if editor.plugin_migration_progress().unwrap().0 >= 16 {
            break;
        }
        std::thread::park_timeout(Duration::from_millis(1));
    }
    // Intent changes must revoke staged authority even after successful batches.
    let changed = ToolContext {
        selection: None,
        ..context
    };
    ensure(
        !editor.poll_plugin_work(changed, None)?.errors.is_empty(),
        "selection change accepted migration",
    )?;
    ensure(
        editor.document.model() == &many,
        "selection change leaked migration",
    )?;
    let view = os_plugin_host::worker::ViewContext {
        id: os_core::Id::new(),
        settings_revision: 0,
    };
    editor.start_plugin_tool(&draft(&editor, context)?, context, Some(view))?;
    loop {
        let report = editor.poll_plugin_work(context, Some(view))?;
        ensure(
            report.errors.is_empty() && !report.committed,
            "view-bound migration settled prematurely",
        )?;
        ensure(
            editor.document.model() == &many,
            "view-bound candidate leaked",
        )?;
        if editor.plugin_migration_progress().unwrap().0 >= 16 {
            break;
        }
        std::thread::park_timeout(Duration::from_millis(1));
    }
    let changed_view = os_plugin_host::worker::ViewContext {
        settings_revision: 1,
        ..view
    };
    ensure(
        !editor
            .poll_plugin_work(context, Some(changed_view))?
            .errors
            .is_empty(),
        "changed view accepted migration",
    )?;
    ensure(
        editor.document.model() == &many
            && editor.document.revision() == 0
            && !editor.document.can_undo(),
        "view rejection changed live history",
    )?;
    ensure(
        !editor.plugin_work_pending(),
        "view rejection left pending intent",
    )?;
    let mut review = os_ui::migration_tools::MigrationDraft::begin(
        &editor.host,
        &editor.document,
        OWNER,
        context,
    )?;
    ensure(
        editor
            .start_plugin_migration(&review, context, None)
            .is_err(),
        "unreviewed plan started",
    )?;
    review.choose(
        &editor.host,
        &editor.document,
        context,
        "org.example.columns.rectangular",
        "org.example.columns.migrate",
    )?;
    editor.start_plugin_migration(&review, context, None)?;
    drain(&mut editor, context)?;
    ensure(
        editor.document.revision() == 1 && editor.scene.len() == 33,
        "editor did not commit once and regenerate all columns",
    )?;
    let migrated = editor.document.model().clone();
    editor.undo()?;
    ensure(
        editor.document.model() == &many && editor.scene.is_empty(),
        "editor staged undo mismatch",
    )?;
    editor.redo()?;
    drain(&mut editor, context)?;
    editor.save(&directory.path().join("batched-editor.osb"))?;
    ensure(
        ZipJsonStorage
            .open(&directory.path().join("batched-editor.osb"))?
            .model()
            == &migrated,
        "editor batched persistence mismatch",
    )?;
    println!(
        "PASS: Editor staged 33-column migration, selection/view-change rejection after partial progress, one commit, geometry regeneration, undo/redo and save/open."
    );
    println!(
        "PASS: installed v1/v2 column, unavailable staged open, cancelled/rejected/stale migration, two-entity payload conversion, atomic requirement/history, geometry regeneration, save/reopen and absent-plugin preservation. Native UI not exercised."
    );
    if let Some(installed) = args.get(2) {
        mixed(&mut editor, &before, installed)?;
    }
    Ok(())
}

fn mixed(
    editor: &mut Editor,
    source: &os_model::Model,
    installed: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    const BLOCK: &str = "org.example.columns.test-block";
    editor.host.unload(OWNER)?;
    load(editor, installed)?;
    let mut model = source.clone();
    let template = model.extensions.values().next().unwrap().clone();
    while model.extensions.len() < 8 {
        let mut entity = template.clone();
        entity.id = os_core::Id::new();
        model.extensions.insert(entity.id, entity);
    }
    for _ in 0..5 {
        let mut entity = template.clone();
        entity.id = os_core::Id::new();
        entity.type_id = BLOCK.into();
        entity.name = "Migration fixture block".into();
        model.extensions.insert(entity.id, entity);
    }
    let last_block = model
        .extensions
        .values()
        .filter(|e| e.type_id == BLOCK)
        .map(|e| e.id)
        .max()
        .unwrap();
    for reject in [true, false] {
        let mut original = model.clone();
        if reject {
            original.extensions.get_mut(&last_block).unwrap().payload["width_m"] =
                serde_json::json!("vendor collision");
        }
        editor.document = os_document::Document::from_model(original.clone())?;
        editor.regenerate()?;
        let context = ToolContext {
            level: *original.levels.keys().next().unwrap(),
            selection: None,
        };
        let mut plan = os_ui::migration_tools::MigrationDraft::begin(
            &editor.host,
            &editor.document,
            OWNER,
            context,
        )?;
        ensure(
            plan.types().count() == 2,
            "mixed review did not find both types",
        )?;
        plan.choose(
            &editor.host,
            &editor.document,
            context,
            "org.example.columns.rectangular",
            "org.example.columns.migrate",
        )?;
        ensure(
            editor.start_plugin_migration(&plan, context, None).is_err(),
            "partial type review accepted",
        )?;
        plan.choose(
            &editor.host,
            &editor.document,
            context,
            BLOCK,
            "org.example.columns.migrate-block",
        )?;
        editor.start_plugin_migration(&plan, context, None)?;
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let progress = editor.plugin_migration_progress().map(|p| p.0).unwrap_or(0);
            let report = editor.poll_plugin_work(context, None)?;
            if reject && !report.errors.is_empty() {
                ensure(
                    progress == 12,
                    "mixed rejection did not occur in final second-type batch",
                )?;
                ensure(
                    editor.document.model() == &original
                        && editor.document.revision() == 0
                        && !editor.document.can_undo(),
                    "mixed failure changed live data/history",
                )?;
                break;
            }
            ensure(
                report.errors.is_empty(),
                format!("mixed error: {:?}", report.errors),
            )?;
            if report.committed {
                ensure(
                    !reject && editor.document.revision() == 1,
                    "mixed commit mismatch",
                )?;
                break;
            }
            ensure(
                editor.document.model() == &original && editor.document.revision() == 0,
                "mixed candidate leaked",
            )?;
            ensure(Instant::now() < deadline, "mixed migration timed out")?;
            std::thread::park_timeout(Duration::from_millis(1));
        }
        drain(editor, context)?;
        if reject {
            continue;
        }
        let migrated = editor.document.model().clone();
        ensure(
            editor.scene.len() == 13 && migrated.plugin_requirements[OWNER].version == "2.0.0",
            "mixed geometry or requirement mismatch",
        )?;
        for (id, entity) in &migrated.extensions {
            let old = &original.extensions[id].payload;
            let expected_volume = old["width"].as_f64().unwrap()
                * old["depth"].as_f64().unwrap()
                * old["height"].as_f64().unwrap();
            ensure(
                (editor.scene[id].signed_volume() - expected_volume).abs() < 1e-9,
                "mixed migration changed element volume",
            )?;
            ensure(
                entity.payload_schema_version == 2
                    && entity.payload["height_m"] == original.extensions[id].payload["height"],
                "mixed height conversion mismatch",
            )?;
            if entity.type_id == BLOCK {
                ensure(
                    entity.payload.get("width").is_none()
                        && entity.payload["width_m"] == original.extensions[id].payload["width"],
                    "wrong per-type transformation",
                )?;
            } else {
                ensure(
                    entity.payload.get("width_m").is_none(),
                    "block transformation applied to column",
                )?;
            }
        }
        editor.undo()?;
        ensure(
            editor.document.model() == &original && !editor.document.can_undo(),
            "mixed undo not atomic",
        )?;
        editor.redo()?;
        drain(editor, context)?;
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("mixed.osb");
        editor.save(&path)?;
        ensure(
            ZipJsonStorage.open(&path)?.model() == &migrated,
            "mixed persistence mismatch",
        )?;
    }
    println!(
        "PASS: independently installed mixed-type guest, explicit complete review, second-type final-batch rejection after 12 conversions, distinct transforms, 13 meshes, atomic history and storage."
    );
    Ok(())
}
