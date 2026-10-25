//! Portable frozen drawing packages. No live document/render dependency on read.
use os_core::{Error, Result, ensure};
use os_model::{DrawingIssue, IssueManifest, IssueState, sha256};
use std::{
    io::{Cursor, Read, Write},
    path::Path,
};
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

pub const MAX_ISSUE_BUNDLE_BYTES: usize = 32 * 1024 * 1024;
const MAX_MANIFEST: usize = 2 * 1024 * 1024;
fn err(e: impl std::fmt::Display) -> Error {
    Error::Storage(e.to_string())
}

#[derive(Clone, Debug)]
pub struct IssueBundle {
    manifest: IssueManifest,
    manifest_bytes: Vec<u8>,
    pdf: Vec<u8>,
}

impl IssueBundle {
    pub fn new(manifest: IssueManifest, pdf: Vec<u8>) -> Result<Self> {
        manifest.validate()?;
        let manifest_bytes = manifest.bytes()?;
        ensure(
            manifest_bytes.len() <= MAX_MANIFEST
                && !pdf.is_empty()
                && pdf.len() <= MAX_ISSUE_BUNDLE_BYTES - MAX_MANIFEST,
            "issue artifact size limit",
        )?;
        ensure(
            pdf.starts_with(b"%PDF-1.4")
                && pdf.ends_with(b"%%EOF\n")
                && sha256(&pdf) == manifest.pdf_sha256,
            "issue PDF integrity mismatch",
        )?;
        Ok(Self {
            manifest,
            manifest_bytes,
            pdf,
        })
    }
    pub fn manifest(&self) -> &IssueManifest {
        &self.manifest
    }
    pub fn pdf(&self) -> &[u8] {
        &self.pdf
    }
    pub fn manifest_sha256(&self) -> String {
        sha256(&self.manifest_bytes)
    }
    pub fn verify_record(&self, issue: &DrawingIssue) -> Result<()> {
        issue.validate()?;
        ensure(
            issue.manifest.as_ref() == Some(&self.manifest)
                && issue.manifest_sha256.as_deref() == Some(self.manifest_sha256().as_str()),
            "archive does not match frozen issue record",
        )
    }
    pub fn bytes(&self) -> Result<Vec<u8>> {
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        let integrity = format!(
            "{}  manifest.json\n{}  drawings.pdf\n",
            self.manifest_sha256(),
            self.manifest.pdf_sha256
        );
        for (name, bytes) in [
            ("manifest.json", self.manifest_bytes.as_slice()),
            ("drawings.pdf", self.pdf.as_slice()),
            ("integrity.sha256", integrity.as_bytes()),
        ] {
            zip.start_file(name, options).map_err(err)?;
            zip.write_all(bytes).map_err(err)?;
        }
        let bytes = zip.finish().map_err(err)?.into_inner();
        ensure(
            bytes.len() <= MAX_ISSUE_BUNDLE_BYTES,
            "issue bundle exceeds size limit",
        )?;
        Ok(bytes)
    }
    /// No overwrite path. The final filename appears only after all bytes are synced.
    pub fn publish(&self, issue: &DrawingIssue, path: &Path) -> Result<()> {
        ensure(
            issue.state == IssueState::Issued,
            "publication requires reserved Issued transition",
        )?;
        self.verify_record(issue)?;
        let bytes = self.bytes()?;
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(err)?;
        temp.write_all(&bytes).map_err(err)?;
        temp.as_file_mut().sync_all().map_err(err)?;
        temp.persist_noclobber(path).map_err(err)?;
        Ok(())
    }
    pub fn read(path: &Path) -> Result<Self> {
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .map_err(err)?
            .take(MAX_ISSUE_BUNDLE_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(err)?;
        Self::parse(&bytes)
    }
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        ensure(
            bytes.len() <= MAX_ISSUE_BUNDLE_BYTES,
            "issue bundle exceeds size limit",
        )?;
        let mut zip = ZipArchive::new(Cursor::new(bytes)).map_err(err)?;
        crate::validate_directory(Cursor::new(bytes), zip.central_directory_start(), zip.len())?;
        ensure(
            zip.len() == 3
                && zip
                    .file_names()
                    .all(|n| matches!(n, "manifest.json" | "drawings.pdf" | "integrity.sha256")),
            "unexpected issue bundle entries",
        )?;
        let mut read = |name: &str, limit: usize| -> Result<Vec<u8>> {
            let entry = zip.by_name(name).map_err(err)?;
            ensure(
                !entry.is_dir()
                    && entry
                        .unix_mode()
                        .is_none_or(|mode| matches!(mode & 0o170000, 0 | 0o100000)),
                "issue entries must be regular files",
            )?;
            ensure(
                entry.size() <= limit as u64,
                "issue entry exceeds size limit",
            )?;
            let mut out = Vec::new();
            entry
                .take(limit as u64 + 1)
                .read_to_end(&mut out)
                .map_err(err)?;
            ensure(out.len() <= limit, "issue entry exceeds size limit")?;
            Ok(out)
        };
        let manifest_bytes = read("manifest.json", MAX_MANIFEST)?;
        crate::json_guard::validate(std::str::from_utf8(&manifest_bytes).map_err(err)?)
            .map_err(err)?;
        let manifest: IssueManifest = serde_json::from_slice(&manifest_bytes).map_err(err)?;
        let pdf = read("drawings.pdf", MAX_ISSUE_BUNDLE_BYTES - MAX_MANIFEST)?;
        let integrity = read("integrity.sha256", 512)?;
        let bundle = Self::new(manifest, pdf)?;
        ensure(
            bundle.manifest_bytes == manifest_bytes,
            "noncanonical issue manifest",
        )?;
        let expected = format!(
            "{}  manifest.json\n{}  drawings.pdf\n",
            bundle.manifest_sha256(),
            bundle.manifest.pdf_sha256
        );
        ensure(
            integrity == expected.as_bytes(),
            "issue manifest integrity mismatch",
        )?;
        Ok(bundle)
    }
}
