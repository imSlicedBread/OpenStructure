//! Callable, read-only plan service; desktop/worker adaptation is separate.
use crate::{PluginHost, generic::entity_to_wire, invalid, require};
use os_core::{Id, Result, ensure};
use os_document::Document;
use os_plugin_api::{Capability, Permission, RegistrationKind, generic, plan};
use std::{
    collections::BTreeSet,
    time::{Duration, Instant},
};

pub struct PlanRequest {
    pub provider: String,
    pub element: Id,
    pub view: Id,
    pub read: BTreeSet<Id>,
    pub timeout: Duration,
}

pub struct PlanGraphics {
    stamp: generic::Stamp,
    view: plan::Context,
    element: Id,
    plugin: String,
    activation: Id,
    segments: Vec<plan::Segment>,
}

pub(super) struct PreparedPlan {
    #[cfg(feature = "wasm")]
    pub(super) id: Id,
    pub(super) input: String,
    pub(super) deadline: Instant,
    request: plan::Request,
    element: Id,
    view: Id,
    plugin: String,
    activation: Id,
}

fn stamp(doc: &Document) -> generic::Stamp {
    generic::Stamp {
        project: doc.model().project.id().to_string(),
        session: doc.session_id().to_string(),
        revision: doc.revision(),
    }
}

fn context(doc: &Document, view: Id) -> Result<plan::Context> {
    let view = doc
        .model()
        .views
        .get(&view)
        .ok_or_else(|| invalid("plan view missing"))?;
    let settings = view
        .parameters
        .plan
        .ok_or_else(|| invalid("not a supported floor plan"))?;
    settings.validate()?;
    let level_id = view
        .parameters
        .level
        .ok_or_else(|| invalid("plan level missing"))?;
    let level = doc
        .model()
        .levels
        .get(&level_id)
        .ok_or_else(|| invalid("plan level missing"))?;
    settings.range.at_level(level.parameters.elevation)?;
    Ok(plan::Context {
        view_id: view.id().to_string(),
        settings_revision: view.parameters.settings_revision,
        level_id: level_id.to_string(),
        level_elevation: level.parameters.elevation,
        origin: [settings.basis.origin.x, settings.basis.origin.y],
        yaw: settings.basis.rotation,
        range: [
            settings.range.top,
            settings.range.cut,
            settings.range.bottom,
            settings.range.depth,
        ],
        crop: settings.crop.map(|c| [c.min.x, c.min.y, c.max.x, c.max.y]),
        scale_denominator: settings.scale_denominator,
        show_walls: settings.visibility.walls,
        show_extensions: settings.visibility.extensions,
    })
}

impl PlanGraphics {
    /// Unclipped semantic segments, not an issued drawing or invisible pick targets.
    /// The consumer must apply host clipping/styles before display and picking.
    pub fn segments(
        &self,
        host: &PluginHost,
        doc: &Document,
        active_view: Id,
    ) -> Result<&[plan::Segment]> {
        ensure(self.stamp == stamp(doc), "stale plan document")?;
        ensure(self.view == context(doc, active_view)?, "stale plan view")?;
        ensure(
            host.plugin(&self.plugin)?.activation_id == self.activation,
            "plan provider changed",
        )?;
        Ok(&self.segments)
    }
    pub fn element(&self) -> Id {
        self.element
    }
}

impl PluginHost {
    /// Explicit read authority for exactly the target and its plan level. Guest
    /// execution is synchronous: desktop code must not call this on a frame path.
    /// Existing API-2 guests that do not implement service 1 reject the request;
    /// ViewProvider metadata alone is not proof that it is supported.
    pub fn generate_plan_graphics(
        &self,
        plugin: &str,
        doc: &Document,
        invocation: PlanRequest,
    ) -> Result<PlanGraphics> {
        let prepared = self.prepare_plan_graphics(plugin, doc, invocation)?;
        let output = self.plugin(plugin)?.plugin.invoke_json(&prepared.input)?;
        self.finish_plan_graphics(doc, &prepared, &output)
    }

