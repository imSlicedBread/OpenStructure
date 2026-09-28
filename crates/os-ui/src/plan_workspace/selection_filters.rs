//! Session-owned selection eligibility. Keep this state across plan view switches;
//! create a fresh default for each document session. These filters do not control
//! visibility, snapping, editing, or 3D selection.

use eframe::egui;
use os_core::Id;
use os_model::{Model, OpeningKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub(super) enum Category {
    Wall,
    Door,
    Window,
    Floor,
    Ceiling,
    Column,
    Roof,
    Stair,
    Room,
    Grid,
    Annotation,
    Other,
}

impl Category {
    pub(super) const ALL: [Self; 12] = [
        Self::Wall,
        Self::Door,
        Self::Window,
        Self::Floor,
        Self::Ceiling,
        Self::Column,
        Self::Roof,
        Self::Stair,
        Self::Room,
        Self::Grid,
        Self::Annotation,
        Self::Other,
    ];

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Wall => "Wall",
            Self::Door => "Door",
            Self::Window => "Window",
            Self::Floor => "Floor",
            Self::Ceiling => "Ceiling",
            Self::Column => "Column",
            Self::Roof => "Roof",
            Self::Stair => "Stair",
            Self::Room => "Room",
            Self::Grid => "Grid",
            Self::Annotation => "Annotation",
            Self::Other => "Other",
        }
    }

    pub(super) fn classify(model: &Model, id: Id) -> Self {
        if model.walls.contains_key(&id) {
            Self::Wall
        } else if let Some(opening) = model.openings.get(&id) {
            match model.resolve_opening(&opening.parameters) {
                Ok(opening) => match opening.kind {
                    OpeningKind::Door => Self::Door,
                    OpeningKind::Window => Self::Window,
                },
                Err(_) => Self::Other,
            }
        } else if model.floors.contains_key(&id) {
            Self::Floor
        } else if model.ceilings.contains_key(&id) {
            Self::Ceiling
        } else if model.columns.contains_key(&id) {
            Self::Column
        } else if model.roofs.contains_key(&id) {
            Self::Roof
        } else if model.stairs.contains_key(&id) {
            Self::Stair
        } else if model.rooms.contains_key(&id) {
            Self::Room
        } else if model.grids.contains_key(&id) {
            Self::Grid
        } else if model.dimensions.contains_key(&id)
            || model.opening_tags.contains_key(&id)
            || model.room_tags.contains_key(&id)
            || model.detail_lines.contains_key(&id)
            || model.room_separation_lines.contains_key(&id)
        {
            Self::Annotation
        } else {
            Self::Other
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SelectionFilters {
    enabled: [bool; Category::ALL.len()],
}

impl Default for SelectionFilters {
    fn default() -> Self {
        Self {
            enabled: [true; Category::ALL.len()],
        }
    }
}

impl SelectionFilters {
    /// Returns true only when eligibility changes, so callers can cancel drafts.
    pub(super) fn set_enabled(&mut self, category: Category, enabled: bool) -> bool {
        let was_enabled = &mut self.enabled[category as usize];
        let changed = *was_enabled != enabled;
        *was_enabled = enabled;
        changed
    }

    /// Enables every category and reports whether eligibility changed.
    pub(super) fn reset_all(&mut self) -> bool {
        let changed = self.enabled.iter().any(|&enabled| !enabled);
        self.enabled.fill(true);
        changed
    }

    pub(super) fn allows(&self, model: &Model, id: Id) -> bool {
        self.enabled[category_for_id(model, id) as usize]
    }

    /// Returns whether eligibility changed. The caller must cancel transient
    /// selection drafts on change, while preserving the current selection.
    pub(super) fn show(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;
        let count = self.enabled.iter().filter(|&&enabled| enabled).count();
        // A stable ID keeps the popup open when the enabled count changes.
        ui.push_id("plan_selection_filters", |ui| {
            ui.menu_button("Selection filters", |ui| {
                ui.label(format!(
                    "{count}/{} categories enabled",
                    Category::ALL.len()
                ));
                ui.separator();
                for category in Category::ALL {
                    let mut enabled = self.enabled[category as usize];
                    if ui.checkbox(&mut enabled, category.label()).changed() {
                        changed |= self.set_enabled(category, enabled);
                    }
                }
                ui.separator();
                if ui.button("Reset to All").clicked() {
                    changed |= self.reset_all();
                }
            });
        });
        changed
    }
}

pub(super) fn category_for_id(model: &Model, id: Id) -> Category {
    Category::classify(model, id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use os_model::{Opening, OpeningDefinition, OpeningParams, OpeningType, OpeningTypeParams};

    fn insert_opening(model: &mut Model, kind: OpeningKind, typed: bool) -> Id {
        let definition = if typed {
            let ty = OpeningType::new(
                "core.opening_type",
                OpeningTypeParams {
                    window_operation: Default::default(),
                    family: Default::default(),
                    name: "Test type".into(),
                    kind,
                    width: 0.9,
                    height: 2.1,
                    sill: 0.0,
                    pane_position: Default::default(),
                },
            );
            let type_id = ty.id();
            model.opening_types.insert(type_id, ty);
            OpeningDefinition::Typed { type_id }
        } else {
            OpeningDefinition::Legacy {
                kind,
                width: 0.9,
                height: 2.1,
                sill: 0.0,
            }
        };
        let opening = Opening::new(
            "core.opening",
            OpeningParams {
                name: "Test opening".into(),
                host: Id::new(),
                offset: 1.0,
                definition,
                width_override: None,
                height_override: None,
                sill_override: None,
                pane_position_override: None,
                lite_side_override: None,
                hinge: Default::default(),
                swing: Default::default(),
            },
        );
        let id = opening.id();
        model.resolve_opening(&opening.parameters).unwrap();
        model.openings.insert(id, opening);
        id
    }

    #[test]
    fn defaults_enable_every_category() {
        let filters = SelectionFilters::default();
        for category in Category::ALL {
            assert!(filters.enabled[category as usize], "{category:?}");
        }
        assert!(filters.allows(&Model::new("Filters"), Id::new()));
    }

    #[test]
    fn setters_and_reset_report_only_changes() {
        let mut filters = SelectionFilters::default();
        assert!(!filters.reset_all());
        for category in Category::ALL {
            assert!(!filters.set_enabled(category, true));
            assert!(filters.set_enabled(category, false));
            assert!(!filters.set_enabled(category, false));
            for other in Category::ALL {
                assert_eq!(filters.enabled[other as usize], other != category);
            }
            assert!(filters.set_enabled(category, true));
            assert!(!filters.reset_all());
        }
        for category in Category::ALL {
            assert!(filters.set_enabled(category, false));
        }
        assert!(filters.reset_all());
        assert_eq!(filters, SelectionFilters::default());
        assert!(!filters.reset_all());
    }

    #[test]
    fn typed_and_legacy_doors_and_windows_filter_independently() {
        for typed in [false, true] {
            let mut model = Model::new("Filters");
            let door = insert_opening(&mut model, OpeningKind::Door, typed);
            let window = insert_opening(&mut model, OpeningKind::Window, typed);
            assert_eq!(category_for_id(&model, door), Category::Door);
            assert_eq!(category_for_id(&model, window), Category::Window);
            let before = model.clone();
            let mut filters = SelectionFilters::default();
            assert!(filters.allows(&model, door));
            assert!(filters.allows(&model, window));
            filters.set_enabled(Category::Door, false);
            assert!(!filters.allows(&model, door));
            assert!(filters.allows(&model, window));
            filters.set_enabled(Category::Door, true);
            filters.set_enabled(Category::Window, false);
            assert!(filters.allows(&model, door));
            assert!(!filters.allows(&model, window));
            assert_eq!(model, before);
        }
    }

    #[test]
    fn typed_classification_resolves_current_type_and_missing_type_falls_back() {
        let mut model = Model::new("Filters");
        let id = insert_opening(&mut model, OpeningKind::Door, true);
        let OpeningDefinition::Typed { type_id } = model.openings[&id].parameters.definition else {
            panic!("expected typed opening");
        };
        model
            .opening_types
            .get_mut(&type_id)
            .unwrap()
            .parameters
            .kind = OpeningKind::Window;
        assert_eq!(category_for_id(&model, id), Category::Window);
        model.opening_types.remove(&type_id);
        assert_eq!(category_for_id(&model, id), Category::Other);
        let mut filters = SelectionFilters::default();
        assert!(filters.allows(&model, id));
        filters.set_enabled(Category::Other, false);
        assert!(!filters.allows(&model, id));
    }

    #[test]
    fn unknown_hits_and_invalid_legacy_openings_use_other() {
        let mut model = Model::new("Filters");
        let unknown = Id::new();
        let provider = Id::new();
        model.extensions.insert(
            provider,
            os_model::ExtensionEntity {
                envelope_version: os_model::EXTENSION_ENVELOPE_VERSION,
                id: provider,
                owner: "test.provider".into(),
                type_id: "test.provider.object".into(),
                name: "Provider object".into(),
                payload_schema_version: 1,
                relationships: Default::default(),
                depends_on: Default::default(),
                payload: Default::default(),
            },
        );
        let invalid = insert_opening(&mut model, OpeningKind::Door, false);
        model
            .openings
            .get_mut(&invalid)
            .unwrap()
            .parameters
            .width_override = Some(1.0);
        let mut filters = SelectionFilters::default();
        for id in [unknown, provider, model.project.id(), invalid] {
            assert_eq!(category_for_id(&model, id), Category::Other);
            assert!(filters.allows(&model, id));
        }
        filters.set_enabled(Category::Other, false);
        for id in [unknown, provider, model.project.id(), invalid] {
            assert!(!filters.allows(&model, id));
        }
    }

    #[test]
    fn drawing_the_popup_control_preserves_filters_without_reporting_changes() {
        let mut filters = SelectionFilters::default();
        filters.set_enabled(Category::Wall, false);
        let before = filters.clone();
        // The state belongs to the session, independent of a view's UI context.
        for _ in 0..2 {
            let ctx = egui::Context::default();
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    assert!(!filters.show(ui));
                });
            });
            assert_eq!(filters, before);
        }
    }
}
