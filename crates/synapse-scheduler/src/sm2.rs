//! Anki-compatible SM-2 scheduler.
//!
//! Phases: New/Learning/Relearning step through minute-based steps, then
//! graduate to Review. Review answers scale the interval by ease (Again lapses
//! into Relearning, reducing ease by 200 and the interval by `lapse_factor`;
//! Hard −150 ease ×1.2; Good ×ease; Easy +150 ease ×ease×bonus). Ease floors at
//! 1300. Interval fuzz is applied later, at apply time.

use synapse_core::scheduling::{
    AnswerOutcome, CardPhase, CardState, Interval, SchedConfig, SchedContext, Scheduler,
};
use synapse_core::{Algorithm, Rating};

const MIN_EASE_MILLI: u32 = 1300;
const EASE_STEP_MILLI: u32 = 150;
const LAPSE_EASE_PENALTY_MILLI: u32 = 200;

#[derive(Debug, Default, Clone, Copy)]
pub struct Sm2Scheduler;

impl Scheduler for Sm2Scheduler {
    fn algorithm(&self) -> Algorithm {
        Algorithm::Sm2
    }

    fn answer(&self, state: &CardState, rating: Rating, ctx: &SchedContext) -> AnswerOutcome {
        let cfg = &ctx.config;
        match state.phase {
            CardPhase::New | CardPhase::Learning => {
                learning(state, rating, ctx, &cfg.learning_steps_min, false)
            }
            CardPhase::Relearning => learning(state, rating, ctx, &cfg.relearning_steps_min, true),
            CardPhase::Review => review(state, rating, ctx),
        }
    }
}

fn clamp_interval(days: f64, cfg: &SchedConfig) -> u32 {
    days.round().clamp(
        cfg.minimum_interval_days as f64,
        cfg.maximum_interval_days as f64,
    ) as u32
}

fn graduated(state: &CardState, interval_days: u32, ctx: &SchedContext) -> AnswerOutcome {
    AnswerOutcome {
        next: CardState {
            phase: CardPhase::Review,
            steps_remaining: 0,
            interval_days,
            reps: state.reps + 1,
            last_review_day: Some(ctx.today),
            ..*state
        },
        interval: Interval::Days(interval_days),
    }
}

/// (Re)learning-step transition over `steps` (minutes).
fn learning(
    state: &CardState,
    rating: Rating,
    ctx: &SchedContext,
    steps: &[u32],
    is_relearn: bool,
) -> AnswerOutcome {
    let cfg = &ctx.config;
    let total = steps.len() as u32;
    let phase = if is_relearn {
        CardPhase::Relearning
    } else {
        CardPhase::Learning
    };

    // Interval applied when a (re)learning card graduates back to Review.
    let graduate_interval = |rating: Rating| -> u32 {
        if is_relearn {
            state.interval_days.max(cfg.minimum_interval_days)
        } else if rating == Rating::Easy {
            cfg.easy_interval_days
        } else {
            cfg.graduating_interval_days
        }
    };

    // No steps configured: graduate immediately (except Again, which still
    // shows the minimum step of 1 minute).
    if total == 0 {
        return match rating {
            Rating::Again => again_in_learning(state, phase, 1),
            _ => graduated(state, graduate_interval(rating), ctx),
        };
    }

    // Remaining steps; a New card hasn't started, so all steps remain.
    let remaining = if state.phase == CardPhase::New {
        total
    } else {
        state.steps_remaining.min(total)
    };

    match rating {
        Rating::Again => again_in_learning(state, phase, steps[0]),
        Rating::Hard => {
            let idx = (total - remaining).min(total - 1) as usize;
            AnswerOutcome {
                next: CardState {
                    phase,
                    steps_remaining: remaining.max(1),
                    ..*state
                },
                interval: Interval::Minutes(steps[idx]),
            }
        }
        Rating::Good => {
            let next_remaining = remaining.saturating_sub(1);
            if next_remaining == 0 {
                graduated(state, graduate_interval(Rating::Good), ctx)
            } else {
                let idx = (total - next_remaining) as usize;
                AnswerOutcome {
                    next: CardState {
                        phase,
                        steps_remaining: next_remaining,
                        ..*state
                    },
                    interval: Interval::Minutes(steps[idx]),
                }
            }
        }
        Rating::Easy => graduated(state, graduate_interval(Rating::Easy), ctx),
    }
}

fn again_in_learning(state: &CardState, phase: CardPhase, first_step_min: u32) -> AnswerOutcome {
    AnswerOutcome {
        next: CardState {
            phase,
            steps_remaining: state.steps_remaining.max(1),
            ..*state
        },
        interval: Interval::Minutes(first_step_min),
    }
}

