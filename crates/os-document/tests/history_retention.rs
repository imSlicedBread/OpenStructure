use os_document::{Command, Document, HistoryLimits};
use std::collections::{BTreeMap, VecDeque};

fn rename(document: &mut Document, name: &str) {
    document
        .execute("Rename", vec![Command::RenameProject(name.into())])
        .unwrap();
}

#[test]
fn entry_limit_preserves_whole_transactions_events_and_auxiliary_files() {
    let original = Document::new("Original").unwrap();
    let files = BTreeMap::from([("assets/opaque.bin".into(), vec![1, 2, 3, 255])]);
    let mut document =
        Document::from_model_and_files(original.model().clone(), files.clone()).unwrap();
    document
        .set_history_limits(HistoryLimits {
            max_entries: 2,
            ..HistoryLimits::default()
        })
        .unwrap();
    let identity = document.model().project.id();
    for name in ["First", "Second", "Third"] {
        document
            .execute(
                name,
                vec![
                    Command::RenameProject("Intermediate".into()),
                    Command::RenameProject(name.into()),
                ],
            )
            .unwrap();
    }
    let stats = document.history_stats();
    assert_eq!(
        (
            stats.undo_entries,
            stats.redo_entries,
            stats.evicted_entries
        ),
        (2, 0, 1)
    );
    assert_eq!(document.drain_events().len(), 3);
    assert!(document.undo());
    assert_eq!(document.model().project.parameters.name, "Second");
    assert!(document.undo());
    assert_eq!(document.model().project.parameters.name, "First");
    assert!(!document.undo());
    assert!(document.redo());
    assert!(document.redo());
    assert!(!document.redo());
    assert_eq!(document.model().project.parameters.name, "Third");
    assert_eq!(document.model().project.id(), identity);
    assert_eq!(document.auxiliary_files(), &files);
    let events = document.drain_events();
    assert_eq!(
        events
            .iter()
            .map(|event| event.revision)
            .collect::<Vec<_>>(),
        [4, 5, 6, 7]
    );
    assert_eq!(
        events
            .iter()
            .map(|event| event.label.as_str())
            .collect::<Vec<_>>(),
        ["Undo Third", "Undo Second", "Redo Second", "Redo Third"]
    );
    assert_eq!(
        document.history_stats().estimated_bytes,
        stats.estimated_bytes
    );
}

#[test]
fn byte_limit_evicts_oldest_without_gaps_and_branching_releases_redo() {
    let mut document = Document::new("State 0").unwrap();
    rename(&mut document, "State 1");
    let entry_bytes = document.history_stats().estimated_bytes;
    let limits = HistoryLimits {
        max_entries: 100,
        max_estimated_bytes: entry_bytes * 3 - 1,
    };
    document.set_history_limits(limits).unwrap();
    for index in 2..10 {
        rename(&mut document, &format!("State {index}"));
        let stats = document.history_stats();
        assert!(stats.estimated_bytes <= limits.max_estimated_bytes);
        assert!(stats.undo_entries <= 2);
    }
    assert_eq!(document.history_stats().undo_entries, 2);
    assert_eq!(document.history_stats().evicted_entries, 7);
    assert!(document.undo());
    assert_eq!(document.model().project.parameters.name, "State 8");
    let bytes = document.history_stats().estimated_bytes;
    assert!(document.undo());
    assert_eq!(document.model().project.parameters.name, "State 7");
    assert_eq!(document.history_stats().estimated_bytes, bytes);
    assert!(!document.undo());
    rename(&mut document, "Branched");
    assert!(!document.can_redo());
    assert_eq!(document.history_stats().undo_entries, 1);
    assert_eq!(document.history_stats().evicted_entries, 7);
    assert!(document.history_stats().estimated_bytes < bytes);
    assert!(document.undo());
    assert_eq!(document.model().project.parameters.name, "State 7");
}

