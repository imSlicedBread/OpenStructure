use super::*;
use std::{collections::VecDeque, mem::size_of};

/// Session-local retention policy, not saved in the project file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HistoryLimits {
    pub max_entries: usize,
    /// Conservative owned-data accounting; not an exact allocator/RSS limit.
    pub max_estimated_bytes: usize,
}

impl Default for HistoryLimits {
    fn default() -> Self {
        Self {
            max_entries: 128,
            max_estimated_bytes: 256 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HistoryStats {
    pub undo_entries: usize,
    pub redo_entries: usize,
    pub estimated_bytes: usize,
    /// Oldest transactions released by retention, excluding normal redo branching.
    pub evicted_entries: u64,
    pub limits: HistoryLimits,
}

pub(super) struct History {
    pub label: String,
    pub before: Model,
    pub after: Model,
    pub changed: BTreeSet<Id>,
    estimated_bytes: usize,
}

impl History {
    pub fn new(
        label: &str,
        before: &Model,
        after: &Model,
        changed: BTreeSet<Id>,
        limits: HistoryLimits,
    ) -> Result<Self> {
        // Count before cloning: a failed reservation does not allocate both retained
        // snapshots or discard existing history. Cloning can reduce capacities, so
        // this can intentionally overcharge a snapshot. This remains an estimate,
        // not a guarantee about allocator layout or process memory.
        let estimated_bytes = before
            .estimated_memory_bytes()
            .saturating_add(after.estimated_memory_bytes())
            .saturating_add(label.len())
            .saturating_add(
                changed
                    .len()
                    .saturating_mul(16 * size_of::<Id>() + 16 * size_of::<usize>() + 64),
            )
            // Include inline history plus spare deque slots conservatively.
            .saturating_add(2 * size_of::<Self>());
        ensure(
            estimated_bytes < usize::MAX && estimated_bytes <= limits.max_estimated_bytes,
            format!(
                "transaction needs {estimated_bytes} estimated history bytes; limit is {}. No changes were committed",
                limits.max_estimated_bytes
            ),
        )?;
        Ok(Self {
            label: label.into(),
            before: before.clone(),
            after: after.clone(),
            changed,
            estimated_bytes,
        })
    }
}

#[derive(Default)]
pub(super) struct HistoryStore {
    pub undo: VecDeque<History>,
    pub redo: VecDeque<History>,
    limits: HistoryLimits,
    estimated_bytes: usize,
    evicted_entries: u64,
}

impl HistoryStore {
    pub fn stats(&self) -> HistoryStats {
        HistoryStats {
            undo_entries: self.undo.len(),
            redo_entries: self.redo.len(),
            estimated_bytes: self.estimated_bytes,
            evicted_entries: self.evicted_entries,
            limits: self.limits,
        }
    }

    pub fn set_limits(&mut self, limits: HistoryLimits) -> Result<()> {
        ensure(
            limits.max_entries > 0 && limits.max_estimated_bytes > 0,
            "history limits must be positive",
        )?;
        // Do not silently discard the next undo or redo because a setting is too
        // small for one transaction. Invalid settings leave all state unchanged.
        ensure(
            self.undo
                .back()
                .into_iter()
                .chain(self.redo.back())
                .all(|entry| entry.estimated_bytes <= limits.max_estimated_bytes),
            "history limit cannot fit the next undo/redo transaction",
        )?;
        self.limits = limits;
        self.trim();
        Ok(())
    }

    pub fn record(&mut self, history: History) {
        for entry in self.redo.drain(..) {
            self.estimated_bytes -= entry.estimated_bytes;
        }
        self.redo.shrink_to_fit();
        // Evict before adding to avoid counter overflow even with user-supplied
        // very large budgets. The incoming entry already passed reservation.
        while self.undo.len() >= self.limits.max_entries
            || self.estimated_bytes > self.limits.max_estimated_bytes - history.estimated_bytes
        {
            self.evict_oldest();
        }
        self.estimated_bytes += history.estimated_bytes;
        self.undo.push_back(history);
        self.undo.shrink_to_fit();
    }

    fn trim(&mut self) {
        while self.undo.len() + self.redo.len() > self.limits.max_entries
            || self.estimated_bytes > self.limits.max_estimated_bytes
        {
            self.evict_oldest();
        }
        self.undo.shrink_to_fit();
        self.redo.shrink_to_fit();
    }

    fn evict_oldest(&mut self) {
        // Keep a contiguous path around the current state: earliest undo first,
        // then farthest future redo (the next redo is at the back).
        let entry = self
            .undo
            .pop_front()
            .or_else(|| self.redo.pop_front())
            .expect("retention only evicts when an entry exists");
        self.estimated_bytes -= entry.estimated_bytes;
        self.evicted_entries = self.evicted_entries.saturating_add(1);
    }
}
