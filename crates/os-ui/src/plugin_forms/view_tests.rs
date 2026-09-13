use super::*;
use os_plugin_api::Permission;

#[test]
fn switching_same_level_plans_revokes_draft_without_losing_values() {
    let mut app = DesktopApp::new().unwrap();
    app.editor
        .host
        .load(
            Box::new(crate::plugin_jobs::tests::NoWorker),
            [
                Permission::ModelRead,
                Permission::ModelWrite,
                Permission::UiTool,
            ]
            .into(),
        )
        .unwrap();
    let a = app.editor.create_floor_plan("A", app.active_level).unwrap();
    let b = app.editor.create_floor_plan("B", app.active_level).unwrap();
    app.focus_plan(Some(a));
    let context = ToolContext {
        level: app.active_level,
        selection: None,
    };
    let draft = ToolDraft::begin(
        &app.editor.host,
        &app.editor.document,
        "org.example.columns",
        "org.example.columns.create",
        context,
    )
    .unwrap();
    let values = draft.values().clone();
    app.plugin_form.text = values
        .iter()
        .map(|(k, v)| (k.clone(), v.to_string()))
        .collect();
    app.plugin_form.view = app.plugin_view_context();
    app.plugin_form.from_gesture = true;
    assert_eq!(app.plugin_form.view.unwrap().id, a);
    app.plugin_form.draft = Some(draft);
    let original = app.editor.document.model().clone();
    let ctx = egui::Context::default();
    let _ = ctx.run(egui::RawInput::default(), |ctx| app.plugin_forms(ctx));
    assert!(!app.plugin_form.view_revoked);
    app.focus_plan(Some(b));
    let _ = ctx.run(egui::RawInput::default(), |ctx| app.plugin_forms(ctx));
    assert!(app.plugin_form.view_revoked);
    assert_eq!(app.plugin_form.draft.as_ref().unwrap().values(), &values);
    app.focus_plan(Some(a));
    let _ = ctx.run(egui::RawInput::default(), |ctx| app.plugin_forms(ctx));
    assert!(
        app.plugin_form.view_revoked,
        "returning to the old view must not revive authorization"
    );
    assert_eq!(app.editor.document.model(), &original);
    assert!(!app.editor.plugin_work_pending());
}

#[test]
fn failed_pointer_admission_preserves_live_gesture_and_document() {
    let mut app = DesktopApp::new().unwrap();
    let view = app
        .editor
        .create_floor_plan("Plan", app.active_level)
        .unwrap();
    app.focus_plan(Some(view));
    app.wall_gesture = Some(
        crate::plan_gesture::WallGesture::begin(&app.editor, view, app.draft.clone()).unwrap(),
    );
    app.editor
        .host
        .load(
            Box::new(crate::plugin_jobs::tests::NoWorker),
            [
                Permission::ModelRead,
                Permission::ModelWrite,
                Permission::UiTool,
            ]
            .into(),
        )
        .unwrap();
    let context = ToolContext {
        level: app.active_level,
        selection: None,
    };
    let draft = ToolDraft::begin(
        &app.editor.host,
        &app.editor.document,
        "org.example.columns",
        "org.example.columns.create",
        context,
    )
    .unwrap();
    let before = app.editor.document.model().clone();
    assert!(
        app.submit_installed_wall(draft, context, app.plugin_view_context().unwrap())
            .is_err()
    );
    assert!(app.wall_gesture.is_some());
    assert!(app.plugin_form.draft.is_none());
    assert!(!app.plugin_form.from_gesture);
    assert!(!app.editor.plugin_work_pending());
    assert_eq!(*app.editor.document.model(), before);
}
