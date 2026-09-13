use os_core::{Id, Point2};
use os_geometry::{GeometryKernel, PrismKernel};
use os_ifc::{IfcAdapter, WallIfc};
use os_model::{Level, LevelParams, Model, Wall, WallParams};

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
