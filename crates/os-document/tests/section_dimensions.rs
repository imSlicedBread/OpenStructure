#[path = "../../os-model/tests/support/section_dimensions.rs"]
mod fixture;
use os_document::{Command, Document};
use os_model::*;

#[test]
fn section_dimensions_atomic_commit_orphan_repair_undo_and_redo() {
    let (m, host, opening, _, p) = fixture::fixture();
    let d = Dimension::new("core.dimension", p);
    let id = d.id();
    let mut doc = Document::from_model(m.clone()).unwrap();
    doc.execute("Dimension", vec![Command::AddDimension(d.clone())])
        .unwrap();
    assert_eq!(doc.history_stats().undo_entries, 1);
    let committed = doc.model().clone();
    assert!(doc.undo());
    assert_eq!(doc.model(), &m);
    assert!(doc.redo());
    assert_eq!(doc.model(), &committed);
    doc.execute("Remove opening", vec![Command::RemoveOpening(opening)])
        .unwrap();
    assert_eq!(doc.model().dimensions[&id], d);
    assert_eq!(
        d.parameters.resolve(doc.model()),
        Err(DimensionDiagnostic::MissingOpening)
    );
    let before = doc.model().clone();
    let history = doc.history_stats();
    let mut invalid = d.parameters.clone();
    invalid.first = DimensionReference::SectionWall {
        wall: os_core::Id::new(),
        feature: ElevationDimensionFeature::Start,
    };
    assert!(
        doc.execute(
            "Invalid",
            vec![Command::UpdateDimension {
                id,
                parameters: invalid
            }]
        )
        .is_err()
    );
    assert_eq!(doc.model(), &before);
    assert_eq!(doc.history_stats(), history);
    let mut p = d.parameters.clone();
    p.first = DimensionReference::SectionWall {
        wall: host,
        feature: ElevationDimensionFeature::Start,
    };
    doc.execute(
        "Repair first",
        vec![Command::UpdateDimension {
            id,
            parameters: p.clone(),
        }],
    )
    .unwrap();
    assert_eq!(
        p.resolve(doc.model()),
        Err(DimensionDiagnostic::MissingOpening)
    );
    p.second = DimensionReference::SectionWall {
        wall: host,
        feature: ElevationDimensionFeature::End,
    };
    doc.execute(
        "Repair second",
        vec![Command::UpdateDimension {
            id,
            parameters: p.clone(),
        }],
    )
    .unwrap();
    assert_eq!(p.resolve(doc.model()).unwrap().length_metres, 10.);
    assert_eq!(doc.model().dimensions[&id].header, d.header);
    assert!(doc.undo());
    assert!(
        doc.model().dimensions[&id]
            .parameters
            .resolve(doc.model())
            .is_err()
    );
    assert!(doc.redo());
    assert_eq!(doc.model().dimensions[&id].parameters, p);
}
