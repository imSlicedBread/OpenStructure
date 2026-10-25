//! Ordered sets and issue metadata. PDF/archive bytes deliberately live outside model history.
use crate::Model;
use os_core::{Id, Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub const ISSUE_BUNDLE_VERSION: u32 = 1;
pub const MAX_ISSUE_SHEETS: usize = 100;

pub fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SheetSet {
    pub id: Id,
    pub name: String,
    /// Explicit paper order, never UUID/map order.
    pub sheets: Vec<Id>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum IssueState {
    Draft,
    Review,
    Issued,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IssueWarning {
    pub message: String,
    /// A nonempty review decision is required for every nonblocking warning.
    pub acknowledgment: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IssueSheet {
    pub id: Id,
    pub number: String,
    pub name: String,
    pub views: Vec<(Id, u64)>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IssueManifest {
    pub format_version: u32,
    pub state: IssueState,
    pub issue: Id,
    pub project: Id,
    pub set: Id,
    pub sequence: u32,
    pub revision: String,
    pub description: String,
    /// User-entered ISO calendar date, not inferred from the host clock.
    pub date: String,
    pub recipients: Vec<String>,
    pub supersedes: Option<Id>,
    pub schema_version: u32,
    pub native_api_version: u32,
    pub application_version: String,
    pub source_session: Id,
    pub source_revision: u64,
    pub source_sha256: String,
    pub providers: Vec<(String, Id)>,
    /// Provider ID, software version and native/provider API version.
    pub provider_versions: Vec<(String, String, u32)>,
    pub sheets: Vec<IssueSheet>,
    pub warnings: Vec<IssueWarning>,
    pub pdf_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DrawingIssue {
    pub id: Id,
    pub set: Id,
    pub sequence: u32,
    pub revision: String,
    pub description: String,
    pub date: String,
    pub recipients: Vec<String>,
    pub sheets: Vec<Id>,
    pub supersedes: Option<Id>,
    pub state: IssueState,
    pub manifest: Option<IssueManifest>,
    pub manifest_sha256: Option<String>,
    /// User-selected standalone bundle. Moving it does not change the record.
    pub archive_path: Option<String>,
}

fn text(value: &str, limit: usize) -> bool {
    !value.trim().is_empty() && value.len() <= limit && !value.chars().any(char::is_control)
}
fn ordered_ids(ids: &[Id]) -> bool {
    !ids.is_empty()
        && ids.len() <= MAX_ISSUE_SHEETS
        && ids.iter().all(|id| !id.0.is_nil())
        && ids.iter().collect::<BTreeSet<_>>().len() == ids.len()
}
fn hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn date(value: &str) -> bool {
    let parts: Vec<_> = value.split('-').collect();
    if parts.len() != 3 || parts[0].len() != 4 || parts[1].len() != 2 || parts[2].len() != 2 {
        return false;
    }
    let (Ok(y), Ok(m), Ok(d)) = (
        parts[0].parse::<u32>(),
        parts[1].parse::<u32>(),
        parts[2].parse::<u32>(),
    ) else {
        return false;
    };
    let days = match m {
        4 | 6 | 9 | 11 => 30,
        2 if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) => 29,
        2 => 28,
        1..=12 => 31,
        _ => 0,
    };
    y > 0 && d > 0 && d <= days
}

impl IssueManifest {
    pub fn bytes(&self) -> Result<Vec<u8>> {
        serde_json::to_vec_pretty(self).map_err(|e| os_core::Error::Invalid(e.to_string()))
    }
    pub fn validate(&self) -> Result<()> {
        ensure(
            self.format_version == ISSUE_BUNDLE_VERSION
                && self.state == IssueState::Issued
                && self.schema_version > 0
                && self.native_api_version > 0,
            "unsupported issue manifest version/state",
        )?;
        ensure(
            !self.issue.0.is_nil()
                && !self.project.0.is_nil()
                && !self.set.0.is_nil()
                && !self.source_session.0.is_nil(),
            "invalid issue manifest identity",
        )?;
        ensure(
            self.sequence > 0
                && text(&self.revision, 64)
                && text(&self.description, 2048)
                && date(&self.date),
            "invalid issue metadata",
        )?;
        ensure(
            !self.recipients.is_empty()
                && self.recipients.len() <= 100
                && self.recipients.iter().all(|r| text(r, 256)),
            "invalid issue recipients",
        )?;
        ensure(
            hash(&self.pdf_sha256) && hash(&self.source_sha256),
            "invalid issue digest",
        )?;
        ensure(
            ordered_ids(&self.sheets.iter().map(|s| s.id).collect::<Vec<_>>()),
            "invalid issue sheet order",
        )?;
        let mut numbers = BTreeSet::new();
        for s in &self.sheets {
            ensure(
                text(&s.number, 64)
                    && text(&s.name, 256)
                    && numbers.insert(s.number.trim().to_ascii_lowercase()),
                "invalid or duplicate issued sheet number",
            )?;
            ensure(
                !s.views.is_empty()
                    && s.views.len() <= 64
                    && s.views.iter().all(|(id, _)| !id.0.is_nil()),
                "invalid issued views",
            )?;
        }
        ensure(
            self.warnings.len() <= 1000
                && self
                    .warnings
                    .iter()
                    .all(|w| text(&w.message, 2048) && text(&w.acknowledgment, 2048)),
            "all issue warnings require explicit acknowledgment",
        )?;
        ensure(
            text(&self.application_version, 128)
                && self.providers.len() <= 256
                && self
                    .providers
                    .iter()
                    .all(|(name, id)| text(name, 256) && !id.0.is_nil()),
            "invalid issue provider signature",
        )?;
        ensure(
            !self.provider_versions.is_empty()
                && self.provider_versions.len() <= 256
                && self
                    .provider_versions
                    .iter()
                    .all(|(id, version, api)| text(id, 256) && text(version, 128) && *api > 0),
            "invalid issue provider versions",
        )
    }
}

impl DrawingIssue {
    pub fn validate(&self) -> Result<()> {
        ensure(
            !self.id.0.is_nil() && !self.set.0.is_nil() && self.sequence > 0,
            "invalid issue identity/sequence",
        )?;
        ensure(
            text(&self.revision, 64) && text(&self.description, 2048) && date(&self.date),
            "invalid issue revision, description or ISO date",
        )?;
        ensure(
            !self.recipients.is_empty()
                && self.recipients.len() <= 100
                && self.recipients.iter().all(|r| text(r, 256)),
            "issue requires recipients",
        )?;
        ensure(
            ordered_ids(&self.sheets),
            "issue requires unique ordered sheets",
        )?;
        if self.state == IssueState::Draft {
            return ensure(
                self.manifest.is_none()
                    && self.manifest_sha256.is_none()
                    && self.archive_path.is_none(),
                "draft cannot contain frozen artifacts",
            );
        }
        let m = self
            .manifest
            .as_ref()
            .ok_or_else(|| os_core::Error::Invalid("issue manifest missing".into()))?;
        m.validate()?;
        ensure(
            m.issue == self.id
                && m.set == self.set
                && m.sequence == self.sequence
                && m.revision == self.revision
                && m.description == self.description
                && m.date == self.date
                && m.recipients == self.recipients
                && m.supersedes == self.supersedes
                && m.sheets
                    .iter()
                    .map(|s| s.id)
                    .eq(self.sheets.iter().copied()),
            "issue/manifest mismatch",
        )?;
        ensure(
            self.manifest_sha256.as_deref() == Some(sha256(&m.bytes()?).as_str()),
            "issue manifest integrity mismatch",
        )?;
        ensure(
            match self.state {
                IssueState::Issued => self.archive_path.as_deref().is_some_and(|p| text(p, 4096)),
                IssueState::Review => self.archive_path.is_none(),
                IssueState::Draft => false,
            },
            "invalid issue archive state",
        )
    }
}

pub(crate) fn validate(model: &Model, ids: &mut BTreeSet<Id>) -> Result<()> {
    ensure(
        model.sheet_sets.len() <= 100 && model.drawing_issues.len() <= 1000,
        "too many sheet sets or issues",
    )?;
    for (id, set) in &model.sheet_sets {
        ensure(
            *id == set.id
                && !id.0.is_nil()
                && ids.insert(*id)
                && text(&set.name, 256)
                && ordered_ids(&set.sheets),
            "invalid sheet set",
        )?;
        ensure(
            set.sheets.iter().all(|id| model.sheets.contains_key(id)),
            "sheet set references missing sheet",
        )?;
    }
    let mut sequences = BTreeSet::new();
    let mut superseded = BTreeSet::new();
    for (id, issue) in &model.drawing_issues {
        ensure(
            *id == issue.id && ids.insert(*id) && sequences.insert((issue.set, issue.sequence)),
            "duplicate issue identity/sequence",
        )?;
        issue.validate()?;
        if issue.state != IssueState::Issued {
            ensure(
                model.sheet_sets.contains_key(&issue.set)
                    && issue.sheets.iter().all(|s| model.sheets.contains_key(s)),
                "unissued issue references missing set/sheet",
            )?;
        }
        if let Some(m) = &issue.manifest {
            ensure(m.project == model.project.id(), "issue project mismatch")?;
        }
        if let Some(previous) = issue.supersedes {
            ensure(
                model.drawing_issues.get(&previous).is_some_and(|p| {
                    p.state == IssueState::Issued
                        && p.set == issue.set
                        && p.sequence < issue.sequence
                }),
                "invalid supersession",
            )?;
            if issue.state == IssueState::Issued {
                ensure(superseded.insert(previous), "issue already superseded")?;
            }
        }
    }
    Ok(())
}
