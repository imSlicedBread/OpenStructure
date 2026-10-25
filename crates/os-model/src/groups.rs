//! Typed, reusable model-group definitions for native straight-wall assemblies.
//!
//! Definitions contain canonical wall/opening/join/tag templates. Instances
//! materialize those members in the ordinary model maps so existing geometry,
//! schedules, tags, and references continue to use their normal entity IDs.
use super::*;
use os_core::Error;

pub const MAX_GROUP_DEFINITIONS: usize = 10_000;
pub const MAX_GROUP_INSTANCES: usize = 100_000;
pub const MAX_GROUP_WALLS: usize = 64;
pub const MAX_GROUP_DEPENDENTS: usize = 4_096;

pub type GroupDefinition = Entity<GroupDefinitionParams>;
pub type GroupInstance = Entity<GroupInstanceParams>;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupDefinitionParams {
    pub name: String,
    pub template: GroupTemplate,
}

/// Supported typed group content. Slots are the keys in these maps; every
/// instance maps each slot to exactly one live entity UUID.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupTemplate {
    pub walls: BTreeMap<Id, Wall>,
    pub openings: BTreeMap<Id, Opening>,
    pub joins: BTreeMap<Id, WallJoin>,
    pub opening_tags: BTreeMap<Id, OpeningTag>,
    pub wall_type_assignments: BTreeMap<Id, WallTypeAssignment>,
    pub opening_clearances: BTreeMap<Id, OpeningClearance>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupTransform {
    /// World-space pivot used for the instance's rigid XY rotation.
    pub pivot: Point2,
    /// Rigid rotation in radians, applied before translation.
    pub rotation_radians: f64,
    pub translation: Point2,
}

impl GroupTransform {
    pub fn validate(self) -> Result<()> {
        ensure(
            self.pivot.is_finite()
                && self.translation.is_finite()
                && self.pivot.x.abs() <= 1e6
                && self.pivot.y.abs() <= 1e6
                && self.translation.x.abs() <= 1e6
                && self.translation.y.abs() <= 1e6
                && self.rotation_radians.is_finite()
                && self.rotation_radians.abs() <= 1e6,
            "invalid model-group transform",
        )
    }

