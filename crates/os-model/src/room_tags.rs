//! View-owned room labels; text is always resolved from the live room.
use crate::{Entity, Model, Room, ViewKind};
use os_core::{Id, Point2, Result, ensure};
use serde::{Deserialize, Serialize};

pub const MAX_ROOM_TAGS: usize = 10_000;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoomTagParams {
    pub view: Id,
    pub room: Id,
    /// World XY in metres, independent of the view basis.
    pub position: Point2,
}
pub type RoomTag = Entity<RoomTagParams>;

impl RoomTagParams {
    pub fn validate(&self, model: &Model) -> Result<()> {
        ensure(!self.room.0.is_nil(), "room tag target must not be nil")?;
        ensure(
            self.position.is_finite()
                && self.position.x.abs() <= 1e6
                && self.position.y.abs() <= 1e6,
            "room tag position must be finite and within +/- 1000000 metres",
        )?;
        ensure(
            model.views.get(&self.view).is_some_and(|v| {
                v.parameters.kind == ViewKind::Plan && v.parameters.level.is_some()
            }),
            "room tag requires a plan view",
        )
    }

    pub fn resolve<'a>(&self, model: &'a Model) -> std::result::Result<&'a Room, &'static str> {
        let level = model
            .views
            .get(&self.view)
            .filter(|v| v.parameters.kind == ViewKind::Plan)
            .and_then(|v| v.parameters.level)
            .ok_or("Missing plan view")?;
        let room = model.rooms.get(&self.room).ok_or("Missing room")?;
        if room.parameters.level != level {
            return Err("Room is on another level");
        }
        Ok(room)
    }

    pub fn validate_creation(&self, model: &Model) -> Result<()> {
        self.validate(model)?;
        self.resolve(model)
            .map_err(|reason| os_core::Error::Invalid(reason.into()))?;
        Ok(())
    }
}

pub(crate) fn validate(model: &Model) -> Result<()> {
    ensure(
        model.room_tags.len() <= MAX_ROOM_TAGS,
        "room tags exceed 10000 elements",
    )?;
    let mut pairs = std::collections::BTreeSet::new();
    for tag in model.room_tags.values() {
        tag.parameters.validate(model)?;
        ensure(
            pairs.insert((tag.parameters.view, tag.parameters.room)),
            "only one room tag per room per view",
        )?;
    }
    Ok(())
}
