use os_core::{Id, Point2};
use os_geometry::{
    SurfaceIdentity,
    plan::{HorizontalBasis, PlanCrop, PlanFootprint, PlanRange, PlanRole},
};
use os_render::plan::{
    PlanContext, PlanDrawing, PlanElementAppearance, PlanStroke, PlanWallContact,
};
use std::collections::BTreeMap;

type Cells = BTreeMap<Id, Vec<(SurfaceIdentity, PlanFootprint)>>;
fn context(crop: Option<PlanCrop>) -> PlanContext {
    PlanContext {
        session_id: Id::new(),
        model_revision: 0,
        view_id: Id::new(),
        settings_revision: 0,
        basis: HorizontalBasis::default(),
        range: PlanRange::default(),
        crop,
        annotation_crop: None,
        scale_denominator: 100.0,
        show_walls: true,
        show_extensions: true,
    }
}
fn rect(role: PlanRole, bounds: [f64; 4]) -> PlanFootprint {
    let [x0, y0, x1, y1] = bounds;
    PlanFootprint::from_convex(
        role,
        vec![
            Point2::new(x0, y0),
            Point2::new(x1, y0),
            Point2::new(x1, y1),
            Point2::new(x0, y1),
        ],
        None,
    )
    .unwrap()
    .unwrap()
}
fn pair(roles: [PlanRole; 2]) -> (Cells, PlanWallContact) {
    let (a, b) = (Id::new(), Id::new());
    (
        BTreeMap::from([
            (
                a,
                vec![(
                    SurfaceIdentity::default(),
                    rect(roles[0], [0., -1., 2., 1.]),
                )],
            ),
            (
                b,
                vec![(
                    SurfaceIdentity::default(),
                    rect(roles[1], [2., -1., 4., 1.]),
                )],
            ),
        ]),
        PlanWallContact {
            members: [a, b],
            owner: a.min(b),
            trace: (Point2::new(2., -1.), Point2::new(2., 1.)),
        },
    )
}
fn drawing(ctx: PlanContext, cells: &Cells) -> PlanDrawing {
    let clipped = cells
        .iter()
        .map(|(id, cells)| {
            (
                *id,
                cells
                    .iter()
                    .filter_map(|(s, f)| {
                        PlanFootprint::from_convex(f.role, f.vertices().to_vec(), ctx.crop)
                            .unwrap()
                            .map(|f| (*s, f))
                    })
                    .collect(),
            )
        })
        .collect();
    PlanDrawing::from_layered_footprints(ctx, &clipped, vec![]).unwrap()
}
fn contact_length(d: &PlanDrawing, ctx: PlanContext, entity: Id) -> f64 {
    d.items(ctx)
        .unwrap()
        .iter()
        .filter(|i| i.entity == entity)
        .flat_map(|i| i.outline())
        .filter(|(a, b)| (a.x - 2.).abs() < 1e-8 && (b.x - 2.).abs() < 1e-8)
        .map(|(a, b)| a.distance(b))
        .sum()
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-8, "{a} != {b}");
}
fn style() -> PlanStroke {
    PlanStroke {
        color: [21, 54, 87],
        weight_mm: 0.35,
        dashed: false,
    }
}
fn appearance(role: PlanRole, style: PlanStroke) -> PlanElementAppearance {
    let mut a = PlanElementAppearance::default();
    match role {
        PlanRole::Cut => a.cut = Some(style),
        PlanRole::Overhead => a.overhead = Some(style),
        _ => a.projected = Some(style),
    }
    a
}

