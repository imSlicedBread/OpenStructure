use crate::{
    Exchange,
    step::{guid, quote},
};
use os_core::{Id, Result, ensure};
use os_model::{DoorHinge, DoorSwing, Model, OpeningFamily, OpeningKind, WindowPanePosition};
use std::collections::BTreeMap;

#[cfg(test)]
mod floor_tests {
    use super::*;
    #[test]
    fn floors_are_explicitly_refused_instead_of_dropped() {
        let mut m = Model::new("Floors");
        let floor = os_model::Floor::new(
            "core.floor",
            os_model::FloorParams {
                name: "Slab".into(),
                level: *m.levels.keys().next().unwrap(),
                material: None,
                boundary: vec![
                    os_core::Point2::new(0., 0.),
                    os_core::Point2::new(2., 0.),
                    os_core::Point2::new(0., 2.),
                ],
                thickness: 0.2,
                top_offset: 0.0,
            },
        );
        m.floors.insert(floor.id(), floor);
        let error = export(&m).unwrap_err();
        assert!(matches!(error, os_core::Error::Unsupported(_)));
        assert!(error.to_string().contains("floors/slabs"));
    }
}

#[derive(Default)]
struct Writer {
    rows: Vec<String>,
}
impl Writer {
    fn add(&mut self, kind: &str, args: String) -> u32 {
        let id = self.rows.len() as u32 + 1;
        self.rows.push(format!("#{id}={kind}({args});\n"));
        id
    }
    fn point(&mut self, x: f64, y: f64, z: f64) -> u32 {
        self.add("IFCCARTESIANPOINT", format!("({x:?},{y:?},{z:?})"))
    }
    fn axis(&mut self, x: f64, y: f64, z: f64, dx: f64, dy: f64) -> u32 {
        let p = self.point(x, y, z);
        let direction = self.add("IFCDIRECTION", format!("({dx:?},{dy:?},0.)"));
        let up = self.add("IFCDIRECTION", "(0.,0.,1.)".into());
        self.add("IFCAXIS2PLACEMENT3D", format!("#{p},#{up},#{direction}"))
    }
    fn placement(&mut self, parent: Option<u32>, xyz: [f64; 3], xy: [f64; 2]) -> u32 {
        let a = self.axis(xyz[0], xyz[1], xyz[2], xy[0], xy[1]);
        self.add(
            "IFCLOCALPLACEMENT",
            format!("{},#{a}", parent.map_or("$".into(), |p| format!("#{p}"))),
        )
    }
    fn opening_body(&mut self, context: u32, width: f64, height: f64, depth: f64) -> u32 {
        // Profile XY is wall XZ; profile +Z points across the wall toward -Y.
        let center = self.add(
            "IFCCARTESIANPOINT",
            format!("({:?},{:?})", width / 2., height / 2.),
        );
        let pa = self.add("IFCAXIS2PLACEMENT2D", format!("#{center},$"));
        let profile = self.add(
            "IFCRECTANGLEPROFILEDEF",
            format!(".AREA.,$,#{pa},{width:?},{height:?}"),
        );
        let origin = self.point(0., depth / 2., 0.);
        let across = self.add("IFCDIRECTION", "(0.,-1.,0.)".into());
        let along = self.add("IFCDIRECTION", "(1.,0.,0.)".into());
        let axis = self.add(
            "IFCAXIS2PLACEMENT3D",
            format!("#{origin},#{across},#{along}"),
        );
        let extrusion = self.add("IFCDIRECTION", "(0.,0.,1.)".into());
        let solid = self.add(
            "IFCEXTRUDEDAREASOLID",
            format!("#{profile},#{axis},#{extrusion},{depth:?}"),
        );
        let shape = self.add(
            "IFCSHAPEREPRESENTATION",
            format!("#{context},'Body','SweptSolid',(#{solid})"),
        );
        self.add("IFCPRODUCTDEFINITIONSHAPE", format!("$,$,(#{shape})"))
    }
    fn relation(&mut self, parent: u32, children: &[u32], containment: bool) {
        if children.is_empty() {
            return;
        }
        let list = children
            .iter()
            .map(|n| format!("#{n}"))
            .collect::<Vec<_>>()
            .join(",");
        let tail = if containment {
            format!("({list}),#{parent}")
        } else {
            format!("#{parent},({list})")
        };
        self.add(
            if containment {
                "IFCRELCONTAINEDINSPATIALSTRUCTURE"
            } else {
                "IFCRELAGGREGATES"
            },
            format!("'{}',$,$,$,{tail}", guid(Id::new())),
        );
    }
}

