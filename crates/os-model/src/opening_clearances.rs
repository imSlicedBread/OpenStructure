//! Persistent distances from native host endpoints, measured along the analytic path.
use crate::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClearanceEnd {
    Start,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpeningClearance {
    pub end: ClearanceEnd,
    pub distance: f64,
}

impl OpeningClearance {
    pub fn offset(self, length: f64, width: f64) -> Result<f64> {
        ensure(
            self.distance.is_finite() && self.distance >= 0.,
            "Clearance must be finite and nonnegative",
        )?;
        ensure(
            length.is_finite() && width.is_finite() && length > 0. && width > 0.,
            "Invalid host length or opening width",
        )?;
        Ok(match self.end {
            ClearanceEnd::Start => self.distance,
            ClearanceEnd::End => length - width - self.distance,
        })
    }
}

impl Model {
    fn clearance_offset(&self, id: Id, lock: OpeningClearance) -> Result<f64> {
        let opening = self
            .openings
            .get(&id)
            .ok_or_else(|| os_core::Error::Invalid("Clearance opening missing".into()))?;
        let host = self
            .walls
            .get(&opening.parameters.host)
            .ok_or_else(|| os_core::Error::Invalid("Clearance host missing".into()))?;
        lock.offset(
            host.parameters.length(),
            self.resolve_opening(&opening.parameters)?.width,
        )
    }

    pub fn validate_opening_clearances(&self) -> Result<()> {
        for (&id, &lock) in &self.opening_clearances {
            let expected = self.clearance_offset(id, lock)?;
            ensure(
                (self.openings[&id].parameters.offset - expected).abs() <= 1e-8,
                format!(
                    "Opening {id} conflicts with its {:?} clearance lock; edit the clearance or unlock before moving",
                    lock.end
                ),
            )?;
        }
        Ok(())
    }

    /// Prepare a disposable candidate before validation/history or preview.
    /// Callers must discard the candidate on error. No live model is mutated.
    pub fn settle_opening_clearances(&mut self, before: &Model) -> Result<BTreeSet<Id>> {
        // Removed openings also remove their owned constraint. Rehost is deliberately
        // rejected even when an update attempts to clear the lock in the same edit.
        self.opening_clearances
            .retain(|id, _| self.openings.contains_key(id));
        for id in before.opening_clearances.keys() {
            if let (Some(old), Some(new)) = (before.openings.get(id), self.openings.get(id)) {
                ensure(
                    old.parameters.host == new.parameters.host,
                    format!(
                        "Opening {id} has a clearance lock; unlock before rehosting or splitting its host"
                    ),
                )?;
            }
        }
        let mut changed = BTreeSet::new();
        for (&id, &lock) in &self.opening_clearances {
            let expected = self.clearance_offset(id, lock)?;
            let new = &self.openings[&id].parameters;
            if let Some(old) = before.openings.get(&id)
                && before.opening_clearances.get(&id) == Some(&lock)
                && (new.offset - old.parameters.offset).abs() > 1e-8
            {
                ensure(
                    (new.offset - expected).abs() <= 1e-8,
                    format!(
                        "Opening {id} conflicts with its {:?} clearance lock; edit the clearance or unlock before moving",
                        lock.end
                    ),
                )?;
            }
            if new.offset != expected {
                changed.insert(id);
            }
        }
        // Compute first, then write, so one constraint cannot affect another's math.
        for id in &changed {
            let offset = self.clearance_offset(*id, self.opening_clearances[id])?;
            self.openings.get_mut(id).unwrap().parameters.offset = offset;
        }
        Ok(changed)
    }
}
