//! Explicit host adapter: generic wire proposals -> existing native wall commands.
use os_core::{Id, Point2, Result, ensure};
use os_document::{Command, Document};
use os_model::{ExtensionEntity, Wall, WallParams};
use os_plugin_api::{generic as wire, wall};

pub(super) fn project(w: &Wall) -> Result<wire::Element> {
    ensure(
        w.header.type_id == wall::TYPE,
        "unsupported native wall type",
    )?;
    let p = &w.parameters;
    Ok(wire::Element {
        id: w.id().to_string(),
        owner: wall::OWNER.into(),
        type_id: wall::TYPE.into(),
        name: p.name.clone(),
        payload_schema_version: wall::PAYLOAD_VERSION,
        payload: serde_json::to_value(wall::Parameters {
            start_x: p.start.x,
            start_y: p.start.y,
            end_x: p.end.x,
            end_y: p.end.y,
            thickness: p.thickness,
            height: p.height,
            level: p.level.to_string(),
        })
        .map_err(crate::invalid)?,
        relationships: Default::default(),
        depends_on: [p.level.to_string()].into(),
    })
}

pub(super) fn command(doc: &Document, entity: ExtensionEntity, replace: bool) -> Result<Command> {
    ensure(
        entity.owner == wall::OWNER
            && entity.type_id == wall::TYPE
            && entity.payload_schema_version == wall::PAYLOAD_VERSION,
        "invalid native wall owner/type/schema",
    )?;
    ensure(
        entity.relationships.is_empty(),
        "native wall metadata is host-preserved, not editable through this projection",
    )?;
    let p: wall::Parameters = serde_json::from_value(entity.payload).map_err(crate::invalid)?;
    let level: Id = serde_json::from_value(serde_json::Value::String(p.level.clone()))
        .map_err(crate::invalid)?;
    ensure(
        !level.0.is_nil() && level.to_string() == p.level && entity.depends_on == [level].into(),
        "wall prerequisite must match its canonical level",
    )?;
    let previous = doc.model().walls.get(&entity.id);
    ensure(
        !replace || previous.is_some(),
        "native wall replacement target missing",
    )?;
    let parameters = WallParams {
        name: entity.name,
        start: Point2::new(p.start_x, p.start_y),
        end: Point2::new(p.end_x, p.end_y),
        thickness: p.thickness,
        height: p.height,
        level,
        // Material and arbitrary header metadata were not disclosed or editable.
        material: previous.and_then(|w| w.parameters.material),
    };
    parameters.validate()?;
    if replace {
        Ok(Command::UpdateWall {
            id: entity.id,
            parameters,
        })
    } else {
        let mut wall = Wall::new(wall::TYPE, parameters);
        wall.header.id = entity.id;
        Ok(Command::AddWall(wall))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn document() -> Document {
        let mut model = os_model::Model::new("Native metadata");
        let mut wall = Wall::new(
            wall::TYPE,
            WallParams {
                name: "Existing".into(),
                start: Point2::new(1.0, 2.0),
                end: Point2::new(4.0, 6.0),
                thickness: 0.2,
                height: 3.0,
                level: *model.levels.keys().next().unwrap(),
                material: model.materials.keys().next().copied(),
            },
        );
        wall.header
            .properties
            .insert("private_vendor_data".into(), json!({"keep":true}));
        wall.header
            .relationships
            .insert("datum".into(), vec![wall.parameters.level]);
        model.walls.insert(wall.id(), wall);
        Document::from_model(model).unwrap()
    }
    fn envelope(wall: &Wall) -> ExtensionEntity {
        let mut value = serde_json::to_value(project(wall).unwrap()).unwrap();
        value["envelope_version"] = json!(1);
        serde_json::from_value(value).unwrap()
    }
    #[test]
    fn native_projection_edits_preserve_undisclosed_metadata_and_material() {
        let mut doc = document();
        let original = doc.model().walls.values().next().unwrap().clone();
        let projection = project(&original).unwrap();
        assert!(projection.relationships.is_empty());
        assert!(projection.payload.get("private_vendor_data").is_none());
        assert!(projection.payload.get("material").is_none());
        let mut proposal = envelope(&original);
        proposal.payload["height"] = json!(4.0);
        let edit = command(&doc, proposal, true).unwrap();
        doc.execute("API2 edit", vec![edit]).unwrap();
        let edited = &doc.model().walls[&original.id()];
        assert_eq!(edited.header, original.header);
        assert_eq!(edited.parameters.material, original.parameters.material);
        assert_eq!(edited.parameters.height, 4.0);
        assert!(doc.undo());
        assert_eq!(doc.model().walls[&original.id()], original);
    }
    #[test]
    fn invalid_native_projections_fail_without_mutation() {
        let doc = document();
        let wall = doc.model().walls.values().next().unwrap();
        for fault in [
            "owner",
            "schema",
            "metadata",
            "dependency",
            "payload",
            "dimensions",
        ] {
            let mut proposal = envelope(wall);
            match fault {
                "owner" => proposal.owner = "org.other.walls".into(),
                "schema" => proposal.payload_schema_version = 2,
                "metadata" => {
                    proposal
                        .relationships
                        .insert("extra".into(), vec![wall.parameters.level]);
                }
                "dependency" => proposal.depends_on.clear(),
                "payload" => proposal.payload["unknown"] = json!(true),
                "dimensions" => proposal.payload["height"] = json!(-1),
                _ => unreachable!(),
            }
            assert!(command(&doc, proposal, true).is_err(), "{fault}");
            assert_eq!(doc.revision(), 0);
        }
    }
}