// IFC4 swing is toward filling +Y. A 180-degree yaw reverses both X and Y,
// so the native jamb must be inverted when determining the IFC hinge side.
fn operation(kind: OpeningKind, hinge: DoorHinge, swing: DoorSwing) -> &'static str {
    if kind == OpeningKind::Window {
        "SINGLE_PANEL"
    } else if (hinge == DoorHinge::Start) == (swing == DoorSwing::Left) {
        "SINGLE_SWING_LEFT"
    } else {
        "SINGLE_SWING_RIGHT"
    }
}

pub fn export(m: &Model) -> Result<Exchange<Vec<u8>>> {
    m.validate()?;
    if !m.wall_types.is_empty() || !m.wall_type_assignments.is_empty() {
        return Err(os_core::Error::Unsupported("IFC export cannot preserve reusable compound wall types/layers; keep the native .osb document".into()));
    }
    if !m.wall_joins.is_empty() {
        return Err(os_core::Error::Unsupported(
            "IFC wall export cannot preserve explicit butt joins; keep the native .osb document"
                .into(),
        ));
    }
    if !m.floors.is_empty() {
        return Err(os_core::Error::Unsupported(
            "IFC wall export does not support floors/slabs; keep the native .osb document".into(),
        ));
    }
    if !m.columns.is_empty() {
        return Err(os_core::Error::Unsupported(
            "IFC export does not support native columns; keep the native .osb document".into(),
        ));
    }
    let mut type_operations = BTreeMap::new();
    for e in m.openings.values() {
        let p = m.resolve_opening(&e.parameters)?;
        if p.family != OpeningFamily::default() || p.pane_position != WindowPanePosition::Center {
            return Err(os_core::Error::Unsupported("IFC hosted openings require the exact default rectangular family and centered pane".into()));
        }
        if let Some(id) = p.type_id {
            let op = operation(p.kind, p.hinge, p.swing);
            if type_operations.insert(id, op).is_some_and(|old| old != op) {
                return Err(os_core::Error::Unsupported("IFC shared door type requires one operation type; native instances require conflicting IFC hands (type splitting is not supported)".into()));
            }
        }
    }
    if m.opening_types
        .keys()
        .any(|id| !type_operations.contains_key(id))
    {
        return Err(os_core::Error::Unsupported("IFC cannot preserve unused opening types: dimensions and sill are reconstructed from their occurrences".into()));
    }
    if !m.extensions.is_empty() {
        return Err(os_core::Error::Unsupported(format!(
            "IFC wall export has no mapping for {} plugin elements; keep the native .osb document",
            m.extensions.len()
        )));
    }
    for name in std::iter::once(m.project.parameters.name.as_str())
        .chain(m.sites.values().map(|e| e.parameters.name.as_str()))
        .chain(m.buildings.values().map(|e| e.parameters.name.as_str()))
        .chain(m.levels.values().map(|e| e.parameters.name.as_str()))
        .chain(m.walls.values().map(|e| e.parameters.name.as_str()))
        .chain(m.openings.values().map(|e| e.parameters.name.as_str()))
        .chain(m.opening_types.values().map(|e| e.parameters.name.as_str()))
    {
        ensure(
            name.chars().count() <= 255,
            "IFC Label names must not exceed 255 characters",
        )?;
    }
    ensure(
        m.sites.len()
            + m.buildings.len()
            + m.levels.len()
            + m.walls.len()
            + m.openings.len()
            + m.opening_types.len()
            < 3000,
        "IFC subset supports fewer than 3000 spatial/wall/opening/type entities",
    )?;
    let mut warnings = Vec::new();
    if !m.openings.is_empty() {
        warnings.push("Hosted opening voids, filling identity/dimensions/orientation and shared types are exported. Door panels, window panes, frames, component geometry, family parameters and materials are not represented in IFC; fillings have no Body. Import regenerates only the native default rectangular family. Keep the native .osb document.".into());
    }
    if !m.grids.is_empty() {
        warnings.push(format!("{} native architectural grids and their metadata are not exported; keep the native .osb document.", m.grids.len()));
    }
    if !m.plugin_requirements.is_empty() {
        warnings.push("Native plugin requirement records are not exported.".into());
    }
    if !m.views.is_empty() {
        warnings.push("Native views are not exported; import creates a default 3D view.".into());
    }
    if !m.materials.is_empty() {
        warnings.push(
            "Native materials, densities and wall material assignments are not exported.".into(),
        );
    }
    let headers = std::iter::once(&m.project.header)
        .chain(m.sites.values().map(|e| &e.header))
        .chain(m.buildings.values().map(|e| &e.header))
        .chain(m.levels.values().map(|e| &e.header))
        .chain(m.walls.values().map(|e| &e.header))
        .chain(m.openings.values().map(|e| &e.header))
        .chain(m.opening_types.values().map(|e| &e.header));
    if headers
        .into_iter()
        .any(|h| !h.properties.is_empty() || !h.relationships.is_empty())
    {
        warnings.push(
            "Extensible properties and additional named relationships are not exported.".into(),
        );
    }
    let mut w = Writer::default();
    let world = w.axis(0., 0., 0., 1., 0.);
    let context = w.add(
        "IFCGEOMETRICREPRESENTATIONCONTEXT",
        format!("$,'Model',3,1.E-7,#{world},$"),
    );
    let metre = w.add("IFCSIUNIT", "*,.LENGTHUNIT.,$,.METRE.".into());
    let units = w.add("IFCUNITASSIGNMENT", format!("(#{metre})"));
    let project = w.add(
        "IFCPROJECT",
        format!(
            "'{}',$,{},$,$,$,$,(#{context}),#{units}",
            guid(m.project.id()),
            quote(&m.project.parameters.name)
        ),
    );
    let mut entities = BTreeMap::from([(m.project.id(), project)]);
    let mut placements = BTreeMap::new();
    for e in m.sites.values() {
        let p = w.placement(None, [0., 0., 0.], [1., 0.]);
        placements.insert(e.id(), p);
        let id = w.add(
            "IFCSITE",
            format!(
                "'{}',$,{},$,$,#{p},$,$,.ELEMENT.,$,$,$,$,$",
                guid(e.id()),
                quote(&e.parameters.name)
            ),
        );
        entities.insert(e.id(), id);
    }
    for e in m.buildings.values() {
        let p = w.placement(Some(placements[&e.parameters.site]), [0., 0., 0.], [1., 0.]);
        placements.insert(e.id(), p);
        let id = w.add(
            "IFCBUILDING",
            format!(
                "'{}',$,{},$,$,#{p},$,$,.ELEMENT.,$,$,$",
                guid(e.id()),
                quote(&e.parameters.name)
            ),
        );
        entities.insert(e.id(), id);
    }
    for e in m.levels.values() {
        let z = e.parameters.elevation;
        let p = w.placement(
            Some(placements[&e.parameters.building]),
            [0., 0., z],
            [1., 0.],
        );
        placements.insert(e.id(), p);
        let id = w.add(
            "IFCBUILDINGSTOREY",
            format!(
                "'{}',$,{},$,$,#{p},$,$,.ELEMENT.,{z:?}",
                guid(e.id()),
                quote(&e.parameters.name)
            ),
        );
        entities.insert(e.id(), id);
    }
    for e in m.walls.values() {
        let v = &e.parameters;
        let derived = os_geometry::walls::NativeWall::from_model(m, e.id())?;
        // IFC void relationships subtract from the complete, uncut wall body.
        let len = v.length();
        ensure(
            derived.net_volume()?.is_finite(),
            "IFC wall volume overflow",
        )?;
        let p = w.placement(
            Some(placements[&v.level]),
            [v.start.x, v.start.y, 0.],
            [(v.end.x - v.start.x) / len, (v.end.y - v.start.y) / len],
        );
        placements.insert(e.id(), p);
        let cp = w.add("IFCCARTESIANPOINT", format!("({:?},0.)", len / 2.));
        let pa = w.add("IFCAXIS2PLACEMENT2D", format!("#{cp},$"));
        let profile = w.add(
            "IFCRECTANGLEPROFILEDEF",
            format!(".AREA.,$,#{pa},{len:?},{:?}", v.thickness),
        );
        let sa = w.axis(0., 0., 0., 1., 0.);
        let z = w.add("IFCDIRECTION", "(0.,0.,1.)".into());
        let solid = w.add(
            "IFCEXTRUDEDAREASOLID",
            format!("#{profile},#{sa},#{z},{:?}", v.height),
        );
        let shape = w.add(
            "IFCSHAPEREPRESENTATION",
            format!("#{context},'Body','SweptSolid',(#{solid})"),
        );
        let definition = w.add("IFCPRODUCTDEFINITIONSHAPE", format!("$,$,(#{shape})"));
        let wall = w.add(
            "IFCWALL",
            format!(
                "'{}',$,{},$,$,#{p},#{definition},$,.NOTDEFINED.",
                guid(e.id()),
                quote(&v.name)
            ),
        );
        entities.insert(e.id(), wall);
    }
    for e in m.opening_types.values() {
        let door = e.parameters.kind == OpeningKind::Door;
        let kind = if door { "IFCDOORTYPE" } else { "IFCWINDOWTYPE" };
        let predefined = if door { "DOOR" } else { "WINDOW" };
        let op = type_operations[&e.id()];
        let id = w.add(
            kind,
            format!(
                "'{}',$,{},$,$,$,$,$,$,.{predefined}.,.{op}.,$,$",
                guid(e.id()),
                quote(&e.parameters.name)
            ),
        );
        entities.insert(e.id(), id);
    }
    for e in m.openings.values() {
        let v = m.resolve_opening(&e.parameters)?;
        let host = &m.walls[&v.host].parameters;
        let p = w.placement(Some(placements[&v.host]), [v.offset, 0., v.sill], [1., 0.]);
        let body = w.opening_body(context, v.width, v.height, host.thickness);
        let void = w.add(
            "IFCOPENINGELEMENT",
            format!(
                "'{}',$,{},$,$,#{p},#{body},$,.OPENING.",
                guid(Id::new()),
                quote(&v.name)
            ),
        );
        w.add(
            "IFCRELVOIDSELEMENT",
            format!("'{}',$,$,$,#{},#{void}", guid(Id::new()), entities[&v.host]),
        );
        let reverse = v.kind == OpeningKind::Door && v.swing == DoorSwing::Right;
        let fill_p = w.placement(
            Some(p),
            [if reverse { v.width } else { 0. }, 0., 0.],
            [if reverse { -1. } else { 1. }, 0.],
        );
        let door = v.kind == OpeningKind::Door;
        let kind = if door { "IFCDOOR" } else { "IFCWINDOW" };
        let predefined = if door { "DOOR" } else { "WINDOW" };
        let op = operation(v.kind, v.hinge, v.swing);
        // Description is an explicit bounded dialect discriminator: deleting a
        // type relationship must not silently turn a Typed instance into Legacy.
        let (description, classification) = if v.type_id.is_some() {
            ("OpenStructure.Typed.v1", "$,$".into())
        } else {
            ("OpenStructure.Legacy.v1", format!(".{predefined}.,.{op}."))
        };
        let fill = w.add(
            kind,
            format!(
                "'{}',$,{},'{description}',$,#{fill_p},$,$,{:?},{:?},{classification},$",
                guid(e.id()),
                quote(&v.name),
                v.height,
                v.width
            ),
        );
        entities.insert(e.id(), fill);
        w.add(
            "IFCRELFILLSELEMENT",
            format!("'{}',$,$,$,#{void},#{fill}", guid(Id::new())),
        );
    }
    for e in m.opening_types.values() {
        let fills = m
            .openings
            .values()
            .filter(|o| o.parameters.type_id() == Some(e.id()))
            .map(|o| format!("#{}", entities[&o.id()]))
            .collect::<Vec<_>>()
            .join(",");
        w.add(
            "IFCRELDEFINESBYTYPE",
            format!(
                "'{}',$,$,$,({fills}),#{}",
                guid(Id::new()),
                entities[&e.id()]
            ),
        );
    }
    w.relation(
        project,
        &m.sites.keys().map(|id| entities[id]).collect::<Vec<_>>(),
        false,
    );
    for e in m.sites.values() {
        w.relation(
            entities[&e.id()],
            &m.buildings
                .values()
                .filter(|b| b.parameters.site == e.id())
                .map(|b| entities[&b.id()])
                .collect::<Vec<_>>(),
            false,
        );
    }
    for e in m.buildings.values() {
        w.relation(
            entities[&e.id()],
            &m.levels
                .values()
                .filter(|l| l.parameters.building == e.id())
                .map(|l| entities[&l.id()])
                .collect::<Vec<_>>(),
            false,
        );
    }
    for e in m.levels.values() {
        w.relation(
            entities[&e.id()],
            &m.walls
                .values()
                .filter(|v| v.parameters.level == e.id())
                .map(|v| entities[&v.id()])
                .chain(
                    m.openings
                        .values()
                        .filter(|o| m.walls[&o.parameters.host].parameters.level == e.id())
                        .map(|o| entities[&o.id()]),
                )
                .collect::<Vec<_>>(),
            true,
        );
    }
    let bytes = format!("ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('OpenStructure restricted wall exchange'),'2;1');\nFILE_NAME('model.ifc','2026-09-12T00:00:00',(''),(''),'OpenStructure 0.1','OpenStructure','');\nFILE_SCHEMA(('IFC4'));\nENDSEC;\nDATA;\n{}ENDSEC;\nEND-ISO-10303-21;\n",w.rows.concat()).into_bytes();
    ensure(
        bytes.len() <= 16 * 1024 * 1024,
        "IFC export exceeds 16 MiB limit",
    )?;
    Ok(Exchange {
        value: bytes,
        warnings,
    })
}
