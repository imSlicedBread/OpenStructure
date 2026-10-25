#[path = "support/plan_duplication.rs"]
mod support;
use os_core::{Id, Point2};
use os_document::{Command, Document, PlanDuplicateMode as Mode};
use os_model::*;

#[test]
fn duplicate_all_owned_kinds_headers_settings_history_and_independent_edits() {
    for mode in [Mode::Plain, Mode::WithDetailing] {
        let (m, source) = support::fixture();
        let mut doc = Document::from_model(m.clone()).unwrap();
        let history = doc.history_stats();
        let draft = doc
            .prepare_plan_duplicate(source, "Copy".into(), mode)
            .unwrap();
        let target = draft.target();
        assert_eq!(doc.model(), &m);
        assert_eq!(doc.history_stats(), history);
        doc.apply_plan_duplicate(draft).unwrap();
        let after = doc.model().clone();
        let mut expected = m.views[&source].parameters.clone();
        expected.name = "Copy".into();
        expected.settings_revision = 0;
        assert_eq!(after.views[&target].parameters, expected);
        assert_eq!(after.plan_graphics[&target], m.plan_graphics[&source]);
        assert_eq!(after.walls, m.walls);
        assert_eq!(after.openings, m.openings);
        assert_eq!(after.rooms, m.rooms);
        assert_eq!(after.sheets, m.sheets);
        assert_eq!(after.drawing_issues, m.drawing_issues);
        assert_eq!(doc.history_stats().undo_entries, history.undo_entries + 1);
        if mode == Mode::Plain {
            assert_eq!(after.dimensions, m.dimensions);
            assert_eq!(after.text_notes, m.text_notes);
            assert_eq!(after.plan_callouts, m.plan_callouts);
        } else {
            assert_eq!(
                doc.plan_duplicate_counts(target).unwrap(),
                doc.plan_duplicate_counts(source).unwrap()
            );
            macro_rules! check {
                ($field:ident) => {
                    for old in m.$field.values() {
                        let mut parameters = old.parameters.clone();
                        parameters.view = target;
                        let copy = after
                            .$field
                            .values()
                            .find(|e| e.parameters == parameters)
                            .unwrap();
                        assert_ne!(copy.id(), old.id());
                        assert_eq!(copy.header.type_id, old.header.type_id);
                        assert_eq!(copy.header.properties, old.header.properties);
                    }
                };
            }
            check!(dimensions);
            check!(opening_tags);
            check!(room_tags);
            check!(detail_lines);
            check!(text_notes);
            let note = after
                .text_notes
                .values()
                .find(|n| n.parameters.view == target)
                .unwrap();
            let line = after
                .detail_lines
                .values()
                .find(|n| n.parameters.view == target)
                .unwrap();
            assert_eq!(note.header.relationships["peer"][..2], [line.id(), target]);
            assert_eq!(
                after.views[&target].header.relationships["annotation"],
                vec![note.id()]
            );
            for marker in after
                .plan_callouts
                .values()
                .filter(|c| c.parameters.parent == target)
            {
                assert!(!marker.parameters.owns_target);
                assert!(
                    m.plan_callouts
                        .values()
                        .any(|c| c.parameters.target == marker.parameters.target)
                );
            }
        }
        assert!(doc.undo());
        assert_eq!(doc.model(), &m);
        assert!(doc.redo());
        assert_eq!(doc.model(), &after);
        let mut p = after.views[&target].parameters.clone();
        p.plan.as_mut().unwrap().basis.rotation = -0.5;
        doc.execute(
            "Independent edit",
            vec![Command::UpdateView {
                id: target,
                parameters: p,
            }],
        )
        .unwrap();
        assert_eq!(doc.model().views[&source], m.views[&source]);
        doc.delete_plan(target).unwrap();
        assert_eq!(doc.model(), &m);
        assert!(doc.undo());
        assert!(doc.model().views.contains_key(&target));
    }
}

