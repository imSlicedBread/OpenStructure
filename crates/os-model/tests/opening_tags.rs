use os_core::{Id, Point2};
use os_model::*;
fn setup() -> (Model, OpeningTag) {
    let mut model = Model::new("Opening tags");
    let level = *model.levels.keys().next().unwrap();
    let wall = Wall::new(
        "org.openstructure.walls.wall",
        WallParams {
            name: "Host".into(),
            path: os_model::WallPath::Straight {
                start: Point2::new(0., 0.),
                end: Point2::new(8., 0.),
            },
            thickness: 0.2,
            height: 3.,
            level,
            material: None,
        },
    );
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
            name: "D-01".into(),
            host: wall.id(),
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
    let view = View::new("core.view", ViewParams::floor_plan("Plan", level));
    let tag = OpeningTag::new(
        "core.opening_tag",
        OpeningTagParams {
            view: view.id(),
            opening: opening.id(),
            position: Point2::new(2., 1.5),
            label_preset: Default::default(),
        },
    );
    model.walls.insert(wall.id(), wall);
    model.opening_types.insert(ty.id(), ty);
    model.openings.insert(opening.id(), opening);
    model.views.insert(view.id(), view);
    (model, tag)
}

#[test]
fn opening_tags_resolve_live_dimensions_type_instance_rehost_and_orphans() {
    let (mut model, tag) = setup();
    model.opening_tags.insert(tag.id(), tag.clone());
    model.validate().unwrap();
    assert_eq!(
        tag.parameters.label(&model).0,
        "D-01 · D900 · 0.900 × 2.100 m"
    );
    let id = tag.parameters.opening;
    let ty = model.openings[&id].parameters.type_id().unwrap();
    model.opening_types.get_mut(&ty).unwrap().parameters.width = 1.2;
    model.opening_types.get_mut(&ty).unwrap().parameters.name = "D1200".into();
    model.openings.get_mut(&id).unwrap().parameters.name = "D-02".into();
    model
        .openings
        .get_mut(&id)
        .unwrap()
        .parameters
        .height_override = Some(2.2);
    assert_eq!(
        tag.parameters.label(&model).0,
        "D-02 · D1200 · 1.200 × 2.200 m"
    );
    let mut host = model.walls.values().next().unwrap().clone();
    host.header.id = Id::new();
    host.parameters.path.straight_start_mut().unwrap().y = 4.;
    host.parameters.path.straight_end_mut().unwrap().y = 4.;
    let host_id = host.id();
    model.walls.insert(host_id, host);
    model.openings.get_mut(&id).unwrap().parameters.host = host_id;
    model.validate().unwrap();
    assert!(tag.parameters.label(&model).1.is_none());
    let old_level = model.levels.values().next().unwrap().clone();
    let upper = Level::new(
        "core.level",
        LevelParams {
            name: "Upper".into(),
            elevation: 3.,
            building: old_level.parameters.building,
        },
    );
    model.walls.get_mut(&host_id).unwrap().parameters.level = upper.id();
    model.levels.insert(upper.id(), upper);
    model.validate().unwrap();
    assert_eq!(
        tag.parameters.label(&model).1.as_deref(),
        Some("Opening is on another level")
    );
    assert!(tag.parameters.validate_creation(&model).is_err());
    model.openings.remove(&id);
    model.validate().unwrap();
    assert_eq!(
        tag.parameters.label(&model).0,
        "Opening tag · Missing opening"
    );
    assert_eq!(model.opening_tags[&tag.id()], tag);
}

