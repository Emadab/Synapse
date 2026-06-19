//! Scheduling value types and the [`Scheduler`] port.
//!
//! These types are framework-free and live in core so the port can reference
//! them; the SM-2 and FSRS state machines that implement [`Scheduler`] live in
//! `synapse-scheduler`. Everything here is pure: scheduling is a function of the
//! card's state, the rating, and the [`SchedContext`] (today + config). Interval
//! fuzz is applied at *apply* time (seeded by card id), not here, so previews
//! and tests stay deterministic.

use serde::{Deserialize, Serialize};

use crate::model::Algorithm;
use crate::Rating;

/// Which phase a card is in. Mirrors Anki's card `type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CardPhase {
    New,
    Learning,
    Review,
    Relearning,
}

/// The scheduling-relevant subset of a card. The application layer maps a full
/// `Card` row to/from this when answering.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CardState {
    pub phase: CardPhase,
    /// Remaining (re)learning steps.
    pub steps_remaining: u32,
    /// Review interval in days.
    pub interval_days: u32,
    /// SM-2 ease in permille (2500 = 250%).
    pub ease_milli: u32,
    pub reps: u32,
    pub lapses: u32,
    /// FSRS memory stability (days) — `None` until first FSRS review.
    pub stability: Option<f64>,
    /// FSRS difficulty in [1, 10].
    pub difficulty: Option<f64>,
    /// Day number of the last review (for FSRS elapsed-time).
    pub last_review_day: Option<i32>,
}

impl CardState {
    /// A brand-new, never-studied card.
    pub fn new(starting_ease_milli: u32) -> Self {
        Self {
            phase: CardPhase::New,
            steps_remaining: 0,
            interval_days: 0,
            ease_milli: starting_ease_milli,
            reps: 0,
            lapses: 0,
            stability: None,
            difficulty: None,
            last_review_day: None,
        }
    }
}

/// The next interval an answer produces. Learning answers are sub-day (minutes);
/// graduated/review answers are in days.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Interval {
    Minutes(u32),
    Days(u32),
}

/// Result of answering: the card's next state and the interval applied.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnswerOutcome {
    pub next: CardState,
    pub interval: Interval,
}

/// The four buttons' intervals, for UI preview.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RatingPreviews {
    pub again: Interval,
    pub hard: Interval,
    pub good: Interval,
    pub easy: Interval,
}

/// Per-deck scheduling configuration. Defaults match Anki's out-of-the-box deck
/// options and the published FSRS-5 default weights.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SchedConfig {
    pub algorithm: Algorithm,
    /// Learning steps in minutes (Anki default: 1m, 10m).
    pub learning_steps_min: Vec<u32>,
    /// Relearning steps in minutes (Anki default: 10m).
    pub relearning_steps_min: Vec<u32>,
    pub graduating_interval_days: u32,
    pub easy_interval_days: u32,
    pub starting_ease_milli: u32,
    pub easy_bonus: f64,
    pub hard_interval_factor: f64,
    pub interval_modifier: f64,
    /// New interval after a lapse, as a fraction of the old interval.
    pub lapse_interval_factor: f64,
    pub minimum_interval_days: u32,
    pub maximum_interval_days: u32,
    pub leech_threshold: u32,
    pub fsrs_weights: [f64; 19],
    pub desired_retention: f64,
}

/// Published FSRS-5 default weights.
pub const FSRS5_DEFAULT_WEIGHTS: [f64; 19] = [
    0.40255, 1.18385, 3.173, 15.69105, 7.1949, 0.5345, 1.4604, 0.0046, 1.54575, 0.1192, 1.01925,
    1.9395, 0.11, 0.29605, 2.2698, 0.2315, 2.9898, 0.51655, 0.6621,
];

impl Default for SchedConfig {
    fn default() -> Self {
        Self {
            algorithm: Algorithm::Sm2,
            learning_steps_min: vec![1, 10],
            relearning_steps_min: vec![10],
            graduating_interval_days: 1,
            easy_interval_days: 4,
            starting_ease_milli: 2500,
            easy_bonus: 1.3,
            hard_interval_factor: 1.2,
            interval_modifier: 1.0,
            lapse_interval_factor: 0.0,
            minimum_interval_days: 1,
            maximum_interval_days: 36_500,
            leech_threshold: 8,
            fsrs_weights: FSRS5_DEFAULT_WEIGHTS,
            desired_retention: 0.9,
        }
    }
}

/// Context for a scheduling decision.
#[derive(Debug, Clone)]
pub struct SchedContext {
    /// Today's day number (days since collection creation).
    pub today: i32,
    pub config: SchedConfig,
}

/// A spaced-repetition scheduler. Implemented by `synapse-scheduler`.
///
/// `answer` is the single source of truth; `preview` derives the four button
/// intervals from it by default, so implementations only write `answer`.
pub trait Scheduler: Send + Sync {
    fn algorithm(&self) -> Algorithm;

    fn answer(&self, state: &CardState, rating: Rating, ctx: &SchedContext) -> AnswerOutcome;

    fn preview(&self, state: &CardState, ctx: &SchedContext) -> RatingPreviews {
        RatingPreviews {
            again: self.answer(state, Rating::Again, ctx).interval,
            hard: self.answer(state, Rating::Hard, ctx).interval,
            good: self.answer(state, Rating::Good, ctx).interval,
            easy: self.answer(state, Rating::Easy, ctx).interval,
        }
    }
}
