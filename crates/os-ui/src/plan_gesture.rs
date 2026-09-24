//! Native two-point wall draft. Previews never enter document history.
//! The explicit installed adapter freezes API-2 inputs for worker-only commit.
use crate::Editor;
use os_core::{Id, Point2, Result, ensure};
use os_model::WallParams;
use os_render::plan::PlanContext;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WallEdit {
    Move,
    ResizeStart,
    ResizeEnd,
    OffsetCopy,
}

pub struct WallGesture {
    context: PlanContext,
    activation: Id,
    installed: bool,
    parameters: WallParams,
    edit: Option<(Id, WallEdit)>,
    pub start: Option<Point2>,
    pub length: String,
    pub angle_degrees: String,
}
impl WallGesture {
    #[cfg(feature = "external-plugins")]
    pub(crate) fn is_installed(&self) -> bool {
        self.installed
    }
    pub fn begin(editor: &Editor, view: Id, parameters: WallParams) -> Result<Self> {
        Self::begin_with_provider(editor, view, parameters, false)
    }
    fn begin_with_provider(
        editor: &Editor,
        view: Id,
        mut parameters: WallParams,
        installed: bool,
    ) -> Result<Self> {
        ensure(
            editor.host.manifests().any(|m| {
                m.id == os_walls::PLUGIN_ID && (installed || m.entrypoint == "builtin:os-walls")
            }),
            "Pointer Wall currently requires the trusted bundled Wall provider",
        )?;
        let context = editor.native_plan_context(view)?;
        parameters.level = editor.document.model().views[&view]
            .parameters
            .level
            .expect("validated plan level");
        parameters.validate()?;
        Ok(Self {
            context,
            installed,
            activation: editor
                .host
                .activation_id(os_walls::PLUGIN_ID)
                .expect("checked provider"),
            parameters,
            edit: None,
            start: None,
            length: String::new(),
            angle_degrees: String::new(),
        })
    }
    /// Explicit independent Wall adapter. It uses the existing API-2 metre-field
    /// contract, not arbitrary plugin gesture callbacks. No guest runs in preview.
    #[cfg(feature = "external-plugins")]
    pub fn begin_installed(editor: &Editor, view: Id, parameters: WallParams) -> Result<Self> {
        ensure(
            editor.host.worker_supported(os_walls::PLUGIN_ID),
            "Installed pointer Wall needs a bounded Wasm worker",
        )?;
        ensure(
            parameters.material.is_none(),
            "Installed Wall creation does not support material assignment",
        )?;
        Self::installed_descriptor(editor, os_plugin_api::generic::Mode::Create)?;
        Self::begin_with_provider(editor, view, parameters, true)
    }

    #[cfg(feature = "external-plugins")]
    fn installed_descriptor(editor: &Editor, mode: os_plugin_api::generic::Mode) -> Result<&str> {
        Self::command(
            editor.host.catalog(os_walls::PLUGIN_ID).ok_or_else(|| {
                os_core::Error::Invalid("Installed Wall catalog is unavailable".into())
            })?,
            mode,
        )
    }

    #[cfg(feature = "external-plugins")]
    fn command(
        catalog: &os_plugin_api::generic::Catalog,
        mode: os_plugin_api::generic::Mode,
    ) -> Result<&str> {
        use os_plugin_api::generic::{FieldKind, Unit};
        let commands: Vec<_> = catalog
            .commands
            .iter()
            .filter(|c| c.mode == mode && c.element_type == os_plugin_api::wall::TYPE)
            .collect();
        ensure(
            commands.len() == 1,
            "Installed pointer Wall requires one unambiguous matching command",
        )?;
        let command = commands[0];
        let fields = [
            "start_x",
            "start_y",
            "end_x",
            "end_y",
            "thickness",
            "height",
        ];
        ensure(
            command.fields.len() == fields.len()
                && command.fields.iter().all(|f| {
                    fields.contains(&f.key.as_str())
                        && matches!(
                            f.kind,
                            FieldKind::Number {
                                unit: Unit::Metres,
                                ..
                            }
                        )
                }),
            "Installed Wall does not expose the supported six metre inputs",
        )?;
        ensure(
            command.enabled,
            command
                .disabled_reason
                .as_deref()
                .unwrap_or("Installed Wall command is disabled"),
        )?;
        Ok(&command.id)
    }

