use os_core::{Id, Point2};
use os_geometry::{GeometryKernel, PrismKernel};
use os_ifc::{IfcAdapter, WallIfc};
use os_model::{
    Column, ColumnParams, DoorHinge, DoorSwing, Level, LevelParams, Model, Opening,
    OpeningDefinition, OpeningFamily, OpeningHostCut, OpeningKind, OpeningParams, OpeningType,
    OpeningTypeParams, Wall, WallParams, WindowPanePosition,
};

fn model() -> Model {
    let mut m = Model::new("Project 'quoted' \\ café 🧱");
    let building = *m.buildings.keys().next().unwrap();
    let level = Level::new(
        "core.level",
        LevelParams {
            name: "Upper étage".into(),
            elevation: 4.2,
            building,
        },
    );
    let upper = level.id();
    m.levels.insert(upper, level);
    let ground = *m
        .levels
        .values()
        .find(|l| l.parameters.elevation == 0.)
        .map(|l| &l.header.id)
        .unwrap();
    for (level, start, end) in [
        (ground, Point2::new(-2., 1.), Point2::new(2., 4.)),
        (upper, Point2::new(8., -3.), Point2::new(5., -7.)),
    ] {
        let wall = Wall::new(
            "org.openstructure.walls.wall",
            WallParams {
                name: "Wall 'α'".into(),
                start,
                end,
                thickness: 0.3,
                height: 3.5,
                level,
                material: None,
            },
        );
        m.walls.insert(wall.id(), wall);
    }
    m
}

#[test]
fn semantic_geometry_round_trip_and_explicit_losses() {
    let m = model();
    let output = WallIfc.export_report(&m).unwrap();
    assert_eq!(output.warnings.len(), 1);
    assert!(WallIfc.export(&m).is_err());
    let input = WallIfc.import_report(&output.value).unwrap();
    let b = input.value;
    assert!(!input.warnings.is_empty());
    assert_eq!(m.project, b.project);
    assert_eq!(m.sites, b.sites);
    assert_eq!(m.buildings, b.buildings);
    assert_eq!(m.levels, b.levels);
    assert_eq!(m.walls, b.walls);
    for (id, wall) in &m.walls {
        let original = PrismKernel
            .tessellate(
                &os_walls::wall_solid(
                    &wall.parameters,
                    m.levels[&wall.parameters.level].parameters.elevation,
                )
                .unwrap(),
            )
            .unwrap();
        let imported = &b.walls[id];
        let regenerated = PrismKernel
            .tessellate(
                &os_walls::wall_solid(
                    &imported.parameters,
                    b.levels[&imported.parameters.level].parameters.elevation,
                )
                .unwrap(),
            )
            .unwrap();
        assert_eq!(original, regenerated);
        assert!((original.signed_volume() - 5.25).abs() < 1e-8);
    }
    assert_ne!(m.views, b.views);
    let again = WallIfc
        .import(&WallIfc.export_report(&b).unwrap().value)
        .unwrap();
    assert_eq!(b.walls, again.walls);
}

#[test]
fn ifc_export_fails_closed_for_native_columns() {
    let mut m = Model::new("Columns");
    let level = *m.levels.keys().next().unwrap();
    let column = Column::new(
        "core.column",
        ColumnParams {
            name: "C1".into(),
            level,
            center: Point2::new(1.0, 2.0),
            width: 0.4,
            depth: 0.6,
            height: 3.0,
            base_offset: 0.0,
            material: None,
        },
    );
    m.columns.insert(column.id(), column);
    m.validate().unwrap();
    assert!(WallIfc.export_report(&m).is_err());
}

