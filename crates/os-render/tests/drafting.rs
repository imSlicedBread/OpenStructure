use os_core::{Id, Point2};
use os_geometry::plan::*;
use os_model::*;
use os_render::{plan::*, sheet::*};
use std::collections::BTreeMap;
#[test]
fn note_layout_pick_alignment_leader_and_sheet_pdf_share_paper_anchors_at_multiple_scales() {
    for scale in [25., 100.] {
        for alignment in [
            NoteAlignment::Left,
            NoteAlignment::Center,
            NoteAlignment::Right,
        ] {
            let c = PlanContext {
                annotation_crop: None,
                session_id: Id::new(),
                model_revision: 0,
                view_id: Id::new(),
                settings_revision: 0,
                basis: HorizontalBasis::default(),
                range: PlanRange::default(),
                crop: Some(PlanCrop {
                    min: Point2::new(-5., -5.),
                    max: Point2::new(5., 5.),
                }),
                scale_denominator: scale,
                show_walls: true,
                show_extensions: false,
            };
            let mut p = TextNoteParams::new(c.view_id);
            p.text = "AB\nC".into();
            p.width_mm = 20.;
            p.text_height_mm = 2.;
            p.alignment = alignment;
            p.position = Point2::new(-1., 1.);
            p.leader = Some(Point2::new(-2., 0.));
            let note = NoteGraphic {
                entity: Id::new(),
                parameters: p,
            };
            let layout = note.layout(scale).unwrap();
            assert_eq!(layout.glyphs.len(), 3);
            assert_eq!(layout.leader.len(), 3);
            assert!((layout.bounds.max.x - layout.bounds.min.x - 20. * scale / 1000.).abs() < 1e-9);
            let d = PlanDrawing::from_prisms(c, &BTreeMap::new(), vec![])
                .unwrap()
                .with_text_notes(vec![note.clone()])
                .unwrap();
            assert_eq!(
                d.pick_text_note(c, Point2::new(-0.99, 0.99), 0.01).unwrap(),
                Some(note.entity)
            );
            let vp = PaperViewport {
                center_mm: Point2::new(210., 148.5),
                width_mm: 300.,
                height_mm: 250.,
                model_center_m: Point2::default(),
                scale_denominator: scale,
            };
            let page = compose_view_sheet(
                PaperSheetInfo {
                    width_mm: 420.,
                    height_mm: 297.,
                    number: "A1",
                    name: "Notes",
                },
                "Detail",
                vp,
                c,
                &d,
            )
            .unwrap();
            let pdf = String::from_utf8(page.to_pdf().unwrap()).unwrap();
            for (anchor, ch) in layout.glyphs {
                let p = vp.model_to_paper(anchor).unwrap();
                assert!(page.marks().iter().any(|m|matches!(&m.kind,PaperMarkKind::Text {baseline_mm,text,size_mm,..} if *baseline_mm==p && text==&ch.to_string() && *size_mm==2.)));
                assert!(pdf.contains(&format!(
                    "1 0 0 1 {:.6} {:.6} Tm <{:02X}>",
                    p.x * 72. / 25.4,
                    (297. - p.y) * 72. / 25.4,
                    ch as u32
                )));
            }
            for (a, b) in layout.leader {
                let points = [vp.model_to_paper(a).unwrap(), vp.model_to_paper(b).unwrap()];
                assert!(page.marks().iter().any(|m|matches!(&m.kind,PaperMarkKind::Path {points_mm,..} if points_mm.as_slice()==points)));
            }
            // A sheet scale override reflows paper-sized placement, not the persisted note.
            let other = note.layout(scale * 2.).unwrap();
            assert!((other.bounds.max.x - other.bounds.min.x - 40. * scale / 1000.).abs() < 1e-9);
            let mut overridden = vp;
            overridden.scale_denominator = scale * 2.;
            let other_page = compose_view_sheet(
                PaperSheetInfo {
                    width_mm: 420.,
                    height_mm: 297.,
                    number: "A2",
                    name: "Override",
                },
                "Detail",
                overridden,
                c,
                &d,
            )
            .unwrap();
            for (anchor, ch) in other.glyphs {
                let p = overridden.model_to_paper(anchor).unwrap();
                assert!(other_page.marks().iter().any(|m|matches!(&m.kind,PaperMarkKind::Text {baseline_mm,text,size_mm,..} if *baseline_mm==p && text==&ch.to_string() && *size_mm==2.)));
            }
            let mut stale = c;
            stale.model_revision += 1;
            assert!(d.text_notes(stale).is_err());
        }
    }
}