    /// Freeze the exact destination into a normal scoped tool draft. Callers must
    /// submit with this view ticket and retain cancellation until worker commit.
    #[cfg(feature = "external-plugins")]
    pub fn installed_command(
        &self,
        editor: &Editor,
        active: Option<Id>,
        point: Point2,
    ) -> Result<(
        crate::plugin_tools::ToolDraft,
        crate::plugin_tools::ToolContext,
        os_plugin_host::worker::ViewContext,
    )> {
        ensure(
            self.installed && self.current(editor, active),
            "Installed Wall gesture is stale or belongs to another provider",
        )?;
        let p = self.parameters(point)?;
        let context = crate::plugin_tools::ToolContext {
            level: p.level,
            selection: self
                .edit
                .and_then(|(id, mode)| (mode != WallEdit::OffsetCopy).then_some(id)),
        };
        let mode = if context.selection.is_some() {
            os_plugin_api::generic::Mode::Edit
        } else {
            os_plugin_api::generic::Mode::Create
        };
        let mut draft = crate::plugin_tools::ToolDraft::begin(
            &editor.host,
            &editor.document,
            os_walls::PLUGIN_ID,
            Self::installed_descriptor(editor, mode)?,
            context,
        )?;
        for (key, value) in [
            ("start_x", p.start.x),
            ("start_y", p.start.y),
            ("end_x", p.end.x),
            ("end_y", p.end.y),
            ("thickness", p.thickness),
            ("height", p.height),
        ] {
            draft.set(key, serde_json::Value::from(value))?;
        }
        draft.validate()?;
        Ok((
            draft,
            context,
            os_plugin_host::worker::ViewContext {
                id: self.context.view_id,
                settings_revision: self.context.settings_revision,
            },
        ))
    }
    pub fn begin_edit(editor: &Editor, view: Id, id: Id, mode: WallEdit) -> Result<Self> {
        Self::begin_edit_with_provider(editor, view, id, mode, false)
    }
    #[cfg(feature = "external-plugins")]
    pub fn begin_edit_installed(editor: &Editor, view: Id, id: Id, mode: WallEdit) -> Result<Self> {
        ensure(
            editor.host.worker_supported(os_walls::PLUGIN_ID),
            "Installed pointer Wall needs a bounded Wasm worker",
        )?;
        Self::installed_descriptor(
            editor,
            if mode == WallEdit::OffsetCopy {
                os_plugin_api::generic::Mode::Create
            } else {
                os_plugin_api::generic::Mode::Edit
            },
        )?;
        Self::begin_edit_with_provider(editor, view, id, mode, true)
    }
    fn begin_edit_with_provider(
        editor: &Editor,
        view: Id,
        id: Id,
        mode: WallEdit,
        installed: bool,
    ) -> Result<Self> {
        let original = editor.document.model().resolve_wall(id)?.parameters;
        ensure(
            !installed
                || !editor
                    .document
                    .model()
                    .wall_type_assignments
                    .contains_key(&id),
            "Generic API 2 cannot edit or copy typed walls; use native wall tools",
        )?;
        let mut gesture = Self::begin_with_provider(editor, view, original.clone(), installed)?;
        if installed {
            ensure(
                original.level == gesture.parameters.level,
                "Installed Wall edits require the source wall's level plan",
            )?;
            ensure(
                mode != WallEdit::OffsetCopy || original.material.is_none(),
                "Installed Wall copies do not support material assignment",
            )?;
        }
        let c = gesture.context;
        let elevation = editor.document.model().levels[&original.level]
            .parameters
            .elevation;
        ensure(
            c.show_walls
                && os_geometry::plan::rectangular_plan(
                    &os_walls::wall_solid(&original, elevation)?,
                    c.range,
                    c.basis,
                    c.crop,
                )?
                .is_some(),
            "Selected wall is not visible in this plan",
        )?;
        gesture.parameters = original.clone(); // Editing never reassigns the wall's level.
        gesture.edit = Some((id, mode));
        gesture.start = match mode {
            WallEdit::Move => None,
            WallEdit::ResizeStart => Some(c.basis.world_to_plane(original.end)?),
            WallEdit::ResizeEnd | WallEdit::OffsetCopy => {
                Some(c.basis.world_to_plane(original.start)?)
            }
        };
        Ok(gesture)
    }
    pub fn edit_mode(&self) -> Option<WallEdit> {
        self.edit.map(|(_, mode)| mode)
    }
    /// Fully specified drafts do not need snap acquisition. Invalid exact text
    /// still follows this route so its validation error is not masked by snapping.
    pub fn has_exact_destination(&self) -> bool {
        self.start.is_some()
            && !self.length.trim().is_empty()
            && (self.edit_mode() == Some(WallEdit::OffsetCopy)
                || !self.angle_degrees.trim().is_empty())
    }
    pub fn snap_exclusion(&self) -> Option<Id> {
        self.start.and(self.edit.map(|(id, _)| id))
    }
    pub fn offset_measurement(&self, preview: &WallParams) -> Option<f64> {
        (self.edit_mode() == Some(WallEdit::OffsetCopy)).then(|| {
            let dx = self.parameters.end.x - self.parameters.start.x;
            let dy = self.parameters.end.y - self.parameters.start.y;
            ((preview.start.x - self.parameters.start.x) * (-dy / self.parameters.length()))
                + ((preview.start.y - self.parameters.start.y) * (dx / self.parameters.length()))
        })
    }
    pub fn prompt(&self) -> &'static str {
        match self.edit_mode() {
            None => {
                if self.start.is_none() {
                    "Click wall start"
                } else {
                    "Click wall end"
                }
            }
            Some(WallEdit::Move) => {
                if self.start.is_none() {
                    "Click move base point"
                } else {
                    "Click move destination"
                }
            }
            Some(WallEdit::ResizeStart) => "Place wall start (end stays fixed)",
            Some(WallEdit::ResizeEnd) => "Place wall end (start stays fixed)",
            Some(WallEdit::OffsetCopy) => "Place parallel copy (+ left, - right)",
        }
    }
    pub fn current(&self, editor: &Editor, active: Option<Id>) -> bool {
        active == Some(self.context.view_id)
            && editor.host.activation_id(os_walls::PLUGIN_ID) == Some(self.activation)
            && editor
                .native_plan_context(self.context.view_id)
                .is_ok_and(|c| c == self.context)
            && editor.host.manifests().any(|m| {
                m.id == os_walls::PLUGIN_ID
                    && (self.installed || m.entrypoint == "builtin:os-walls")
            })
    }
    pub fn parameters(&self, pointer: Point2) -> Result<WallParams> {
        ensure(pointer.is_finite(), "invalid wall pointer")?;
        let start = self
            .start
            .ok_or_else(|| os_core::Error::Invalid("Choose a start point".into()))?;
        ensure(start.is_finite(), "invalid wall start")?;
        if self.edit_mode() == Some(WallEdit::OffsetCopy) {
            ensure(
                self.angle_degrees.trim().is_empty(),
                "Offset keeps the source direction; angle is not supported",
            )?;
            let length = self.parameters.length();
            let nx = -(self.parameters.end.y - self.parameters.start.y) / length;
            let ny = (self.parameters.end.x - self.parameters.start.x) / length;
            // Express the original world normal in the plan basis, without a large-origin roundtrip.
            let (s, c) = self.context.basis.rotation.sin_cos();
            let distance = if self.length.trim().is_empty() {
                (pointer.x - start.x) * (c * nx + s * ny)
                    + (pointer.y - start.y) * (-s * nx + c * ny)
            } else {
                self.length
                    .trim()
                    .parse::<f64>()
                    .map_err(|_| os_core::Error::Invalid("Offset needs a number".into()))?
            };
            ensure(
                distance.is_finite() && distance.abs() > 1e-6,
                "Offset magnitude must exceed one micrometre",
            )?;
            let mut result = self.parameters.clone();
            result.start = Point2::new(
                result.start.x + nx * distance,
                result.start.y + ny * distance,
            );
            result.end = Point2::new(result.end.x + nx * distance, result.end.y + ny * distance);
            result.validate()?;
            ensure(
                result.start != self.parameters.start && result.end != self.parameters.end,
                "Offset is below coordinate precision",
            )?;
            return Ok(result);
        }
        let delta = Point2::new(pointer.x - start.x, pointer.y - start.y);
        let measured = delta.x.hypot(delta.y);
        let moving = self.edit_mode() == Some(WallEdit::Move);
        if moving
            && measured == 0.0
            && self.length.trim().is_empty()
            && self.angle_degrees.trim().is_empty()
        {
            return Ok(self.parameters.clone());
        }
        let parse = |value: &str, label: &str| -> Result<f64> {
            let number = value
                .trim()
                .parse::<f64>()
                .map_err(|_| os_core::Error::Invalid(format!("{label} needs a number")))?;
            ensure(number.is_finite(), format!("{label} must be finite"))?;
            Ok(number)
        };
        let length = if self.length.trim().is_empty() {
            measured
        } else {
            parse(&self.length, "Length")?
        };
        ensure(
            length.is_finite() && (length > 1e-6 || (moving && length == 0.0)),
            "Length must exceed one micrometre (move distance may be zero)",
        )?;
        let angle = if self.angle_degrees.trim().is_empty() {
            ensure(
                measured.is_finite() && measured > 1e-9,
                "Move the pointer or enter an exact angle",
            )?;
            delta.y.atan2(delta.x)
        } else {
            parse(&self.angle_degrees, "Angle")?.to_radians()
        };
        let end = Point2::new(
            start.x + length * angle.cos(),
            start.y + length * angle.sin(),
        );
        let mut parameters = self.parameters.clone();
        let destination = if self.length.trim().is_empty() && self.angle_degrees.trim().is_empty() {
            pointer
        } else {
            end
        };
        if moving {
            // Rotate displacement, without adding/subtracting a large basis origin.
            let dx = destination.x - start.x;
            let dy = destination.y - start.y;
            let (s, c) = self.context.basis.rotation.sin_cos();
            let world = Point2::new(c * dx - s * dy, s * dx + c * dy);
            parameters.start =
                Point2::new(parameters.start.x + world.x, parameters.start.y + world.y);
            parameters.end = Point2::new(parameters.end.x + world.x, parameters.end.y + world.y);
            parameters.validate()?;
            return Ok(parameters);
        }
        parameters.start = self.context.basis.plane_to_world(start)?;
        // With no exact override, preserve the snapped endpoint's original bits.
        parameters.end = self.context.basis.plane_to_world(
            if self.length.trim().is_empty() && self.angle_degrees.trim().is_empty() {
                pointer
            } else {
                end
            },
        )?;
        match self.edit_mode() {
            Some(WallEdit::ResizeStart) => {
                parameters.start = parameters.end;
                parameters.end = self.parameters.end;
            }
            Some(WallEdit::ResizeEnd) => parameters.start = self.parameters.start,
            _ => {}
        }
        parameters.validate()?;
        Ok(parameters)
    }
    pub fn commit(&self, editor: &mut Editor, active: Option<Id>, point: Point2) -> Result<()> {
        ensure(
            !self.installed,
            "Installed Wall gestures must commit through a bounded worker",
        )?;
        ensure(
            self.current(editor, active),
            "Wall gesture is stale; start again",
        )?;
        let mut parameters = self.parameters(point)?;
        if let Some((source, _)) = self.edit
            && editor
                .document
                .model()
                .wall_type_assignments
                .contains_key(&source)
        {
            let independent = &editor.document.model().walls[&source].parameters;
            parameters.thickness = independent.thickness;
            parameters.material = independent.material;
        }
        if let Some((source, WallEdit::OffsetCopy)) = self.edit
            && let Some(assignment) = editor
                .document
                .model()
                .wall_type_assignments
                .get(&source)
                .copied()
        {
            let wall = os_model::Wall::new(os_walls::WALL_TYPE, parameters);
            let id = wall.id();
            editor.document.execute(
                "Offset typed wall in plan",
                vec![
                    os_document::Command::AddWall(wall),
                    os_document::Command::AssignWallType {
                        wall: id,
                        assignment: Some(assignment),
                    },
                ],
            )?;
            return editor.regenerate();
        }
        let elevation = editor.document.model().levels[&parameters.level]
            .parameters
            .elevation;
        os_geometry::GeometryKernel::tessellate(
            &os_geometry::PrismKernel,
            &os_walls::wall_solid(&parameters, elevation)?,
        )?;
        let (label, request) = match self.edit {
            None => (
                "Draw wall in plan",
                os_plugin_api::Request::CreateWall(parameters),
            ),
            Some((_, WallEdit::OffsetCopy)) => (
                "Offset wall in plan",
                os_plugin_api::Request::CreateWall(parameters),
            ),
            Some((id, mode)) => (
                if mode == WallEdit::Move {
                    "Move wall in plan"
                } else {
                    "Resize wall in plan"
                },
                os_plugin_api::Request::EditWall { id, parameters },
            ),
        };
        editor.wall_command(label, request)
    }
}

