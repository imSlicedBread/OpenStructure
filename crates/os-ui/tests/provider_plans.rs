use os_core::{Id, Point2, Result};
use os_document::Document;
use os_model::{ExtensionEntity, PlanViewCrop, PluginRequirement};
use os_plugin_api::{Capability, Manifest, Plugin, Registration, RegistrationKind, generic, plan};
use os_plugin_host::plan_graphics::{PlanGraphics, PlanRequest};
use os_render::{
    plan::PlanCamera,
    snapping::{SnapKind, SnapQuery},
};
use os_ui::Editor;
use std::time::Duration;
const OWNER: &str = "org.example.columns";
struct Provider;
impl Plugin for Provider {
    fn manifest(&self) -> Manifest {
        let mut m =
            Manifest::from_toml(include_str!("../../../fixtures/generic-column/plugin.toml"))
                .unwrap();
        m.capabilities.push(Capability::Views);
        m.registrations.push(Registration {
            id: format!("{OWNER}.plan"),
            name: "Plan test".into(),
            kind: RegistrationKind::ViewProvider,
        });
        m
    }
    fn invoke_json(&self, input: &str) -> Result<String> {
        if let Ok(request) = serde_json::from_str::<plan::Request>(input) {
            return Ok(serde_json::to_string(&plan::Response {
                api_version: 2,
                service: plan::Service::Graphics,
                service_version: 1,
                request_id: request.request_id,
                context: request.context,
                element_id: request.element_id,
                view: request.view,
                result: plan::Reply::Graphics(vec![plan::Segment {
                    feature: 41,
                    start: [0.0, 0.0],
                    end: [10.0, 0.0],
                    role: plan::Role::Cut,
                }]),
            })
            .unwrap());
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
fn setup() -> (Editor, Id, Id, Id) {
    let mut editor = Editor::new().unwrap();
    let level = *editor.document.model().levels.keys().next().unwrap();
    let view = editor.create_floor_plan("Provider view", level).unwrap();
    let mut settings = editor.document.model().views[&view]
        .parameters
        .plan
        .unwrap();
    settings.basis.origin = Point2::new(1_000_000.0, -2_000_000.0);
    settings.basis.rotation = 0.73;
    settings.crop = Some(PlanViewCrop {
        min: Point2::new(2.0, -1.0),
        max: Point2::new(8.0, 1.0),
    });
    editor
        .update_floor_plan(view, "Provider view", level, settings)
        .unwrap();
    let mut model = editor.document.model().clone();
    let mut entity: ExtensionEntity =
        serde_json::from_str(include_str!("../../../fixtures/extension-envelope-v1.json")).unwrap();
    entity.payload = serde_json::json!({"width":0.4,"depth":0.6,"height":3.0});
    let id = entity.id;
    model.extensions.insert(id, entity.clone());
    entity.id = Id::new();
    let missing = entity.id;
    model.extensions.insert(missing, entity);
    model.plugin_requirements.insert(
        OWNER.into(),
        PluginRequirement {
            version: "1.0.0".into(),
        },
    );
    editor.document = Document::from_model(model).unwrap();
    editor
        .host
        .load(Box::new(Provider), Provider.manifest().permissions)
        .unwrap();
    (editor, view, id, missing)
}
fn graphics(editor: &Editor, view: Id, element: Id) -> PlanGraphics {
    let level = editor.document.model().views[&view]
        .parameters
        .level
        .unwrap();
    editor
        .host
        .generate_plan_graphics(
            OWNER,
            &editor.document,
            PlanRequest {
                provider: format!("{OWNER}.plan"),
                element,
                view,
                read: [element, level].into(),
                timeout: Duration::from_secs(5),
            },
        )
        .unwrap()
}

#[test]
fn checked_provider_results_compose_clipped_lines_and_snaps_without_retransforming() {
    let (editor, view, id, missing) = setup();
    let before = editor.document.model().clone();
    let context = editor.native_plan_context(view).unwrap();
    let composed = editor
        .plan_with_provider_graphics(view, vec![graphics(&editor, view, id)])
        .unwrap();
    let drawing = composed.drawing(&editor, view).unwrap();
    assert_eq!(drawing.unavailable(context).unwrap(), &[missing]);
    let line = &drawing.provider_lines(context).unwrap()[0];
    assert_eq!(line.start, Point2::new(2.0, 0.0));
    assert_eq!(line.end, Point2::new(8.0, 0.0));
    let camera = PlanCamera {
        center: Point2::new(5.0, 0.0),
        pixels_per_metre: 100.0,
    };
    let q = SnapQuery {
        camera,
        viewport: [800.0, 600.0],
        pointer: Point2::new(400.0, 300.0),
        radius_pixels: 6.0,
        endpoints: true,
        midpoints: true,
        intersections: false,
        perpendicular_from: None,
        nearest: true,
        axis_extensions: false,
        exclude_entity: None,
    };
    let hit = drawing
        .snap(context, q)
        .unwrap()
        .candidate(context, q)
        .unwrap()
        .unwrap();
    assert_eq!(
        (hit.entity, hit.feature, hit.kind),
        (id, 41, SnapKind::Midpoint)
    );
    assert_eq!(
        drawing
            .pick_screen(context, camera, q.viewport, q.pointer, 6.0)
            .unwrap(),
        Some(id)
    );
    assert_eq!(editor.document.model(), &before);
    assert!(!editor.document.can_undo());
}

#[test]
fn composed_provider_authority_rejects_duplicates_stale_results_and_unload() {
    let (mut editor, view, id, _) = setup();
    assert!(
        editor
            .plan_with_provider_graphics(
                view,
                vec![graphics(&editor, view, id), graphics(&editor, view, id)]
            )
            .is_err()
    );
    let raw = graphics(&editor, view, id);
    let composed = editor
        .plan_with_provider_graphics(view, vec![graphics(&editor, view, id)])
        .unwrap();
    let level = editor.document.model().views[&view]
        .parameters
        .level
        .unwrap();
    let other = editor.create_floor_plan("Other", level).unwrap();
    assert!(composed.drawing(&editor, other).is_err());
    assert!(composed.drawing(&editor, view).is_err());
    assert!(editor.plan_with_provider_graphics(view, vec![raw]).is_err());
    let fresh = editor
        .plan_with_provider_graphics(view, vec![graphics(&editor, view, id)])
        .unwrap();
    editor.host.unload(OWNER).unwrap();
    assert!(fresh.drawing(&editor, view).is_err());
    editor
        .host
        .load(Box::new(Provider), Provider.manifest().permissions)
        .unwrap();
    assert!(
        fresh.drawing(&editor, view).is_err(),
        "reloading does not revive old graphics"
    );
}

#[test]
fn prepared_provider_drawing_runs_off_thread_and_cannot_rebase_after_changes() {
    for case in ["current", "edit", "reload"] {
        let (mut editor, view, id, _) = setup();
        let prepared = editor
            .prepare_provider_plan(view, vec![graphics(&editor, view, id)])
            .unwrap();
        let before = editor.document.model().clone();
        let (started_tx, started_rx) = std::sync::mpsc::sync_channel(1);
        let (resume_tx, resume_rx) = std::sync::mpsc::sync_channel(1);
        let worker = std::thread::spawn(move || {
            started_tx.send(()).unwrap();
            resume_rx.recv().unwrap();
            prepared.derive()
        });
        started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        match case {
            "edit" => editor
                .document
                .execute(
                    "Rename",
                    vec![os_document::Command::RenameProject("New name".into())],
                )
                .unwrap(),
            "reload" => {
                editor.host.unload(OWNER).unwrap();
                editor
                    .host
                    .load(Box::new(Provider), Provider.manifest().permissions)
                    .unwrap();
            }
            _ => {}
        }
        resume_tx.send(()).unwrap();
        let drawing = worker.join().unwrap().unwrap();
        if case == "current" {
            let context = editor.native_plan_context(view).unwrap();
            assert_eq!(
                drawing
                    .drawing(&editor, view)
                    .unwrap()
                    .provider_lines(context)
                    .unwrap()
                    .len(),
                1
            );
            assert_eq!(editor.document.model(), &before);
        } else {
            assert!(
                drawing.drawing(&editor, view).is_err(),
                "{case} must not adopt a stale worker result"
            );
        }
    }
}