#[test]
fn duplicate_rejects_names_orphans_and_stale_contexts_without_mutation() {
    let (m, source) = support::fixture();
    let mut doc = Document::from_model(m.clone()).unwrap();
    assert_eq!(doc.plan_duplicate_name(source).unwrap(), "Source Copy");
    for name in ["", "  ", " source ", "bad\nname", &"é".repeat(129)] {
        assert!(
            doc.prepare_plan_duplicate(source, name.into(), Mode::Plain)
                .is_err()
        );
        assert_eq!(doc.model(), &m);
    }
    let draft = doc
        .prepare_plan_duplicate(source, "Copy".into(), Mode::Plain)
        .unwrap();
    doc.execute("Change", vec![Command::RenameProject("Changed".into())])
        .unwrap();
    let changed = doc.model().clone();
    let history = doc.history_stats();
    assert!(doc.apply_plan_duplicate(draft).is_err());
    assert_eq!(doc.model(), &changed);
    assert_eq!(doc.history_stats(), history);
    let draft = doc
        .prepare_plan_duplicate(source, "Copy".into(), Mode::Plain)
        .unwrap();
    assert!(
        Document::from_model(changed)
            .unwrap()
            .apply_plan_duplicate(draft)
            .is_err()
    );
    for kind in 0..3 {
        let mut orphan = m.clone();
        let id = match kind {
            0 => {
                let e = orphan.dimensions.values_mut().next().unwrap();
                e.parameters.first = DimensionReference::WallEndpoint {
                    wall: Id::new(),
                    endpoint: DimensionEndpoint::Start,
                };
                e.id()
            }
            1 => {
                let e = orphan.opening_tags.values_mut().next().unwrap();
                e.parameters.opening = Id::new();
                e.id()
            }
            _ => {
                let e = orphan.room_tags.values_mut().next().unwrap();
                e.parameters.room = Id::new();
                e.id()
            }
        };
        let d = Document::from_model(orphan.clone()).unwrap();
        let error = d
            .prepare_plan_duplicate(source, "Copy".into(), Mode::WithDetailing)
            .err()
            .unwrap();
        assert!(error.to_string().contains(&id.to_string()), "{error}");
        assert!(
            d.prepare_plan_duplicate(source, "Copy".into(), Mode::Plain)
                .is_ok()
        );
        assert_eq!(d.model(), &orphan);
    }
}

#[test]
fn duplicate_limits_and_unicode_names() {
    let (mut m, source) = support::fixture();
    m.views.get_mut(&source).unwrap().parameters.name = "界".repeat(85);
    let mut d = Document::from_model(m.clone()).unwrap();
    let name = d.plan_duplicate_name(source).unwrap();
    assert!(name.len() <= 256);
    let draft = d
        .prepare_plan_duplicate(source, name.clone(), Mode::Plain)
        .unwrap();
    d.apply_plan_duplicate(draft).unwrap();
    let next = d.plan_duplicate_name(source).unwrap();
    assert!(next.ends_with(" Copy 2"));
    assert!(next.len() <= 256);
    for _ in 0..4096 {
        let line = DetailLine::new(
            "core.detail_line",
            DetailLineParams {
                view: source,
                start: Point2::default(),
                end: Point2::new(1., 0.),
            },
        );
        m.detail_lines.insert(line.id(), line);
    }
    let d = Document::from_model(m.clone()).unwrap();
    assert!(
        d.prepare_plan_duplicate(source, "Copy".into(), Mode::WithDetailing)
            .err()
            .unwrap()
            .to_string()
            .contains("4096")
    );
    assert_eq!(d.model(), &m);
    m.detail_lines.retain(|_, e| e.parameters.start.x == 30.);
    for _ in 0..40 {
        let mut p = TextNoteParams::new(source);
        p.text = "X".repeat(1500);
        p.width_mm = 500.;
        let n = TextNote::new("core.text_note", p);
        m.text_notes.insert(n.id(), n);
    }
    let d = Document::from_model(m.clone()).unwrap();
    assert!(
        d.prepare_plan_duplicate(source, "Copy".into(), Mode::WithDetailing)
            .err()
            .unwrap()
            .to_string()
            .contains("budget")
    );
    assert_eq!(d.model(), &m);
}

