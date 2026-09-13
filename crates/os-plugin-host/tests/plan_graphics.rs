use os_core::{Id, Result};
use os_document::{Command, Document};
use os_model::{Entity, ExtensionEntity, PluginRequirement, ViewParams};
use os_plugin_api::{Capability, Manifest, Plugin, Registration, RegistrationKind, generic, plan};
use os_plugin_host::{PluginHost, plan_graphics::PlanRequest};
use std::{cell::RefCell, rc::Rc, time::Duration};

const OWNER: &str = "org.example.columns";
const PROVIDER: &str = "org.example.columns.plan";

struct Provider {
    fault: &'static str,
    calls: Rc<RefCell<Vec<plan::Request>>>,
}
impl Plugin for Provider {
    fn manifest(&self) -> Manifest {
        let mut m =
            Manifest::from_toml(include_str!("../../../fixtures/generic-column/plugin.toml"))
                .unwrap();
        m.capabilities.push(Capability::Views);
        m.registrations.push(Registration {
            id: PROVIDER.into(),
            name: "Test plan graphics".into(),
            kind: RegistrationKind::ViewProvider,
        });
        m
    }
    fn invoke_json(&self, input: &str) -> Result<String> {
        if let Ok(request) = serde_json::from_str::<plan::Request>(input) {
            self.calls.borrow_mut().push(request.clone());
            if self.fault == "oversize" {
                return Ok("x".repeat(generic::MAX_BYTES + 1));
            }
            if self.fault == "malformed" {
                return Ok("{\"api_version\":2,\"api_version\":2}".into());
            }
            let width = request.snapshot.elements[0].payload["width"]
                .as_f64()
                .unwrap();
            let segment = plan::Segment {
                feature: 19,
                start: [0.0, 0.0],
                end: [width, 0.0],
                role: plan::Role::Cut,
            };
            let mut response = plan::Response {
                api_version: 2,
                service: plan::Service::Graphics,
                service_version: 1,
                request_id: request.request_id,
                context: request.context,
                element_id: request.element_id,
                view: request.view,
                result: plan::Reply::Graphics(vec![segment.clone()]),
            };
            match self.fault {
                "request" => response.request_id = Id::new().to_string(),
                "session" => response.context.session = Id::new().to_string(),
                "revision" => response.context.revision += 1,
                "view" => response.view.view_id = Id::new().to_string(),
                "settings" => response.view.settings_revision += 1,
                "range" => response.view.range[1] += 1.0,
                "target" => response.element_id = Id::new().to_string(),
                "version" => response.service_version = 2,
                "duplicate" => {
                    response.result = plan::Reply::Graphics(vec![segment.clone(), segment.clone()])
                }
                "degenerate" => {
                    response.result = plan::Reply::Graphics(vec![plan::Segment {
                        end: segment.start,
                        ..segment.clone()
                    }])
                }
                "nonfinite" => {
                    response.result = plan::Reply::Graphics(vec![plan::Segment {
                        end: [f64::NAN, 0.0],
                        ..segment.clone()
                    }])
                }
                "count" => {
                    response.result = plan::Reply::Graphics(
                        (0..257)
                            .map(|feature| plan::Segment {
                                feature,
                                ..segment.clone()
                            })
                            .collect(),
                    )
                }
                "unsupported" => {
                    response.result = plan::Reply::Error(generic::PluginError {
                        code: generic::ErrorCode::Unsupported,
                        message: "Plan service unsupported".into(),
                        field: None,
                    })
                }
                _ => {}
            }
            return Ok(serde_json::to_string(&response).unwrap());
        }
        let request: generic::Request = serde_json::from_str(input).unwrap();
        assert!(matches!(request.operation, generic::Operation::Describe));
        Ok(serde_json::to_string(&generic::Response {
            api_version: 2,
            request_id: request.request_id,
            context: request.context,
            result: generic::Reply::Catalog(
                serde_json::from_str(include_str!(
                    "../../../fixtures/generic-column/catalog.json"
                ))
                .unwrap(),
            ),
        })
        .unwrap())
    }
}

