//! # synapse-scheduler
//!
//! Pure scheduling logic. Functions are deterministic over plain data plus the
//! [`SchedContext`] (today + per-deck config); there is no IO, no database and
//! no direct wall-clock access. Both algorithms implement the
//! [`synapse_core::scheduling::Scheduler`] port, so the application layer never
//! names a concrete algorithm — it asks [`scheduler_for`].
//!
//! Interval fuzz is applied by the caller at apply time (seeded by card id) so
//! that previews and these tests stay deterministic.

mod fsrs;
pub mod optimizer;
mod sm2;

pub use fsrs::FsrsScheduler;
pub use optimizer::{optimize, OptimizeResult, MIN_REVIEWS};
pub use sm2::Sm2Scheduler;

use synapse_core::scheduling::Scheduler;
use synapse_core::Algorithm;

/// Construct the scheduler for a deck's configured algorithm. Switching is just
/// a config change — both SM-2 and FSRS state persist on every card.
pub fn scheduler_for(algorithm: Algorithm) -> Box<dyn Scheduler> {
    match algorithm {
        Algorithm::Sm2 => Box::new(Sm2Scheduler),
        Algorithm::Fsrs => Box::new(FsrsScheduler),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn factory_returns_matching_algorithm() {
        assert_eq!(scheduler_for(Algorithm::Sm2).algorithm(), Algorithm::Sm2);
        assert_eq!(scheduler_for(Algorithm::Fsrs).algorithm(), Algorithm::Fsrs);
    }
}
