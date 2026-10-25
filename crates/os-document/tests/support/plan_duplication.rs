use os_core::{Id, Point2};
use os_model::*;

pub fn fixture() -> (Model, Id) {
    let mut m = Model::new("Duplication");
    let level = *m.levels.keys().next().unwrap();
    let mut p = ViewParams::floor_plan("Source", level);
    p.settings_revision = 9;
    let s = p.plan.as_mut().unwrap();
    s.basis.rotation = 0.63;
    s.crop = Some(PlanViewCrop {
        min: Point2::new(-2., -2.),
        max: Point2::new(2., 2.),
    });
    s.annotation_crop = s.crop;
    s.scale_denominator = 25.;
    s.visibility.doors = false;
    s.range.top = 4.;
    s.target_phase = m.phases.keys().next().copied();
    s.phase_filter = PhaseFilter::ShowNew;
    let view = View::new("core.view", p);
    let source = view.id();
    m.views.insert(source, view);
    let template = PlanGraphicsTemplate::new(
        "core.plan_graphics_template",
        PlanGraphicsTemplateParams {
            name: "Office".into(),
            styles: PlanGraphicsStyles::default(),
        },
    );
    let mut local = PlanGraphicsStyles::default();
    local.walls.cut.color = [20, 40, 80];
    m.plan_graphics.insert(
        source,
        PlanGraphicsBinding {
            template: Some(template.id()),
            local,
        },
    );
    m.plan_graphics_templates.insert(template.id(), template);
    let mut walls = Vec::new();
    for path in [
        WallPath::Straight {
            start: Point2::new(0., 0.),
            end: Point2::new(8., 0.),
        },
        WallPath::Straight {
            start: Point2::new(0., 0.),
            end: Point2::new(0., 8.),
        },
        WallPath::CircularArc {
            center: Point2::new(20., 20.),
            radius: 3.,
            start_angle_rad: 0.,
            signed_sweep_rad: 1.5,
        },
    ] {
        let w = Wall::new(
            "org.openstructure.walls.wall",
            WallParams {
                name: "Host".into(),
                path,
                thickness: 0.2,
                height: 3.,
                level,
                material: None,
            },
        );
        walls.push(w.id());
        m.walls.insert(w.id(), w);
    }
    for layout in [
        DimensionLayout::Aligned,
        DimensionLayout::Chain,
        DimensionLayout::Baseline,
        DimensionLayout::Angular,
        DimensionLayout::ArcLength,
    ] {
        let wall = if layout == DimensionLayout::ArcLength {
            walls[2]
        } else {
            walls[0]
        };
        let mut p = DimensionParams {
            view: source,
            layout,
            first: DimensionReference::WallEndpoint {
                wall,
                endpoint: DimensionEndpoint::Start,
            },
            second: DimensionReference::WallEndpoint {
                wall,
                endpoint: DimensionEndpoint::End,
            },
            additional: vec![],
            baseline_spacing_m: 0.25,
            offset_m: 1.,
            orphan_hint: Point2::new(1., 1.),
        };
        if layout == DimensionLayout::Angular {
            p.first = DimensionReference::WallEndpoint {
                wall: walls[0],
                endpoint: DimensionEndpoint::End,
            };
            p.second = DimensionReference::WallEndpoint {
                wall: walls[1],
                endpoint: DimensionEndpoint::End,
            };
        }
        if matches!(layout, DimensionLayout::Chain | DimensionLayout::Baseline) {
            p.second = DimensionReference::WallFace {
                wall,
                side: DimensionWallSide::Left,
                station_m: 4.,
            };
            p.first = DimensionReference::WallFace {
                wall,
                side: DimensionWallSide::Left,
                station_m: 0.,
            };
            p.additional.push(DimensionReference::WallFace {
                wall,
                side: DimensionWallSide::Left,
                station_m: 8.,
            });
        }
        let d = Dimension::new("core.dimension", p);
        m.dimensions.insert(d.id(), d);
    }
    let ty = OpeningType::new(
        "core.opening_type",
        OpeningTypeParams {
            window_operation: Default::default(),
            name: "D900".into(),
            family: OpeningFamily::default(),
            kind: OpeningKind::Door,
            width: 0.9,
            height: 2.1,
            sill: 0.,
            pane_position: WindowPanePosition::Center,
        },
    );
    let opening = Opening::new(
        "core.opening",
        OpeningParams {
            open_state: Default::default(),
            name: "Door".into(),
            host: walls[0],
            offset: 1.,
            definition: OpeningDefinition::Typed { type_id: ty.id() },
            width_override: None,
            height_override: None,
            sill_override: None,
            pane_position_override: None,
            lite_side_override: None,
            hinge: DoorHinge::Start,
            swing: DoorSwing::Left,
        },
    );
    let tag = OpeningTag::new(
        "core.opening_tag",
        OpeningTagParams {
            view: source,
            opening: opening.id(),
            position: Point2::new(30., 30.),
            label_preset: OpeningTagLabelPreset::DimensionsOnly,
        },
    );
    m.opening_types.insert(ty.id(), ty);
    m.openings.insert(opening.id(), opening);
    m.opening_tags.insert(tag.id(), tag);
    let room = Room::new(
        "core.room",
        RoomParams {
            number: "101".into(),
            name: "Office".into(),
            level,
            seed: Point2::new(1., 1.),
            floor_material: None,
            wall_material: None,
            ceiling_material: None,
            floor_finish: None,
            wall_finish: None,
            ceiling_finish: None,
            boundary_signature: vec![(walls[0], true), (walls[1], true), (walls[2], false)],
        },
    );
    let tag = RoomTag::new(
        "core.room_tag",
        RoomTagParams {
            view: source,
            room: room.id(),
            position: Point2::new(30., 30.),
        },
    );
    m.rooms.insert(room.id(), room);
    m.room_tags.insert(tag.id(), tag);
    let line = DetailLine::new(
        "core.detail_line",
        DetailLineParams {
            view: source,
            start: Point2::new(30., 30.),
            end: Point2::new(31., 30.),
        },
    );
    let mut note = TextNote::new("core.text_note", TextNoteParams::new(source));
    note.parameters.text = "Detail\nSecond line".into();
    note.parameters.position = Point2::new(30., 30.);
    note.parameters.alignment = NoteAlignment::Right;
    note.parameters.leader = Some(Point2::new(29., 29.));
    note.header
        .relationships
        .insert("peer".into(), vec![line.id(), source, walls[0]]);
    note.header
        .properties
        .insert("opaque".into(), serde_json::json!(line.id().to_string()));
    m.views
        .get_mut(&source)
        .unwrap()
        .header
        .relationships
        .insert("annotation".into(), vec![note.id()]);
    m.detail_lines.insert(line.id(), line);
    m.text_notes.insert(note.id(), note);
    let target = View::new("core.view", ViewParams::floor_plan("Target", level));
    let target_id = target.id();
    m.views.insert(target_id, target);
    for owns_target in [true, false] {
        let c = PlanCallout::new(
            "core.plan_callout",
            PlanCalloutParams {
                parent: source,
                target: target_id,
                owns_target,
                boundary: PlanViewCrop {
                    min: Point2::new(-1., -1.),
                    max: Point2::new(1., 1.),
                },
                destination: None,
            },
        );
        m.plan_callouts.insert(c.id(), c);
    }
    (m, source)
}