fn document() -> (Document, Id, Id, Id) {
    let mut model = os_model::Model::new("Plan service");
    let level = *model.levels.keys().next().unwrap();
    let view = Entity::new("core.view", ViewParams::floor_plan("Plan", level));
    let view_id = view.id();
    model.views.insert(view_id, view);
    let mut entity: ExtensionEntity =
        serde_json::from_str(include_str!("../../../fixtures/extension-envelope-v1.json")).unwrap();
    entity.payload = serde_json::json!({"width":0.4,"depth":0.6,"height":3.0});
    let id = entity.id;
    model.extensions.insert(id, entity);
    model.plugin_requirements.insert(
        OWNER.into(),
        PluginRequirement {
            version: "1.0.0".into(),
        },
    );
    (Document::from_model(model).unwrap(), id, view_id, level)
}
fn invocation(element: Id, view: Id, level: Id) -> PlanRequest {
    PlanRequest {
        provider: PROVIDER.into(),
        element,
        view,
        read: [element, level].into(),
        timeout: Duration::from_secs(5),
    }
}
fn load(fault: &'static str) -> (PluginHost, Rc<RefCell<Vec<plan::Request>>>) {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let provider = Provider {
        fault,
        calls: calls.clone(),
    };
    let grants = provider.manifest().permissions;
    let mut host = PluginHost::default();
    host.load(Box::new(provider), grants).unwrap();
    (host, calls)
}

#[test]
fn callable_plan_service_is_scoped_read_only_and_revocable() {
    let (mut doc, element, view, level) = document();
    let before = doc.model().clone();
    let (mut host, calls) = load("");
    let result = host
        .generate_plan_graphics(OWNER, &doc, invocation(element, view, level))
        .unwrap();
    assert_eq!(result.element(), element);
    assert_eq!(
        result.segments(&host, &doc, view).unwrap()[0].end,
        [0.4, 0.0]
    );
    let requests = calls.borrow();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].snapshot.elements.len(), 1);
    assert_eq!(requests[0].snapshot.levels.len(), 1);
    assert_eq!(requests[0].view.range, [2.5, 1.2, 0.0, -1.0]);
    drop(requests);
    let other = *doc.model().views.keys().find(|id| **id != view).unwrap();
    assert!(result.segments(&host, &doc, other).is_err());
    assert!(
        result
            .segments(&host, &Document::from_model(before.clone()).unwrap(), view)
            .is_err()
    );
    assert_eq!(doc.model(), &before);
    assert!(!doc.can_undo());
    doc.execute("Rename", vec![Command::RenameProject("Changed".into())])
        .unwrap();
    assert!(result.segments(&host, &doc, view).is_err());
    assert!(doc.undo());
    assert!(result.segments(&host, &doc, view).is_err());
    let fresh = host
        .generate_plan_graphics(OWNER, &doc, invocation(element, view, level))
        .unwrap();
    host.unload(OWNER).unwrap();
    assert!(fresh.segments(&host, &doc, view).is_err());
}

#[test]
fn invalid_plan_replies_fail_wholly_without_model_or_history_changes() {
    for fault in [
        "oversize",
        "malformed",
        "request",
        "session",
        "revision",
        "view",
        "settings",
        "range",
        "target",
        "version",
        "duplicate",
        "degenerate",
        "nonfinite",
        "count",
        "unsupported",
    ] {
        let (doc, element, view, level) = document();
        let before = doc.model().clone();
        let (host, calls) = load(fault);
        assert!(
            host.generate_plan_graphics(OWNER, &doc, invocation(element, view, level))
                .is_err(),
            "{fault}"
        );
        assert_eq!(calls.borrow().len(), 1);
        assert_eq!(doc.model(), &before);
        assert_eq!(doc.revision(), 0);
        assert!(!doc.can_undo());
    }
}

#[test]
fn invalid_plan_authority_fails_before_invocation() {
    let (doc, element, view, level) = document();
    let (host, calls) = load("");
    for case in 0..5 {
        let mut request = invocation(element, view, level);
        match case {
            0 => request.read.clear(),
            1 => {
                request.read.insert(doc.model().project.id());
            }
            2 => request.provider = "org.other.plan".into(),
            3 => request.view = Id::new(),
            4 => request.timeout = Duration::ZERO,
            _ => unreachable!(),
        }
        assert!(host.generate_plan_graphics(OWNER, &doc, request).is_err());
    }
    assert!(calls.borrow().is_empty());
}

