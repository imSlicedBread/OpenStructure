use super::*;
use os_model::{WallAnchor, WallEndpoint, WallJoin, WallJoinParams};

impl Editor {
    pub fn join_wall_ends(&mut self, a: WallAnchor, b: WallAnchor) -> Result<Id> {
        self.join_walls(WallJoinParams::Butt { a, b })
    }
    pub fn join_walls(&mut self, parameters: WallJoinParams) -> Result<Id> {
        let join = WallJoin::new("core.wall_join", parameters);
        let id = join.id();
        self.command("Join walls", Command::AddWallJoin(join))?;
        Ok(id)
    }
    pub fn unjoin_wall_ends(&mut self, id: Id) -> Result<()> {
        self.command("Unjoin walls", Command::RemoveWallJoin(id))
    }
}

impl DesktopApp {
    pub(super) fn wall_join_controls(&mut self, ui: &mut egui::Ui) {
        let Some(id) = self
            .selected
            .filter(|id| self.editor.document.model().walls.contains_key(id))
        else {
            return;
        };
        egui::CollapsingHeader::new("Wall end joins").show(ui, |ui| {
            ui.label("Equal-profile butt, corner and T-junction joins");
            let host_joins: Vec<_> = self
                .editor
                .document
                .model()
                .wall_joins
                .values()
                .filter(|j| matches!(j.parameters, WallJoinParams::Tee { host, .. } if host == id))
                .map(|j| j.id())
                .collect();
            for join in host_joins {
                ui.push_id(join, |ui| {
                    if ui.button("Unjoin branch from host").clicked() {
                        self.cancel_plan_wall();
                        let result = self.editor.unjoin_wall_ends(join);
                        self.report(result, "Walls unjoined.");
                    }
                });
            }
            for endpoint in [WallEndpoint::Start, WallEndpoint::End] {
                let anchor = WallAnchor { wall: id, endpoint };
                let model = self.editor.document.model();
                let joined = model
                    .wall_joins
                    .values()
                    .find(|j| j.parameters.anchors().contains(&anchor))
                    .map(|j| j.id());
                if let Some(join) = joined {
                    if ui.button(format!("Unjoin {endpoint:?}")).clicked() {
                        self.cancel_plan_wall();
                        let result = self.editor.unjoin_wall_ends(join);
                        self.report(result, "Wall ends unjoined.");
                    }
                } else {
                    let candidates: Vec<_> = model
                        .walls
                        .values()
                        .filter(|w| w.id() != id)
                        .flat_map(|w| {
                            [WallEndpoint::Start, WallEndpoint::End].map(|e| {
                                (
                                    WallAnchor {
                                        wall: w.id(),
                                        endpoint: e,
                                    },
                                    w.parameters.name.clone(),
                                )
                            })
                        })
                        .filter_map(|(b, name)| {
                            let butt = WallJoinParams::Butt { a: anchor, b };
                            let corner = WallJoinParams::Corner {
                                a: anchor,
                                b,
                                owner: id.min(b.wall),
                            };
                            if butt.validate(model).is_ok() {
                                Some((
                                    butt,
                                    format!("Join {endpoint:?} to {name} {:?}", b.endpoint),
                                ))
                            } else if corner.validate(model).is_ok() {
                                Some((
                                    corner,
                                    format!("Corner {endpoint:?} to {name} {:?}", b.endpoint),
                                ))
                            } else {
                                None
                            }
                        })
                        .collect();
                    let node = endpoint.point(&model.walls[&id].parameters);
                    let tees: Vec<_> = model
                        .walls
                        .values()
                        .filter(|w| w.id() != id)
                        .filter_map(|w| {
                            let d = os_model::axis(&w.parameters);
                            let station = (node.x - w.parameters.start.x) * d.x
                                + (node.y - w.parameters.start.y) * d.y;
                            let p = WallJoinParams::Tee {
                                host: w.id(),
                                station,
                                branch: anchor,
                            };
                            p.validate(model).is_ok().then(|| {
                                (p, format!("T-join {endpoint:?} to {}", w.parameters.name))
                            })
                        })
                        .collect();
                    if candidates.is_empty() && tees.is_empty() {
                        ui.label(format!("{endpoint:?}: no compatible touching end"));
                    }
                    for (parameters, label) in candidates.into_iter().chain(tees) {
                        if ui.button(label).clicked() {
                            self.cancel_plan_wall();
                            let result = self.editor.join_walls(parameters).map(|_| ());
                            self.report(
                                result,
                                "Wall ends joined. Connected edits must preserve the junction.",
                            );
                        }
                    }
                }
            }
        });
    }
}
