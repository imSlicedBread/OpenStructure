//! Independent linear/radial repeats; stable identities and aggregate resource limits.
use super::{wall_set_copy::Identities, wall_set_move::Candidate};
use os_core::{Id, Point2, Result, ensure};
use os_model::Model;
use std::collections::BTreeSet;

pub(super) struct Array {
    copies: Vec<Identities>,
}

impl Array {
    pub(super) fn new(model: &Model, walls: &BTreeSet<Id>, view: Id, count: usize) -> Result<Self> {
        ensure(
            (2..=32).contains(&count),
            "Array count must be 2–32, including the source",
        )?;
        ensure(
            !walls.is_empty() && walls.len() * (count - 1) <= 64,
            "Array exceeds 64 added walls",
        )?;
        let first = Identities::new(model, walls, view)?;
        ensure(
            first.dependent_count() * (count - 1) <= 4096,
            "Array exceeds 4096 added dependents, including clearance locks",
        )?;
        let mut copies = vec![first];
        for _ in 2..count {
            copies.push(Identities::new(model, walls, view)?);
        }
        Ok(Self { copies })
    }

    pub(super) fn count(&self) -> usize {
        self.copies.len() + 1
    }

    pub(super) fn summary(&self) -> String {
        self.copies[0].summary_repeated(self.copies.len())
    }

    pub(super) fn walls(&self) -> BTreeSet<Id> {
        self.copies
            .iter()
            .flat_map(|c| c.walls.values().copied())
            .collect()
    }

    pub(super) fn contains_tag(&self, id: Id) -> bool {
        self.copies
            .iter()
            .any(|c| c.tags.values().any(|copy| *copy == id))
    }

    pub(super) fn candidate(&self, original: &Model, spacing: Point2) -> Result<Candidate> {
        self.build(
            |copy, model, n| copy.candidate(model, Point2::new(spacing.x * n, spacing.y * n)),
            original,
        )
    }

    pub(super) fn radial_candidate(
        &self,
        original: &Model,
        pivot: Point2,
        degrees: f64,
    ) -> Result<Candidate> {
        ensure(
            degrees.is_finite()
                && degrees != 0.0
                && degrees.abs() * ((self.count() - 1) as f64) < 360.0,
            "Radial step must be finite and nonzero, with total sweep below 360°",
        )?;
        self.build(
            |copy, model, n| copy.candidate_rotated(model, pivot, degrees * n),
            original,
        )
    }

    fn build(
        &self,
        candidate: impl Fn(&Identities, &Model, f64) -> Result<Candidate>,
        original: &Model,
    ) -> Result<Candidate> {
        let mut model = original.clone();
        let mut commands = Vec::new();
        for (index, copy) in self.copies.iter().enumerate() {
            let n = (index + 1) as f64;
            // Every repeat reads the unchanged source identities, while checking
            // collision and room invariants against all preceding repeats too.
            let candidate = candidate(copy, &model, n)?;
            model = candidate.model;
            commands.extend(candidate.commands);
        }
        Ok(Candidate { model, commands })
    }
}