#[test]
fn failed_reservation_validation_and_no_op_preserve_all_state() {
    let mut document = Document::new("Start").unwrap();
    rename(&mut document, "First");
    rename(&mut document, "Final");
    assert!(document.undo());
    let stats = document.history_stats();
    document
        .set_history_limits(HistoryLimits {
            max_estimated_bytes: stats.estimated_bytes,
            ..stats.limits
        })
        .unwrap();
    document.drain_events();
    let before = document.model().clone();
    let revision = document.revision();
    let session = document.session_id();
    let stats = document.history_stats();
    for (label, name) in [
        ("Too large", "x".repeat(stats.estimated_bytes)),
        ("Invalid", " ".into()),
    ] {
        assert!(
            document
                .execute(label, vec![Command::RenameProject(name)])
                .is_err()
        );
        assert_eq!(document.model(), &before);
        assert_eq!(document.history_stats(), stats);
        assert_eq!(document.revision(), revision);
        assert_eq!(document.session_id(), session);
        assert!(document.drain_events().is_empty());
    }
    rename(&mut document, &before.project.parameters.name);
    assert_eq!(document.history_stats(), stats);
    assert_eq!(document.revision(), revision);
    assert!(document.drain_events().is_empty());
    assert!(document.redo());
    assert_eq!(document.model().project.parameters.name, "Final");
}

#[test]
fn limits_are_atomic_and_trimming_keeps_nearest_redo_order() {
    let mut document = Document::new("State 0").unwrap();
    for index in 1..=5 {
        rename(&mut document, &format!("State {index}"));
    }
    for _ in 0..5 {
        assert!(document.undo());
    }
    let stats = document.history_stats();
    for limits in [
        HistoryLimits {
            max_entries: 0,
            ..stats.limits
        },
        HistoryLimits {
            max_estimated_bytes: 1,
            ..stats.limits
        },
    ] {
        assert!(document.set_history_limits(limits).is_err());
        assert_eq!(document.history_stats(), stats);
    }
    document.drain_events();
    let revision = document.revision();
    document
        .set_history_limits(HistoryLimits {
            max_entries: 2,
            ..stats.limits
        })
        .unwrap();
    assert_eq!(document.history_stats().evicted_entries, 3);
    assert_eq!(document.revision(), revision);
    assert!(document.drain_events().is_empty());
    assert!(document.redo());
    assert_eq!(document.model().project.parameters.name, "State 1");
    assert!(document.redo());
    assert_eq!(document.model().project.parameters.name, "State 2");
    assert!(!document.redo());
    assert!(document.undo());
    assert_eq!(document.model().project.parameters.name, "State 1");
}

#[test]
fn default_policy_is_bounded_and_new_document_starts_fresh() {
    let mut document = Document::new("Start").unwrap();
    for index in 0..140 {
        rename(&mut document, &format!("Name {index}"));
    }
    let stats = document.history_stats();
    assert_eq!(stats.undo_entries, 128);
    assert_eq!(stats.evicted_entries, 12);
    assert!(stats.estimated_bytes <= stats.limits.max_estimated_bytes);
    let reopened = Document::from_model(document.model().clone()).unwrap();
    assert_eq!(reopened.history_stats().estimated_bytes, 0);
    assert_eq!(reopened.history_stats().evicted_entries, 0);
    assert!(!reopened.can_undo());
    assert!(!reopened.can_redo());
    assert_ne!(reopened.session_id(), document.session_id());
}

#[test]
fn deterministic_long_sequence_matches_a_bounded_reference_timeline() {
    let mut document = Document::new("Initial").unwrap();
    document
        .set_history_limits(HistoryLimits {
            max_entries: 7,
            ..HistoryLimits::default()
        })
        .unwrap();
    let mut current = "Initial".to_owned();
    let mut undo = VecDeque::<(String, String)>::new();
    let mut redo = VecDeque::<(String, String)>::new();
    let mut seed = 42_u64;
    let mut revision = 0;
    for index in 0..2000 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        match (seed >> 32) % 4 {
            0 => {
                let expected = undo.pop_back();
                assert_eq!(document.undo(), expected.is_some());
                if let Some(entry) = expected {
                    current = entry.0.clone();
                    redo.push_back(entry);
                    revision += 1;
                }
            }
            1 => {
                let expected = redo.pop_back();
                assert_eq!(document.redo(), expected.is_some());
                if let Some(entry) = expected {
                    current = entry.1.clone();
                    undo.push_back(entry);
                    revision += 1;
                }
            }
            _ => {
                let next = format!("State {index}");
                rename(&mut document, &next);
                undo.push_back((current, next.clone()));
                if undo.len() > 7 {
                    undo.pop_front();
                }
                redo.clear();
                current = next;
                revision += 1;
            }
        }
        assert_eq!(document.model().project.parameters.name, current);
        assert_eq!(document.revision(), revision);
        let stats = document.history_stats();
        assert_eq!(
            (stats.undo_entries, stats.redo_entries),
            (undo.len(), redo.len())
        );
        assert!(stats.estimated_bytes <= stats.limits.max_estimated_bytes);
        document.drain_events();
    }
}
