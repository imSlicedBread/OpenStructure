use crate::{
    Exchange,
    step::{self, Entities, Entity, Value, bad, identity},
};
use os_core::{Id, Point2, Result, ensure};
use os_model::*;
use std::collections::{BTreeMap, BTreeSet};

fn entity<'a>(es: &'a Entities, id: u32, kind: &str) -> Result<&'a Entity> {
    let e = es.get(&id).ok_or_else(|| bad("missing reference"))?;
    ensure(
        e.kind == kind,
        format!("IFC #{id}: expected {kind}, found {}", e.kind),
    )?;
    Ok(e)
}
fn target<'a>(es: &'a Entities, v: &Value, kind: &str) -> Result<&'a Entity> {
    entity(es, v.reference()?, kind)
}
fn eq(v: &Value, wanted: Value) -> Result<()> {
    ensure(
        *v == wanted,
        format!("IFC subset does not support {v:?}; expected {wanted:?}"),
    )
}
fn nulls(e: &Entity, indices: &[usize]) -> Result<()> {
    for i in indices {
        eq(e.at(*i)?, Value::Null)?;
    }
    Ok(())
}
fn one(v: &Value) -> Result<&Value> {
    let a = v.list()?;
    ensure(a.len() == 1, "IFC subset requires exactly one item")?;
    Ok(&a[0])
}
fn coords(es: &Entities, v: &Value, kind: &str) -> Result<Vec<f64>> {
    target(es, v, kind)?
        .at(0)?
        .list()?
        .iter()
        .map(Value::number)
        .collect()
}
fn axis(es: &Entities, v: &Value) -> Result<([f64; 3], [f64; 2])> {
    let a = target(es, v, "IFCAXIS2PLACEMENT3D")?;
    ensure(
        (*a.at(1)? == Value::Null) == (*a.at(2)? == Value::Null),
        "IFC: axis and reference direction must both be provided or both omitted",
    )?;
    let p = coords(es, a.at(0)?, "IFCCARTESIANPOINT")?;
    ensure(p.len() == 3, "IFC: expected 3D point")?;
    if *a.at(1)? != Value::Null {
        ensure(
            coords(es, a.at(1)?, "IFCDIRECTION")? == [0., 0., 1.],
            "IFC: tilted axes unsupported",
        )?;
    }
    let d = if *a.at(2)? == Value::Null {
        vec![1., 0., 0.]
    } else {
        coords(es, a.at(2)?, "IFCDIRECTION")?
    };
    ensure(
        d.len() == 3 && d[2] == 0.,
        "IFC: only horizontal reference directions supported",
    )?;
    let norm = d[0].hypot(d[1]);
    ensure(norm.is_finite() && norm > 1e-12, "IFC: invalid direction")?;
    Ok(([p[0], p[1], p[2]], [d[0] / norm, d[1] / norm]))
}
fn placement(es: &Entities, e: &Entity, parent: Option<u32>) -> Result<([f64; 3], [f64; 2])> {
    let p = target(es, e.at(5)?, "IFCLOCALPLACEMENT")?;
    eq(p.at(0)?, parent.map_or(Value::Null, Value::Ref))?;
    axis(es, p.at(1)?)
}
fn parent_of(map: &BTreeMap<u32, u32>, id: u32) -> Result<u32> {
    map.get(&id)
        .copied()
        .ok_or_else(|| bad(format!("#{id} has no spatial parent")))
}
fn id_of(ids: &BTreeMap<u32, Id>, n: u32) -> Result<Id> {
    ids.get(&n)
        .copied()
        .ok_or_else(|| bad("parent is not a spatial entity"))
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-8
}

