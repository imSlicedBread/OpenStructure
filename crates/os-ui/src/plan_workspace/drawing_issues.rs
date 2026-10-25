//! Frozen drawing-set workflow. Pending bytes belong to this session, never to history.
use super::*;
use os_model::{
    DrawingIssue, IssueManifest, IssueSheet, IssueState, IssueWarning, SheetSet, sha256,
};
use os_storage::drawing_issue::IssueBundle;

#[derive(Default)]
pub(super) struct State {
    open: bool,
    set: Option<Id>,
    set_name: Option<(Id, String)>,
    issue: Option<Id>,
    revision: String,
    description: String,
    date: String,
    recipients: String,
    path: String,
    pending: Option<Pending>,
    archived: Option<IssueBundle>,
}
#[derive(Debug)]
struct Pending {
    manifest: IssueManifest,
    pdf: Vec<u8>,
    review_revision: Option<u64>,
}

impl DesktopApp {
    fn preflight_drawing_issue(&self, id: Id) -> Result<Pending> {
        let doc = &self.editor.document;
        let model = doc.model();
        model.validate()?;
        os_core::ensure(
            !self
                .plans
                .sheet_draft
                .as_ref()
                .is_some_and(sheet_layout::Draft::dirty),
            "Apply or reset the paper layout before issue preflight",
        )?;
        let issue = model
            .drawing_issues
            .get(&id)
            .ok_or_else(|| Error::Invalid("select a draft issue".into()))?;
        os_core::ensure(issue.state == IssueState::Draft, "preflight requires Draft")?;
        model.validate_callout_issue_references(&issue.sheets)?;
        let mut warnings = vec!["PDF uses standard Helvetica/WinAnsi without embedded fonts; unsupported characters block export.".into(), "This package contains the currently supported A3 drawing geometry; it does not establish complete construction-document or Revit parity.".into()];
        let mut pages = Vec::new();
        let mut sheets = Vec::new();
        for id in &issue.sheets {
            let p = &model.sheets[id].parameters;
            pages.push(
                self.sheet_page_checked(p, true, &mut warnings)
                    .map_err(|e| Error::Invalid(format!("Sheet {}: {e}", p.number)))?,
            );
            sheets.push(IssueSheet {
                id: *id,
                number: p.number.clone(),
                name: p.name.clone(),
                views: p
                    .viewports
                    .iter()
                    .map(|v| (v.view, model.views[&v.view].parameters.settings_revision))
                    .collect(),
            });
        }
        warnings.sort();
        warnings.dedup();
        let pdf = SheetPage::ordered_pdf(&pages)?;
        let mut provider_versions = vec![(
            "os-render/native".into(),
            env!("CARGO_PKG_VERSION").into(),
            os_plugin_api::API_VERSION,
        )];
        provider_versions.extend(
            self.editor
                .host
                .manifests()
                .map(|m| (m.id.clone(), m.version.clone(), m.api_version)),
        );
        let source = serde_json::to_vec(model).map_err(|e| Error::Invalid(e.to_string()))?;
        Ok(Pending {
            manifest: IssueManifest {
                format_version: os_model::ISSUE_BUNDLE_VERSION,
                state: IssueState::Issued,
                issue: issue.id,
                project: model.project.id(),
                set: issue.set,
                sequence: issue.sequence,
                revision: issue.revision.clone(),
                description: issue.description.clone(),
                date: issue.date.clone(),
                recipients: issue.recipients.clone(),
                supersedes: issue.supersedes,
                schema_version: model.schema_version,
                native_api_version: os_plugin_api::API_VERSION,
                application_version: env!("CARGO_PKG_VERSION").into(),
                source_session: doc.session_id(),
                source_revision: doc.revision(),
                source_sha256: sha256(&source),
                providers: plan_provider_signature(&self.editor),
                provider_versions,
                sheets,
                warnings: warnings
                    .into_iter()
                    .map(|message| IssueWarning {
                        message,
                        acknowledgment: String::new(),
                    })
                    .collect(),
                pdf_sha256: sha256(&pdf),
            },
            pdf,
            review_revision: None,
        })
    }

    fn review_issue_pending(&mut self, pending: &mut Pending) -> Result<()> {
        os_core::ensure(
            pending.manifest.providers == plan_provider_signature(&self.editor),
            "providers changed; run preflight again",
        )?;
        // Validate the complete portable artifact before the document transition.
        IssueBundle::new(pending.manifest.clone(), pending.pdf.clone())?.bytes()?;
        self.editor
            .document
            .review_drawing_issue(pending.manifest.issue, pending.manifest.clone())?;
        pending.review_revision = Some(self.editor.document.revision());
        Ok(())
    }

