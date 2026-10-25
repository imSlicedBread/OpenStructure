use os_core::Id;
use os_model::{DrawingIssue, IssueState, Model, Sheet, SheetParams, SheetSet};

fn fixture() -> (Model, Id, Id) {
    let mut model = Model::new("Issue validation");
    let first = Sheet::new("core.sheet", SheetParams::new("A101", "Ground"));
    let second = Sheet::new("core.sheet", SheetParams::new("A201", "Upper"));
    let sheets = vec![second.id(), first.id()];
    model.sheets.insert(first.id(), first);
    model.sheets.insert(second.id(), second);
    let set = Id::new();
    model.sheet_sets.insert(
        set,
        SheetSet {
            id: set,
            name: "Construction set".into(),
            sheets: sheets.clone(),
        },
    );
    let issue = Id::new();
    model.drawing_issues.insert(
        issue,
        DrawingIssue {
            id: issue,
            set,
            sequence: 1,
            revision: "R1".into(),
            description: "Review issue".into(),
            date: "2024-02-29".into(),
            recipients: vec!["Design team".into()],
            sheets,
            supersedes: None,
            state: IssueState::Draft,
            manifest: None,
            manifest_sha256: None,
            archive_path: None,
        },
    );
    model.validate().unwrap();
    (model, set, issue)
}

#[test]
fn ordered_issue_metadata_round_trips_without_sorting_or_live_set_reordering() {
    let (mut model, set, issue) = fixture();
    let original_order = model.sheet_sets[&set].sheets.clone();
    model.sheet_sets.get_mut(&set).unwrap().sheets.reverse();
    // A draft captures the selected order; editing the reusable set must not
    // silently change a previously created issue's drawing list.
    assert_eq!(model.drawing_issues[&issue].sheets, original_order);
    let bytes = serde_json::to_vec(&model).unwrap();
    let reopened: Model = serde_json::from_slice(&bytes).unwrap();
    reopened.validate().unwrap();
    assert_eq!(reopened, model);
    assert_eq!(reopened.drawing_issues[&issue].sheets, original_order);
}

#[test]
fn invalid_sheet_references_sequences_dates_and_global_identities_are_rejected() {
    let (model, set, issue) = fixture();
    let mut invalid = model.clone();
    invalid.sheet_sets.get_mut(&set).unwrap().sheets[0] = Id::new();
    assert!(invalid.validate().is_err());

    let mut invalid = model.clone();
    let sheets = &mut invalid.sheet_sets.get_mut(&set).unwrap().sheets;
    sheets[1] = sheets[0];
    assert!(invalid.validate().is_err());

    let mut invalid = model.clone();
    invalid.drawing_issues.get_mut(&issue).unwrap().sheets[0] = Id::new();
    assert!(invalid.validate().is_err());

    let mut invalid = model.clone();
    let mut duplicate = invalid.drawing_issues[&issue].clone();
    duplicate.id = Id::new();
    invalid.drawing_issues.insert(duplicate.id, duplicate);
    assert!(
        invalid.validate().is_err(),
        "sequences are unique within a set"
    );

    let mut invalid = model.clone();
    let mut collision = invalid.sheet_sets.remove(&set).unwrap();
    collision.id = model.project.id();
    invalid.sheet_sets.insert(collision.id, collision);
    invalid.drawing_issues.get_mut(&issue).unwrap().set = model.project.id();
    assert!(
        invalid.validate().is_err(),
        "set IDs share the global namespace"
    );

    for date in ["2023-02-29", "2024-04-31", "0000-01-01", "2024-13-01"] {
        let mut invalid = model.clone();
        invalid.drawing_issues.get_mut(&issue).unwrap().date = date.into();
        assert!(invalid.validate().is_err(), "invalid calendar date {date}");
    }
    let mut invalid = model.clone();
    invalid.drawing_issues.get_mut(&issue).unwrap().supersedes = Some(issue);
    assert!(
        invalid.validate().is_err(),
        "an issue cannot supersede itself"
    );
}

#[test]
fn review_and_issued_records_require_frozen_manifest_and_draft_cannot_claim_archive() {
    let (model, _, issue) = fixture();
    for state in [IssueState::Review, IssueState::Issued] {
        let mut invalid = model.clone();
        invalid.drawing_issues.get_mut(&issue).unwrap().state = state;
        assert!(invalid.validate().is_err());
    }
    let mut invalid = model.clone();
    invalid.drawing_issues.get_mut(&issue).unwrap().archive_path =
        Some("unpublished.osissue".into());
    assert!(invalid.validate().is_err());
    let mut invalid = model;
    invalid
        .drawing_issues
        .get_mut(&issue)
        .unwrap()
        .manifest_sha256 = Some("0".repeat(64));
    assert!(invalid.validate().is_err());
}