/// Exact bounded vertical rectangular profile, extruded across host thickness.
fn opening_size(es: &Entities, e: &Entity, context: &Value, thickness: f64) -> Result<(f64, f64)> {
    let definition = target(es, e.at(6)?, "IFCPRODUCTDEFINITIONSHAPE")?;
    nulls(definition, &[0, 1])?;
    let shape = target(es, one(definition.at(2)?)?, "IFCSHAPEREPRESENTATION")?;
    eq(shape.at(0)?, context.clone())?;
    eq(shape.at(1)?, Value::Text("Body".into()))?;
    eq(shape.at(2)?, Value::Text("SweptSolid".into()))?;
    let solid = target(es, one(shape.at(3)?)?, "IFCEXTRUDEDAREASOLID")?;
    let a = target(es, solid.at(1)?, "IFCAXIS2PLACEMENT3D")?;
    let origin = coords(es, a.at(0)?, "IFCCARTESIANPOINT")?;
    ensure(
        origin.len() == 3
            && near(origin[0], 0.)
            && near(origin[1], thickness / 2.)
            && near(origin[2], 0.),
        "IFC: opening solid origin unsupported",
    )?;
    ensure(
        coords(es, a.at(1)?, "IFCDIRECTION")? == [0., -1., 0.]
            && coords(es, a.at(2)?, "IFCDIRECTION")? == [1., 0., 0.],
        "IFC: opening requires a vertical XZ profile with extrusion toward host -Y",
    )?;
    ensure(
        coords(es, solid.at(2)?, "IFCDIRECTION")? == [0., 0., 1.]
            && near(solid.at(3)?.number()?, thickness),
        "IFC: opening must cut the full wall thickness",
    )?;
    let profile = target(es, solid.at(0)?, "IFCRECTANGLEPROFILEDEF")?;
    eq(profile.at(0)?, Value::Enum("AREA".into()))?;
    nulls(profile, &[1])?;
    let pa = target(es, profile.at(2)?, "IFCAXIS2PLACEMENT2D")?;
    nulls(pa, &[1])?;
    let center = coords(es, pa.at(0)?, "IFCCARTESIANPOINT")?;
    let width = profile.at(3)?.number()?;
    let height = profile.at(4)?.number()?;
    ensure(
        width > 0.
            && height > 0.
            && center.len() == 2
            && near(center[0], width / 2.)
            && near(center[1], height / 2.),
        "IFC: opening profile dimensions/center unsupported",
    )?;
    Ok((width, height))
}