    fn publish_issue_pending(&mut self, pending: &Pending, path: &str) -> Result<()> {
        os_core::ensure(
            pending.manifest.providers == plan_provider_signature(&self.editor),
            "providers changed; create a new draft/preflight",
        )?;
        let path = std::path::absolute(path.trim()).map_err(|e| Error::Storage(e.to_string()))?;
        os_core::ensure(
            path.extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("osissue")),
            "choose a new .osissue bundle path",
        )?;
        let bundle = IssueBundle::new(pending.manifest.clone(), pending.pdf.clone())?;
        let revision = pending
            .review_revision
            .ok_or_else(|| Error::Invalid("review all warnings first".into()))?;
        self.editor.document.publish_drawing_issue(
            pending.manifest.issue,
            pending.manifest.source_session,
            revision,
            path.to_string_lossy().into_owned(),
            |issue| bundle.publish(issue, &path),
        )
    }

    pub(super) fn drawing_issues_ui(&mut self, ui: &mut egui::Ui) {
        if ui.button("Drawing sets and issues").clicked() {
            self.plans.issues.open = true;
        }
        let mut state = std::mem::take(&mut self.plans.issues);
        if !state.open {
            self.plans.issues = state;
            return;
        }
        let mut open = true;
        egui::Window::new("Drawing sets and issues").open(&mut open).default_width(650.0).vscroll(true).show(ui.ctx(), |ui| {
            ui.label("Ordered A3 package · Draft → Review → Issued");
            ui.label("Issuing freezes the record and clears undo/redo history. Save the native project after issuing. Corrections create a superseding issue.");
            let sets: Vec<_> = self.editor.document.model().sheet_sets.values().cloned().collect();
            let old_set = state.set;
            egui::ComboBox::from_id_salt("issue_set").selected_text(state.set.and_then(|id| sets.iter().find(|s| s.id == id)).map_or("Select set",|s|s.name.as_str())).show_ui(ui, |ui| {
                for set in &sets { ui.selectable_value(&mut state.set,Some(set.id),&set.name); }
            });
            if state.set != old_set { state.issue=None; state.pending=None; state.archived=None; state.set_name=None; }
            if ui.button("New set from current sheet").clicked() && let Some(sheet) = self.plans.active_sheet {
                let id = Id::new();
                let result = self.editor.command("Create drawing set",Command::PutSheetSet(SheetSet { id,name: format!("Drawing set {}",sets.len()+1), sheets: vec![sheet] }));
                if result.is_ok() { state.set = Some(id); state.issue=None; state.pending=None; state.archived=None; state.set_name=None; }
                self.report(result,"Drawing set created.");
            }
            if let Some(mut set) = state.set.and_then(|id| self.editor.document.model().sheet_sets.get(&id)).cloned() {
                if state.set_name.as_ref().is_none_or(|(id,_)| *id != set.id) { state.set_name=Some((set.id,set.name.clone())); }
                let name=&mut state.set_name.as_mut().expect("set name initialized").1;
                ui.text_edit_singleline(name);
                let mut changed=false;
                if ui.button("Apply set name").clicked() { set.name=name.clone(); changed=true; }
                let mut swap = None; let mut remove = None;
                for (index,id) in set.sheets.iter().enumerate() {
                    let s = &self.editor.document.model().sheets[id].parameters;
                    ui.push_id(id, |ui| { ui.horizontal(|ui| {
                        ui.label(format!("{}. {} — {}",index+1,s.number,s.name));
                        if ui.add_enabled(index>0,egui::Button::new("↑")).clicked() { swap=Some((index,index-1)); }
                        if ui.add_enabled(index+1<set.sheets.len(),egui::Button::new("↓")).clicked() { swap=Some((index,index+1)); }
                        if ui.add_enabled(set.sheets.len()>1,egui::Button::new("Remove")).clicked() { remove=Some(index); }
                    }); });
                }
                if let Some((a,b))=swap { set.sheets.swap(a,b); changed=true; }
                if let Some(i)=remove { set.sheets.remove(i); changed=true; }
                egui::ComboBox::from_id_salt("issue_add_sheet").selected_text("Add sheet to set").show_ui(ui, |ui| {
                    for s in self.editor.document.model().sheets.values() {
                        if !set.sheets.contains(&s.id()) && ui.button(format!("{} — {}",s.parameters.number,s.parameters.name)).clicked() { set.sheets.push(s.id()); changed=true; }
                    }
                });
                if changed && self.editor.document.model().sheet_sets.get(&set.id) != Some(&set) {
                    let result = self.editor.command("Edit ordered drawing set",Command::PutSheetSet(set.clone())); self.report(result,"Drawing set updated.");
                }
                ui.separator();
                ui.label("New issue metadata (date YYYY-MM-DD; recipients separated by semicolons)");
                for (label,value) in [("Revision",&mut state.revision),("Description",&mut state.description),("Date",&mut state.date),("Recipients",&mut state.recipients)] {
                    ui.horizontal(|ui| { ui.label(label); ui.text_edit_singleline(value); });
                }
                let issues: Vec<_> = self.editor.document.model().drawing_issues.values().filter(|i|i.set==set.id).cloned().collect();
                let previous = issues.iter().filter(|i|i.state==IssueState::Issued).max_by_key(|i|i.sequence).map(|i|i.id);
                if ui.button(if previous.is_some() { "Create superseding draft" } else { "Create draft issue" }).clicked() {
                    let Some(sequence) = issues.iter().map(|i|i.sequence).max().unwrap_or(0).checked_add(1) else {
                        self.report(Err(Error::Invalid("issue sequence exhausted".into())), ""); return;
                    };
                    let id=Id::new();
                    let issue=DrawingIssue { id,set:set.id,sequence,revision:state.revision.clone(),description:state.description.clone(),date:state.date.clone(),recipients:state.recipients.split(';').map(str::trim).filter(|s|!s.is_empty()).map(str::to_owned).collect(),sheets:set.sheets.clone(),supersedes:previous,state:IssueState::Draft,manifest:None,manifest_sha256:None,archive_path:None };
                    let result=self.editor.command("Create drawing issue",Command::PutDrawingIssue(issue));
                    if result.is_ok() { state.issue=Some(id); state.pending=None; }
                    self.report(result,"Draft created. Wait for all set graphics, then preflight.");
                }
                egui::ComboBox::from_id_salt("drawing_issue").selected_text(state.issue.and_then(|id|issues.iter().find(|i|i.id==id)).map_or("Select issue".into(),|i|format!("{} / {} / {:?}",i.sequence,i.revision,i.state))).show_ui(ui, |ui| {
                    for issue in &issues { if ui.selectable_value(&mut state.issue,Some(issue.id),format!("{} / {} / {:?}",issue.sequence,issue.revision,issue.state)).changed() { state.pending=None; state.archived=None; } }
                });
                if let Some(issue)=state.issue.and_then(|id|issues.iter().find(|i|i.id==id)) {
                    if issue.state==IssueState::Draft && ui.button("Preflight all sheets").clicked() {
                        match self.preflight_drawing_issue(issue.id) { Ok(p)=>{state.pending=Some(p);self.report(Ok(()),"Preflight passed. Review every warning below.");},Err(e)=>self.report(Err(e),"") }
                    }
                    if issue.state==IssueState::Review && state.pending.is_none() { ui.label("Frozen review bytes are no longer in this session. Create a fresh draft to preflight again."); }
                    if issue.state==IssueState::Issued && ui.button("Read archived bundle").clicked() {
                        let result=IssueBundle::read(Path::new(issue.archive_path.as_deref().unwrap_or(""))).and_then(|b| { b.verify_record(issue)?; Ok(b) });
                        match result { Ok(b)=>{state.archived=Some(b);self.report(Ok(()),"Archived hashes verified; no live regeneration.");},Err(e)=>self.report(Err(e),"") }
                    }
                }
            }
            if let Some(pending)=&mut state.pending {
                let current = pending.manifest.source_session==self.editor.document.session_id() && pending.review_revision.unwrap_or(pending.manifest.source_revision)==self.editor.document.revision() && pending.manifest.providers==plan_provider_signature(&self.editor);
                if !current { ui.colored_label(theme::ERROR,"Document/providers changed. Create or preflight a current draft."); }
                ui.label(format!("Frozen pages: {} · PDF SHA256 {}",pending.manifest.sheets.len(),pending.manifest.pdf_sha256));
                for (index,w) in pending.manifest.warnings.iter_mut().enumerate() {
                    ui.push_id(index, |ui| { ui.label(&w.message); ui.add_enabled(pending.review_revision.is_none(),egui::TextEdit::singleline(&mut w.acknowledgment).hint_text("Required: your review decision")); });
                }
                if ui.add_enabled(current && pending.review_revision.is_none(),egui::Button::new("Accept review")).clicked() { let result=self.review_issue_pending(pending);self.report(result,"Review frozen. Publish before making further edits."); }
                ui.horizontal(|ui| {ui.label("New bundle path (.osissue)");ui.text_edit_singleline(&mut state.path);});
                if ui.add_enabled(current && pending.review_revision.is_some(),egui::Button::new("Issue frozen package")).clicked() { let result=self.publish_issue_pending(pending,&state.path);self.report(result,"Issued immutable package. Undo/redo cleared; save the native project."); }
            }
            if let Some(bundle)=&state.archived {
                ui.label(format!("Archived {} ordered pages; {} PDF bytes; manifest {}",bundle.manifest().sheets.len(),bundle.pdf().len(),bundle.manifest_sha256()));
                for (i,s) in bundle.manifest().sheets.iter().enumerate() { ui.label(format!("{}. {} — {}",i+1,s.number,s.name)); }
                ui.label("Portable bundle contains drawings.pdf, manifest.json and integrity.sha256. Extract drawings.pdf in your ZIP/PDF viewer to inspect archived pages.");
            }
        });
        state.open = open;
        self.plans.issues = state;
    }
}

#[cfg(test)]
mod tests;
