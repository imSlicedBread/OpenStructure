//! Persistent room intent. Boundaries and areas are always derived, never stored.
use crate::{Entity, Model};
use os_core::{Id, Point2, Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoomParams {
    pub number: String,
    pub name: String,
    /// Optional finish codes, not physical material assignments.
    #[serde(deserialize_with = "Option::<String>::deserialize")]
    pub floor_finish: Option<String>,
    #[serde(deserialize_with = "Option::<String>::deserialize")]
    pub wall_finish: Option<String>,
    #[serde(deserialize_with = "Option::<String>::deserialize")]
    pub ceiling_finish: Option<String>,
    /// Stable project material assignments, independent of the legacy finish codes.
    #[serde(deserialize_with = "Option::<Id>::deserialize")]
    pub floor_material: Option<Id>,
    #[serde(deserialize_with = "Option::<Id>::deserialize")]
    pub wall_material: Option<Id>,
    #[serde(deserialize_with = "Option::<Id>::deserialize")]
    pub ceiling_material: Option<Id>,
    pub level: Id,
    pub seed: Point2,
    /// Accepted ordered boundary entity UUID/orientation cycle from `FaceKey::as_signature`.
    /// Orientation follows lexicographically increasing quantized endpoints.
    /// This is topology, not geometry. IDs may refer to deleted walls or separators: unresolved
    /// enclosure is a derived diagnostic and never a model validation failure.
    pub boundary_signature: Vec<(Id, bool)>,
}
pub type Room = Entity<RoomParams>;

impl RoomParams {
    /// Validate intent only. Loss of enclosure must never prevent a wall edit.
    pub fn validate(&self) -> Result<()> {
        for code in [&self.floor_finish, &self.wall_finish, &self.ceiling_finish]
            .into_iter()
            .flatten()
        {
            ensure(
                !code.is_empty()
                    && code.len() <= 128
                    && code.trim() == code
                    && !code.chars().any(char::is_control),
                "room finish code must be trimmed, nonempty, and at most 128 bytes without control characters",
            )?;
        }
        for (value, field) in [(&self.number, "number"), (&self.name, "name")] {
            ensure(
                !value.trim().is_empty()
                    && value.len() <= 256
                    && !value.chars().any(char::is_control),
                format!("room {field} must be 1-256 bytes without control characters"),
            )?;
        }
        ensure(
            self.seed.is_finite() && self.seed.x.abs() <= 1e6 && self.seed.y.abs() <= 1e6,
            "room seed must be finite and within +/- 1000000 metres",
        )?;
        let unique: std::collections::BTreeSet<_> =
            self.boundary_signature.iter().copied().collect();
        ensure(
            (3..=1024).contains(&self.boundary_signature.len())
                && unique.len() == self.boundary_signature.len()
                && self.boundary_signature.iter().all(|(id, _)| !id.0.is_nil()),
            "room boundary signature needs 3-1024 distinct directed non-nil boundary IDs",
        )
    }
}

impl Model {
    /// Native same-level wall centerlines and room-separation lines, sorted by UUID. Pass directly to
    /// `os_geometry::rooms::derive_faces`. Crop, visibility, elevation, thickness
    /// and hosted apertures are deliberately absent from this derivation input.
    /// Extension/plugin entities are not native room boundary sources.
    pub fn room_boundary_segments(&self, level: Id) -> Result<Vec<(Id, Point2, Point2)>> {
        ensure(
            self.levels.contains_key(&level),
            "room boundary level missing",
        )?;
        let mut segments: Vec<_> = self
            .walls
            .values()
            .filter(|w| w.parameters.level == level)
            .filter(|w| w.parameters.path.is_straight())
            .map(|w| (w.id(), w.parameters.start(), w.parameters.end()))
            .chain(
                self.room_separation_lines
                    .values()
                    .filter(|line| line.parameters.level == level)
                    .map(|line| (line.id(), line.parameters.start, line.parameters.end)),
            )
            .collect();
        segments.sort_by_key(|segment| segment.0);
        Ok(segments)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn room_intent_validates_without_enclosure_and_rejects_invalid_data() {
        let mut model = Model::new("Rooms");
        let room = Room::new(
            "core.room",
            RoomParams {
                floor_material: None,
                wall_material: None,
                ceiling_material: None,
                floor_finish: None,
                wall_finish: None,
                ceiling_finish: None,
                number: "101".into(),
                name: "Office".into(),
                level: *model.levels.keys().next().unwrap(),
                seed: Point2::new(1.0, 1.0),
                boundary_signature: vec![(Id::new(), true), (Id::new(), true), (Id::new(), false)],
            },
        );
        model.rooms.insert(room.id(), room.clone());
        model.validate().unwrap();
        for case in 0..7 {
            let mut bad = model.clone();
            let entity = bad.rooms.get_mut(&room.id()).unwrap();
            match case {
                0 => entity.parameters.number = " ".into(),
                1 => entity.parameters.name = "bad\nname".into(),
                2 => entity.parameters.seed.x = f64::NAN,
                3 => entity.parameters.level = Id::new(),
                4 => entity.header.id = Id::new(),
                5 => entity.parameters.name = "x".repeat(257),
                _ => entity.parameters.seed.y = 1e7,
            }
            assert!(bad.validate().is_err());
        }
        let value = serde_json::to_value(&model.rooms[&room.id()]).unwrap();
        assert!(value["parameters"].get("boundary").is_none());
        assert!(value["parameters"].get("area").is_none());
        let mut frozen = value;
        frozen["parameters"]["boundary"] = serde_json::json!([]);
        assert!(serde_json::from_value::<Room>(frozen).is_err());
    }

    #[test]
    fn room_numbers_are_unique_per_level_but_not_globally() {
        let mut model = Model::new("Rooms");
        let level = *model.levels.keys().next().unwrap();
        let room = |number: &str| {
            Room::new(
                "core.room",
                RoomParams {
                    floor_material: None,
                    wall_material: None,
                    ceiling_material: None,
                    floor_finish: None,
                    wall_finish: None,
                    ceiling_finish: None,
                    number: number.into(),
                    name: format!("Room {number}"),
                    level,
                    seed: Point2::new(1.0, 1.0),
                    boundary_signature: vec![
                        (Id::new(), true),
                        (Id::new(), false),
                        (Id::new(), true),
                    ],
                },
            )
        };
        let first = room("101");
        let second = room("101");
        model.rooms.insert(first.id(), first);
        model.rooms.insert(second.id(), second.clone());
        assert!(model.validate().is_err());
        model.rooms.get_mut(&second.id()).unwrap().parameters.number = "102".into();
        model.validate().unwrap();
    }
}
