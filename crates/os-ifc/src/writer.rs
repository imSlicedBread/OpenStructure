use crate::{
    Exchange,
    step::{guid, quote},
};
use os_core::{Id, Result, ensure};
use os_model::Model;
use std::collections::BTreeMap;

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
pub fn export(m: &Model) -> Result<Exchange<Vec<u8>>> {
    m.validate()?;
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
    {
        ensure(
            name.chars().count() <= 255,
            "IFC Label names must not exceed 255 characters",
        )?;
    }
    ensure(
        m.sites.len() + m.buildings.len() + m.levels.len() + m.walls.len() < 3000,
        "IFC subset supports fewer than 3000 spatial/wall entities",
    )?;
    let mut warnings = Vec::new();
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
        .chain(m.walls.values().map(|e| &e.header));
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
        let len = v.length();
        ensure(
            (len * v.thickness * v.height).is_finite(),
            "IFC wall volume overflow",
        )?;
        let p = w.placement(
            Some(placements[&v.level]),
            [v.start.x, v.start.y, 0.],
            [(v.end.x - v.start.x) / len, (v.end.y - v.start.y) / len],
        );
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
