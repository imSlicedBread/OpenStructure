//! Detail views retain the native typed-opening source pipeline and source identities.
use super::*;
use os_model::{
    LiteSide, OpeningAttachment, OpeningExtrusion, SectionCalloutBounds, SectionDisplayMode,
    SideLite,
};

#[test]
fn section_callout_details_keep_typed_frames_lites_accessories_pins_and_paper() {
    for mode in [
        SectionDisplayMode::CutOnly,
        SectionDisplayMode::CutAndProjection,
    ] {
        let (mut e, level, _, _, window, door) = fixture();
        let original = e.document.model().clone();
        let window_type = original
            .resolve_opening(&original.openings[&window].parameters)
            .unwrap()
            .type_id
            .unwrap();
        let mut ty = original.opening_types[&window_type].parameters.clone();
        ty.family.side_lite = Some(SideLite {
            side: LiteSide::End,
            width_fraction: 0.2,
            mullion_width: 0.04,
            material: ty.family.panel_material,
        });
        let mut accessory = OpeningExtrusion::new("Detail accessory");
        accessory.attachment = OpeningAttachment::OpeningFixed;
        accessory.normal_offset = 0.;
        let accessory_id = accessory.id;
        ty.family.extrusions.push(accessory);
        let mut door_ty = ty.clone();
        door_ty.kind = OpeningKind::Door;
        door_ty.width = 1.;
        door_ty.height = 2.1;
        door_ty.sill = 0.;
        let door_ty = OpeningType::new("core.opening_type", door_ty);
        let door_type = door_ty.id();
        let mut door_p = original.openings[&door].parameters.clone();
        door_p.definition = OpeningDefinition::Typed { type_id: door_type };
        door_p.width_override = Some(1.1);
        let mut window_p = original.openings[&window].parameters.clone();
        window_p.height_override = Some(1.6);
        window_p.lite_side_override = Some(LiteSide::Start);
        e.document
            .execute(
                "Typed details",
                vec![
                    Command::UpdateOpeningType {
                        id: window_type,
                        parameters: ty,
                    },
                    Command::AddOpeningType(door_ty),
                    Command::UpdateOpening {
                        id: door,
                        parameters: door_p,
                    },
                    Command::UpdateOpening {
                        id: window,
                        parameters: window_p,
                    },
                ],
            )
            .unwrap();
        let mut settings =
            SectionViewSettings::new(Point2::new(0., 0.), Point2::new(8., 0.), -1., 5.);
        settings.mode = mode;
        let parent = e.create_section_view("Parent", level, settings).unwrap();
        let source_before = e.document.model().openings.clone();
        let (marker, target, commands) = e
            .document
            .enlarged_section_commands(
                parent,
                "Detail".into(),
                SectionCalloutBounds {
                    min: Point2::new(1., 0.),
                    max: Point2::new(7., 4.),
                },
                25.,
            )
            .unwrap();
        e.document.execute("Section detail", commands).unwrap();
        assert_eq!(e.document.model().openings, source_before);
        let context = e.native_section_context(target).unwrap();
        let drawing = e.native_drawing(target).unwrap();
        assert_eq!(context.scale_denominator, 25.);
        let parent_context = e.native_section_context(parent).unwrap();
        let parent_drawing = e.native_drawing(parent).unwrap();
        assert_eq!(
            parent_drawing.callouts(parent_context).unwrap()[0].entity,
            marker
        );
        assert_eq!(
            parent_drawing.callouts(parent_context).unwrap()[0].label,
            "Unplaced"
        );
        let viewport = PaperViewport {
            center_mm: Point2::new(210., 148.5),
            width_mm: 320.,
            height_mm: 220.,
            model_center_m: Point2::new(4., 2.),
            scale_denominator: 25.,
        };
        let page = compose_view_sheet(
            PaperSheetInfo {
                width_mm: 420.,
                height_mm: 297.,
                number: "A201",
                name: "Detail",
            },
            "Detail",
            viewport,
            context,
            &drawing,
        )
        .unwrap();
        let pdf = String::from_utf8(page.to_pdf().unwrap()).unwrap();
        for id in [window, door] {
            let lines: Vec<_> = drawing
                .provider_lines(context)
                .unwrap()
                .iter()
                .filter(|l| l.entity == id)
                .collect();
            assert!(lines.len() > 12, "frame, panel, lite and accessory edges");
            assert!(
                lines
                    .iter()
                    .any(|l| drawing.line_surface(id, l.feature).layer == Some(accessory_id))
            );
            assert!(
                lines
                    .iter()
                    .any(|l| drawing.line_surface(id, l.feature).material.is_some())
            );
            for line in lines {
                assert!(line.start.x >= 1. && line.end.x <= 7.);
                let points = [
                    viewport.model_to_paper(line.start).unwrap(),
                    viewport.model_to_paper(line.end).unwrap(),
                ];
                assert!(page.marks().iter().any(|m| matches!(&m.kind,PaperMarkKind::Path { points_mm,closed:false,.. } if points_mm.as_slice()==points)));
                let pt = 72. / 25.4;
                assert!(pdf.contains(&format!(
                    "{:.6} {:.6} m\n{:.6} {:.6} l",
                    points[0].x * pt,
                    (297. - points[0].y) * pt,
                    points[1].x * pt,
                    (297. - points[1].y) * pt
                )));
            }
        }
        // The detail remains live when an instance pin changes.
        let mut p = e.document.model().openings[&window].parameters.clone();
        p.height_override = Some(1.8);
        e.document
            .execute(
                "Change pin",
                vec![Command::UpdateOpening {
                    id: window,
                    parameters: p,
                }],
            )
            .unwrap();
        let current = e.native_section_context(target).unwrap();
        let updated = e.native_drawing(target).unwrap();
        assert!(drawing.provider_lines(current).is_err());
        assert_ne!(
            drawing.provider_lines(context).unwrap(),
            updated.provider_lines(current).unwrap()
        );
    }
}
