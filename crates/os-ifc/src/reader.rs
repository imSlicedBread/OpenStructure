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

pub fn import(bytes: &[u8]) -> Result<Exchange<Model>> {
    let es = step::parse(bytes)?;
    // Arity and entity whitelist prevent silently ignoring additional product types,
    // properties, materials, styles, openings or unsupported geometry.
    let arities: BTreeMap<&str, usize> = [
        ("IFCPROJECT", 9),
        ("IFCSITE", 14),
        ("IFCBUILDING", 12),
        ("IFCBUILDINGSTOREY", 10),
        ("IFCWALL", 9),
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
                    pk == "IFCBUILDINGSTOREY" && ck == "IFCWALL"
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
    model.validate()?;
    Ok(Exchange{value:model,warnings:vec!["Imported the restricted IFC4 wall subset; created a default native 3D view. Original native views, materials and extension properties cannot be recovered from this exchange.".into()]})
}
