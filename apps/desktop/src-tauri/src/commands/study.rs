//! Study commands. The shell is where the scheduling decision (synapse-scheduler)
//! and HTML rendering (synapse-render) meet the core's queue + persistence.
//! M4 uses the default SM-2 config for every deck; per-deck config (and FSRS
//! selection) is read from deck options in a later milestone.

use synapse_core::ipc::{IpcError, IpcErrorKind, StudyCardDto};
use synapse_core::scheduling::{CardPhase, Interval, SchedConfig, SchedContext};
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

    let config = SchedConfig::default();
    let scheduler = scheduler_for(config.algorithm);
    let today = collection.today();
    let ctx = SchedContext { today, config };
    let outcome = scheduler.answer(&card.state, rating, &ctx);

    let now = collection.now_ms();
    let due = match outcome.interval {
        Interval::Days(days) => i64::from(today) + i64::from(days),
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
    collection.apply_answer(card_id, &outcome.next, due, &log)?;

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

/// Render a card and compute its four button labels for the UI.
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

    let config = SchedConfig::default();
    let scheduler = scheduler_for(config.algorithm);
    let ctx = SchedContext {
        today: collection.today(),
        config,
    };
    let previews = scheduler.preview(&card.state, &ctx);

    StudyCardDto {
        card_id: card.id,
        deck_id: card.deck_id,
        question: rendered.question,
        answer: rendered.answer,
        again: label(previews.again),
        hard: label(previews.hard),
        good: label(previews.good),
        easy: label(previews.easy),
    }
}

/// Human-readable interval label, Anki-style.
fn label(interval: Interval) -> String {
    match interval {
        Interval::Minutes(m) if m < 60 => format!("{m}m"),
        Interval::Minutes(m) => format!("{}h", m / 60),
        Interval::Days(d) if d < 30 => format!("{d}d"),
        Interval::Days(d) if d < 365 => format!("{:.1}mo", d as f64 / 30.0),
        Interval::Days(d) => format!("{:.1}y", d as f64 / 365.0),
    }
}