#[test]
fn all_roles_require_explicit_contact_and_resolved_color_and_stroke_agreement() {
    for role in [
        PlanRole::Cut,
        PlanRole::Projected,
        PlanRole::Depth,
        PlanRole::Overhead,
    ] {
        let (mut cells, contact) = pair([role; 2]);
        let ctx = context(None);
        for id in contact.members {
            close(contact_length(&drawing(ctx, &cells), ctx, id), 2.);
        }
        let materials = [Id::new(), Id::new()];
        for (id, material) in contact.members.into_iter().zip(materials) {
            cells.get_mut(&id).unwrap()[0].0.material = Some(material);
        }
        for same_color in [true, false] {
            for mismatch in 0..4 {
                let mut other = style();
                match mismatch {
                    1 => other.color = [1, 2, 3],
                    2 => other.weight_mm = 0.7,
                    3 => other.dashed = true,
                    _ => {}
                }
                let d = drawing(ctx, &cells)
                    .with_material_colors(BTreeMap::from([
                        (materials[0], [10, 20, 30]),
                        (
                            materials[1],
                            if same_color {
                                [10, 20, 30]
                            } else {
                                [30, 20, 10]
                            },
                        ),
                    ]))
                    .with_appearances(BTreeMap::from([
                        (contact.members[0], appearance(role, style())),
                        (contact.members[1], appearance(role, other)),
                    ]))
                    .unwrap()
                    .with_wall_join_graphics(&[contact], &cells)
                    .unwrap();
                for id in contact.members {
                    close(
                        contact_length(&d, ctx, id),
                        if (!same_color || mismatch != 0) && id == contact.owner {
                            2.
                        } else {
                            0.
                        },
                    );
                }
                close(
                    d.items(ctx)
                        .unwrap()
                        .iter()
                        .map(|i| i.footprint.area())
                        .sum(),
                    8.,
                );
            }
        }
    }
}

#[test]
fn role_discontinuities_retain_owner_even_when_strokes_match() {
    for roles in [
        [PlanRole::Cut, PlanRole::Projected],
        [PlanRole::Projected, PlanRole::Depth],
        [PlanRole::Depth, PlanRole::Overhead],
    ] {
        let (cells, mut contact) = pair(roles);
        let ctx = context(None);
        for owner in contact.members {
            contact.owner = owner;
            let d = drawing(ctx, &cells)
                .with_appearances(
                    contact
                        .members
                        .into_iter()
                        .zip(roles)
                        .map(|(id, r)| (id, appearance(r, style())))
                        .collect(),
                )
                .unwrap()
                .with_wall_join_graphics(&[contact], &cells)
                .unwrap();
            for id in contact.members {
                close(
                    contact_length(&d, ctx, id),
                    if id == owner { 2. } else { 0. },
                );
            }
        }
    }
}

#[test]
fn layer_intervals_keep_only_discontinuity_and_preserve_surface_identity() {
    let (mut cells, mut contact) = pair([PlanRole::Cut; 2]);
    contact.owner = contact.members[0];
    cells.insert(
        contact.members[1],
        vec![
            (
                SurfaceIdentity::default(),
                rect(PlanRole::Cut, [2., -1., 4., 0.]),
            ),
            (
                SurfaceIdentity {
                    material: Some(Id::new()),
                    layer: Some(Id::new()),
                },
                rect(PlanRole::Cut, [2., 0., 4., 1.]),
            ),
        ],
    );
    let ctx = context(None);
    let before = drawing(ctx, &cells);
    let d = drawing(ctx, &cells)
        .with_wall_join_graphics(&[contact], &cells)
        .unwrap();
    close(contact_length(&d, ctx, contact.owner), 1.);
    close(contact_length(&d, ctx, contact.members[1]), 0.);
    for (old, new) in before.items(ctx).unwrap().iter().zip(d.items(ctx).unwrap()) {
        assert_eq!(old.entity, new.entity);
        assert_eq!(old.surface, new.surface);
        assert_eq!(old.footprint.vertices(), new.footprint.vertices());
    }
}

