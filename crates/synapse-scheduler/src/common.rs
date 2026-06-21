//! Shared step-progression logic for SM-2 and FSRS learning/relearning phases.

use synapse_core::scheduling::{CardPhase, CardState, SchedConfig};
use synapse_core::Rating;

/// Clamp a fractional interval (days) to the configured [min, max] range.
pub(crate) fn clamp_interval(days: f64, cfg: &SchedConfig) -> u32 {
    days.round().clamp(
        cfg.minimum_interval_days as f64,
        cfg.maximum_interval_days as f64,
    ) as u32
}

/// Result of applying a rating to a (re)learning step sequence.
pub(crate) enum StepAction {
    /// Stay in (re)learning; show the given step interval.
    Continue { steps_remaining: u32, minutes: u32 },
    /// Graduate to Review. The calling scheduler computes the review interval.
    Graduate { rating: Rating },
    /// Again pressed — restart all steps from the beginning.
    Restart { minutes: u32, total: u32 },
}

/// Compute the step action for a (re)learning card.
///
/// `steps` is the configured step list in minutes. `state.phase` distinguishes
/// a brand-new card (all steps remaining) from one mid-sequence.
pub(crate) fn step_action(state: &CardState, rating: Rating, steps: &[u32]) -> StepAction {
    let total = steps.len() as u32;

    // No steps: graduate immediately except on Again (show 1-minute delay).
    if total == 0 {
        return match rating {
            Rating::Again => StepAction::Restart { minutes: 1, total: 0 },
            _ => StepAction::Graduate { rating },
        };
    }

    // A New card hasn't started, so all steps remain.
    let remaining = if state.phase == CardPhase::New {
        total
    } else {
        state.steps_remaining.min(total)
    };

    match rating {
        Rating::Again => StepAction::Restart { minutes: steps[0], total },
        Rating::Hard => {
            // Repeat the current step.
            let idx = (total - remaining).min(total - 1) as usize;
            StepAction::Continue {
                steps_remaining: remaining.max(1),
                minutes: steps[idx],
            }
        }
        Rating::Good => {
            let next_remaining = remaining.saturating_sub(1);
            if next_remaining == 0 {
                StepAction::Graduate { rating }
            } else {
                let idx = (total - next_remaining) as usize;
                StepAction::Continue {
                    steps_remaining: next_remaining,
                    minutes: steps[idx],
                }
            }
        }
        Rating::Easy => StepAction::Graduate { rating },
    }
}