fn import_openings(
    es: &Entities,
    ids: &BTreeMap<u32, Id>,
    containers: &BTreeMap<u32, u32>,
    wall_profile_origins: &BTreeMap<u32, [f64; 2]>,
    context: &Value,
    model: &mut Model,
) -> Result<()> {
    let mut hosts = BTreeMap::new(); // void -> wall
    let mut fills = BTreeMap::new(); // filling -> void
    let mut filled = BTreeSet::new();
    let mut types = BTreeMap::new(); // filling -> type
    let mut used_types = BTreeSet::new();
    for e in es.values() {
        match e.kind.as_str() {
            "IFCRELVOIDSELEMENT" => {
                nulls(e, &[1, 2, 3])?;
                let host = e.at(4)?.reference()?;
                let void = e.at(5)?.reference()?;
                entity(es, host, "IFCWALL")?;
                entity(es, void, "IFCOPENINGELEMENT")?;
                ensure(
                    hosts.insert(void, host).is_none(),
                    "IFC: duplicate void relationship",
                )?;
            }
            "IFCRELFILLSELEMENT" => {
                nulls(e, &[1, 2, 3])?;
                let void = e.at(4)?.reference()?;
                let fill = e.at(5)?.reference()?;
                entity(es, void, "IFCOPENINGELEMENT")?;
                ensure(
                    matches!(es[&fill].kind.as_str(), "IFCDOOR" | "IFCWINDOW"),
                    "IFC: filling must be a door/window",
                )?;
                ensure(
                    fills.insert(fill, void).is_none() && filled.insert(void),
                    "IFC: duplicate filling relationship",
                )?;
            }
            "IFCRELDEFINESBYTYPE" => {
                nulls(e, &[1, 2, 3])?;
                let ty = e.at(5)?.reference()?;
                let kind = match es[&ty].kind.as_str() {
                    "IFCDOORTYPE" => "IFCDOOR",
                    "IFCWINDOWTYPE" => "IFCWINDOW",
                    _ => return Err(bad("unsupported opening type target")),
                };
                ensure(
                    used_types.insert(ty),
                    "IFC: multiple relationships for one opening type",
                )?;
                let children = e.at(4)?.list()?;
                ensure(!children.is_empty(), "IFC: empty type relationship")?;
                for v in children {
                    let fill = v.reference()?;
                    entity(es, fill, kind)?;
                    ensure(
                        types.insert(fill, ty).is_none(),
                        "IFC: duplicate type assignment",
                    )?;
                }
            }
            _ => {}
        }
    }
    for (n, e) in es {
        match e.kind.as_str() {
            "IFCOPENINGELEMENT" => ensure(
                hosts.contains_key(n) && filled.contains(n),
                "IFC: orphan void",
            )?,
            "IFCDOORTYPE" | "IFCWINDOWTYPE" => {
                ensure(used_types.contains(n), "IFC: unused opening type")?;
                nulls(e, &[1, 3, 4, 5, 6, 7, 8, 11, 12])?;
                eq(
                    e.at(9)?,
                    Value::Enum(
                        if e.kind == "IFCDOORTYPE" {
                            "DOOR"
                        } else {
                            "WINDOW"
                        }
                        .into(),
                    ),
                )?;
                if e.kind == "IFCWINDOWTYPE" {
                    eq(e.at(10)?, Value::Enum("SINGLE_PANEL".into()))?;
                } else {
                    ensure(
                        matches!(e.at(10)?, Value::Enum(op) if op == "SINGLE_SWING_LEFT" || op == "SINGLE_SWING_RIGHT"),
                        "IFC: unsupported door type operation",
                    )?;
                }
            }
            _ => {}
        }
    }
    for (n, e) in es {
        if !matches!(e.kind.as_str(), "IFCDOOR" | "IFCWINDOW") {
            continue;
        }
        nulls(e, &[1, 4, 6, 7, 12])?; // no fictitious component body
        let typed = match e.at(3)?.text()? {
            "OpenStructure.Typed.v1" => true,
            "OpenStructure.Legacy.v1" => false,
            _ => return Err(bad("unsupported hosted opening exchange description")),
        };
        ensure(
            typed == types.contains_key(n),
            "IFC: typed/legacy definition disagrees with type relationship",
        )?;
        let void_id = *fills.get(n).ok_or_else(|| bad("orphan filling"))?;
        let host_id = *hosts
            .get(&void_id)
            .ok_or_else(|| bad("filling void has no host"))?;
        let void = &es[&void_id];
        nulls(void, &[1, 3, 4, 7])?;
        eq(void.at(8)?, Value::Enum("OPENING".into()))?;
        eq(void.at(2)?, e.at(2)?.clone())?;
        ensure(
            parent_of(containers, *n)? == parent_of(containers, host_id)?,
            "IFC: filling and host must share a storey",
        )?;
        let wall = &model.walls[&ids[&host_id]].parameters;
        let (origin, d) = placement(es, void, Some(es[&host_id].at(5)?.reference()?))?;
        let wall_origin = wall_profile_origins[&host_id];
        ensure(
            d == [1., 0.] && near(origin[1], wall_origin[1]),
            "IFC: rotated/transversely displaced opening unsupported",
        )?;
        let (width, height) = opening_size(es, void, context, wall.thickness)?;
        ensure(
            near(e.at(8)?.number()?, height) && near(e.at(9)?.number()?, width),
            "IFC: filling and void dimensions disagree",
        )?;
        let (fill_origin, fill_dir) = placement(es, e, Some(void.at(5)?.reference()?))?;
        let reverse = fill_dir == [-1., 0.];
        ensure(
            (fill_dir == [1., 0.] || reverse)
                && near(fill_origin[0], if reverse { width } else { 0. })
                && near(fill_origin[1], 0.)
                && near(fill_origin[2], 0.),
            "IFC: unsupported filling placement",
        )?;
        let kind = if e.kind == "IFCDOOR" {
            OpeningKind::Door
        } else {
            OpeningKind::Window
        };
        let op = if let Some(ty) = types.get(n) {
            nulls(e, &[10, 11])?;
            es[ty].at(10)?
        } else {
            eq(
                e.at(10)?,
                Value::Enum(
                    if kind == OpeningKind::Door {
                        "DOOR"
                    } else {
                        "WINDOW"
                    }
                    .into(),
                ),
            )?;
            e.at(11)?
        };
        let (hinge, swing) = if kind == OpeningKind::Window {
            eq(op, Value::Enum("SINGLE_PANEL".into()))?;
            ensure(!reverse, "IFC: rotated window unsupported")?;
            (DoorHinge::Start, DoorSwing::Left)
        } else {
            let left = match op {
                Value::Enum(op) if op == "SINGLE_SWING_LEFT" => true,
                Value::Enum(op) if op == "SINGLE_SWING_RIGHT" => false,
                _ => return Err(bad("unsupported door operation")),
            };
            (
                if left != reverse {
                    DoorHinge::Start
                } else {
                    DoorHinge::End
                },
                if reverse {
                    DoorSwing::Right
                } else {
                    DoorSwing::Left
                },
            )
        };
        let sill = origin[2];
        let definition = if let Some(ty) = types.get(n) {
            let type_id = ids[ty];
            let params = OpeningTypeParams {
                name: es[ty].at(2)?.text()?.into(),
                kind,
                width,
                height,
                sill,
                pane_position: WindowPanePosition::Center,
                family: OpeningFamily::default(),
            };
            if let Some(existing) = model.opening_types.get(&type_id) {
                ensure(
                    existing.parameters == params,
                    "IFC: shared type dimensions/sill are inconsistent",
                )?;
            } else {
                let mut ty = OpeningType::new("core.opening_type", params);
                ty.header.id = type_id;
                model.opening_types.insert(type_id, ty);
            }
            OpeningDefinition::Typed { type_id }
        } else {
            OpeningDefinition::Legacy {
                kind,
                width,
                height,
                sill,
            }
        };
        let mut opening = Opening::new(
            "core.opening",
            OpeningParams {
                name: e.at(2)?.text()?.into(),
                host: ids[&host_id],
                offset: origin[0] - wall_origin[0],
                definition,
                hinge,
                swing,
            },
        );
        opening.header.id = ids[n];
        model.openings.insert(opening.id(), opening);
    }
    Ok(())
}

