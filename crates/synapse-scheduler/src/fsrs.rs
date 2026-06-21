//! FSRS-6 scheduler with Anki-compatible learning steps.
//!
//! New/Learning/Relearning cards progress through minute-based steps before
//! the FSRS memory model takes over at the Review phase. This matches Anki's
//! FSRS behaviour (v25.09+).
//!
//! Key FSRS-6 changes vs FSRS-5:
//! - Weights expanded to 21 (w[17..=20] are new).
//! - DECAY = -w[20] is trainable (was hardcoded -0.5).
//! - FACTOR = 0.9^(1/|DECAY|) − 1 (derived from fixed 90% threshold at t=S).
//! - Short-term stability formula for same-day reviews (w[17..=19]).
//! - S_min floor in forget_stability prevents catastrophic lapse on stable cards.

use crate::common::{step_action, StepAction};
use synapse_core::scheduling::{
    AnswerOutcome, CardPhase, CardState, Interval, SchedConfig, SchedContext, Scheduler,
};
use synapse_core::{Algorithm, Rating};

#[derive(Debug, Default, Clone, Copy)]
pub struct FsrsScheduler;

impl Scheduler for FsrsScheduler {
    fn algorithm(&self) -> Algorithm {
        Algorithm::Fsrs
    }

    fn answer(&self, state: &CardState, rating: Rating, ctx: &SchedContext) -> AnswerOutcome {
        let cfg = &ctx.config;

        match state.phase {
            CardPhase::New | CardPhase::Learning => {
                fsrs_learning(state, rating, ctx, &cfg.learning_steps_min, false)
            }
            CardPhase::Relearning => {
                fsrs_learning(state, rating, ctx, &cfg.relearning_steps_min, true)
            }
            CardPhase::Review => fsrs_review(state, rating, ctx),
        }
    }
}

// ---------------------------------------------------------------------------
// Learning / relearning step handler
// ---------------------------------------------------------------------------