#[cfg(feature = "wasm")]
mod workers {
    use super::*;
    use os_plugin_host::worker::{JobFailure, JobOutcome, PendingJob, ViewContext};
    use std::time::Instant;

    fn installed(
        doc: &Document,
        element: Id,
        view: Id,
        level: Id,
        fault: &'static str,
    ) -> (PluginHost, tempfile::TempDir) {
        let (probe, calls) = load("");
        probe
            .generate_plan_graphics(OWNER, doc, invocation(element, view, level))
            .unwrap();
        let request = calls.borrow()[0].clone();
        let input = serde_json::to_string(&request).unwrap();
        let provider = Provider {
            fault,
            calls: Rc::new(RefCell::new(Vec::new())),
        };
        let output = provider.invoke_json(&input).unwrap();
        let placeholder = "00000000-0000-4000-8000-000000000000";
        let describe_request = serde_json::to_string(&generic::Request {
            api_version: 2,
            request_id: placeholder.into(),
            context: None,
            operation: generic::Operation::Describe,
        })
        .unwrap();
        let describe = provider.invoke_json(&describe_request).unwrap();
        let escape = |s: &str| s.bytes().map(|b| format!("\\{b:02x}")).collect::<String>();
        let input_offset = input.find(&request.request_id).unwrap();
        let output_offset = output.find(&request.request_id).unwrap_or(0);
        let describe_input_offset = describe_request.find(placeholder).unwrap();
        let describe_output_offset = describe.find(placeholder).unwrap();
        let prefix = match fault {
            "trap" => "unreachable",
            "loop" => "(loop $forever br $forever)",
            _ => "",
        };
        let wat = format!(
            r#"(module
            (memory (export "memory") 3 256)
            (data (i32.const 0) "{}") (data (i32.const 8192) "{}")
            (func (export "os_abi_version") (result i32) i32.const 1)
            (func (export "os_alloc") (param i32) (result i32) i32.const 65536)
            (func (export "os_invoke") (param $ptr i32) (param $len i32) (result i64)
                local.get $len i32.const 200 i32.lt_u if (result i64)
                    i32.const {describe_output_offset} local.get $ptr i32.const {describe_input_offset} i32.add i32.const 36 memory.copy
                    i64.const {} i64.const 32 i64.shl
                else {prefix}
                    i32.const {} local.get $ptr i32.const {input_offset} i32.add i32.const 36 memory.copy
                    i64.const {} i64.const 32 i64.shl i64.const 8192 i64.or
                end))"#,
            escape(&describe),
            escape(&output),
            describe.len(),
            8192 + output_offset,
            output.len()
        );
        let dir = tempfile::tempdir().unwrap();
        let manifest = include_str!("../../../fixtures/generic-column/plugin.toml")
            .replace("builtin:generic-column-test", "wasm:plan.wasm")
            .replace("[\"Modeling\"]", "[\"Modeling\", \"Views\"]")
            + "\n[[registrations]]\nid = \"org.example.columns.plan\"\nname = \"Plan fixture\"\nkind = \"ViewProvider\"\n";
        std::fs::write(dir.path().join("plugin.toml"), manifest).unwrap();
        std::fs::write(dir.path().join("plan.wasm"), wat::parse_str(wat).unwrap()).unwrap();
        let mut host = PluginHost::default();
        host.load_wasm_directory(dir.path(), provider.manifest().permissions)
            .unwrap();
        (host, dir)
    }
    fn view_context(doc: &Document, id: Id) -> Option<ViewContext> {
        Some(ViewContext {
            id,
            settings_revision: doc.model().views[&id].parameters.settings_revision,
        })
    }
    fn drain(host: &PluginHost) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while host.active_jobs() > 0 {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
    }
    fn finish(
        host: &PluginHost,
        doc: &mut Document,
        job: &mut PendingJob,
        view: Id,
    ) -> std::result::Result<JobOutcome, JobFailure> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(result) =
                host.poll_job(job, doc, view_context(doc, view), "Plan graphics")?
            {
                return Ok(result);
            }
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
    }

    #[test]
    fn wasm_plan_reply_is_read_only_single_consumption_and_shared_pool_bounded() {
        let (mut doc, element, view, level) = document();
        let before = doc.model().clone();
        let (host, _dir) = installed(&doc, element, view, level, "");
        let mut job = host
            .start_plan_graphics_job(OWNER, &doc, invocation(element, view, level))
            .unwrap();
        assert_eq!(host.active_jobs(), 1);
        assert!(
            host.start_plan_graphics_job(OWNER, &doc, invocation(element, view, level))
                .is_err()
        );
        assert!(
            host.generate_plan_graphics(OWNER, &doc, invocation(element, view, level))
                .is_err()
        );
        let JobOutcome::PlanGraphics(result) = finish(&host, &mut doc, &mut job, view).unwrap()
        else {
            panic!("wrong outcome")
        };
        assert_eq!(
            result.segments(&host, &doc, view).unwrap()[0].end,
            [0.4, 0.0]
        );
        assert!(matches!(
            host.poll_job(&mut job, &mut doc, None, "Replay"),
            Err(JobFailure::AlreadySettled)
        ));
        drain(&host);
        assert_eq!(doc.model(), &before);
        assert_eq!(doc.revision(), 0);
        assert!(!doc.can_undo());
    }

    #[test]
    fn expired_plan_reply_releases_its_worker_slot_without_publishing() {
        let (mut doc, element, view, level) = document();
        let (host, _dir) = installed(&doc, element, view, level, "");
        let mut request = invocation(element, view, level);
        request.timeout = Duration::from_secs(1);
        let mut job = host.start_plan_graphics_job(OWNER, &doc, request).unwrap();
        std::thread::sleep(Duration::from_millis(1010));
        assert_eq!(host.active_jobs(), 1, "unconsumed reply owns the slot");
        let current_view = view_context(&doc, view);
        assert!(matches!(
            host.poll_job(&mut job, &mut doc, current_view, "Expired"),
            Err(JobFailure::Deadline)
        ));
        drain(&host);
        assert_eq!(doc.revision(), 0);
        assert!(!doc.can_undo());
    }

    #[test]
    fn wasm_plan_cancellation_stale_context_unload_and_faults_cannot_publish() {
        for case in [
            "cancel",
            "view",
            "edit",
            "undo",
            "reopen",
            "unload",
            "drop",
            "trap",
            "loop",
            "range",
            "duplicate",
            "malformed",
        ] {
            let (mut doc, element, view, level) = document();
            let (mut host, _dir) = installed(&doc, element, view, level, case);
            let mut job = host
                .start_plan_graphics_job(OWNER, &doc, invocation(element, view, level))
                .unwrap();
            match case {
                "cancel" => job.cancel(),
                "edit" | "undo" => {
                    doc.execute("Edit", vec![Command::RenameProject("Other".into())])
                        .unwrap();
                    if case == "undo" {
                        assert!(doc.undo());
                    }
                }
                "reopen" => doc = Document::from_model(doc.model().clone()).unwrap(),
                "unload" => {
                    host.unload(OWNER).unwrap();
                }
                "drop" => {
                    drop(job);
                    drain(&host);
                    assert!(!doc.can_undo());
                    continue;
                }
                _ => {}
            }
            let before = doc.model().clone();
            let revision = doc.revision();
            let outcome = if case == "view" {
                host.poll_job(&mut job, &mut doc, None, "Inactive")
                    .map(|v| v.unwrap())
            } else {
                finish(&host, &mut doc, &mut job, view)
            };
            match case {
                "cancel" => assert!(matches!(outcome, Err(JobFailure::Cancelled))),
                "view" => assert!(matches!(outcome, Err(JobFailure::StaleView))),
                "edit" | "undo" | "reopen" => {
                    assert!(matches!(outcome, Err(JobFailure::StaleDocument)))
                }
                "unload" => assert!(matches!(outcome, Err(JobFailure::PluginUnloaded))),
                _ => assert!(matches!(outcome, Err(JobFailure::Rejected(_))), "{case}"),
            }
            drain(&host);
            assert_eq!(doc.model(), &before);
            assert_eq!(doc.revision(), revision);
        }
    }
}
