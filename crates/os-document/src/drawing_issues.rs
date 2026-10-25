use super::*;

impl Document {
    /// Freeze reviewed metadata. Artifact bytes remain in the caller's bounded pending package.
    pub fn review_drawing_issue(&mut self, id: Id, manifest: IssueManifest) -> Result<()> {
        ensure(
            manifest.source_session == self.session_id && manifest.source_revision == self.revision,
            "issue preflight is stale",
        )?;
        let source = serde_json::to_vec(&self.model).map_err(|e| Error::Invalid(e.to_string()))?;
        self.model.validate_callout_issue_references(
            &manifest.sheets.iter().map(|s| s.id).collect::<Vec<_>>(),
        )?;
        ensure(
            manifest.source_sha256 == sha256(&source),
            "issue source hash changed",
        )?;
        let mut issue = self
            .model
            .drawing_issues
            .get(&id)
            .cloned()
            .ok_or_else(|| Error::Invalid("issue missing".into()))?;
        ensure(
            issue.state == IssueState::Draft,
            "only Draft can enter Review",
        )?;
        ensure(
            manifest.issue == id
                && manifest.sheets.iter().all(|s| {
                    self.model.sheets.get(&s.id).is_some_and(|live| {
                        live.parameters.number == s.number
                            && live.parameters.name == s.name
                            && live
                                .parameters
                                .viewports
                                .iter()
                                .map(|v| {
                                    (
                                        v.view,
                                        self.model.views[&v.view].parameters.settings_revision,
                                    )
                                })
                                .eq(s.views.iter().copied())
                    })
                }),
            "preflight sheets do not match document",
        )?;
        issue.state = IssueState::Review;
        issue.manifest_sha256 = Some(sha256(&manifest.bytes()?));
        issue.manifest = Some(manifest);
        let mut candidate = self.model.clone();
        candidate.drawing_issues.insert(id, issue);
        os_constraints::validate(&candidate)?;
        let h = History::new(
            "Review drawing issue",
            &self.model,
            &candidate,
            BTreeSet::from([id]),
            self.history.stats().limits,
        )?;
        self.emit(&h, "Review drawing issue");
        self.model = candidate;
        self.history.record(h);
        Ok(())
    }

    /// Reserve and validate the full transition before the caller atomically publishes.
    /// On callback failure absolutely no document/history/event state changes.
    /// Successful issuance is an explicit durable history boundary.
    pub fn publish_drawing_issue(
        &mut self,
        id: Id,
        session: Id,
        revision: u64,
        path: String,
        publish: impl FnOnce(&DrawingIssue) -> Result<()>,
    ) -> Result<()> {
        ensure(
            session == self.session_id && revision == self.revision,
            "issue review is stale",
        )?;
        let mut candidate = self.model.clone();
        let issue = candidate
            .drawing_issues
            .get_mut(&id)
            .ok_or_else(|| Error::Invalid("issue missing".into()))?;
        ensure(
            issue.state == IssueState::Review
                && issue.manifest.as_ref().is_some_and(|m| {
                    m.source_session == session
                        && m.source_revision.checked_add(1) == Some(revision)
                }),
            "only a current Review can be issued; preflight again after edits or reopening",
        )?;
        issue.state = IssueState::Issued;
        issue.archive_path = Some(path);
        os_constraints::validate(&candidate)?;
        let limits = self.history.stats().limits;
        let h = History::new(
            "Issue drawing set",
            &self.model,
            &candidate,
            BTreeSet::from([id]),
            limits,
        )?;
        publish(&candidate.drawing_issues[&id])?;
        self.emit(&h, "Issue drawing set (history boundary)");
        self.model = candidate;
        self.history = HistoryStore::default();
        // Already validated positive limits; empty history cannot reject them.
        self.history
            .set_limits(limits)
            .expect("previously validated history limits");
        Ok(())
    }
}
