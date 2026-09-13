//! Prepared, consent-driven IFC exchange. No native-file association is reused.
use super::*;
use std::{
    fs::File,
    io::{Read, Write},
};

pub(super) struct PreparedImport {
    document: Document,
    scene: Scene,
    warnings: Vec<String>,
}
pub(super) enum PendingExchange {
    Import {
        prepared: Box<PreparedImport>,
        path: PathBuf,
    },
    Export {
        bytes: Vec<u8>,
        warnings: Vec<String>,
        path: PathBuf,
        replace: bool,
    },
}
fn io_error(e: impl std::fmt::Display) -> Error {
    Error::Storage(e.to_string())
}
fn ifc_path(text: &str) -> Result<PathBuf> {
    let path = PathBuf::from(text.trim());
    os_core::ensure(
        path.extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("ifc")),
        "Choose a file path ending in .ifc.",
    )?;
    Ok(path)
}
impl Editor {
    pub(super) fn prepare_ifc(&self, path: &Path) -> Result<PreparedImport> {
        let mut bytes = Vec::new();
        File::open(path)
            .map_err(io_error)?
            .take(16 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(io_error)?;
        let report = os_ifc::WallIfc.import_report(&bytes)?;
        let document = Document::from_model(report.value)?;
        let mut scene = Scene::new();
        for id in document.model().walls.keys() {
            let Response::Solid(solid) = self.host.request(
                os_walls::PLUGIN_ID,
                document.model(),
                Request::GenerateWall { id: *id },
            )?
            else {
                return Err(Error::Invalid("expected wall solid".into()));
            };
            scene.insert(*id, PrismKernel.tessellate(&solid)?);
        }
        Ok(PreparedImport {
            document,
            scene,
            warnings: report.warnings,
        })
    }
    fn accept_ifc(&mut self, prepared: PreparedImport) {
        self.document = prepared.document;
        self.scene = prepared.scene;
        self.pending_geometry.clear();
        self.saved_model = None;
    }
}
fn write_exchange(path: &Path, bytes: &[u8], replace: bool) -> Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut file = tempfile::NamedTempFile::new_in(parent).map_err(io_error)?;
    file.write_all(bytes).map_err(io_error)?;
    file.as_file_mut().sync_all().map_err(io_error)?;
    if replace {
        file.persist(path).map_err(io_error)?;
    } else {
        file.persist_noclobber(path).map_err(io_error)?;
    }
    Ok(())
}
impl DesktopApp {
    pub(super) fn prepare_ifc_export(&mut self) {
        let result = (|| {
            let path = ifc_path(&self.ifc_path)?;
            // An invalid regeneration must not be exported as a valid exchange copy.
            self.editor.regenerate()?;
            let report = os_ifc::WallIfc.export_report_with_files(
                self.editor.document.model(),
                self.editor
                    .document
                    .auxiliary_files()
                    .keys()
                    .map(String::as_str),
            )?;
            let replace = path.exists();
            Ok(PendingExchange::Export {
                bytes: report.value,
                warnings: report.warnings,
                path,
                replace,
            })
        })();
        match result {
            Ok(pending) => self.exchange_pending = Some(pending),
            Err(e) => self.report(Err(e), ""),
        }
    }
    pub(super) fn prepare_ifc_import(&mut self) {
        let result = (|| {
            let path = ifc_path(&self.ifc_path)?;
            let prepared = Box::new(self.editor.prepare_ifc(&path)?);
            Ok(PendingExchange::Import { prepared, path })
        })();
        match result {
            Ok(pending) => self.exchange_pending = Some(pending),
            Err(e) => self.report(Err(e), ""),
        }
    }
    pub(super) fn exchange_confirmation(&mut self, ctx: &egui::Context) {
        let Some(pending) = self.exchange_pending.take() else {
            return;
        };
        let mut accept = false;
        let mut cancel = false;
        egui::Modal::new(egui::Id::new("confirm_ifc_exchange")).show(ctx, |ui| {
            ui.set_width(460.0);
            let (path, warnings, import, replace) = match &pending {
                PendingExchange::Import {prepared, path} => (path, &prepared.warnings, true, false),
                PendingExchange::Export {warnings, path, replace, ..} => (path, warnings, false, *replace),
            };
            ui.heading(if import {"Import IFC into a new document?"} else {"Export IFC exchange copy?"});
            egui::ScrollArea::vertical().max_height(230.0).show(ui, |ui| {
                ui.label(path.display().to_string());
                ui.label("Restricted IFC4 rectangular walls only. This is not a complete native backup.");
                for warning in warnings {ui.label(warning);}
                if import {
                    ui.label(if self.editor.is_dirty() {
                        "Your current project has unsaved changes. Cancel and save it first, or discard it to import."
                    } else {"This replaces the current document. Save the imported model to a new .osb path."});
                    ui.label("Unapplied property drafts will also be discarded. Import cannot be undone.");
                } else {
                    ui.label("Only committed model changes are exported; unapplied property drafts are excluded.");
                    if replace {ui.colored_label(theme::ERROR, "The existing IFC file will be replaced.");}
                    ui.label("Export does not save your native project or clear unsaved changes.");
                }
            });
            ui.horizontal(|ui| {
                cancel = ui.button("Cancel").clicked();
                accept = ui.button(if import {
                    if self.editor.is_dirty() {"Discard and import"} else {"Import document"}
                } else if replace {"Accept losses and replace IFC"} else {"Accept losses and export"}).clicked();
            });
        });
        if accept {
            match pending {
                PendingExchange::Import { prepared, path } => {
                    let warnings = prepared.warnings.join("\n");
                    self.editor.accept_ifc(*prepared);
                    self.path.clear();
                    self.opened_path = None;
                    self.selected = None;
                    self.camera = Camera::default();
                    self.refresh_document();
                    self.fit_requested = true;
                    self.status_error = false;
                    self.status = format!(
                        "Imported {}. Choose a new .osb path to save.\n{warnings}",
                        path.display()
                    );
                }
                PendingExchange::Export {
                    bytes,
                    warnings,
                    path,
                    replace,
                } => {
                    // A race-created destination needs fresh, explicit replace consent.
                    if !replace && path.exists() {
                        self.exchange_pending = Some(PendingExchange::Export {
                            bytes,
                            warnings,
                            path,
                            replace: true,
                        });
                    } else {
                        let result = write_exchange(&path, &bytes, replace);
                        self.report(
                            result,
                            &format!(
                                "Exported {}. Native project save state unchanged.\n{}",
                                path.display(),
                                warnings.join("\n")
                            ),
                        );
                    }
                }
            }
        } else if !cancel {
            self.exchange_pending = Some(pending);
        }
    }
}