#[test]
fn duplicate_callout_destinations_enlarged_target_and_deletion_guards() {
    let (mut m, source) = support::fixture();
    let target = m.plan_callouts.values().next().unwrap().parameters.target;
    let viewport = Id::new();
    let mut p = SheetParams::new("A101", "Detail");
    p.viewports.push(SheetViewport {
        id: viewport,
        view: target,
        detail_number: "1".into(),
        model_center_m: Point2::default(),
        paper_center_mm: Point2::new(200., 150.),
        width_mm: 100.,
        height_mm: 100.,
        scale_denominator: 50.,
        title_override: None,
    });
    let sheet = Sheet::new("core.sheet", p);
    let sheet_id = sheet.id();
    m.sheets.insert(sheet_id, sheet);
    for c in m.plan_callouts.values_mut() {
        c.parameters.destination = Some(viewport);
    }
    let mut d = Document::from_model(m.clone()).unwrap();
    let draft = d
        .prepare_plan_duplicate(target, "Enlarged copy".into(), Mode::WithDetailing)
        .unwrap();
    let copy = d.apply_plan_duplicate(draft).unwrap();
    assert_eq!(d.model().plan_callouts, m.plan_callouts);
    assert!(
        !d.model()
            .plan_callouts
            .values()
            .any(|c| c.parameters.target == copy)
    );
    d.delete_plan(copy).unwrap();
    assert_eq!(d.model(), &m);
    let draft = d
        .prepare_plan_duplicate(source, "Copy".into(), Mode::WithDetailing)
        .unwrap();
    let copy = d.apply_plan_duplicate(draft).unwrap();
    for c in d
        .model()
        .plan_callouts
        .values()
        .filter(|c| c.parameters.parent == copy)
    {
        assert_eq!(c.parameters.destination, Some(viewport));
        assert_eq!(
            d.model().callout_reference(&c.parameters).label(),
            "1 / A101"
        );
    }
    let before = d.model().clone();
    let history = d.history_stats();
    assert!(d.delete_plan(target).is_err());
    assert_eq!(d.model(), &before);
    assert_eq!(d.history_stats(), history);
    let mut placement = m.sheets[&sheet_id].parameters.clone();
    placement.number = "A102".into();
    placement.viewports[0].id = Id::new();
    placement.viewports[0].view = copy;
    let copy_sheet = Sheet::new("core.sheet", placement);
    let copy_sheet_id = copy_sheet.id();
    d.execute("Place copy", vec![Command::AddSheet(copy_sheet)])
        .unwrap();
    let placed = d.model().clone();
    assert!(d.delete_plan(copy).is_err());
    assert_eq!(d.model(), &placed);
    assert!(
        d.model()
            .validate_callout_issue_references(&[copy_sheet_id])
            .is_err()
    );
    d.model()
        .validate_callout_issue_references(&[copy_sheet_id, sheet_id])
        .unwrap();
    d.execute("Unplace copy", vec![Command::RemoveSheet(copy_sheet_id)])
        .unwrap();
    d.delete_plan(copy).unwrap();
    assert_eq!(d.model(), &m);
    // Incoming markers independently guard a new duplicate, even without placement.
    let draft = d
        .prepare_plan_duplicate(source, "Copy".into(), Mode::Plain)
        .unwrap();
    let copy = d.apply_plan_duplicate(draft).unwrap();
    let mut c = m.plan_callouts.values().next().unwrap().clone();
    c.header.id = Id::new();
    c.parameters.target = copy;
    c.parameters.owns_target = false;
    c.parameters.destination = None;
    d.execute("Incoming", vec![Command::AddPlanCallout(c)])
        .unwrap();
    let before = d.model().clone();
    assert!(d.delete_plan(copy).is_err());
    assert_eq!(d.model(), &before);
}