#[test]
fn partial_contacts_crop_and_reversed_traces_preserve_exposed_edges() {
    let (cells, mut contact) = pair([PlanRole::Cut; 2]);
    contact.trace = (Point2::new(2., 0.5), Point2::new(2., -0.5));
    let ctx = context(None);
    let d = drawing(ctx, &cells)
        .with_wall_join_graphics(&[contact], &cells)
        .unwrap();
    for id in contact.members {
        close(contact_length(&d, ctx, id), 1.);
    }
    let cropped = context(Some(PlanCrop {
        min: Point2::new(0., 0.),
        max: Point2::new(4., 1.),
    }));
    let d = drawing(cropped, &cells)
        .with_wall_join_graphics(&[contact], &cells)
        .unwrap();
    for id in contact.members {
        close(contact_length(&d, cropped, id), 0.5);
    }
    let cropped = context(Some(PlanCrop {
        min: Point2::new(0., -1.),
        max: Point2::new(2., 1.),
    }));
    let d = drawing(cropped, &cells)
        .with_wall_join_graphics(&[contact], &cells)
        .unwrap();
    close(contact_length(&d, cropped, contact.members[0]), 2.);
}

#[test]
fn coincident_crop_edges_are_preserved_even_with_original_contact_provenance() {
    let (mut cells, contact) = pair([PlanRole::Cut; 2]);
    // Two contributors on the visible side can arise from projected cells.
    // A real original boundary at the trace must still preserve the crop edge.
    cells.get_mut(&contact.members[0]).unwrap()[0].1 = rect(PlanRole::Cut, [2., -1., 3., 1.]);
    let ctx = context(Some(PlanCrop {
        min: Point2::new(2., -1.),
        max: Point2::new(4., 1.),
    }));
    let d = drawing(ctx, &cells)
        .with_wall_join_graphics(&[contact], &cells)
        .unwrap();
    for id in contact.members {
        close(contact_length(&d, ctx, id), 2.);
    }
    // Repeat when the visible boundary was introduced by crop alone.
    cells.get_mut(&contact.members[0]).unwrap()[0].1 = rect(PlanRole::Cut, [0., -1., 3., 1.]);
    let d = drawing(ctx, &cells)
        .with_wall_join_graphics(&[contact], &cells)
        .unwrap();
    for id in contact.members {
        close(contact_length(&d, ctx, id), 2.);
    }
}

#[test]
fn overhead_removed_contact_cannot_be_picked_and_stale_context_is_rejected() {
    let (cells, contact) = pair([PlanRole::Overhead; 2]);
    let ctx = context(None);
    let d = drawing(ctx, &cells)
        .with_wall_join_graphics(&[contact], &cells)
        .unwrap();
    let camera = os_render::plan::PlanCamera::default();
    for viewport in [[1280., 800.], [1000. / 1.5, 650. / 1.5]] {
        let pointer = camera.project(Point2::new(2., 0.), viewport).unwrap();
        assert!(
            d.hits_screen(ctx, camera, viewport, pointer, 0.1)
                .unwrap()
                .is_empty()
        );
    }
    assert!(
        d.items(PlanContext {
            model_revision: 1,
            ..ctx
        })
        .is_err()
    );
    assert!(
        d.items(PlanContext {
            settings_revision: 1,
            ..ctx
        })
        .is_err()
    );
}

#[test]
fn allowed_cell_counts_cannot_trigger_unbounded_quadratic_partition_work() {
    let (mut cells, contact) = pair([PlanRole::Cut; 2]);
    // Below the 10,000-cell drawing limit but enough distinct stations to
    // exceed the partition work budget. Different layers avoid a quadratic
    // provenance match and exercise the partition guard itself.
    for id in contact.members {
        cells.insert(
            id,
            (0..300)
                .map(|i| {
                    (
                        SurfaceIdentity {
                            layer: Some(Id::new()),
                            material: None,
                        },
                        rect(
                            PlanRole::Cut,
                            if id == contact.members[0] {
                                [0., -1. + i as f64 / 300., 2., 1.]
                            } else {
                                [2., -1. + i as f64 / 300., 4., 1.]
                            },
                        ),
                    )
                })
                .collect(),
        );
    }
    let error = drawing(context(None), &cells)
        .with_wall_join_graphics(&[contact], &cells)
        .err()
        .unwrap();
    assert!(
        error.to_string().contains("work budget exceeded"),
        "{error}"
    );
}