#[test]
fn opening_tag_presets_resolve_all_typed_and_legacy_doors_and_windows_live() {
    use OpeningTagLabelPreset::*;
    assert_eq!(OpeningTagLabelPreset::default(), Full);
    for kind in [OpeningKind::Door, OpeningKind::Window] {
        for typed in [true, false] {
            let (mut model, mut tag) = setup();
            let id = tag.parameters.opening;
            let ty = model.openings[&id].parameters.type_id().unwrap();
            model.opening_types.get_mut(&ty).unwrap().parameters.kind = kind;
            if !typed {
                model.openings.get_mut(&id).unwrap().parameters.definition =
                    OpeningDefinition::Legacy {
                        kind,
                        width: 0.9,
                        height: 2.1,
                        sill: 0.,
                    };
            }
            for edited in [false, true] {
                if edited {
                    let opening = &mut model.openings.get_mut(&id).unwrap().parameters;
                    opening.name = "Changed instance".into();
                    if typed {
                        opening.height_override = Some(2.2);
                        let parameters = &mut model.opening_types.get_mut(&ty).unwrap().parameters;
                        parameters.name = "Changed type".into();
                        parameters.width = 1.2;
                    } else {
                        opening.definition = OpeningDefinition::Legacy {
                            kind,
                            width: 1.2,
                            height: 2.2,
                            sill: 0.,
                        };
                    }
                }
                model.validate().unwrap();
                let name = if edited { "Changed instance" } else { "D-01" };
                let type_name = if !typed {
                    "Legacy"
                } else if edited {
                    "Changed type"
                } else {
                    "D900"
                };
                let dimensions = if edited {
                    "1.200 × 2.200 m"
                } else {
                    "0.900 × 2.100 m"
                };
                for (preset, expected) in [
                    (Full, format!("{name} · {type_name} · {dimensions}")),
                    (InstanceName, name.into()),
                    (TypeAndDimensions, format!("{type_name} · {dimensions}")),
                    (DimensionsOnly, dimensions.into()),
                ] {
                    tag.parameters.label_preset = preset;
                    assert_eq!(tag.parameters.label(&model), (expected, None));
                }
            }
        }
    }
}

#[test]
fn opening_tag_presets_keep_every_orphan_diagnostic() {
    use OpeningTagLabelPreset::*;
    for reason in [
        "Missing plan view",
        "Missing opening",
        "Missing host wall",
        "Opening is on another level",
        "Invalid opening",
    ] {
        let (mut model, mut tag) = setup();
        let opening = tag.parameters.opening;
        let host = model.openings[&opening].parameters.host;
        match reason {
            "Missing plan view" => {
                model.views.remove(&tag.parameters.view);
            }
            "Missing opening" => {
                model.openings.remove(&opening);
            }
            "Missing host wall" => {
                model.walls.remove(&host);
            }
            "Opening is on another level" => {
                model.walls.get_mut(&host).unwrap().parameters.level = Id::new();
            }
            _ => {
                model
                    .openings
                    .get_mut(&opening)
                    .unwrap()
                    .parameters
                    .definition = OpeningDefinition::Typed { type_id: Id::new() };
            }
        }
        for preset in [Full, InstanceName, TypeAndDimensions, DimensionsOnly] {
            tag.parameters.label_preset = preset;
            assert_eq!(
                tag.parameters.label(&model),
                (format!("Opening tag · {reason}"), Some(reason.into()))
            );
        }
    }
}

#[test]
fn opening_tag_identity_uniqueness_view_ownership_and_serialization_are_strict() {
    let (mut model, tag) = setup();
    tag.parameters.validate_creation(&model).unwrap();
    model.opening_tags.insert(tag.id(), tag.clone());
    let mut duplicate = OpeningTag::new("core.opening_tag", tag.parameters.clone());
    model.opening_tags.insert(duplicate.id(), duplicate.clone());
    assert!(model.validate().is_err());
    model.opening_tags.remove(&duplicate.id());
    let level = model.views[&tag.parameters.view].parameters.level.unwrap();
    let view = View::new("core.view", ViewParams::floor_plan("Second", level));
    duplicate.parameters.view = view.id();
    model.views.insert(view.id(), view);
    model.opening_tags.insert(duplicate.id(), duplicate);
    model.validate().unwrap();
    for position in [
        Point2::new(f64::NAN, 0.),
        Point2::new(0., f64::INFINITY),
        Point2::new(1e6 + 1., 0.),
    ] {
        let mut bad = tag.parameters.clone();
        bad.position = position;
        assert!(bad.validate(&model).is_err());
    }
    let mut bad = serde_json::to_value(&tag).unwrap();
    bad["parameters"]["label"] = "Frozen".into();
    assert!(serde_json::from_value::<OpeningTag>(bad).is_err());
    model.views.remove(&tag.parameters.view);
    assert!(model.validate().is_err());
}
