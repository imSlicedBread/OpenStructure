//! Experimental, restricted IFC4 wall exchange. Desktop availability is separately gated.
use os_core::{Error, Result};
use os_model::Model;

/// Restricted wall-subset desktop exchange passed independent viewer acceptance.
pub const AVAILABLE: bool = true;
pub const UNAVAILABLE_REASON: &str = "IFC exchange is disabled for this adapter. Use WallIfc for the supported IFC4 wall subset; see docs/ifc-roadmap.md.";
mod reader;
mod step;
mod writer;

/// The output plus an explicit account of data outside the supported exchange subset.
#[derive(Debug)]
pub struct Exchange<T> {
    pub value: T,
    pub warnings: Vec<String>,
}

pub struct WallIfc;
impl WallIfc {
    pub fn export_report(&self, model: &Model) -> Result<Exchange<Vec<u8>>> {
        writer::export(model)
    }
    /// Document callers must include auxiliary names so attachment loss is explicit.
    pub fn export_report_with_files<'a>(
        &self,
        model: &Model,
        names: impl IntoIterator<Item = &'a str>,
    ) -> Result<Exchange<Vec<u8>>> {
        let mut report = self.export_report(model)?;
        let count = names
            .into_iter()
            .filter(|name| !name.ends_with('/'))
            .count();
        if count > 0 {
            report.warnings.push(format!(
                "{count} native auxiliary file(s) are not included in IFC; keep the original .osb."
            ));
        }
        Ok(report)
    }
    pub fn import_report(&self, bytes: &[u8]) -> Result<Exchange<Model>> {
        reader::import(bytes)
    }
}
impl IfcAdapter for WallIfc {
    fn export(&self, model: &Model) -> Result<Vec<u8>> {
        let report = self.export_report(model)?;
        if !report.warnings.is_empty() {
            return Err(Error::Unsupported(format!(
                "Export requires loss acknowledgement: {}",
                report.warnings.join("; ")
            )));
        }
        Ok(report.value)
    }
    fn import(&self, bytes: &[u8]) -> Result<Model> {
        Ok(self.import_report(bytes)?.value)
    }
}
pub trait IfcAdapter {
    fn export(&self, model: &Model) -> Result<Vec<u8>>;
    fn import(&self, bytes: &[u8]) -> Result<Model>;
}
pub struct UnavailableIfc;
impl IfcAdapter for UnavailableIfc {
    fn export(&self, _model: &Model) -> Result<Vec<u8>> {
        Err(Error::Unsupported(UNAVAILABLE_REASON.into()))
    }
    fn import(&self, _bytes: &[u8]) -> Result<Model> {
        Err(Error::Unsupported(UNAVAILABLE_REASON.into()))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn never_claims_or_emits_ifc_support() {
        assert!(UnavailableIfc.export(&Model::new("Test")).is_err());
        assert!(UnavailableIfc.import(b"ISO-10303-21;").is_err());
    }
}