#[cfg(all(test, feature = "external-plugins"))]
mod installed_tests {
    use super::*;
    use os_plugin_api::generic::*;

    fn catalog() -> Catalog {
        Catalog {
            types: vec![], // Host catalog validation is upstream of this adapter.
            commands: vec![CommandDescriptor {
                id: "org.openstructure.walls.create".into(),
                element_type: os_plugin_api::wall::TYPE.into(),
                mode: Mode::Create,
                fields: [
                    "start_x",
                    "start_y",
                    "end_x",
                    "end_y",
                    "thickness",
                    "height",
                ]
                .into_iter()
                .map(|key| Field {
                    key: key.into(),
                    label: key.into(),
                    kind: FieldKind::Number {
                        unit: Unit::Metres,
                        min: -1e6,
                        max: 1e6,
                        default: 1.0,
                    },
                })
                .collect(),
                enabled: true,
                disabled_reason: None,
            }],
        }
    }
    #[test]
    fn installed_adapter_requires_exact_units_shape_and_unambiguous_command() {
        let original = catalog();
        assert_eq!(
            WallGesture::command(&original, Mode::Create).unwrap(),
            original.commands[0].id
        );
        let mut c = original.clone();
        c.commands.push(c.commands[0].clone());
        assert!(WallGesture::command(&c, Mode::Create).is_err());
        let mut c = original.clone();
        c.commands[0].fields.pop();
        assert!(WallGesture::command(&c, Mode::Create).is_err());
        let mut c = original.clone();
        c.commands[0].fields[0].kind = FieldKind::Boolean { default: false };
        assert!(WallGesture::command(&c, Mode::Create).is_err());
        let mut c = original.clone();
        if let FieldKind::Number { unit, .. } = &mut c.commands[0].fields[0].kind {
            *unit = Unit::Scalar;
        }
        assert!(WallGesture::command(&c, Mode::Create).is_err());
        let mut c = original.clone();
        c.commands[0].fields[0].key = "screen_x".into();
        assert!(WallGesture::command(&c, Mode::Create).is_err());
        let mut c = original.clone();
        c.commands[0].element_type = "other.wall".into();
        assert!(WallGesture::command(&c, Mode::Create).is_err());
        let mut c = original;
        c.commands[0].enabled = false;
        c.commands[0].disabled_reason = Some("Not supported".into());
        assert!(
            WallGesture::command(&c, Mode::Create)
                .unwrap_err()
                .to_string()
                .contains("Not supported")
        );
    }

    #[test]
    fn installed_edit_selection_does_not_fall_back_to_creation() {
        let mut c = catalog();
        assert!(WallGesture::command(&c, Mode::Edit).is_err());
        let mut edit = c.commands[0].clone();
        edit.id = "org.openstructure.walls.edit".into();
        edit.mode = Mode::Edit;
        c.commands.push(edit.clone());
        assert_eq!(WallGesture::command(&c, Mode::Edit).unwrap(), edit.id);
        assert_eq!(
            WallGesture::command(&c, Mode::Create).unwrap(),
            c.commands[0].id
        );
        c.commands.push(edit);
        assert!(WallGesture::command(&c, Mode::Edit).is_err());
        assert!(WallGesture::command(&c, Mode::Create).is_ok());
    }
}