#[test]
fn independently_validated_fixture_imports_and_reexports() {
    let fixture = include_bytes!("../../../fixtures/wall-exchange.ifc");
    let m = WallIfc.import_report(fixture).unwrap().value;
    let wall = m.walls.values().next().unwrap();
    assert_eq!(m.walls.len(), 1);
    assert_eq!(wall.parameters.length(), 7.);
    assert_eq!(m.levels[&wall.parameters.level].parameters.elevation, 3.);
    assert_eq!(
        WallIfc
            .import_report(&WallIfc.export_report(&m).unwrap().value)
            .unwrap()
            .value
            .walls,
        m.walls
    );
    let rotated = WallIfc
        .import_report(include_bytes!("../../../fixtures/rotated-walls.ifc"))
        .unwrap()
        .value;
    assert_eq!(rotated.walls.len(), 2);
    assert_eq!(
        WallIfc
            .import_report(&WallIfc.export_report(&rotated).unwrap().value)
            .unwrap()
            .value
            .walls,
        rotated.walls
    );
}

#[test]
fn parser_limits_and_schema_label_length_are_enforced() {
    let mut m = model();
    m.project.parameters.name = "n".repeat(256);
    assert!(WallIfc.export_report(&m).is_err());
    let deep = format!(
        "ISO-10303-21;HEADER;FILE_DESCRIPTION({}0{});",
        "(".repeat(40),
        ")".repeat(40)
    );
    assert!(WallIfc.import(deep.as_bytes()).is_err());
    let valid = WallIfc.export_report(&model()).unwrap().value;
    // Reproducible byte mutations exercise error paths without a fuzz runtime.
    for index in (0..valid.len()).step_by(7) {
        let mut data = valid.clone();
        data[index] = 0xff;
        assert!(WallIfc.import(&data).is_err());
    }
}

#[test]
fn no_silent_native_extension_loss() {
    let mut m = model();
    m.project
        .header
        .properties
        .insert("custom".into(), "value".into());
    m.project
        .header
        .relationships
        .insert("site".into(), m.sites.keys().copied().collect());
    let output = WallIfc.export_report(&m).unwrap();
    assert_eq!(output.warnings.len(), 2);
    assert!(WallIfc.export(&m).is_err());
}

#[test]
fn malformed_or_unsupported_files_are_rejected_without_panics() {
    let good = String::from_utf8(WallIfc.export_report(&model()).unwrap().value).unwrap();
    for (a, b) in [
        ("'IFC4'", "'IFC2X3'"),
        ("IFCWALL(", "IFCSLAB("),
        (".METRE.", ".FOOT."),
        (".NOTDEFINED.", ".USERDEFINED."),
        ("'SweptSolid'", "'Brep'"),
        ("3.5)", "-3.5)"),
        ("1.E-7", "1.E999"),
        ("END-ISO-10303-21;", ""),
        ("'Body'", "'Axis'"),
        ("#1=", "#0="),
    ] {
        assert_ne!(good.replace(a, b), good, "missing mutation {a}");
        assert!(
            WallIfc.import(good.replace(a, b).as_bytes()).is_err(),
            "accepted {a} => {b}"
        );
    }
    for end in (0..good.len()).step_by(17) {
        assert!(WallIfc.import(&good.as_bytes()[..end]).is_err());
    }
    let duplicate = good.replace(
        "ENDSEC;\nEND-ISO",
        "#1=IFCCARTESIANPOINT((0.,0.,0.));\nENDSEC;\nEND-ISO",
    );
    assert!(WallIfc.import(duplicate.as_bytes()).is_err());
    assert!(WallIfc.import(&vec![b' '; 16 * 1024 * 1024 + 1]).is_err());
}

#[test]
fn identity_and_empty_project() {
    let mut m = Model::new("Empty");
    m.views.clear();
    assert!(WallIfc.export(&m).is_ok());
    assert_eq!(
        WallIfc
            .import(&WallIfc.export(&m).unwrap())
            .unwrap()
            .project
            .id(),
        m.project.id()
    );
    m.project.header.id = Id(uuid::Uuid::nil());
    assert!(WallIfc.export_report(&m).is_err());
}

