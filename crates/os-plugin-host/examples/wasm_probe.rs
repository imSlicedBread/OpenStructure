//! Assemble once, then load a separately stored artifact without rebuilding host.
use os_core::{Id, ensure};
use os_model::Model;
use os_plugin_api::{Permission, Request, Response};
use os_plugin_host::PluginHost;
use std::{io::Write, path::Path};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [flag, directory] if flag == "--worker" => {
            use os_plugin_host::worker::JobOutcome;
            use std::time::Duration;
            let mut host = PluginHost::default();
            host.load_wasm_directory(Path::new(directory), [Permission::ModelRead].into())?;
            let mut document = os_document::Document::new("Worker probe")?;
            let mut job = host.start_job(
                "org.openstructure.probe",
                &document,
                Request::GenerateWall { id: Id::new() },
                None,
                Duration::from_secs(5),
            )?;
            loop {
                if let Some(outcome) = host.poll_job(&mut job, &mut document, None, "Probe")? {
                    let JobOutcome::Geometry(reply) = outcome else {
                        return Err("unexpected modeling reply".into());
                    };
                    ensure(
                        reply.solid(&document, None)?.height == 1.0,
                        "unexpected geometry",
                    )?;
                    println!(
                        "PASS: external Wasm worker {} returned session/revision-checked geometry; no document edits.",
                        job.id()
                    );
                    break;
                }
                std::thread::park_timeout(Duration::from_millis(1));
            }
        }
        [flag, directory] if flag == "--assemble" => {
            // Fail rather than replacing any existing installation or user data.
            let directory = Path::new(directory);
            std::fs::create_dir(directory)?;
            let bytes = wat::parse_str(include_str!("../../../fixtures/wasm-probe/probe.wat"))
                .map_err(|e| os_core::Error::Invalid(e.to_string()))?;
            for (name, bytes) in [
                ("probe.wasm", bytes.as_slice()),
                (
                    "plugin.toml",
                    include_bytes!("../../../fixtures/wasm-probe/plugin.toml").as_slice(),
                ),
            ] {
                std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(directory.join(name))?
                    .write_all(bytes)?;
            }
            println!(
                "Assembled probe.wasm and plugin.toml; now run this executable with the directory path."
            );
        }
        [directory] => {
            let started = std::time::Instant::now();
            let mut host = PluginHost::default();
            host.load_wasm_directory(Path::new(directory), [Permission::ModelRead].into())?;
            let response = host.request(
                "org.openstructure.probe",
                &Model::new("Probe"),
                Request::GenerateWall { id: Id::new() },
            )?;
            let Response::Solid(solid) = response else {
                return Err(os_core::Error::Invalid("expected geometry response".into()).into());
            };
            ensure(
                solid.height == 1.0 && solid.profile.vertices.len() == 4,
                "unexpected probe geometry",
            )?;
            println!(
                "Loaded external Wasm binary; validated v1 JSON unit-prism response in {:?}. No document edits or writes.",
                started.elapsed()
            );
        }
        _ => {
            return Err(os_core::Error::Invalid(
                "usage: wasm_probe [--assemble|--worker] <plugin-directory>".into(),
            )
            .into());
        }
    }
    Ok(())
}