    pub(super) fn prepare_plan_graphics(
        &self,
        plugin: &str,
        doc: &Document,
        invocation: PlanRequest,
    ) -> Result<PreparedPlan> {
        let PlanRequest {
            provider,
            element,
            view,
            read,
            timeout,
        } = invocation;
        let started = Instant::now();
        ensure(
            !timeout.is_zero() && timeout <= Duration::from_secs(30),
            "invalid plan deadline",
        )?;
        let loaded = self.plugin(plugin)?;
        require(loaded, Permission::ModelRead)?;
        ensure(
            loaded.manifest.api_version == generic::VERSION,
            "plan service requires API 2",
        )?;
        ensure(
            loaded.manifest.capabilities.contains(&Capability::Views)
                && loaded
                    .manifest
                    .registrations
                    .iter()
                    .any(|r| r.id == provider && r.kind == RegistrationKind::ViewProvider),
            "plan provider not registered",
        )?;
        #[cfg(feature = "wasm")]
        ensure(
            !loaded.worker.as_ref().is_some_and(|r| r.is_busy()),
            "plugin already has a live worker",
        )?;
        let view_context = context(doc, view)?;
        let level = doc.model().views[&view]
            .parameters
            .level
            .expect("context checked");
        ensure(
            read == BTreeSet::from([element, level]),
            "plan scope must contain target and plan level only",
        )?;
        let native = doc.model().walls.get(&element);
        let entity = if let Some(wall) = native {
            ensure(view_context.show_walls, "native walls hidden in plan")?;
            crate::native_wall::project_checked(doc.model(), wall)?
        } else {
            ensure(view_context.show_extensions, "extensions hidden in plan")?;
            entity_to_wire(
                doc.model()
                    .extensions
                    .get(&element)
                    .ok_or_else(|| invalid("plan target missing"))?,
            )
        };
        ensure(
            entity.owner == plugin,
            "plan target belongs to another owner",
        )?;
        let kind = loaded
            .catalog
            .as_ref()
            .and_then(|c| c.types.iter().find(|t| t.id == entity.type_id))
            .ok_or_else(|| invalid("plan target type not registered"))?;
        ensure(
            kind.payload_schema_version == entity.payload_schema_version,
            "plan target requires migration",
        )?;
        ensure(
            doc.model()
                .plugin_requirements
                .get(plugin)
                .map_or(native.is_some(), |r| r.version == loaded.manifest.version),
            "plan target requires another plugin version",
        )?;
        let level_params = &doc.model().levels[&level].parameters;
        let request_id = Id::new();
        let request = plan::Request {
            api_version: generic::VERSION,
            service: plan::Service::Graphics,
            service_version: plan::VERSION,
            request_id: request_id.to_string(),
            context: stamp(doc),
            provider_id: provider,
            element_id: element.to_string(),
            view: view_context.clone(),
            snapshot: generic::Snapshot {
                elements: vec![entity],
                levels: vec![generic::Level {
                    id: level.to_string(),
                    name: level_params.name.clone(),
                    elevation_metres: level_params.elevation,
                }],
            },
        };
        let input = serde_json::to_string(&request).map_err(invalid)?;
        ensure(
            input.len() <= generic::MAX_BYTES && started.elapsed() < timeout,
            "plan request too large or expired",
        )?;
        Ok(PreparedPlan {
            #[cfg(feature = "wasm")]
            id: request_id,
            input,
            deadline: started + timeout,
            request,
            element,
            view,
            plugin: plugin.into(),
            activation: loaded.activation_id,
        })
    }

    pub(super) fn finish_plan_graphics(
        &self,
        doc: &Document,
        prepared: &PreparedPlan,
        output: &str,
    ) -> Result<PlanGraphics> {
        let request = &prepared.request;
        ensure(request.context == stamp(doc), "stale plan document")?;
        ensure(
            request.view == context(doc, prepared.view)?,
            "stale plan view",
        )?;
        ensure(
            self.plugin(&prepared.plugin)?.activation_id == prepared.activation,
            "plan provider changed",
        )?;
        ensure(
            output.len() <= generic::MAX_BYTES && Instant::now() < prepared.deadline,
            "plan response too large or expired",
        )?;
        let response: plan::Response = serde_json::from_str(output).map_err(invalid)?;
        ensure(
            response.api_version == generic::VERSION
                && response.service_version == plan::VERSION
                && response.request_id == request.request_id
                && response.context == request.context
                && response.element_id == request.element_id
                && response.view == request.view,
            "plan reply version/context/target mismatch",
        )?;
        let mut segments = match response.result {
            plan::Reply::Graphics(segments) => segments,
            plan::Reply::Error(error) => {
                ensure(
                    error.message.len() <= 4096
                        && error.field.as_ref().is_none_or(|f| f.len() <= 64),
                    "oversized plan error",
                )?;
                return Err(invalid(format!(
                    "Plan provider {:?}: {}",
                    error.code, error.message
                )));
            }
        };
        plan::validate_segments(&segments)?;
        segments.sort_by_key(|segment| segment.feature);
        ensure(
            Instant::now() < prepared.deadline,
            "plan acceptance deadline expired",
        )?;
        Ok(PlanGraphics {
            stamp: request.context.clone(),
            view: request.view.clone(),
            element: prepared.element,
            plugin: prepared.plugin.clone(),
            activation: prepared.activation,
            segments,
        })
    }
}
