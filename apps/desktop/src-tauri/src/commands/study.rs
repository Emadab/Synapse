//! Study commands. The shell is where the scheduling decision (synapse-scheduler)
//! and HTML rendering (synapse-render) meet the core's queue + persistence.

use synapse_core::ipc::{IpcError, IpcErrorKind, StudyCardDto};
use synapse_core::scheduling::{CardPhase, Interval, SchedContext};
use synapse_core::{Collection, Rating, Revlog, StudyCard};
use synapse_render::{render, RenderRequest, Template};
use synapse_scheduler::scheduler_for;
use tauri::State;

type IpcResult<T> = Result<T, IpcError>;

#[tauri::command]
pub fn get_next_card(
    collection: State<'_, Collection>,
    deck_id: i64,
) -> IpcResult<Option<StudyCardDto>> {
    collection.start_study_session(deck_id)?;
    match collection.next_card(deck_id)? {
        Some(card) => Ok(Some(present(&collection, card))),
        None => Ok(None),
    }
}

#[tauri::command]
pub fn answer_card(
    collection: State<'_, Collection>,
    card_id: i64,
    rating: u8,
) -> IpcResult<Option<StudyCardDto>> {
    let rating = rating_from(rating)?;
    let card = collection.study_card(card_id)?.ok_or_else(|| IpcError {
        kind: IpcErrorKind::NotFound,
        message: format!("card {card_id}"),
    })?;

    let config = collection
        .get_sched_config(card.deck_id)
        .unwrap_or_default();
    let leech_threshold = config.leech_threshold;
    let scheduler = scheduler_for(config.algorithm);
    let today = collection.today();
    let ctx = SchedContext { today, config };
    let outcome = scheduler.answer(&card.state, rating, &ctx);

    let now = collection.now_ms();
    let due = match outcome.interval {
        Interval::Days(days) => {
            let fuzzed = fuzz_days(days, card_id);
            i64::from(today) + i64::from(fuzzed)
        }
        Interval::Minutes(minutes) => now + i64::from(minutes) * 60_000,
    };
    let review_kind = match card.state.phase {
        CardPhase::Review => 1,
        CardPhase::Relearning => 2,
        _ => 0,
    };
    let log = Revlog {
        id: now,
        card_id,
        usn: -1,
        ease: rating as i64,
        interval: i64::from(outcome.next.interval_days),
        last_interval: i64::from(card.state.interval_days),
        ease_factor: i64::from(outcome.next.ease_milli),
        taken_ms: 0,
        review_kind,
    };
    collection.apply_answer(
        card_id,
        card.note_id,
        &outcome.next,
        due,
        &log,
        leech_threshold,
    )?;

    match collection.next_card(card.deck_id)? {
        Some(next) => Ok(Some(present(&collection, next))),
        None => Ok(None),
    }
}

fn rating_from(value: u8) -> IpcResult<Rating> {
    match value {
        1 => Ok(Rating::Again),
        2 => Ok(Rating::Hard),
        3 => Ok(Rating::Good),
        4 => Ok(Rating::Easy),
        _ => Err(IpcError {
            kind: IpcErrorKind::Invalid,
            message: format!("bad rating {value}"),
        }),
    }
}

fn present(collection: &Collection, card: StudyCard) -> StudyCardDto {
    let rendered = render(&RenderRequest {
        template: Template {
            qfmt: &card.render.qfmt,
            afmt: &card.render.afmt,
        },
        fields: &card.render.fields,
        card_ord: card.render.card_ord,
        is_cloze: card.render.is_cloze,
    });

    let config = collection
        .get_sched_config(card.deck_id)
        .unwrap_or_default();
    let algorithm = config.algorithm;
    let scheduler = scheduler_for(algorithm);
    let ctx = SchedContext {
        today: collection.today(),
        config,
    };
    let previews = scheduler.preview(&card.state, &ctx);
    let (new_count, learning_count, review_count) = collection
        .count_due_by_type(card.deck_id)
        .unwrap_or((0, 0, 0));

    let card_phase = match card.state.phase {
        CardPhase::New => "new",
        CardPhase::Learning => "learning",
        CardPhase::Review => "review",
        CardPhase::Relearning => "relearning",
    }
    .to_string();

    StudyCardDto {
        card_id: card.id,
        deck_id: card.deck_id,
        question: rendered.question,
        answer: rendered.answer,
        again: label(previews.again),
        hard: label(previews.hard),
        good: label(previews.good),
        easy: label(previews.easy),
        new_count,
        learning_count,
        review_count,
        card_phase,
        algorithm,
    }
}

/// Anki-style interval fuzz: ± a small random range seeded by card_id so that
/// cards due on the same day don't all come back at exactly the same time.
/// The state's `interval_days` is stored unfuzzed; fuzz is applied only to `due`.
fn fuzz_days(days: u32, card_id: i64) -> u32 {
    if days < 2 {
        return days;
    }
    // LCG-based deterministic fuzz seeded by card_id + days.
    let seed = (card_id.unsigned_abs())
        .wrapping_mul(1_664_525)
        .wrapping_add(days as u64)
        .wrapping_add(1_013_904_223);
    let rng = ((seed >> 16) & 0xffff) as f64 / 65535.0; // [0, 1)

    let delta = if days < 7 {
        ((days as f64 * 0.25).round() as u32).max(1)
    } else if days < 30 {
        ((days as f64 * 0.15).round() as u32).max(2)
    } else {
        ((days as f64 * 0.05).round() as u32).max(4)
    };

    let fuzz = (rng * (2 * delta + 1) as f64) as u32;
    days.saturating_sub(delta) + fuzz
}

fn label(interval: Interval) -> String {
    match interval {
        Interval::Minutes(0) => "< 1m".into(),
        Interval::Minutes(m) if m < 60 => format!("{m}m"),
        Interval::Minutes(m) if m % 60 == 0 => format!("{}h", m / 60),
        Interval::Minutes(m) => format!("{:.1}h", m as f64 / 60.0),
        Interval::Days(d) if d < 7 => format!("{d}d"),
        Interval::Days(d) if d < 30 => format!("{}w", d / 7),
        Interval::Days(d) if d < 365 => format!("{}mo", (d as f64 / 30.0).round() as u32),
        Interval::Days(d) => {
            let y = (d as f64 / 365.0 * 10.0).round() / 10.0;
            if y.fract() == 0.0 {
                format!("{}y", y as u32)
            } else {
                format!("{y:.1}y")
            }
        }
    }
}