pub fn import(bytes: &[u8]) -> Result<Exchange<Model>> {
    let es = step::parse(bytes)?;
    // Arity and entity whitelist prevent silently ignoring additional product types,
    // properties, materials, styles, custom openings or unsupported geometry.
    let arities: BTreeMap<&str, usize> = [
        ("IFCPROJECT", 9),
        ("IFCSITE", 14),
        ("IFCBUILDING", 12),
        ("IFCBUILDINGSTOREY", 10),
        ("IFCWALL", 9),
        ("IFCOPENINGELEMENT", 9),
        ("IFCDOOR", 13),
        ("IFCWINDOW", 13),
        ("IFCDOORTYPE", 13),
        ("IFCWINDOWTYPE", 13),
        ("IFCRELVOIDSELEMENT", 6),
        ("IFCRELFILLSELEMENT", 6),
        ("IFCRELDEFINESBYTYPE", 6),
        ("IFCRELAGGREGATES", 6),
        ("IFCRELCONTAINEDINSPATIALSTRUCTURE", 6),
        ("IFCGEOMETRICREPRESENTATIONCONTEXT", 6),
        ("IFCSIUNIT", 4),
        ("IFCUNITASSIGNMENT", 1),
        ("IFCCARTESIANPOINT", 1),
        ("IFCDIRECTION", 1),
        ("IFCAXIS2PLACEMENT3D", 3),
        ("IFCAXIS2PLACEMENT2D", 2),
        ("IFCLOCALPLACEMENT", 2),
        ("IFCRECTANGLEPROFILEDEF", 5),
        ("IFCEXTRUDEDAREASOLID", 4),
        ("IFCSHAPEREPRESENTATION", 4),
        ("IFCPRODUCTDEFINITIONSHAPE", 3),
    ]
    .into();
    for (n, e) in &es {
        let count = arities.get(e.kind.as_str()).ok_or_else(|| {
            os_core::Error::Unsupported(format!("IFC #{n}: {} is outside the wall subset", e.kind))
        })?;
        ensure(
            e.args.len() == *count,
            format!("IFC #{n}: wrong {} argument count", e.kind),
        )?;
    }
    let projects: Vec<_> = es.iter().filter(|(_, e)| e.kind == "IFCPROJECT").collect();
    ensure(projects.len() == 1, "IFC: exactly one project required")?;
    let (&project_step, project) = projects[0];
    nulls(project, &[1, 3, 4, 5, 6])?;
    let context_ref = one(project.at(7)?)?;
    let context = target(&es, context_ref, "IFCGEOMETRICREPRESENTATIONCONTEXT")?;
    nulls(context, &[0, 5])?;
    eq(context.at(1)?, Value::Text("Model".into()))?;
    eq(context.at(2)?, Value::Number(3.))?;
    ensure(
        context.at(3)?.number()? > 0.,
        "IFC: precision must be positive",
    )?;
    ensure(
        axis(&es, context.at(4)?)? == ([0., 0., 0.], [1., 0.]),
        "IFC: nonidentity world context unsupported",
    )?;
    let units = target(&es, project.at(8)?, "IFCUNITASSIGNMENT")?;
    let unit = target(&es, one(units.at(0)?)?, "IFCSIUNIT")?;
    ensure(
        unit.args
            == [
                Value::Derived,
                Value::Enum("LENGTHUNIT".into()),
                Value::Null,
                Value::Enum("METRE".into()),
            ],
        "IFC: only SI metres supported",
    )?;

    let mut ids = BTreeMap::new();
    let mut global = BTreeSet::new();
    for (n, e) in &es {
        if matches!(
            e.kind.as_str(),
            "IFCPROJECT"
                | "IFCSITE"
                | "IFCBUILDING"
                | "IFCBUILDINGSTOREY"
                | "IFCWALL"
                | "IFCOPENINGELEMENT"
                | "IFCDOOR"
                | "IFCWINDOW"
                | "IFCDOORTYPE"
                | "IFCWINDOWTYPE"
                | "IFCRELVOIDSELEMENT"
                | "IFCRELFILLSELEMENT"
                | "IFCRELDEFINESBYTYPE"
                | "IFCRELAGGREGATES"
                | "IFCRELCONTAINEDINSPATIALSTRUCTURE"
        ) {
            let id = identity(e.at(0)?.text()?)?;
            ensure(global.insert(id), "IFC: duplicate GlobalId")?;
            if !e.kind.starts_with("IFCREL") {
                ensure(
                    e.at(2)?.text()?.chars().count() <= 255,
                    "IFC: Label name exceeds 255 characters",
                )?;
                ids.insert(*n, id);
            }
        }
    }
    let mut parents = BTreeMap::new();
    let mut containers = BTreeMap::new();
    for e in es.values() {
        let containment = e.kind == "IFCRELCONTAINEDINSPATIALSTRUCTURE";
        if !containment && e.kind != "IFCRELAGGREGATES" {
            continue;
        }
        nulls(e, &[1, 2, 3])?;
        let parent = e.at(if containment { 5 } else { 4 })?.reference()?;
        let children = e.at(if containment { 4 } else { 5 })?.list()?;
        ensure(!children.is_empty(), "IFC: empty relationship")?;
        for c in children {
            let child = c.reference()?;
            let pk = es[&parent].kind.as_str();
            let ck = es[&child].kind.as_str();
            ensure(
                if containment {
                    pk == "IFCBUILDINGSTOREY" && matches!(ck, "IFCWALL" | "IFCDOOR" | "IFCWINDOW")
                } else {
                    matches!(
                        (pk, ck),
                        ("IFCPROJECT", "IFCSITE")
                            | ("IFCSITE", "IFCBUILDING")
                            | ("IFCBUILDING", "IFCBUILDINGSTOREY")
                    )
                },
                "IFC: unsupported spatial hierarchy",
            )?;
            let map = if containment {
                &mut containers
            } else {
                &mut parents
            };
            ensure(
                map.insert(child, parent).is_none(),
                "IFC: multiple containment/decomposition parents",
            )?;
        }
    }
    let mut model = Model::new(project.at(2)?.text()?);
    model.project.header.id = ids[&project_step];
    model.sites.clear();
    model.buildings.clear();
    model.levels.clear();
    for (n, e) in &es {
        if !matches!(
            e.kind.as_str(),
            "IFCSITE" | "IFCBUILDING" | "IFCBUILDINGSTOREY"
        ) {
            continue;
        }
        nulls(e, &[1, 3, 4, 6, 7])?;
        eq(e.at(8)?, Value::Enum("ELEMENT".into()))?;
        let parent = parent_of(&parents, *n)?;
        let parent_placement = if parent == project_step {
            None
        } else {
            Some(es[&parent].at(5)?.reference()?)
        };
        let (xyz, d) = placement(&es, e, parent_placement)?;
        ensure(
            d == [1., 0.] && xyz[0] == 0. && xyz[1] == 0.,
            "IFC: translated/rotated spatial origins unsupported",
        )?;
        let name = e.at(2)?.text()?.to_owned();
        let id = ids[n];
        match e.kind.as_str() {
            "IFCSITE" => {
                nulls(e, &[9, 10, 11, 12, 13])?;
                ensure(xyz[2] == 0., "IFC: site offset unsupported")?;
                let mut v = Site::new(
                    "core.site",
                    SiteParams {
                        name,
                        project: id_of(&ids, parent)?,
                    },
                );
                v.header.id = id;
                model.sites.insert(id, v);
            }
            "IFCBUILDING" => {
                nulls(e, &[9, 10, 11])?;
                ensure(xyz[2] == 0., "IFC: building offset unsupported")?;
                let mut v = Building::new(
                    "core.building",
                    BuildingParams {
                        name,
                        site: id_of(&ids, parent)?,
                    },
                );
                v.header.id = id;
                model.buildings.insert(id, v);
            }
            _ => {
                ensure(
                    (e.at(9)?.number()? - xyz[2]).abs() < 1e-8,
                    "IFC: storey elevation disagrees with placement",
                )?;
                let mut v = Level::new(
                    "core.level",
                    LevelParams {
                        name,
                        elevation: xyz[2],
                        building: id_of(&ids, parent)?,
                    },
                );
                v.header.id = id;
                model.levels.insert(id, v);
            }
        }
    }
    let mut wall_profile_origins = BTreeMap::new();
    for (n, e) in &es {
        if e.kind != "IFCWALL" {
            continue;
        }
        nulls(e, &[1, 3, 4, 7])?;
        eq(e.at(8)?, Value::Enum("NOTDEFINED".into()))?;
        let parent = parent_of(&containers, *n)?;
        let (origin, d) = placement(&es, e, Some(es[&parent].at(5)?.reference()?))?;
        ensure(origin[2] == 0., "IFC: wall base offset unsupported")?;
        let definition = target(&es, e.at(6)?, "IFCPRODUCTDEFINITIONSHAPE")?;
        nulls(definition, &[0, 1])?;
        let shape = target(&es, one(definition.at(2)?)?, "IFCSHAPEREPRESENTATION")?;
        eq(shape.at(0)?, context_ref.clone())?;
        eq(shape.at(1)?, Value::Text("Body".into()))?;
        eq(shape.at(2)?, Value::Text("SweptSolid".into()))?;
        let solid = target(&es, one(shape.at(3)?)?, "IFCEXTRUDEDAREASOLID")?;
        ensure(
            axis(&es, solid.at(1)?)? == ([0., 0., 0.], [1., 0.]),
            "IFC: solid transform unsupported",
        )?;
        ensure(
            coords(&es, solid.at(2)?, "IFCDIRECTION")? == [0., 0., 1.],
            "IFC: nonvertical extrusion unsupported",
        )?;
        let profile = target(&es, solid.at(0)?, "IFCRECTANGLEPROFILEDEF")?;
        eq(profile.at(0)?, Value::Enum("AREA".into()))?;
        nulls(profile, &[1])?;
        let pa = target(&es, profile.at(2)?, "IFCAXIS2PLACEMENT2D")?;
        nulls(pa, &[1])?;
        let center = coords(&es, pa.at(0)?, "IFCCARTESIANPOINT")?;
        ensure(center.len() == 2, "IFC: expected 2D profile center")?;
        let length = profile.at(3)?.number()?;
        let thickness = profile.at(4)?.number()?;
        let height = solid.at(3)?.number()?;
        ensure(
            length > 1e-6 && (length * thickness * height).is_finite(),
            "IFC: invalid length/volume",
        )?;
        let x = center[0] - length / 2.;
        let y = center[1];
        wall_profile_origins.insert(*n, [x, y]);
        let start = Point2::new(
            origin[0] + d[0] * x - d[1] * y,
            origin[1] + d[1] * x + d[0] * y,
        );
        let mut wall = Wall::new(
            "org.openstructure.walls.wall",
            WallParams {
                name: e.at(2)?.text()?.into(),
                start,
                end: Point2::new(start.x + d[0] * length, start.y + d[1] * length),
                thickness,
                height,
                level: id_of(&ids, parent)?,
                material: None,
            },
        );
        wall.header.id = ids[n];
        model.walls.insert(wall.id(), wall);
    }
    import_openings(
        &es,
        &ids,
        &containers,
        &wall_profile_origins,
        context_ref,
        &mut model,
    )?;
    model.validate()?;
    let mut warnings = vec!["Imported the restricted IFC4 wall subset; created a default native 3D view. Original native views, materials and extension properties cannot be recovered from this exchange.".into()];
    if !model.openings.is_empty() {
        warnings.push("Imported rectangular hosted opening voids and filling semantics. IFC fillings have no component Body; native default rectangular panels/panes are regenerated, not recovered from IFC geometry.".into());
    }
    Ok(Exchange {
        value: model,
        warnings,
    })
}