fn review(state: &CardState, rating: Rating, ctx: &SchedContext) -> AnswerOutcome {
    let cfg = &ctx.config;
    let ivl = state.interval_days.max(1) as f64;
    let ease = state.ease_milli as f64 / 1000.0;
    let modifier = cfg.interval_modifier;

    match rating {
        Rating::Again => {
            // Lapse: reduce ease, shrink interval, enter relearning.
            let new_ease = state
                .ease_milli
                .saturating_sub(LAPSE_EASE_PENALTY_MILLI)
                .max(MIN_EASE_MILLI);
            let reduced = clamp_interval(ivl * cfg.lapse_interval_factor, cfg);
            let relearn_steps = &cfg.relearning_steps_min;
            let next = CardState {
                phase: if relearn_steps.is_empty() {
                    CardPhase::Review
                } else {
                    CardPhase::Relearning
                },
                steps_remaining: relearn_steps.len() as u32,
                interval_days: reduced,
                ease_milli: new_ease,
                lapses: state.lapses + 1,
                last_review_day: Some(ctx.today),
                ..*state
            };
            let interval = match relearn_steps.first() {
                Some(&minutes) => Interval::Minutes(minutes),
                None => Interval::Days(reduced),
            };
            AnswerOutcome { next, interval }
        }
        Rating::Hard => {
            let new_ease = state
                .ease_milli
                .saturating_sub(EASE_STEP_MILLI)
                .max(MIN_EASE_MILLI);
            let days = clamp_interval(ivl * cfg.hard_interval_factor * modifier, cfg);
            review_outcome(state, ctx, days, new_ease)
        }
        Rating::Good => {
            let days = clamp_interval(ivl * ease * modifier, cfg).max(state.interval_days + 1);
            review_outcome(state, ctx, days, state.ease_milli)
        }
        Rating::Easy => {
            let new_ease = state.ease_milli + EASE_STEP_MILLI;
            let days = clamp_interval(ivl * ease * cfg.easy_bonus * modifier, cfg)
                .max(state.interval_days + 1);
            review_outcome(state, ctx, days, new_ease)
        }
    }
}

fn review_outcome(
    state: &CardState,
    ctx: &SchedContext,
    days: u32,
    ease_milli: u32,
) -> AnswerOutcome {
    AnswerOutcome {
        next: CardState {
            phase: CardPhase::Review,
            interval_days: days,
            ease_milli,
            reps: state.reps + 1,
            last_review_day: Some(ctx.today),
            ..*state
        },
        interval: Interval::Days(days),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> SchedContext {
        SchedContext {
            today: 100,
            config: SchedConfig::default(),
        }
    }

    fn new_card() -> CardState {
        CardState::new(2500)
    }

    #[test]
    fn new_card_button_intervals() {
        let s = Sm2Scheduler;
        let c = ctx();
        let p = s.preview(&new_card(), &c);
        assert_eq!(p.again, Interval::Minutes(1));
        assert_eq!(p.good, Interval::Minutes(10)); // second learning step
        assert_eq!(p.easy, Interval::Days(4)); // easy graduates immediately
    }

    #[test]
    fn good_walks_steps_then_graduates() {
        let s = Sm2Scheduler;
        let c = ctx();
        // New --Good--> learning step 2 (10m)
        let o1 = s.answer(&new_card(), Rating::Good, &c);
        assert_eq!(o1.interval, Interval::Minutes(10));
        assert_eq!(o1.next.phase, CardPhase::Learning);
        // --Good--> graduate to 1 day, Review
        let o2 = s.answer(&o1.next, Rating::Good, &c);
        assert_eq!(o2.interval, Interval::Days(1));
        assert_eq!(o2.next.phase, CardPhase::Review);
        assert_eq!(o2.next.reps, 1);
    }

    #[test]
    fn review_good_multiplies_by_ease() {
        let s = Sm2Scheduler;
        let c = ctx();
        let card = CardState {
            phase: CardPhase::Review,
            interval_days: 10,
            ease_milli: 2500,
            reps: 3,
            ..new_card()
        };
        let o = s.answer(&card, Rating::Good, &c);
        assert_eq!(o.interval, Interval::Days(25)); // 10 * 2.5
                                                    // Easy > Good > Hard
        let easy = s.answer(&card, Rating::Easy, &c).interval;
        let hard = s.answer(&card, Rating::Hard, &c).interval;
        assert!(matches!(easy, Interval::Days(d) if d > 25));
        assert!(matches!(hard, Interval::Days(d) if d < 25));
    }

    #[test]
    fn review_again_lapses_into_relearning() {
        let s = Sm2Scheduler;
        let c = ctx();
        let card = CardState {
            phase: CardPhase::Review,
            interval_days: 30,
            ease_milli: 2500,
            reps: 5,
            lapses: 0,
            ..new_card()
        };
        let o = s.answer(&card, Rating::Again, &c);
        assert_eq!(o.next.phase, CardPhase::Relearning);
        assert_eq!(o.next.lapses, 1);
        assert_eq!(o.next.ease_milli, 2300); // 2500 - 200
        assert_eq!(o.interval, Interval::Minutes(10)); // first relearning step
    }

    #[test]
    fn ease_never_drops_below_floor() {
        let s = Sm2Scheduler;
        let c = ctx();
        let card = CardState {
            phase: CardPhase::Review,
            interval_days: 5,
            ease_milli: 1300,
            ..new_card()
        };
        let o = s.answer(&card, Rating::Hard, &c);
        assert_eq!(o.next.ease_milli, MIN_EASE_MILLI);
    }
}