fn hosted(kind: OpeningKind, typed: bool, hinge: DoorHinge, swing: DoorSwing) -> Model {
    let mut m = model();
    let height = if kind == OpeningKind::Door { 2.1 } else { 1.2 };
    let sill = if kind == OpeningKind::Door { 0. } else { 0.9 };
    let ty = OpeningType::new(
        "core.opening_type",
        OpeningTypeParams {
            name: "Shared 'type' 窓".into(),
            kind,
            width: 0.8,
            height,
            sill,
            family: OpeningFamily::default(),
            pane_position: WindowPanePosition::Center,
        },
    );
    for host in m.walls.keys() {
        for offset in [0.4, 2.4] {
            let o = Opening::new(
                "core.opening",
                OpeningParams {
                    name: format!("Opening 'α' {offset}"),
                    host: *host,
                    offset,
                    hinge,
                    swing,
                    definition: if typed {
                        OpeningDefinition::Typed { type_id: ty.id() }
                    } else {
                        OpeningDefinition::Legacy {
                            kind,
                            width: 0.8,
                            height,
                            sill,
                        }
                    },
                },
            );
            m.openings.insert(o.id(), o);
        }
    }
    if typed {
        m.opening_types.insert(ty.id(), ty);
    }
    m.validate().unwrap();
    m
}

fn exported(m: &Model) -> String {
    String::from_utf8(WallIfc.export_report(m).unwrap().value).unwrap()
}