#[test]
fn duplicate_extension_dependencies_fail_closed_and_non_floor_views_are_rejected() {
    let (mut m, source) = support::fixture();
    m.plugin_requirements.insert(
        "example.plugin".into(),
        PluginRequirement {
            version: "1.0.0".into(),
        },
    );
    let e = ExtensionEntity {
        envelope_version: EXTENSION_ENVELOPE_VERSION,
        id: Id::new(),
        owner: "example.plugin".into(),
        type_id: "example.plugin.annotation".into(),
        name: "Custom".into(),
        payload_schema_version: 1,
        relationships: std::collections::BTreeMap::from([("view".into(), vec![source])]),
        depends_on: Default::default(),
        payload: serde_json::json!({"opaque": source.to_string()}),
    };
    let extension = e.id;
    m.extensions.insert(e.id, e);
    let d = Document::from_model(m.clone()).unwrap();
    assert!(
        d.prepare_plan_duplicate(source, "Copy".into(), Mode::WithDetailing)
            .err()
            .unwrap()
            .to_string()
            .contains(&extension.to_string())
    );
    assert!(
        d.prepare_plan_duplicate(source, "Copy".into(), Mode::Plain)
            .is_ok()
    );
    assert_eq!(d.model(), &m);
    let level = m.views[&source].parameters.level.unwrap();
    for p in [
        ViewParams::drafting("Drafting"),
        ViewParams::reflected_ceiling_plan("RCP", level),
    ] {
        let v = View::new("core.view", p);
        let id = v.id();
        m.views.insert(id, v);
        let d = Document::from_model(m.clone()).unwrap();
        assert!(
            d.prepare_plan_duplicate(id, "Copy".into(), Mode::Plain)
                .is_err()
        );
    }
}

#[test]
fn duplicate_preserves_issued_manifest_and_issued_view_cannot_be_deleted() {
    let (mut m, source) = support::fixture();
    let issue = Id::new();
    let set = Id::new();
    let sheet = Id::new();
    let manifest = IssueManifest {
        format_version: ISSUE_BUNDLE_VERSION,
        state: IssueState::Issued,
        issue,
        project: m.project.id(),
        set,
        sequence: 1,
        revision: "A".into(),
        description: "Issued".into(),
        date: "2026-10-11".into(),
        recipients: vec!["Team".into()],
        supersedes: None,
        schema_version: SCHEMA_VERSION,
        native_api_version: 60,
        application_version: "0.1.0".into(),
        source_session: Id::new(),
        source_revision: 1,
        source_sha256: sha256(b"frozen source"),
        providers: vec![],
        provider_versions: vec![("native".into(), "0.1.0".into(), 60)],
        sheets: vec![IssueSheet {
            id: sheet,
            number: "A101".into(),
            name: "Plan".into(),
            views: vec![(source, 9)],
        }],
        warnings: vec![],
        pdf_sha256: sha256(b"frozen pdf"),
    };
    m.drawing_issues.insert(
        issue,
        DrawingIssue {
            id: issue,
            set,
            sequence: 1,
            revision: manifest.revision.clone(),
            description: manifest.description.clone(),
            date: manifest.date.clone(),
            recipients: manifest.recipients.clone(),
            sheets: vec![sheet],
            supersedes: None,
            state: IssueState::Issued,
            manifest_sha256: Some(sha256(&manifest.bytes().unwrap())),
            manifest: Some(manifest),
            archive_path: Some("issued.osissue".into()),
        },
    );
    let mut d = Document::from_model(m.clone()).unwrap();
    let draft = d
        .prepare_plan_duplicate(source, "Copy".into(), Mode::WithDetailing)
        .unwrap();
    let copy = d.apply_plan_duplicate(draft).unwrap();
    assert_eq!(d.model().drawing_issues, m.drawing_issues);
    // No live sheet or incoming marker is needed for this historical guard.
    let before = d.model().clone();
    let history = d.history_stats();
    assert!(
        d.delete_plan(source)
            .unwrap_err()
            .to_string()
            .contains("drawing issue")
    );
    assert_eq!(d.model(), &before);
    assert_eq!(d.history_stats(), history);
    d.delete_plan(copy).unwrap();
    assert_eq!(d.model(), &m);
}
