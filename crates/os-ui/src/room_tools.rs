//! Native room placement and single-transaction property edits.
use crate::Editor;
use os_core::{Error, Id, Point2, Result, ensure};
use os_document::Command;
use os_geometry::rooms::FaceKey;
use os_model::{Room, RoomParams};

pub(super) fn create_at(
    editor: &mut Editor,
    level: Id,
    seed: Point2,
    boundary_signature: Vec<(Id, bool)>,
) -> Result<Id> {
    ensure(
        FaceKey::from_signature(&boundary_signature)
            .is_some_and(|key| key.as_signature() == boundary_signature),
        "room face has an invalid boundary signature",
    )?;
    let number = next_number(editor, level);
    let room = Room::new(
        "core.room",
        RoomParams {
            number: number.to_string(),
            name: format!("Room {number}"),
            level,
            seed,
            boundary_signature,
        },
    );
    let id = room.id();
    room.parameters.validate()?;
    editor.command("Place room", Command::AddRoom(room))?;
    Ok(id)
}

fn next_number(editor: &Editor, level: Id) -> u32 {
    let used: std::collections::BTreeSet<_> = editor
        .document
        .model()
        .rooms
        .values()
        .filter(|room| room.parameters.level == level)
        .filter_map(|room| room.parameters.number.parse::<u32>().ok())
        .collect();
    (1..=used.len() as u32 + 1)
        .find(|number| !used.contains(number))
        .unwrap_or(1)
}

pub(super) fn apply_properties(
    editor: &mut Editor,
    id: Id,
    number: &str,
    name: &str,
) -> Result<()> {
    let mut parameters = editor
        .document
        .model()
        .rooms
        .get(&id)
        .ok_or_else(|| Error::Invalid("room missing".into()))?
        .parameters
        .clone();
    parameters.number = number.into();
    parameters.name = name.into();
    editor.command(
        "Edit room properties",
        Command::UpdateRoom { id, parameters },
    )
}

pub(super) fn delete(editor: &mut Editor, id: Id) -> Result<()> {
    editor.command("Delete room", Command::RemoveRoom(id))
}
