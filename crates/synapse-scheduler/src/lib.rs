//! # synapse-scheduler
//!
//! Pure scheduling logic. Functions are deterministic over plain data plus an
//! injected [`synapse_core::Clock`]; there is no IO, no database and no direct
//! wall-clock access. Both algorithms live behind the
//! [`synapse_core::ports::Scheduler`] port so the application layer never names
//! a concrete algorithm.
//!
//! M3 fills in the real SM-2 and FSRS state machines, validated against golden
//! vectors generated from Anki. M0 ships the wiring: a [`Sm2Scheduler`] that
//! satisfies the port so the dependency direction (scheduler → core) is real.

use synapse_core::model::Algorithm;
use synapse_core::ports::Scheduler;

/// Anki-compatible SM-2 scheduler. State machine arrives in M3.
#[derive(Debug, Default, Clone, Copy)]
pub struct Sm2Scheduler;

impl Scheduler for Sm2Scheduler {
    fn algorithm(&self) -> Algorithm {
        Algorithm::Sm2
    }
}

/// FSRS-5 scheduler. State machine and optimiser arrive in M3.
#[derive(Debug, Default, Clone, Copy)]
pub struct FsrsScheduler;

impl Scheduler for FsrsScheduler {
    fn algorithm(&self) -> Algorithm {
        Algorithm::Fsrs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedulers_report_their_algorithm() {
        assert_eq!(Sm2Scheduler.algorithm(), Algorithm::Sm2);
        assert_eq!(FsrsScheduler.algorithm(), Algorithm::Fsrs);
    }
}