fn fsrs_learning(
    state: &CardState,
    rating: Rating,
    ctx: &SchedContext,
    steps: &[u32],
    is_relearn: bool,
) -> AnswerOutcome {
    let cfg = &ctx.config;
    let phase = if is_relearn { CardPhase::Relearning } else { CardPhase::Learning };
    let w = &cfg.fsrs_weights;
    let g = rating as i64;

    match step_action(state, rating, steps) {
        StepAction::Restart { minutes, total } => AnswerOutcome {
            next: CardState { phase, steps_remaining: total, ..*state },
            interval: Interval::Minutes(minutes),
        },
        StepAction::Continue { steps_remaining, minutes } => AnswerOutcome {
            next: CardState { phase, steps_remaining, ..*state },
            interval: Interval::Minutes(minutes),
        },
        StepAction::Graduate { rating: grad_r } => {
            let g_grad = grad_r as i64;
            let (stability, difficulty) = if is_relearn {
                // After a lapse: recalculate lapse stability, keep difficulty.
                let (s0, d0) = match (state.stability, state.difficulty) {
                    (Some(s), Some(d)) => {
                        let elapsed = (ctx.today
                            - state.last_review_day.unwrap_or(ctx.today))
                        .max(0) as f64;
                        let r = retrievability(elapsed, s, w);
                        let new_d = next_difficulty(w, d, g);
                        let new_s = forget_stability(w, d, s, r);
                        (new_s, new_d)
                    }
                    _ => (initial_stability(w, g_grad), initial_difficulty(w, g_grad)),
                };
                (s0, d0)
            } else {
                // New card graduating: initialize stability from graduation rating.
                (initial_stability(w, g_grad), initial_difficulty(w, g_grad))
            };
            let days = interval_from_stability(stability, cfg).max(1);
            AnswerOutcome {
                next: CardState {
                    phase: CardPhase::Review,
                    steps_remaining: 0,
                    interval_days: days,
                    reps: state.reps + 1,
                    lapses: state.lapses,
                    stability: Some(stability),
                    difficulty: Some(difficulty),
                    last_review_day: Some(ctx.today),
                    ..*state
                },
                interval: Interval::Days(days),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Review handler
// ---------------------------------------------------------------------------

fn fsrs_review(state: &CardState, rating: Rating, ctx: &SchedContext) -> AnswerOutcome {
    let cfg = &ctx.config;
    let w = &cfg.fsrs_weights;
    let g = rating as i64;

    let (stability, difficulty, lapse) = match (state.stability, state.difficulty) {
        (Some(s), Some(d)) => {
            let elapsed =
                (ctx.today - state.last_review_day.unwrap_or(ctx.today)).max(0) as f64;

            let difficulty = next_difficulty(w, d, g);

            let (stability, is_lapse) = if elapsed == 0.0 {
                // Same-day review: use FSRS-6 short-term stability formula.
                (short_term_stability(w, s, g), false)
            } else if g == 1 {
                let r = retrievability(elapsed, s, w);
                (forget_stability(w, d, s, r), true)
            } else {
                let r = retrievability(elapsed, s, w);
                (success_stability(w, d, s, r, g), false)
            };
            (stability, difficulty, is_lapse)
        }
        // First-ever review (card somehow reached Review without going through steps).
        _ => (initial_stability(w, g), initial_difficulty(w, g), false),
    };

    if lapse {
        // Lapse: enter relearning if steps configured, else stay in Review.
        let relearn_steps = &cfg.relearning_steps_min;
        let next = CardState {
            phase: if relearn_steps.is_empty() {
                CardPhase::Review
            } else {
                CardPhase::Relearning
            },
            steps_remaining: relearn_steps.len() as u32,
            interval_days: state.interval_days,
            reps: state.reps + 1,
            lapses: state.lapses + 1,
            stability: Some(stability),
            difficulty: Some(difficulty),
            last_review_day: Some(ctx.today),
            ..*state
        };
        let interval = match relearn_steps.first() {
            Some(&m) => Interval::Minutes(m),
            None => {
                let days = interval_from_stability(stability, cfg).max(1);
                Interval::Days(days)
            }
        };
        AnswerOutcome { next, interval }
    } else {
        let days = interval_from_stability(stability, cfg).max(1);
        AnswerOutcome {
            next: CardState {
                phase: CardPhase::Review,
                interval_days: days,
                reps: state.reps + 1,
                stability: Some(stability),
                difficulty: Some(difficulty),
                last_review_day: Some(ctx.today),
                ..*state
            },
            interval: Interval::Days(days),
        }
    }
}

// ---------------------------------------------------------------------------
// FSRS-6 memory model
// ---------------------------------------------------------------------------

fn fsrs6_decay(w: &[f64; 21]) -> f64 {
    -(w[20].max(0.01))
}

/// FACTOR is derived from the fixed 90% retention-at-stability definition:
/// R(S, S) = (1 + FACTOR)^DECAY = 0.9  ⟹  FACTOR = 0.9^(1/DECAY) - 1
fn fsrs6_factor(w: &[f64; 21]) -> f64 {
    let decay = fsrs6_decay(w);
    0.9_f64.powf(1.0 / decay) - 1.0
}

fn retrievability(elapsed_days: f64, stability: f64, w: &[f64; 21]) -> f64 {
    let decay = fsrs6_decay(w);
    let factor = fsrs6_factor(w);
    (1.0 + factor * elapsed_days / stability.max(0.1)).powf(decay)
}

/// Next interval such that predicted retrievability equals `desired_retention`.
fn interval_from_stability(stability: f64, cfg: &SchedConfig) -> u32 {
    let w = &cfg.fsrs_weights;
    let decay = fsrs6_decay(w);
    let factor = fsrs6_factor(w);
    let days = (stability / factor) * (cfg.desired_retention.powf(1.0 / decay) - 1.0);
    days.round()
        .clamp(cfg.minimum_interval_days as f64, cfg.maximum_interval_days as f64)
        as u32
}

fn initial_stability(w: &[f64; 21], g: i64) -> f64 {
    w[(g - 1) as usize].max(0.1)
}

fn initial_difficulty(w: &[f64; 21], g: i64) -> f64 {
    (w[4] - (w[5] * (g - 1) as f64).exp() + 1.0).clamp(1.0, 10.0)
}

fn difficulty_easy_anchor(w: &[f64; 21]) -> f64 {
    initial_difficulty(w, 4)
}

fn next_difficulty(w: &[f64; 21], d: f64, g: i64) -> f64 {
    let delta = -w[6] * (g - 3) as f64;
    let damped = d + delta * (10.0 - d) / 9.0;
    (w[7] * difficulty_easy_anchor(w) + (1.0 - w[7]) * damped).clamp(1.0, 10.0)
}

fn success_stability(w: &[f64; 21], d: f64, s: f64, r: f64, g: i64) -> f64 {
    let hard_penalty = if g == 2 { w[15] } else { 1.0 };
    let easy_bonus = if g == 4 { w[16] } else { 1.0 };
    let growth = w[8].exp()
        * (11.0 - d)
        * s.powf(-w[9])
        * ((w[10] * (1.0 - r)).exp() - 1.0)
        * hard_penalty
        * easy_bonus;
    (s * (1.0 + growth)).max(0.1)
}

fn forget_stability(w: &[f64; 21], d: f64, s: f64, r: f64) -> f64 {
    let s_forget =
        w[11] * d.powf(-w[12]) * ((s + 1.0).powf(w[13]) - 1.0) * (w[14] * (1.0 - r)).exp();
    // FSRS-6: S_min floor prevents lapse from devastating a highly stable card.
    let s_min = s / (w[17] * w[18]).exp();
    s_forget.max(s_min).clamp(0.1, s)
}

/// FSRS-6 short-term stability for same-day reviews (elapsed == 0).
fn short_term_stability(w: &[f64; 21], s: f64, g: i64) -> f64 {
    let sinc = (w[17] * (g as f64 - 3.0 + w[18])).exp() * s.powf(-w[19]);
    let factor = if g >= 2 { sinc.max(1.0) } else { sinc };
    (s * factor).max(0.1)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use synapse_core::scheduling::{SchedConfig, SchedContext};

    fn ctx() -> SchedContext {
        SchedContext {
            today: 100,
            config: SchedConfig {
                algorithm: Algorithm::Fsrs,
                ..SchedConfig::default()
            },
        }
    }

    fn new_card() -> CardState {
        CardState::new(2500)
    }

    fn days(i: Interval) -> Option<u32> {
        match i { Interval::Days(d) => Some(d), _ => None }
    }

    #[test]
    fn new_card_goes_through_learning_steps() {
        let s = FsrsScheduler;
        let c = ctx();
        // New card: Good → step 2 (10m).
        let o1 = s.answer(&new_card(), Rating::Good, &c);
        assert_eq!(o1.interval, Interval::Minutes(10));
        assert_eq!(o1.next.phase, CardPhase::Learning);
        // Good again → graduates to Review.
        let o2 = s.answer(&o1.next, Rating::Good, &c);
        assert_eq!(o2.next.phase, CardPhase::Review);
        assert!(days(o2.interval).is_some(), "graduated interval must be in days");
        assert!(o2.next.stability.is_some());
    }

    #[test]
    fn first_review_intervals_are_monotonic() {
        let s = FsrsScheduler;
        let c = ctx();
        let again = days(s.answer(&new_card(), Rating::Again, &c).interval).unwrap_or(0);
        let hard = days(s.answer(&new_card(), Rating::Hard, &c).interval).unwrap_or(0);
        let good = days(s.answer(&new_card(), Rating::Good, &c).interval).unwrap_or(0);
        let easy = days(s.answer(&new_card(), Rating::Easy, &c).interval).unwrap_or(0);
        assert!(again <= hard, "{again} <= {hard}");
        assert!(hard <= good, "{hard} <= {good}");
        assert!(good < easy, "{good} < {easy}");
    }

    #[test]
    fn again_resets_learning_steps() {
        let s = FsrsScheduler;
        let c = ctx();
        let mid = s.answer(&new_card(), Rating::Good, &c).next;
        assert_eq!(mid.steps_remaining, 1);
        let reset = s.answer(&mid, Rating::Again, &c).next;
        assert_eq!(reset.steps_remaining, 2);
        assert_eq!(reset.phase, CardPhase::Learning);
    }

    #[test]
    fn review_again_lapses_into_relearning() {
        let s = FsrsScheduler;
        let c = ctx();
        // Graduate through steps first.
        let o1 = s.answer(&new_card(), Rating::Good, &c);
        let o2 = s.answer(&o1.next, Rating::Good, &c);
        let review_card = o2.next;
        assert_eq!(review_card.phase, CardPhase::Review);
        // Lapse.
        let mut later = c.clone();
        later.today = c.today + 10;
        let lapsed = s.answer(&review_card, Rating::Again, &later);
        assert_eq!(lapsed.next.lapses, 1);
        assert_eq!(lapsed.next.phase, CardPhase::Relearning);
    }

    #[test]
    fn repeated_good_grows_stability() {
        let s = FsrsScheduler;
        let c = ctx();
        // Skip through learning steps.
        let o1 = s.answer(&new_card(), Rating::Easy, &c);
        let o2 = s.answer(&o1.next, Rating::Easy, &c);
        let review = o2.next;
        assert_eq!(review.phase, CardPhase::Review);
        let mut later = c.clone();
        later.today = c.today + days(o2.interval).unwrap() as i32;
        let o3 = s.answer(&review, Rating::Good, &later);
        assert!(
            o3.next.stability.unwrap() > review.stability.unwrap(),
            "stability must grow on Good review"
        );
    }
}
