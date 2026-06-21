//! Anki-compatible SM-2 scheduler.
//!
//! Phases: New/Learning/Relearning step through minute-based steps, then
//! graduate to Review. Review answers scale the interval by ease (Again lapses
//! into Relearning, reducing ease by 200 and the interval by `lapse_factor`;
//! Hard −150 ease ×1.2; Good ×ease; Easy +150 ease ×ease×bonus). Ease floors at
//! 1300. Interval fuzz is applied later, at apply time.

use crate::common::{clamp_interval, step_action, StepAction};
use synapse_core::scheduling::{
    AnswerOutcome, CardPhase, CardState, Interval, SchedContext, Scheduler,
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

fn learning(
    state: &CardState,
    rating: Rating,
    ctx: &SchedContext,
    steps: &[u32],
    is_relearn: bool,
) -> AnswerOutcome {
    let cfg = &ctx.config;
    let phase = if is_relearn {
        CardPhase::Relearning
    } else {
        CardPhase::Learning
    };

    let graduate_interval = |r: Rating| -> u32 {
        if is_relearn {
            state.interval_days.max(cfg.minimum_interval_days)
        } else if r == Rating::Easy {
            cfg.easy_interval_days
        } else {
            cfg.graduating_interval_days
        }
    };

    match step_action(state, rating, steps) {
        StepAction::Restart { minutes, total } => AnswerOutcome {
            next: CardState {
                phase,
                steps_remaining: total,
                ..*state
            },
            interval: Interval::Minutes(minutes),
        },
        StepAction::Continue {
            steps_remaining,
            minutes,
        } => AnswerOutcome {
            next: CardState {
                phase,
                steps_remaining,
                ..*state
            },
            interval: Interval::Minutes(minutes),
        },
        StepAction::Graduate { rating: r } => graduated(state, graduate_interval(r), ctx),
    }
}

fn review(state: &CardState, rating: Rating, ctx: &SchedContext) -> AnswerOutcome {
    let cfg = &ctx.config;
    let ivl = state.interval_days.max(1) as f64;
    let ease = state.ease_milli as f64 / 1000.0;
    let modifier = cfg.interval_modifier;

    match rating {
        Rating::Again => {
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
            let days = clamp_interval(ivl * cfg.hard_interval_factor * modifier, cfg)
                .max(state.interval_days + 1);
            review_outcome(state, ctx, days, new_ease)
        }
        Rating::Good => {
            let hard_days = clamp_interval(ivl * cfg.hard_interval_factor * modifier, cfg)
                .max(state.interval_days + 1);
            let days = clamp_interval(ivl * ease * modifier, cfg).max(hard_days + 1);
            review_outcome(state, ctx, days, state.ease_milli)
        }
        Rating::Easy => {
            let new_ease = state.ease_milli + EASE_STEP_MILLI;
            let hard_days = clamp_interval(ivl * cfg.hard_interval_factor * modifier, cfg)
                .max(state.interval_days + 1);
            let good_days = clamp_interval(ivl * ease * modifier, cfg).max(hard_days + 1);
            let days =
                clamp_interval(ivl * ease * cfg.easy_bonus * modifier, cfg).max(good_days + 1);
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
    use synapse_core::scheduling::{Interval, SchedConfig, SchedContext};

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
        let o1 = s.answer(&new_card(), Rating::Good, &c);
        assert_eq!(o1.interval, Interval::Minutes(10));
        assert_eq!(o1.next.phase, CardPhase::Learning);
        let o2 = s.answer(&o1.next, Rating::Good, &c);
        assert_eq!(o2.interval, Interval::Days(1));
        assert_eq!(o2.next.phase, CardPhase::Review);
        assert_eq!(o2.next.reps, 1);
    }

    #[test]
    fn again_resets_all_steps() {
        let s = Sm2Scheduler;
        let c = ctx();
        // Good → step 2 (steps_remaining=1)
        let mid = s.answer(&new_card(), Rating::Good, &c).next;
        assert_eq!(mid.steps_remaining, 1);
        // Again → restart all 2 steps
        let reset = s.answer(&mid, Rating::Again, &c).next;
        assert_eq!(reset.steps_remaining, 2);
        assert_eq!(reset.phase, CardPhase::Learning);
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
        let easy = s.answer(&card, Rating::Easy, &c).interval;
        let hard = s.answer(&card, Rating::Hard, &c).interval;
        assert!(matches!(easy, Interval::Days(d) if d > 25));
        assert!(matches!(hard, Interval::Days(d) if d < 25));
    }

    #[test]
    fn review_ordering_hard_lt_good_lt_easy() {
        let s = Sm2Scheduler;
        // Use low ease (1.3) and high hard_interval_factor to stress ordering.
        let c = SchedContext {
            today: 0,
            config: SchedConfig {
                hard_interval_factor: 1.2,
                interval_modifier: 1.0,
                ..SchedConfig::default()
            },
        };
        let card = CardState {
            phase: CardPhase::Review,
            interval_days: 5,
            ease_milli: 1300,
            reps: 2,
            ..new_card()
        };
        let hard = s.answer(&card, Rating::Hard, &c).interval;
        let good = s.answer(&card, Rating::Good, &c).interval;
        let easy = s.answer(&card, Rating::Easy, &c).interval;
        let h = match hard {
            Interval::Days(d) => d,
            _ => panic!(),
        };
        let g = match good {
            Interval::Days(d) => d,
            _ => panic!(),
        };
        let e = match easy {
            Interval::Days(d) => d,
            _ => panic!(),
        };
        assert!(h < g, "hard={h} must < good={g}");
        assert!(g < e, "good={g} must < easy={e}");
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
        assert_eq!(o.next.ease_milli, 2300);
        assert_eq!(o.interval, Interval::Minutes(10));
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