    pub fn apply(self, point: Point2) -> Result<Point2> {
        self.validate()?;
        ensure(point.is_finite(), "invalid model-group point")?;
        let (sin, cos) = self.rotation_radians.sin_cos();
        let x = point.x - self.pivot.x;
        let y = point.y - self.pivot.y;
        let transformed = Point2::new(
            self.pivot.x + cos * x - sin * y + self.translation.x,
            self.pivot.y + sin * x + cos * y + self.translation.y,
        );
        ensure(
            transformed.is_finite() && transformed.x.abs() <= 1e6 && transformed.y.abs() <= 1e6,
            "model-group transform exceeds coordinate bounds",
        )?;
        Ok(transformed)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupInstanceParams {
    pub definition: Id,
    /// Stable template slot UUID → live entity UUID for this instance.
    pub members: BTreeMap<Id, Id>,
    pub transform: GroupTransform,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GroupMaterialization {
    pub walls: BTreeMap<Id, Wall>,
    pub openings: BTreeMap<Id, Opening>,
    pub joins: BTreeMap<Id, WallJoin>,
    pub opening_tags: BTreeMap<Id, OpeningTag>,
    pub wall_type_assignments: BTreeMap<Id, WallTypeAssignment>,
    pub opening_clearances: BTreeMap<Id, OpeningClearance>,
}

impl GroupTemplate {
    pub fn slot_ids(&self) -> BTreeSet<Id> {
        self.walls
            .keys()
            .chain(self.openings.keys())
            .chain(self.joins.keys())
            .chain(self.opening_tags.keys())
            .copied()
            .collect()
    }

    pub fn dependent_count(&self) -> usize {
        self.openings.len()
            + self.joins.len()
            + self.opening_tags.len()
            + self.opening_clearances.len()
    }

    fn headers(&self) -> impl Iterator<Item = (&Id, &Header, &'static str)> {
        self.walls
            .iter()
            .map(|(id, entity)| (id, &entity.header, "org.openstructure.walls.wall"))
            .chain(
                self.openings
                    .iter()
                    .map(|(id, entity)| (id, &entity.header, "core.opening")),
            )
            .chain(
                self.joins
                    .iter()
                    .map(|(id, entity)| (id, &entity.header, "core.wall_join")),
            )
            .chain(
                self.opening_tags
                    .iter()
                    .map(|(id, entity)| (id, &entity.header, "core.opening_tag")),
            )
    }
}

impl GroupDefinitionParams {
    pub fn capture_wall_assembly(
        model: &Model,
        wall_ids: &BTreeSet<Id>,
        view: Id,
        name: impl Into<String>,
    ) -> Result<(Self, BTreeMap<Id, Id>)> {
        ensure(
            (1..=MAX_GROUP_WALLS).contains(&wall_ids.len()),
            "Group requires 1–64 native straight walls",
        )?;
        let view_level = model
            .views
            .get(&view)
            .filter(|v| {
                v.parameters.kind == ViewKind::Plan
                    && v.parameters
                        .plan
                        .is_some_and(|settings| settings.view_type == PlanViewType::FloorPlan)
            })
            .and_then(|v| v.parameters.level)
            .ok_or_else(|| Error::Invalid("Group requires an active floor plan".into()))?;
        for id in wall_ids {
            let wall = model
                .walls
                .get(id)
                .ok_or_else(|| Error::Invalid("Group member wall is missing".into()))?;
            ensure(
                wall.header.type_id == "org.openstructure.walls.wall"
                    && wall.parameters.path.is_straight()
                    && wall.parameters.level == view_level,
                "Groups currently require same-level native straight walls",
            )?;
        }

        let openings: BTreeSet<_> = model
            .openings
            .values()
            .filter(|opening| wall_ids.contains(&opening.parameters.host))
            .map(|opening| opening.id())
            .collect();
        let mut joins = BTreeSet::new();
        for join in model.wall_joins.values() {
            let members = join.parameters.members();
            if members.iter().any(|id| wall_ids.contains(id)) {
                ensure(
                    members.iter().all(|id| wall_ids.contains(id)),
                    "Group requires complete wall-join membership",
                )?;
                joins.insert(join.id());
            }
        }
        let tags: BTreeSet<_> = model
            .opening_tags
            .values()
            .filter(|tag| tag.parameters.view == view && openings.contains(&tag.parameters.opening))
            .map(|tag| tag.id())
            .collect();
        ensure(
            openings.len() + joins.len() + tags.len() <= MAX_GROUP_DEPENDENTS,
            "Group exceeds the dependent-element limit",
        )?;

        let mut source_to_slot = BTreeMap::new();
        for id in wall_ids.iter().chain(&openings).chain(&joins).chain(&tags) {
            ensure(
                source_to_slot.insert(*id, Id::new()).is_none(),
                "duplicate model-group source member",
            )?;
        }
        let mut template = GroupTemplate::default();
        for (&source, &slot) in &source_to_slot {
            if let Some(wall) = model.walls.get(&source) {
                let mut wall = wall.clone();
                remap_header(&mut wall.header, slot, &source_to_slot);
                template.walls.insert(slot, wall);
            } else if let Some(opening) = model.openings.get(&source) {
                let mut opening = opening.clone();
                remap_header(&mut opening.header, slot, &source_to_slot);
                opening.parameters.host = source_to_slot[&opening.parameters.host];
                template.openings.insert(slot, opening);
            } else if let Some(join) = model.wall_joins.get(&source) {
                let mut join = join.clone();
                remap_header(&mut join.header, slot, &source_to_slot);
                join.parameters = remap_join(&join.parameters, &source_to_slot)?;
                template.joins.insert(slot, join);
            } else if let Some(tag) = model.opening_tags.get(&source) {
                let mut tag = tag.clone();
                remap_header(&mut tag.header, slot, &source_to_slot);
                tag.parameters.opening = source_to_slot[&tag.parameters.opening];
                template.opening_tags.insert(slot, tag);
            }
        }
        for (&wall, assignment) in &model.wall_type_assignments {
            if wall_ids.contains(&wall) {
                template
                    .wall_type_assignments
                    .insert(source_to_slot[&wall], *assignment);
            }
        }
        for (&opening, clearance) in &model.opening_clearances {
            if openings.contains(&opening) {
                template
                    .opening_clearances
                    .insert(source_to_slot[&opening], *clearance);
            }
        }
        Ok((
            Self {
                name: name.into(),
                template,
            },
            source_to_slot
                .into_iter()
                .map(|(source, slot)| (slot, source))
                .collect(),
        ))
    }

    pub fn materialize(&self, instance: &GroupInstanceParams) -> Result<GroupMaterialization> {
        self.template.validate_shape()?;
        instance.transform.validate()?;
        ensure(
            instance.members.keys().copied().collect::<BTreeSet<_>>() == self.template.slot_ids(),
            "Group instance member slots do not match its definition",
        )?;
        let member_ids = instance.members.values().copied().collect::<BTreeSet<_>>();
        ensure(
            member_ids.len() == instance.members.len()
                && member_ids.iter().all(|id| !id.0.is_nil()),
            "Group instance member identities must be unique and non-nil",
        )?;
        let members = &instance.members;
        let mut materialized = GroupMaterialization::default();
        for (&slot, source) in &self.template.walls {
            let id = members[&slot];
            let mut wall = source.clone();
            remap_header(&mut wall.header, id, members);
            let WallPath::Straight { start, end } = wall.parameters.path else {
                return Err(Error::Invalid(
                    "Wall group templates require straight paths".into(),
                ));
            };
            wall.parameters.path = WallPath::Straight {
                start: instance.transform.apply(start)?,
                end: instance.transform.apply(end)?,
            };
            materialized.walls.insert(id, wall);
        }
        for (&slot, source) in &self.template.openings {
            let id = members[&slot];
            let mut opening = source.clone();
            remap_header(&mut opening.header, id, members);
            opening.parameters.host = members[&opening.parameters.host];
            materialized.openings.insert(id, opening);
        }
        for (&slot, source) in &self.template.joins {
            let id = members[&slot];
            let mut join = source.clone();
            remap_header(&mut join.header, id, members);
            join.parameters = remap_join(&join.parameters, members)?;
            materialized.joins.insert(id, join);
        }
        for (&slot, source) in &self.template.opening_tags {
            let id = members[&slot];
            let mut tag = source.clone();
            remap_header(&mut tag.header, id, members);
            tag.parameters.opening = members[&tag.parameters.opening];
            tag.parameters.position = instance.transform.apply(tag.parameters.position)?;
            materialized.opening_tags.insert(id, tag);
        }
        for (&slot, assignment) in &self.template.wall_type_assignments {
            materialized
                .wall_type_assignments
                .insert(members[&slot], *assignment);
        }
        for (&slot, clearance) in &self.template.opening_clearances {
            materialized
                .opening_clearances
                .insert(members[&slot], *clearance);
        }
        Ok(materialized)
    }
}

impl GroupTemplate {
    fn validate_shape(&self) -> Result<()> {
        ensure(
            !self.walls.is_empty() && self.walls.len() <= MAX_GROUP_WALLS,
            "Group definition requires 1–64 walls",
        )?;
        ensure(
            self.dependent_count() <= MAX_GROUP_DEPENDENTS,
            "Group definition exceeds the dependent-element limit",
        )?;
        let slots = self.slot_ids();
        ensure(
            slots.len()
                == self.walls.len()
                    + self.openings.len()
                    + self.joins.len()
                    + self.opening_tags.len(),
            "Group definition has duplicate member identities",
        )?;
        ensure(
            self.openings
                .values()
                .all(|opening| self.walls.contains_key(&opening.parameters.host)),
            "Group opening is missing its wall host",
        )?;
        ensure(
            self.joins.values().all(|join| {
                join.parameters
                    .members()
                    .iter()
                    .all(|member| self.walls.contains_key(member))
            }),
            "Group wall join is missing a member wall",
        )?;
        ensure(
            self.opening_tags
                .values()
                .all(|tag| self.openings.contains_key(&tag.parameters.opening)),
            "Group opening tag is missing its opening",
        )?;
        ensure(
            self.wall_type_assignments
                .keys()
                .all(|id| self.walls.contains_key(id)),
            "Group wall-type assignment has no wall slot",
        )?;
        ensure(
            self.opening_clearances
                .keys()
                .all(|id| self.openings.contains_key(id)),
            "Group opening clearance has no opening slot",
        )?;
        ensure(
            self.walls
                .values()
                .all(|wall| wall.parameters.path.is_straight()),
            "Group wall templates require straight paths",
        )?;
        Ok(())
    }
}

impl GroupDefinition {
    pub fn create(parameters: GroupDefinitionParams) -> Self {
        Entity::new("core.group_definition", parameters)
    }
}

impl GroupInstance {
    pub fn create(parameters: GroupInstanceParams) -> Self {
        Entity::new("core.group_instance", parameters)
    }

    pub fn from_members(definition: Id, members: BTreeMap<Id, Id>) -> Self {
        Self::create(GroupInstanceParams {
            definition,
            members,
            transform: GroupTransform::default(),
        })
    }
}

impl Model {
    pub fn group_instance_for_member(&self, member: Id) -> Option<&GroupInstance> {
        self.group_instances
            .values()
            .find(|instance| instance.parameters.members.values().any(|id| *id == member))
    }

    pub(crate) fn validate_groups(&self, ids: &mut BTreeSet<Id>) -> Result<()> {
        ensure(
            self.group_definitions.len() <= MAX_GROUP_DEFINITIONS
                && self.group_instances.len() <= MAX_GROUP_INSTANCES,
            "model-group collection limit exceeded",
        )?;
        let live_ids = ids.clone();
        let mut template_ids = BTreeSet::new();
        for (&id, definition) in &self.group_definitions {
            ensure(
                id == definition.id() && !definition.parameters.name.trim().is_empty(),
                "invalid model-group definition",
            )?;
            definition.parameters.template.validate_shape()?;
            for (slot, header, type_id) in definition.parameters.template.headers() {
                ensure(
                    *slot == header.id
                        && !slot.0.is_nil()
                        && header.schema_version == SCHEMA_VERSION
                        && header.type_id == type_id
                        && !live_ids.contains(slot)
                        && template_ids.insert(*slot),
                    "invalid or duplicate group template identity",
                )?;
            }
        }
        for definition in self.group_definitions.values() {
            let local_slots = definition.parameters.template.slot_ids();
            for (_, header, _) in definition.parameters.template.headers() {
                for targets in header.relationships.values() {
                    ensure(
                        targets.iter().all(|target| {
                            local_slots.contains(target) || live_ids.contains(target)
                        }),
                        "dangling group template relationship",
                    )?;
                }
            }
            let template = &definition.parameters.template;
            for wall in template.walls.values() {
                ensure(
                    self.levels.contains_key(&wall.parameters.level)
                        && wall
                            .parameters
                            .material
                            .is_none_or(|material| self.materials.contains_key(&material)),
                    "group wall has a missing level or material",
                )?;
            }
            let levels = template
                .walls
                .values()
                .map(|wall| wall.parameters.level)
                .collect::<BTreeSet<_>>();
            ensure(levels.len() == 1, "group walls must share one level")?;
            for opening in template.openings.values() {
                if let OpeningDefinition::Typed { type_id } = &opening.parameters.definition {
                    ensure(
                        self.opening_types.contains_key(type_id),
                        "group opening type is missing",
                    )?;
                }
            }
            for assignment in template.wall_type_assignments.values() {
                ensure(
                    self.wall_types.contains_key(&assignment.type_id),
                    "group wall type is missing",
                )?;
            }
            let mut tag_views = template
                .opening_tags
                .values()
                .map(|tag| tag.parameters.view);
            if let Some(view_id) = tag_views.next() {
                ensure(
                    tag_views.all(|other| other == view_id)
                        && self.views.get(&view_id).is_some_and(|view| {
                            view.parameters.kind == ViewKind::Plan
                                && view.parameters.level == levels.first().copied()
                        }),
                    "group opening tags must use one plan on the group level",
                )?;
            }
        }

        let mut instances_per_definition = BTreeMap::<Id, usize>::new();
        let mut owned_members = BTreeSet::new();
        for (&id, instance) in &self.group_instances {
            ensure(
                id == instance.id()
                    && self
                        .group_definitions
                        .contains_key(&instance.parameters.definition),
                "group instance definition is missing",
            )?;
            *instances_per_definition
                .entry(instance.parameters.definition)
                .or_default() += 1;
            let definition = &self.group_definitions[&instance.parameters.definition];
            let expected = definition.parameters.materialize(&instance.parameters)?;
            let mut instance_members = BTreeSet::new();
            for (&slot, &member) in &instance.parameters.members {
                ensure(
                    owned_members.insert(member),
                    "a model element belongs to more than one group instance",
                )?;
                instance_members.insert(member);
                ensure(
                    definition.parameters.template.slot_ids().contains(&slot),
                    "group instance contains an unknown member slot",
                )?;
            }
            for (member, entity) in &expected.walls {
                ensure(
                    self.walls.get(member) == Some(entity),
                    "group wall instance diverges from its shared definition",
                )?;
            }
            for (member, entity) in &expected.openings {
                ensure(
                    self.openings.get(member) == Some(entity),
                    "group opening instance diverges from its shared definition",
                )?;
            }
            for (member, entity) in &expected.joins {
                ensure(
                    self.wall_joins.get(member) == Some(entity),
                    "group join instance diverges from its shared definition",
                )?;
            }
            for (member, entity) in &expected.opening_tags {
                ensure(
                    self.opening_tags.get(member) == Some(entity),
                    "group tag instance diverges from its shared definition",
                )?;
            }
            for (member, assignment) in &expected.wall_type_assignments {
                ensure(
                    self.wall_type_assignments.get(member) == Some(assignment),
                    "group wall type diverges from its shared definition",
                )?;
            }
            for (member, clearance) in &expected.opening_clearances {
                ensure(
                    self.opening_clearances.get(member) == Some(clearance),
                    "group clearance diverges from its shared definition",
                )?;
            }
            for member in &instance_members {
                ensure(
                    expected.wall_type_assignments.get(member)
                        == self.wall_type_assignments.get(member)
                        && expected.opening_clearances.get(member)
                            == self.opening_clearances.get(member),
                    "group member metadata diverges from its shared definition",
                )?;
            }
            ensure(
                expected.walls.keys().all(|id| self.walls.contains_key(id))
                    && expected
                        .openings
                        .keys()
                        .all(|id| self.openings.contains_key(id))
                    && expected
                        .joins
                        .keys()
                        .all(|id| self.wall_joins.contains_key(id))
                    && expected
                        .opening_tags
                        .keys()
                        .all(|id| self.opening_tags.contains_key(id)),
                "group instance member is missing",
            )?;
        }
        ensure(
            self.group_definitions
                .keys()
                .all(|id| instances_per_definition.get(id).copied().unwrap_or(0) > 0),
            "orphan model-group definition",
        )
    }
}

fn remap_header(header: &mut Header, id: Id, remap: &BTreeMap<Id, Id>) {
    header.id = id;
    for target in header.relationships.values_mut().flatten() {
        if let Some(mapped) = remap.get(target) {
            *target = *mapped;
        }
    }
}

fn remap_join(parameters: &WallJoinParams, remap: &BTreeMap<Id, Id>) -> Result<WallJoinParams> {
    let wall = |id: Id| {
        remap
            .get(&id)
            .copied()
            .ok_or_else(|| Error::Invalid("group join member is missing".into()))
    };
    let anchor = |anchor: WallAnchor| {
        Ok(WallAnchor {
            wall: wall(anchor.wall)?,
            endpoint: anchor.endpoint,
        })
    };
    Ok(match *parameters {
        WallJoinParams::Butt { a, b } => WallJoinParams::Butt {
            a: anchor(a)?,
            b: anchor(b)?,
        },
        WallJoinParams::Corner { a, b, owner } => WallJoinParams::Corner {
            a: anchor(a)?,
            b: anchor(b)?,
            owner: wall(owner)?,
        },
        WallJoinParams::Tee {
            host,
            station,
            branch,
        } => WallJoinParams::Tee {
            host: wall(host)?,
            station,
            branch: anchor(branch)?,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn add_wall(model: &mut Model, name: &str, x: f64) -> Id {
        let level = *model.levels.keys().next().unwrap();
        let wall = Wall::new(
            "org.openstructure.walls.wall",
            WallParams {
                name: name.into(),
                path: WallPath::Straight {
                    start: Point2::new(x, 0.),
                    end: Point2::new(x + 10., 0.),
                },
                thickness: 0.2,
                height: 3.,
                level,
                material: None,
            },
        );
        let id = wall.id();
        model.walls.insert(id, wall);
        id
    }

    fn add_plan_view(model: &mut Model) -> Id {
        let level = *model.levels.keys().next().unwrap();
        let view = View::new(
            "core.view",
            ViewParams {
                drafting: None,
                name: "Plan".into(),
                kind: ViewKind::Plan,
                level: Some(level),
                settings_revision: 0,
                plan: Some(PlanSettings::default()),
                section: None,
                elevation: None,
                facade: None,
            },
        );
        let id = view.id();
        model.views.insert(id, view);
        id
    }

    fn add_group(model: &mut Model, walls: BTreeSet<Id>, view: Id, name: &str) -> Id {
        let (parameters, members) =
            GroupDefinitionParams::capture_wall_assembly(model, &walls, view, name).unwrap();
        let definition = GroupDefinition::create(parameters);
        let definition_id = definition.id();
        model.group_definitions.insert(definition_id, definition);
        let instance = GroupInstance::from_members(definition_id, members);
        let instance_id = instance.id();
        model.group_instances.insert(instance_id, instance);
        instance_id
    }

    fn add_door(model: &mut Model, host: Id) -> Id {
        let opening = Opening::new(
            "core.opening",
            OpeningParams {
                open_state: OpeningState::default(),
                name: "Door".into(),
                host,
                offset: 2.,
                definition: OpeningDefinition::Legacy {
                    kind: OpeningKind::Door,
                    width: 0.9,
                    height: 2.1,
                    sill: 0.,
                },
                width_override: None,
                height_override: None,
                sill_override: None,
                pane_position_override: None,
                lite_side_override: None,
                hinge: DoorHinge::Start,
                swing: DoorSwing::Left,
            },
        );
        let id = opening.id();
        model.openings.insert(id, opening);
        id
    }

    #[test]
    fn group_capture_validates_and_instance_mapping_cannot_collapse_slots() {
        let mut model = Model::new("Groups");
        let first = add_wall(&mut model, "First", 0.);
        let second = add_wall(&mut model, "Second", 0.);
        let view = add_plan_view(&mut model);
        let (definition, _) = GroupDefinitionParams::capture_wall_assembly(
            &model,
            &BTreeSet::from([first, second]),
            view,
            "Pair",
        )
        .unwrap();
        let slots = definition.template.slot_ids();
        let shared_id = Id::new();
        let collapsed = GroupInstanceParams {
            definition: Id::new(),
            members: slots.into_iter().map(|slot| (slot, shared_id)).collect(),
            transform: GroupTransform::default(),
        };
        assert!(definition.materialize(&collapsed).is_err());

        let instance = add_group(&mut model, BTreeSet::from([first, second]), view, "Pair");
        assert!(model.group_instances.contains_key(&instance));
        model.validate().unwrap();
        let encoded = serde_json::to_vec(&model).unwrap();
        let reopened: Model = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(reopened, model);
        reopened.validate().unwrap();
    }

    #[test]
    fn group_capture_requires_a_floor_plan_not_a_reflected_ceiling_plan() {
        let mut model = Model::new("Group plan kind");
        let wall = add_wall(&mut model, "Wall", 0.0);
        let view = add_plan_view(&mut model);
        model
            .views
            .get_mut(&view)
            .unwrap()
            .parameters
            .plan
            .as_mut()
            .unwrap()
            .view_type = PlanViewType::ReflectedCeilingPlan;
        assert!(
            GroupDefinitionParams::capture_wall_assembly(
                &model,
                &BTreeSet::from([wall]),
                view,
                "Wall group",
            )
            .is_err()
        );
    }

    #[test]
    fn template_relationships_cannot_reference_another_definition_slot() {
        let mut model = Model::new("Group identity");
        let first = add_wall(&mut model, "First", 0.);
        let second = add_wall(&mut model, "Second", 20.);
        let view = add_plan_view(&mut model);
        let first_group = add_group(&mut model, BTreeSet::from([first]), view, "First group");
        let second_group = add_group(&mut model, BTreeSet::from([second]), view, "Second group");
        let first_definition = model.group_instances[&first_group].parameters.definition;
        let second_definition = model.group_instances[&second_group].parameters.definition;
        let foreign_slot = *model.group_definitions[&second_definition]
            .parameters
            .template
            .walls
            .keys()
            .next()
            .unwrap();
        let first_wall = model
            .group_definitions
            .get_mut(&first_definition)
            .unwrap()
            .parameters
            .template
            .walls
            .values_mut()
            .next()
            .unwrap();
        first_wall
            .header
            .relationships
            .insert("foreign-template-slot".into(), vec![foreign_slot]);
        assert!(model.validate().is_err());
    }

    #[test]
    fn grouped_hosted_opening_rejects_metadata_not_in_the_shared_definition() {
        let mut model = Model::new("Group opening metadata");
        let wall = add_wall(&mut model, "Host", 0.);
        let door = add_door(&mut model, wall);
        let view = add_plan_view(&mut model);
        add_group(&mut model, BTreeSet::from([wall]), view, "Host and door");
        model.validate().unwrap();

        model.opening_clearances.insert(
            door,
            OpeningClearance {
                end: ClearanceEnd::Start,
                distance: 2.,
            },
        );
        assert!(model.validate().is_err());
    }
}