// Small test-only STEP row editor. It operates on exported one-line entities,
// retaining nested lists and strings so negative tests change one actual field.
fn rows(text: &str, kind: &str) -> Vec<u32> {
    text.lines()
        .filter(|l| l.starts_with('#') && l.contains(&format!("={kind}(")))
        .map(|l| l[1..l.find('=').unwrap()].parse().unwrap())
        .collect()
}
fn row(text: &str, id: u32) -> &str {
    text.lines()
        .find(|l| l.starts_with(&format!("#{id}=")))
        .unwrap()
}
fn args(text: &str, id: u32) -> Vec<String> {
    let r = row(text, id);
    let r = &r[r.find('(').unwrap() + 1..r.len() - 2];
    let (mut depth, mut quoted, mut start) = (0_i32, false, 0);
    let mut out = Vec::new();
    for (i, c) in r.char_indices() {
        if c == '\'' {
            quoted = !quoted;
        }
        if quoted {
            continue;
        }
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                out.push(r[start..i].into());
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(r[start..].into());
    out
}
fn reference(text: &str, id: u32, at: usize) -> u32 {
    args(text, id)[at]
        .trim_matches(['(', ')', '#'])
        .parse()
        .unwrap()
}
fn change(text: &str, id: u32, edit: impl FnOnce(&mut Vec<String>)) -> String {
    let original = row(text, id);
    let mut a = args(text, id);
    edit(&mut a);
    text.replace(
        original,
        &format!("{}({});", original.split('(').next().unwrap(), a.join(",")),
    )
}
fn remove(text: &str, id: u32) -> String {
    text.replace(&format!("{}\n", row(text, id)), "")
}
fn duplicate_relation(text: &str, id: u32) -> String {
    let r = row(text, id);
    let mut a = args(text, id);
    a[0] = "'0000000000000000000001'".into();
    let kind = r.split_once('=').unwrap().1.split('(').next().unwrap();
    text.replace(
        "ENDSEC;\nEND-ISO",
        &format!("#99999={kind}({});\nENDSEC;\nEND-ISO", a.join(",")),
    )
}
fn solid(text: &str, product: u32) -> u32 {
    let definition = reference(text, product, 6);
    let shape = reference(text, definition, 2);
    reference(text, shape, 3)
}
fn placement_axis(text: &str, product: u32) -> u32 {
    reference(text, reference(text, product, 5), 1)
}
#[track_caller]
fn rejected(text: &str) {
    assert!(
        WallIfc.import(text.as_bytes()).is_err(),
        "accepted invalid IFC"
    );
}

#[test]
fn hosted_round_trips_preserve_semantics_types_and_regenerated_holes() {
    for kind in [OpeningKind::Door, OpeningKind::Window] {
        for typed in [false, true] {
            for reverse_host in [false, true] {
                for (hinge, swing) in [
                    (DoorHinge::Start, DoorSwing::Left),
                    (DoorHinge::Start, DoorSwing::Right),
                    (DoorHinge::End, DoorSwing::Left),
                    (DoorHinge::End, DoorSwing::Right),
                ] {
                    if kind == OpeningKind::Window
                        && (hinge != DoorHinge::Start || swing != DoorSwing::Left)
                    {
                        continue;
                    }
                    let mut m = hosted(kind, typed, hinge, swing);
                    if reverse_host {
                        for w in m.walls.values_mut() {
                            std::mem::swap(&mut w.parameters.start, &mut w.parameters.end);
                        }
                    }
                    let out = WallIfc.export_report(&m).unwrap();
                    assert!(out.warnings.iter().any(|w| w.contains("no Body")));
                    assert!(
                        WallIfc
                            .export(&m)
                            .unwrap_err()
                            .to_string()
                            .contains("acknowledgement")
                    );
                    let b = WallIfc.import(&out.value).unwrap();
                    assert_eq!(m.openings, b.openings);
                    assert_eq!(m.opening_types, b.opening_types);
                    assert_eq!(m.walls, b.walls);
                    assert_eq!(m.levels, b.levels);
                    for (id, wall) in &m.walls {
                        let original = os_geometry::walls::NativeWall::from_model(&m, *id).unwrap();
                        let regenerated =
                            os_geometry::walls::NativeWall::from_model(&b, *id).unwrap();
                        assert_eq!(original.mesh().unwrap(), regenerated.mesh().unwrap());
                        let p = &wall.parameters;
                        let h = if kind == OpeningKind::Door { 2.1 } else { 1.2 };
                        let expected =
                            p.length() * p.thickness * p.height - 2. * 0.8 * h * p.thickness;
                        assert!((regenerated.net_volume().unwrap() - expected).abs() < 1e-8);
                    }
                }
            }
        }
    }
}

#[test]
fn ifc_host_prism_void_axes_and_door_hand_match_the_standard_frame() {
    // These expected IFC operation values are from buildingSMART IFC4, not the
    // importer. Check physical hinge position and +Y swing in the host frame.
    for (hinge, swing, operation, direction, x) in [
        (
            DoorHinge::Start,
            DoorSwing::Left,
            ".SINGLE_SWING_LEFT.",
            "(1.0,0.0,0.)",
            0.,
        ),
        (
            DoorHinge::End,
            DoorSwing::Left,
            ".SINGLE_SWING_RIGHT.",
            "(1.0,0.0,0.)",
            0.,
        ),
        (
            DoorHinge::Start,
            DoorSwing::Right,
            ".SINGLE_SWING_RIGHT.",
            "(-1.0,0.0,0.)",
            0.8,
        ),
        (
            DoorHinge::End,
            DoorSwing::Right,
            ".SINGLE_SWING_LEFT.",
            "(-1.0,0.0,0.)",
            0.8,
        ),
    ] {
        let m = hosted(OpeningKind::Door, false, hinge, swing);
        let text = exported(&m);
        for wall in rows(&text, "IFCWALL") {
            let s = solid(&text, wall);
            let p = reference(&text, s, 0);
            assert_eq!(args(&text, p)[3..], ["5.0", "0.3"]);
            assert_eq!(args(&text, s)[3], "3.5");
        }
        for void in rows(&text, "IFCOPENINGELEMENT") {
            let s = solid(&text, void);
            assert_eq!(args(&text, s)[3], "0.3");
            let a = reference(&text, s, 1);
            assert_eq!(args(&text, reference(&text, a, 0))[0], "(0.0,0.15,0.0)");
            assert_eq!(args(&text, reference(&text, a, 1))[0], "(0.,-1.,0.)");
            assert_eq!(args(&text, reference(&text, a, 2))[0], "(1.,0.,0.)");
        }
        for fill in rows(&text, "IFCDOOR") {
            assert_eq!(args(&text, fill)[11], operation);
            assert_eq!(args(&text, fill)[6], "$");
            let a = placement_axis(&text, fill);
            assert_eq!(args(&text, reference(&text, a, 2))[0], direction);
            assert_eq!(
                args(&text, reference(&text, a, 0))[0],
                format!("({x:?},0.0,0.0)")
            );
            let sign = if swing == DoorSwing::Left { 1. } else { -1. };
            let local_hinge = if operation == ".SINGLE_SWING_LEFT." {
                0.
            } else {
                0.8
            };
            let actual_hinge: f64 = x + sign * local_hinge;
            let expected_hinge = if hinge == DoorHinge::Start { 0. } else { 0.8 };
            assert!((actual_hinge - expected_hinge).abs() < 1e-8);
        }
    }
}

#[test]
fn shared_types_preserve_identity_or_reject_conflicting_ifc_operations() {
    let mut m = hosted(OpeningKind::Door, true, DoorHinge::Start, DoorSwing::Left);
    let id = *m.openings.keys().next().unwrap();
    // Reversing both jamb and swing preserves IFC hand using a 180-degree yaw.
    let o = &mut m.openings.get_mut(&id).unwrap().parameters;
    o.hinge = DoorHinge::End;
    o.swing = DoorSwing::Right;
    let b = WallIfc.import(exported(&m).as_bytes()).unwrap();
    assert_eq!(b.openings, m.openings);
    assert_eq!(b.opening_types, m.opening_types);
    m.openings.get_mut(&id).unwrap().parameters.swing = DoorSwing::Left;
    assert!(
        WallIfc
            .export_report(&m)
            .unwrap_err()
            .to_string()
            .contains("conflicting IFC hands")
    );
}

#[test]
fn custom_families_profiles_pane_alignment_and_unused_types_are_rejected() {
    let original = hosted(OpeningKind::Window, true, DoorHinge::Start, DoorSwing::Left);
    for case in 0..6 {
        let mut m = original.clone();
        let p = &mut m.opening_types.values_mut().next().unwrap().parameters;
        match case {
            0 => p.family.depth = 0.03,
            1 => p.family.frame_width = 0.04,
            2 => p.pane_position = WindowPanePosition::LeftFace,
            3 => p.family.profile[2].y = 0.8,
            4 => {
                p.family.host_cut = OpeningHostCut::Profile;
                p.family.cut_profile[2].y = 0.8;
                p.family.profile = p.family.cut_profile.clone();
            }
            _ => m.openings.clear(),
        }
        m.validate().unwrap();
        assert!(matches!(
            WallIfc.export_report(&m),
            Err(os_core::Error::Unsupported(_))
        ));
    }
}

#[test]
fn mixed_kinds_legacy_and_equal_but_distinct_types_are_not_merged() {
    let mut m = hosted(OpeningKind::Window, true, DoorHinge::Start, DoorSwing::Left);
    let original_type = m.opening_types.values().next().unwrap().clone();
    let mut door = OpeningType::new("core.opening_type", original_type.parameters.clone());
    door.parameters.kind = OpeningKind::Door;
    door.parameters.sill = 0.;
    let equal_door = OpeningType::new("core.opening_type", door.parameters.clone());
    let ids: Vec<_> = m.openings.keys().copied().collect();
    m.openings.get_mut(&ids[0]).unwrap().parameters.definition =
        OpeningDefinition::Typed { type_id: door.id() };
    m.openings.get_mut(&ids[1]).unwrap().parameters.definition = OpeningDefinition::Typed {
        type_id: equal_door.id(),
    };
    m.openings.get_mut(&ids[2]).unwrap().parameters.definition = OpeningDefinition::Legacy {
        kind: OpeningKind::Window,
        width: 0.8,
        height: 1.2,
        sill: 0.9,
    };
    m.opening_types.insert(door.id(), door);
    m.opening_types.insert(equal_door.id(), equal_door);
    let result = WallIfc.import(exported(&m).as_bytes()).unwrap();
    assert_eq!(result.opening_types, m.opening_types);
    assert_eq!(result.openings, m.openings);
}

#[test]
fn hosted_exchange_keeps_step_entity_and_value_caps() {
    let text = exported(&hosted(
        OpeningKind::Door,
        false,
        DoorHinge::Start,
        DoorSwing::Left,
    ));
    let header = text.split_once("DATA;\n").unwrap().0;
    let mut entities = format!("{header}DATA;\n");
    for id in 1..=100_001 {
        entities.push_str(&format!("#{id}=IFCCARTESIANPOINT((0.,0.,0.));\n"));
    }
    entities.push_str("ENDSEC;\nEND-ISO-10303-21;\n");
    assert!(
        WallIfc
            .import(entities.as_bytes())
            .unwrap_err()
            .to_string()
            .contains("entity limit")
    );
    let values = format!(
        "{header}DATA;\n#1=IFCCARTESIANPOINT(({}0.));\nENDSEC;\nEND-ISO-10303-21;\n",
        "0.,".repeat(1_000_000)
    );
    assert!(
        WallIfc
            .import(values.as_bytes())
            .unwrap_err()
            .to_string()
            .contains("limit")
    );
}

#[test]
fn hosted_relationships_fail_closed_for_missing_duplicate_and_wrong_targets() {
    let m = hosted(OpeningKind::Door, true, DoorHinge::Start, DoorSwing::Left);
    let text = exported(&m);
    let wall = rows(&text, "IFCWALL")[0];
    let void = rows(&text, "IFCOPENINGELEMENT")[0];
    let door = rows(&text, "IFCDOOR")[0];
    let ty = rows(&text, "IFCDOORTYPE")[0];
    for kind in [
        "IFCRELVOIDSELEMENT",
        "IFCRELFILLSELEMENT",
        "IFCRELDEFINESBYTYPE",
        "IFCRELCONTAINEDINSPATIALSTRUCTURE",
    ] {
        let rel = rows(&text, kind)[0];
        rejected(&remove(&text, rel));
        rejected(&duplicate_relation(&text, rel));
        rejected(&change(&text, rel, |a| a[4] = format!("#{ty}")));
        rejected(&change(&text, rel, |a| {
            a[5] = format!("#{}", rows(&text, "IFCPROJECT")[0])
        }));
    }
    let void_rel = rows(&text, "IFCRELVOIDSELEMENT")[0];
    rejected(&change(&text, void_rel, |a| a.swap(4, 5)));
    let actual_host = reference(&text, void_rel, 4);
    let other_host = rows(&text, "IFCWALL")
        .into_iter()
        .find(|id| *id != actual_host)
        .unwrap();
    rejected(&change(&text, void_rel, |a| {
        a[4] = format!("#{other_host}")
    }));
    let fill_rel = rows(&text, "IFCRELFILLSELEMENT")[0];
    rejected(&change(&text, fill_rel, |a| a.swap(4, 5)));
    rejected(&change(&text, fill_rel, |a| a[5] = format!("#{wall}")));
    let contains = rows(&text, "IFCRELCONTAINEDINSPATIALSTRUCTURE")[0];
    rejected(&change(&text, contains, |a| a[4] = format!("(#{void})")));
    let door_ref = format!("#{door}");
    let containments = rows(&text, "IFCRELCONTAINEDINSPATIALSTRUCTURE");
    let owner = *containments
        .iter()
        .find(|id| {
            args(&text, **id)[4]
                .trim_matches(['(', ')'])
                .split(',')
                .any(|s| s == door_ref)
        })
        .unwrap();
    let other = *containments.iter().find(|id| **id != owner).unwrap();
    let moved = change(&text, owner, |a| {
        a[4] = format!(
            "({})",
            a[4].trim_matches(['(', ')'])
                .split(',')
                .filter(|s| *s != door_ref)
                .collect::<Vec<_>>()
                .join(",")
        );
    });
    let moved = change(&moved, other, |a| {
        a[4] = format!("{},#{door})", a[4].trim_end_matches(')'))
    });
    assert!(
        WallIfc
            .import(moved.as_bytes())
            .unwrap_err()
            .to_string()
            .contains("share a storey")
    );
    let type_rel = rows(&text, "IFCRELDEFINESBYTYPE")[0];
    rejected(&change(&text, type_rel, |a| a[4] = format!("(#{wall})")));
    rejected(&change(&text, type_rel, |a| {
        a[4] = format!("(#{door},#{door})")
    }));
    rejected(&change(&text, ty, |a| a[10] = ".NOTDEFINED.".into()));
    rejected(&change(&text, ty, |a| a[5] = format!("(#{wall})"))); // unsupported family properties
    rejected(&change(&text, ty, |a| a[6] = format!("(#{wall})"))); // unsupported component maps
    rejected(&change(&text, door, |a| {
        a[6] = args(&text, wall)[6].clone()
    }));
    rejected(&text.replace("IFCDOORTYPE(", "IFCWINDOWTYPE("));
    // Remove one assignment while the type is still in use: cannot become Legacy.
    rejected(&change(&text, type_rel, |a| a[4] = format!("(#{door})")));
    rejected(&change(&text, door, |a| {
        a[3] = "'OpenStructure.Legacy.v1'".into()
    }));
    rejected(&change(&text, door, |a| {
        a[0] = args(&text, wall)[0].clone()
    }));
    // Required exact arities on every new entity class.
    for kind in [
        "IFCOPENINGELEMENT",
        "IFCDOOR",
        "IFCDOORTYPE",
        "IFCRELVOIDSELEMENT",
        "IFCRELFILLSELEMENT",
        "IFCRELDEFINESBYTYPE",
    ] {
        rejected(&change(&text, rows(&text, kind)[0], |a| {
            a.pop();
        }));
        rejected(&change(&text, rows(&text, kind)[0], |a| a.push("$".into())));
    }
}

#[test]
fn unsupported_void_fill_transforms_and_second_invalid_opening_are_atomic() {
    let text = exported(&hosted(
        OpeningKind::Window,
        true,
        DoorHinge::Start,
        DoorSwing::Left,
    ));
    let voids = rows(&text, "IFCOPENINGELEMENT");
    let void = voids[1]; // another valid opening precedes this failure
    let window = rows(&text, "IFCWINDOW")[1];
    let s = solid(&text, void);
    let a = reference(&text, s, 1);
    let p = reference(&text, s, 0);
    let placement = reference(&text, void, 5);
    let origin = reference(&text, placement_axis(&text, void), 0);
    for value in ["(0.4,0.01,0.9)", "(0.4,0.0,3.4)", "(-1.0,0.0,0.9)"] {
        rejected(&change(&text, origin, |a| a[0] = value.into()));
    }
    rejected(&change(&text, s, |a| a[3] = "0.15".into())); // partial depth
    rejected(&change(&text, s, |a| a[3] = "0.6".into()));
    rejected(&change(&text, reference(&text, a, 1), |a| {
        a[0] = "(0.,0.,1.)".into()
    }));
    rejected(&change(&text, reference(&text, a, 2), |a| {
        a[0] = "(-1.,0.,0.)".into()
    }));
    rejected(&change(&text, reference(&text, a, 0), |a| {
        a[0] = "(0.,0.,0.)".into()
    }));
    rejected(&change(&text, placement, |a| a[0] = "#99998".into()));
    rejected(&change(&text, p, |a| a[3] = "0.7".into()));
    rejected(&text.replace("IFCRECTANGLEPROFILEDEF(", "IFCARBITRARYCLOSEDPROFILEDEF("));
    let fa = placement_axis(&text, window);
    rejected(&change(&text, reference(&text, fa, 0), |a| {
        a[0] = "(0.,0.1,0.)".into()
    }));
    rejected(&change(&text, reference(&text, fa, 1), |a| {
        a[0] = "(0.,1.,0.)".into()
    }));
    rejected(&change(&text, reference(&text, fa, 2), |a| {
        a[0] = "(0.,1.,0.)".into()
    }));
    rejected(&change(&text, window, |a| a[8] = "2.5".into()));
    // A geometry-valid sill change on one shared instance cannot redefine its type.
    let old = args(&text, origin)[0].clone();
    rejected(&change(&text, origin, |a| {
        a[0] = old.replace("0.9)", "1.0)")
    }));
    // Overlap passes the geometry/relationship readers and is rejected by the
    // final whole-model validation, after all openings have been reconstructed.
    let rels = rows(&text, "IFCRELVOIDSELEMENT");
    let host = reference(&text, rels[0], 4);
    let same_host: Vec<_> = rels
        .into_iter()
        .filter(|r| reference(&text, *r, 4) == host)
        .map(|r| reference(&text, r, 5))
        .collect();
    let first_origin = reference(&text, placement_axis(&text, same_host[0]), 0);
    let second_origin = reference(&text, placement_axis(&text, same_host[1]), 0);
    let overlapped = change(&text, second_origin, |a| {
        a[0] = args(&text, first_origin)[0].clone()
    });
    assert!(
        WallIfc
            .import(overlapped.as_bytes())
            .unwrap_err()
            .to_string()
            .contains("separation")
    );
    // Entire result remains an error; the caller never receives the earlier opening.
    assert_eq!(WallIfc.import(text.as_bytes()).unwrap().openings.len(), 4);
    for kind in ["IFCWINDOW", "IFCWINDOWTYPE"] {
        rejected(&change(&text, rows(&text, kind)[0], |a| {
            a.pop();
        }));
    }
}

#[test]
#[ignore = "requires OS_IFC_PYTHON pointing to Python with IfcOpenShell; independent schema/geometry gate"]
fn independently_validate_hosted_ifc4_schema_geometry_and_identity() {
    let python = std::env::var_os("OS_IFC_PYTHON").expect("set OS_IFC_PYTHON");
    let dir = std::env::temp_dir().join(format!("os-ifc-hosted-{}", Id::new()));
    std::fs::create_dir(&dir).unwrap();
    let mut command = std::process::Command::new(python);
    command.arg(format!(
        "{}/tests/validate_hosted.py",
        env!("CARGO_MANIFEST_DIR")
    ));
    let mut index = 0;
    for kind in [OpeningKind::Door, OpeningKind::Window] {
        for typed in [false, true] {
            for reversed in [false, true] {
                for (hinge, swing) in [
                    (DoorHinge::Start, DoorSwing::Left),
                    (DoorHinge::Start, DoorSwing::Right),
                    (DoorHinge::End, DoorSwing::Left),
                    (DoorHinge::End, DoorSwing::Right),
                ] {
                    if kind == OpeningKind::Window
                        && (hinge != DoorHinge::Start || swing != DoorSwing::Left)
                    {
                        continue;
                    }
                    let mut m = hosted(kind, typed, hinge, swing);
                    if reversed {
                        for w in m.walls.values_mut() {
                            std::mem::swap(&mut w.parameters.start, &mut w.parameters.end);
                        }
                    }
                    if typed && kind == OpeningKind::Door {
                        let p = &mut m.openings.values_mut().next().unwrap().parameters;
                        p.hinge = if hinge == DoorHinge::Start {
                            DoorHinge::End
                        } else {
                            DoorHinge::Start
                        };
                        p.swing = if swing == DoorSwing::Left {
                            DoorSwing::Right
                        } else {
                            DoorSwing::Left
                        };
                    }
                    let path = dir.join(format!("{index}.ifc"));
                    std::fs::write(&path, WallIfc.export_report(&m).unwrap().value).unwrap();
                    // Independent expected source values; no Rust IFC parser is
                    // involved in the Python comparison or OCC boolean meshing.
                    let mut source = String::new();
                    for o in m.openings.values() {
                        let p = m.resolve_opening(&o.parameters).unwrap();
                        let w = &m.walls[&p.host].parameters;
                        source.push_str(&format!("{}\t{}\t{}\t{:?}\t{}\t{}\t{}\t{}\t{:?}\t{:?}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                            o.id(), p.name, p.host, p.kind, p.offset, p.sill, p.width, p.height, p.hinge, p.swing,
                            p.type_id.map(|id| id.to_string()).unwrap_or_default(), p.type_name.unwrap_or_default(),
                            w.start.x, w.start.y, w.end.x, w.end.y, m.levels[&w.level].parameters.elevation));
                    }
                    std::fs::write(path.with_extension("tsv"), source).unwrap();
                    command.arg(path);
                    index += 1;
                }
            }
        }
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "fixtures retained at {}\n{}\n{}",
        dir.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    println!(
        "{}\nFixtures: {}",
        String::from_utf8_lossy(&output.stdout),
        dir.display()
    );
}
