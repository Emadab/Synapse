//! The `Collection` aggregate — the application layer's entry point.
//!
//! It owns the [`Storage`] port, a [`Clock`], the [`EventBus`] and the
//! [`UndoLog`], and exposes use-cases (create/rename/remove deck, undo). Each
//! mutating use-case: validates input, calls storage, records an undo step, and
//! emits a domain event. The shell (Tauri) constructs it with a concrete
//! storage + clock and never reaches past these methods.

use std::sync::atomic::{AtomicI32, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

use crate::error::{CoreError, CoreResult};
use crate::events::{DomainEvent, EventBus, EventSink};
use crate::ipc::{
    AddNoteResult, CardRow, CollectionPrefs, DeckConfig, DeckSummary, FieldRemoveWarning,
    FilteredDeckConfig, NoteDetail, NoteOverview, NotetypeDetail, NotetypeSummary, StatsDto,
};
use crate::model::{CanonicalModel, Deck, ImportSummary, NoteIndexRow, Revlog, StudyCard};
use crate::ports::{Clock, Storage};
use crate::scheduling::CardState;
use crate::undo::UndoLog;

const MS_PER_DAY: i64 = 86_400_000;

/// Minimum time a just-answered card is kept out of resurfacing, in ms.
/// Matches the shortest default learning step, so a card never reappears
/// sub-minute purely because it was the only thing left to show.
const GRACE_MS: i64 = 60_000;

/// Proportional-merge decision for interleaving new and review cards: pick a new
/// card when its stream is no further ahead (by fraction completed) than the
/// review stream. `*_done` are today's studied counts; `*_remaining` are the
/// cards left in each stream. Callers guarantee both streams are non-empty.
fn prefer_new(new_done: u32, new_remaining: u32, rev_done: u32, rev_remaining: u32) -> bool {
    let new_total = new_done + new_remaining;
    let rev_total = rev_done + rev_remaining;
    // Fraction of each stream already completed; smaller = further behind.
    let new_frac = f64::from(new_done) / f64::from(new_total.max(1));
    let rev_frac = f64::from(rev_done) / f64::from(rev_total.max(1));
    new_frac <= rev_frac
}

pub struct Collection {
    storage: Box<dyn Storage>,
    clock: Arc<dyn Clock>,
    events: Arc<EventBus>,
    undo: Mutex<UndoLog>,
    /// Collection creation time (ms); anchors the scheduling day-number.
    created_ms: i64,
    /// Local hour (0-23) at which "today" rolls over, mirroring Anki's day
    /// cutoff. Loaded from storage at construction; updated by
    /// [`Collection::set_collection_prefs`].
    rollover_hour: AtomicU8,
    /// Frontend's local UTC offset in minutes (`-Date#getTimezoneOffset()`),
    /// applied so the rollover hour is evaluated in local, not UTC, time. Set
    /// once at startup via [`Collection::set_local_offset_minutes`]; defaults
    /// to UTC until then.
    tz_offset_minutes: AtomicI32,
}

impl Collection {
    /// Build a collection over the given storage + clock. Ensures the
    /// collection row exists and emits [`DomainEvent::CollectionOpened`].
    pub fn new(storage: Box<dyn Storage>, clock: Arc<dyn Clock>) -> Self {
        let created_ms = storage.ensure_collection(clock.now_ms()).unwrap_or(0);
        let rollover_hour = storage.get_rollover_hour().unwrap_or(4);
        let collection = Self {
            storage,
            clock,
            events: Arc::new(EventBus::new()),
            undo: Mutex::new(UndoLog::default()),
            created_ms,
            rollover_hour: AtomicU8::new(rollover_hour),
            tz_offset_minutes: AtomicI32::new(0),
        };
        collection.events.emit(DomainEvent::CollectionOpened);
        collection
    }

    /// Day index of `ms` in local time, in units of whole days since the
    /// rollover hour last passed — i.e. bucketing `ms` into rollover-aligned
    /// days rather than calendar/UTC days.
    fn local_day_index(&self, ms: i64) -> i64 {
        let tz_off_ms = i64::from(self.tz_offset_minutes.load(Ordering::Relaxed)) * 60_000;
        let rollover_ms = i64::from(self.rollover_hour.load(Ordering::Relaxed)) * 3_600_000;
        (ms + tz_off_ms - rollover_ms).div_euclid(MS_PER_DAY)
    }

    /// Today's day-number (days since collection creation), aligned to the
    /// configured local rollover hour.
    pub fn today(&self) -> i32 {
        (self.local_day_index(self.clock.now_ms()) - self.local_day_index(self.created_ms)) as i32
    }

    /// Start of today in ms (UTC) — the most recent local rollover instant.
    fn today_start_ms(&self) -> i64 {
        self.today_end_ms() - MS_PER_DAY
    }

    /// Start of tomorrow in ms (UTC) — used as the learning-card gate for today's session.
    fn today_end_ms(&self) -> i64 {
        let tz_off_ms = i64::from(self.tz_offset_minutes.load(Ordering::Relaxed)) * 60_000;
        let rollover_ms = i64::from(self.rollover_hour.load(Ordering::Relaxed)) * 3_600_000;
        let next_local_day = self.local_day_index(self.clock.now_ms()) + 1;
        next_local_day * MS_PER_DAY + rollover_ms - tz_off_ms
    }

    /// Current day-rollover preferences.
    pub fn get_collection_prefs(&self) -> CoreResult<CollectionPrefs> {
        Ok(CollectionPrefs {
            rollover_hour: self.rollover_hour.load(Ordering::Relaxed),
        })
    }

    /// Persist the day-rollover hour and apply it immediately. Since the day
    /// number is derived from the current rollover hour rather than stored
    /// per-card, changing it takes effect on the next `today()` call — same
    /// as Anki's behavior when the cutoff hour is changed.
    pub fn set_collection_prefs(&self, prefs: &CollectionPrefs) -> CoreResult<()> {
        if prefs.rollover_hour > 23 {
            return Err(CoreError::Invalid("rollover hour must be 0-23".into()));
        }
        self.storage
            .set_rollover_hour(prefs.rollover_hour, self.now_ms())?;
        self.rollover_hour
            .store(prefs.rollover_hour, Ordering::Relaxed);
        self.events.emit(DomainEvent::SchemaChanged);
        Ok(())
    }

    /// Record the frontend's local UTC offset (minutes, `-Date#getTimezoneOffset()`)
    /// so the rollover hour is evaluated in local rather than UTC time.
    /// Desktop-only; the shell calls this once at startup.
    pub fn set_local_offset_minutes(&self, minutes: i32) {
        self.tz_offset_minutes.store(minutes, Ordering::Relaxed);
    }

    /// Current wall-clock time in ms (from the injected clock).
    pub fn now_ms(&self) -> i64 {
        self.clock.now_ms()
    }

    /// Remaining daily limits `(new, review)` for `deck_id` after subtracting
    /// cards already studied today.
    fn remaining_limits(&self, deck_id: i64) -> CoreResult<(u32, u32)> {
        let deck = self
            .storage
            .deck_by_id(deck_id)?
            .ok_or_else(|| CoreError::NotFound(format!("deck {deck_id}")))?;
        let (new_per_day, rev_per_day) = self.storage.deck_limits(deck.config_id)?;
        let extra_new = self.storage.day_extra_new(deck_id, self.today())?;
        let today_start_ms = self.today_start_ms();
        let (new_studied, rev_studied) = self.storage.today_studied(deck_id, today_start_ms)?;
        Ok((
            (new_per_day + extra_new).saturating_sub(new_studied),
            rev_per_day.saturating_sub(rev_studied),
        ))
    }

    /// Decks with per-type card counts `(new, learning, review)`, capped by daily
    /// limits and rolled up to include every non-filtered subdeck (Anki
    /// behavior: a parent's badge reflects its whole subtree).
    #[allow(clippy::type_complexity)]
    pub fn list_decks_with_counts(&self) -> CoreResult<Vec<(Deck, (u32, u32, u32))>> {
        let decks = self.storage.list_decks()?;
        let raw_counts =
            self.storage
                .deck_due_counts(self.today(), self.now_ms(), self.today_end_ms())?;
        let all_limits = self.storage.all_deck_limits()?;
        let today_start_ms = self.today_start_ms();
        let studied = self.storage.all_today_studied(today_start_ms)?;
        let extra_new = self.storage.all_day_extra_new(self.today())?;

        let own_capped: std::collections::HashMap<i64, (u32, u32, u32)> = decks
            .iter()
            .map(|d| {
                let (new_raw, learning, review_raw) =
                    raw_counts.get(&d.id).copied().unwrap_or((0, 0, 0));
                let (new_per_day, rev_per_day) =
                    all_limits.get(&d.config_id).copied().unwrap_or((20, 200));
                let extra = extra_new.get(&d.id).copied().unwrap_or(0);
                let (new_studied, rev_studied) = studied.get(&d.id).copied().unwrap_or((0, 0));
                let capped = (
                    new_raw.min((new_per_day + extra).saturating_sub(new_studied)),
                    learning,
                    review_raw.min(rev_per_day.saturating_sub(rev_studied)),
                );
                (d.id, capped)
            })
            .collect();

        let parent_of: std::collections::HashMap<i64, Option<i64>> =
            decks.iter().map(|d| (d.id, d.parent_id)).collect();

        // Add every non-filtered deck's own counts into each of its ancestors.
        // Filtered decks pull cards from elsewhere and are excluded from the
        // rollup (studied only directly, never as part of a parent's subtree).
        let mut rolled = own_capped.clone();
        for d in &decks {
            if d.is_filtered {
                continue;
            }
            let (n, l, r) = own_capped.get(&d.id).copied().unwrap_or((0, 0, 0));
            let mut parent = d.parent_id;
            while let Some(pid) = parent {
                let entry = rolled.entry(pid).or_insert((0, 0, 0));
                entry.0 += n;
                entry.1 += l;
                entry.2 += r;
                parent = parent_of.get(&pid).copied().flatten();
            }
        }

        Ok(decks
            .into_iter()
            .map(|d| {
                let capped = rolled.get(&d.id).copied().unwrap_or((0, 0, 0));
                (d, capped)
            })
            .collect())
    }

    /// `deck_id` plus every non-filtered subdeck under it, or just `deck_id`
    /// alone if it is itself a filtered deck (filtered decks pull cards from
    /// elsewhere and are always studied in isolation, never as part of a
    /// parent's subtree).
    fn study_subtree(&self, deck_id: i64) -> CoreResult<Vec<i64>> {
        let deck = self
            .storage
            .deck_by_id(deck_id)?
            .ok_or_else(|| CoreError::NotFound(format!("deck {deck_id}")))?;
        if deck.is_filtered {
            return Ok(vec![deck_id]);
        }
        let filtered_ids: std::collections::HashSet<i64> = self
            .storage
            .list_decks()?
            .into_iter()
            .filter(|d| d.is_filtered)
            .map(|d| d.id)
            .collect();
        Ok(self
            .deck_and_descendants(deck_id)?
            .into_iter()
            .filter(|id| *id == deck_id || !filtered_ids.contains(id))
            .collect())
    }

    /// Count of studyable cards by type `(new, learning, review)` across
    /// `deck_id` and its subtree (respects each subdeck's own daily limits).
    pub fn count_due_by_type(&self, deck_id: i64) -> CoreResult<(u32, u32, u32)> {
        let today = self.today();
        let now_ms = self.now_ms();
        let today_end_ms = self.today_end_ms();
        let mut total = (0u32, 0u32, 0u32);
        for id in self.study_subtree(deck_id)? {
            let (new_limit, review_limit) = self.remaining_limits(id)?;
            let (n, l, r) = self.storage.count_due_by_type(
                id,
                today,
                now_ms,
                today_end_ms,
                new_limit,
                review_limit,
            )?;
            total.0 += n;
            total.1 += l;
            total.2 += r;
        }
        Ok(total)
    }

    /// Total count of studyable cards in `deck_id` and its subtree right now.
    pub fn count_due(&self, deck_id: i64) -> CoreResult<u32> {
        let (n, l, r) = self.count_due_by_type(deck_id)?;
        Ok(n + l + r)
    }

    /// The next card to study in a deck (or its subtree), if any (respects
    /// each subdeck's own daily limits).
    ///
    /// Order: learning cards due now across the subtree (time-critical,
    /// soonest first), then a proportional interleave of new and review cards
    /// so new cards surface throughout the session, then — only if nothing
    /// else is due — the soonest learn-ahead card.
    pub fn next_card(&self, deck_id: i64) -> CoreResult<Option<StudyCard>> {
        match self.next_card_id(deck_id)? {
            Some(id) => self.storage.study_card(id),
            None => Ok(None),
        }
    }

    /// Pick the id of the next card to study, applying the queue ordering
    /// policy across `deck_id`'s subtree. Each subdeck's own `study_queue`
    /// call already caps that deck's streams to its own remaining limits;
    /// this only merges those already-capped streams into one order.
    fn next_card_id(&self, deck_id: i64) -> CoreResult<Option<i64>> {
        let subtree = self.study_subtree(deck_id)?;
        let today = self.today();
        let now_ms = self.now_ms();
        let today_end_ms = self.today_end_ms();

        let mut learning: Vec<i64> = Vec::new();
        let mut new: Vec<i64> = Vec::new();
        let mut review: Vec<i64> = Vec::new();
        let mut learning_ahead: Vec<i64> = Vec::new();
        for &id in &subtree {
            let (new_limit, review_limit) = self.remaining_limits(id)?;
            let queue = self.storage.study_queue(
                id,
                today,
                now_ms,
                today_end_ms,
                new_limit,
                review_limit,
            )?;
            learning.extend(queue.learning);
            new.extend(queue.new);
            review.extend(queue.review);
            learning_ahead.extend(queue.learning_ahead);
        }

        // A just-answered card must not resurface immediately — but only while
        // some other card can be shown instead. Look up last-answered times
        // once for every candidate that could be affected (due-now learning
        // cards plus learn-ahead candidates) and treat anything answered within
        // `GRACE_MS` as deferred, not blocked.
        let grace_candidates: Vec<i64> = learning
            .iter()
            .chain(learning_ahead.iter())
            .copied()
            .collect();
        let last_answered = self.storage.cards_last_answered(&grace_candidates)?;
        let in_grace = |id: &i64| {
            last_answered
                .get(id)
                .is_some_and(|&last| now_ms - last < GRACE_MS)
        };

        // 1. Learning due now — soonest first across the whole subtree,
        //    skipping cards still in grace as long as another due-now card
        //    isn't. A single deck's queue is already sorted by due; merging
        //    several needs an explicit due lookup.
        let due_ms = if subtree.len() == 1 {
            None
        } else {
            Some(self.storage.cards_due_ms(&learning)?)
        };
        let soonest_due = |ids: &[i64]| -> Option<i64> {
            match &due_ms {
                Some(due) => ids
                    .iter()
                    .min_by_key(|id| due.get(id).copied().unwrap_or(i64::MAX))
                    .copied(),
                None => ids.first().copied(),
            }
        };
        let learning_ready: Vec<i64> = learning.iter().copied().filter(|id| !in_grace(id)).collect();
        if !learning_ready.is_empty() {
            return Ok(soonest_due(&learning_ready));
        }

        // 2. Proportional interleave of new vs review. Today's studied counts
        //    across the whole subtree pace the ratio: pick whichever stream is
        //    proportionally behind. The queue is rebuilt each call and
        //    `today_studied` advances after every answer, so the running ratio
        //    stays even without session state.
        let next_new = new.first().copied();
        let next_review = review.first().copied();
        match (next_new, next_review) {
            (Some(n), Some(r)) => {
                let today_start_ms = self.today_start_ms();
                let all_studied = self.storage.all_today_studied(today_start_ms)?;
                let (new_done, rev_done) = subtree.iter().fold((0u32, 0u32), |(nd, rd), id| {
                    let (n2, r2) = all_studied.get(id).copied().unwrap_or((0, 0));
                    (nd + n2, rd + r2)
                });
                let take_new =
                    prefer_new(new_done, new.len() as u32, rev_done, review.len() as u32);
                Ok(Some(if take_new { n } else { r }))
            }
            (Some(n), None) => Ok(Some(n)),
            (None, Some(r)) => Ok(Some(r)),
            // 3. Nothing else due — fall back to the soonest learn-ahead card
            //    (or a due-now card deferred above for being in grace),
            //    preferring one that's out of grace. If every candidate is
            //    still in grace, it's the only thing left to show, so show it
            //    anyway rather than dead-ending the session.
            (None, None) => {
                let fallback: Vec<i64> = learning_ahead
                    .iter()
                    .chain(learning.iter())
                    .copied()
                    .collect();
                if fallback.is_empty() {
                    return Ok(None);
                }
                let due = self.storage.cards_due_ms(&fallback)?;
                let pick_soonest = |ids: &[i64]| -> Option<i64> {
                    ids.iter()
                        .copied()
                        .min_by_key(|id| due.get(id).copied().unwrap_or(i64::MAX))
                };
                let ready: Vec<i64> = fallback.iter().copied().filter(|id| !in_grace(id)).collect();
                Ok(pick_soonest(&ready).or_else(|| pick_soonest(&fallback)))
            }
        }
    }

    /// Current `(new_per_day, review_per_day)` limit for a deck.
    pub fn get_deck_options(&self, deck_id: i64) -> CoreResult<(u32, u32)> {
        let deck = self
            .storage
            .deck_by_id(deck_id)?
            .ok_or_else(|| CoreError::NotFound(format!("deck {deck_id}")))?;
        self.storage.deck_limits(deck.config_id)
    }

    /// Persist updated daily limits for a deck.
    pub fn set_deck_options(
        &self,
        deck_id: i64,
        new_per_day: u32,
        rev_per_day: u32,
    ) -> CoreResult<()> {
        let deck = self
            .storage
            .deck_by_id(deck_id)?
            .ok_or_else(|| CoreError::NotFound(format!("deck {deck_id}")))?;
        self.storage
            .set_deck_limits(deck.config_id, new_per_day, rev_per_day, self.now_ms())
    }

    /// Current extra new-card allowance for `deck_id` for today, if any.
    pub fn get_today_extra_new(&self, deck_id: i64) -> CoreResult<u32> {
        self.storage.day_extra_new(deck_id, self.today())
    }

    /// Temporarily raise today's new-card limit for `deck_id` by `extra_new`
    /// cards, stacking on top of any earlier increase made today. Only
    /// applies for today (`Collection::today()`); it lapses on its own at the
    /// next day rollover since the override is keyed by day number.
    pub fn increase_today_new_limit(&self, deck_id: i64, extra_new: u32) -> CoreResult<()> {
        self.storage
            .deck_by_id(deck_id)?
            .ok_or_else(|| CoreError::NotFound(format!("deck {deck_id}")))?;
        let today = self.today();
        let current = self.storage.day_extra_new(deck_id, today)?;
        self.storage
            .set_day_extra_new(deck_id, today, current + extra_new)?;
        self.events.emit(DomainEvent::DeckChanged { deck_id });
        Ok(())
    }

    /// Full scheduling config for a deck, for the options dialog (M14).
    pub fn get_deck_config(&self, deck_id: i64) -> CoreResult<DeckConfig> {
        let deck = self
            .storage
            .deck_by_id(deck_id)?
            .ok_or_else(|| CoreError::NotFound(format!("deck {deck_id}")))?;
        let s = self.storage.get_deck_config(deck.config_id)?;
        Ok(DeckConfig {
            deck_id,
            config_id: deck.config_id,
            algorithm: s.algorithm,
            new_per_day: s.new_per_day,
            review_per_day: s.review_per_day,
            learning_steps_min: s.learning_steps_min,
            graduating_interval_days: s.graduating_interval_days,
            easy_interval_days: s.easy_interval_days,
            starting_ease_milli: s.starting_ease_milli,
            easy_bonus: s.easy_bonus,
            hard_interval_factor: s.hard_interval_factor,
            interval_modifier: s.interval_modifier,
            maximum_interval_days: s.maximum_interval_days,
            relearning_steps_min: s.relearning_steps_min,
            lapse_interval_factor: s.lapse_interval_factor,
            minimum_interval_days: s.minimum_interval_days,
            leech_threshold: s.leech_threshold,
            fsrs_weights: s.fsrs_weights.to_vec(),
            desired_retention: s.desired_retention,
        })
    }

    /// Persist full scheduling config for a deck. Validates bounds.
    pub fn set_deck_config(&self, cfg: &DeckConfig) -> CoreResult<()> {
        if cfg.starting_ease_milli < 1300 || cfg.starting_ease_milli > 9999 {
            return Err(CoreError::Invalid("starting ease must be 1300–9999".into()));
        }
        if cfg.desired_retention < 0.5 || cfg.desired_retention > 0.99 {
            return Err(CoreError::Invalid(
                "desired retention must be 0.50–0.99".into(),
            ));
        }
        if cfg.interval_modifier < 0.01 || cfg.interval_modifier > 9.99 {
            return Err(CoreError::Invalid(
                "interval modifier must be 0.01–9.99".into(),
            ));
        }
        if cfg.graduating_interval_days < 1 {
            return Err(CoreError::Invalid("graduating interval must be ≥ 1".into()));
        }
        if cfg.maximum_interval_days < 1 {
            return Err(CoreError::Invalid("max interval must be ≥ 1".into()));
        }
        if cfg.fsrs_weights.len() != 21 {
            return Err(CoreError::Invalid(
                "FSRS weights must have exactly 21 elements".into(),
            ));
        }
        let deck = self
            .storage
            .deck_by_id(cfg.deck_id)?
            .ok_or_else(|| CoreError::NotFound(format!("deck {}", cfg.deck_id)))?;
        let mut arr = crate::scheduling::FSRS6_DEFAULT_WEIGHTS;
        for (i, &w) in cfg.fsrs_weights.iter().enumerate().take(21) {
            arr[i] = w;
        }
        let sched = crate::scheduling::SchedConfig {
            algorithm: cfg.algorithm,
            new_per_day: cfg.new_per_day,
            review_per_day: cfg.review_per_day,
            learning_steps_min: cfg.learning_steps_min.clone(),
            relearning_steps_min: cfg.relearning_steps_min.clone(),
            graduating_interval_days: cfg.graduating_interval_days,
            easy_interval_days: cfg.easy_interval_days,
            starting_ease_milli: cfg.starting_ease_milli,
            easy_bonus: cfg.easy_bonus,
            hard_interval_factor: cfg.hard_interval_factor,
            interval_modifier: cfg.interval_modifier,
            lapse_interval_factor: cfg.lapse_interval_factor,
            minimum_interval_days: cfg.minimum_interval_days,
            maximum_interval_days: cfg.maximum_interval_days,
            leech_threshold: cfg.leech_threshold,
            fsrs_weights: arr,
            desired_retention: cfg.desired_retention,
        };
        self.storage
            .set_deck_config(deck.config_id, &sched, self.now_ms())?;
        self.events.emit(DomainEvent::DeckChanged {
            deck_id: cfg.deck_id,
        });
        Ok(())
    }

    /// `SchedConfig` for a deck — used by the study commands for per-deck scheduling.
    pub fn get_sched_config(&self, deck_id: i64) -> CoreResult<crate::scheduling::SchedConfig> {
        let deck = self
            .storage
            .deck_by_id(deck_id)?
            .ok_or_else(|| CoreError::NotFound(format!("deck {deck_id}")))?;
        self.storage.get_deck_config(deck.config_id)
    }

    /// Unbury all buried cards in `deck_id` and its subtree (day rollover /
    /// session start).
    pub fn start_study_session(&self, deck_id: i64) -> CoreResult<()> {
        for id in self.study_subtree(deck_id)? {
            self.storage.unbury_deck(id)?;
        }
        Ok(())
    }

    /// Render inputs + scheduling state for a specific card.
    pub fn study_card(&self, card_id: i64) -> CoreResult<Option<StudyCard>> {
        self.storage.study_card(card_id)
    }

    /// Notes for the browser, optionally filtered by a substring.
    pub fn list_notes(&self, query: Option<&str>) -> CoreResult<Vec<NoteOverview>> {
        self.storage.list_notes(query, 1000)
    }

    /// Full note for the editor.
    pub fn note_detail(&self, note_id: i64) -> CoreResult<Option<NoteDetail>> {
        self.storage.note_detail(note_id)
    }

    /// Save edited note field values + tags.
    pub fn update_note(&self, note_id: i64, fields: &[String], tags: &[String]) -> CoreResult<()> {
        self.storage
            .update_note(note_id, fields, tags, self.clock.now_ms())?;
        self.events.emit(DomainEvent::NoteUpdated { note_id });
        Ok(())
    }

    /// Aggregate statistics for the dashboards. `deck_id` selects a deck plus
    /// all of its subdecks (rollup via `parent_id`), or `None` for the whole
    /// collection. `days` restricts range-scoped aggregates, or `None` for all
    /// time. `tz_offset_minutes` (`-Date#getTimezoneOffset()`) shifts only the
    /// hourly breakdown into local time.
    pub fn stats(
        &self,
        deck_id: Option<i64>,
        days: Option<u32>,
        tz_offset_minutes: i32,
    ) -> CoreResult<StatsDto> {
        let deck_ids = match deck_id {
            Some(root) => Some(self.deck_and_descendants(root)?),
            None => None,
        };
        let (fsrs_weights, retention_goal_pct) = match deck_id {
            Some(id) => {
                let cfg = self.get_sched_config(id)?;
                (cfg.fsrs_weights, cfg.desired_retention * 100.0)
            }
            None => (crate::scheduling::FSRS6_DEFAULT_WEIGHTS, 90.0),
        };
        self.storage.stats(
            deck_ids.as_deref(),
            days,
            tz_offset_minutes,
            self.rollover_hour.load(Ordering::Relaxed),
            &fsrs_weights,
            retention_goal_pct,
            self.today(),
            self.clock.now_ms(),
            self.created_ms,
        )
    }

    /// `root` plus every deck nested under it (transitively), via `parent_id`.
    fn deck_and_descendants(&self, root: i64) -> CoreResult<Vec<i64>> {
        let decks = self.storage.list_decks()?;
        let mut children: std::collections::HashMap<i64, Vec<i64>> =
            std::collections::HashMap::new();
        for d in &decks {
            if let Some(p) = d.parent_id {
                children.entry(p).or_default().push(d.id);
            }
        }
        let mut ids = vec![root];
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            if let Some(kids) = children.get(&id) {
                for &k in kids {
                    ids.push(k);
                    stack.push(k);
                }
            }
        }
        Ok(ids)
    }

    /// All notes flattened for (re)building the search index.
    pub fn index_rows(&self) -> CoreResult<Vec<NoteIndexRow>> {
        self.storage.index_rows()
    }

    /// Anki-flavoured query → card rows (M16). `offset` supports pagination.
    pub fn search_cards(&self, query: &str, limit: i64, offset: i64) -> CoreResult<Vec<CardRow>> {
        self.storage
            .search_cards(query, self.today(), self.clock.now_ms(), limit, offset)
    }

    /// Delete notes (and their cards + revlogs). Emits SchemaChanged.
    pub fn delete_notes(&self, note_ids: &[i64]) -> CoreResult<()> {
        self.storage.delete_notes(note_ids, self.clock.now_ms())?;
        self.events.emit(DomainEvent::SchemaChanged);
        Ok(())
    }

    /// Reassign cards to another deck. Emits DeckChanged for the target deck.
    pub fn move_cards_to_deck(&self, card_ids: &[i64], deck_id: i64) -> CoreResult<()> {
        self.storage.move_cards_to_deck(card_ids, deck_id)?;
        self.events.emit(DomainEvent::DeckChanged { deck_id });
        Ok(())
    }

    /// Add a tag to a single note (idempotent). Emits NoteUpdated.
    pub fn add_note_tag_single(&self, note_id: i64, tag: &str) -> CoreResult<()> {
        self.storage
            .add_note_tag(note_id, tag, self.clock.now_ms())?;
        self.events.emit(DomainEvent::NoteUpdated { note_id });
        Ok(())
    }

    /// Remove a tag from a single note. Emits NoteUpdated.
    pub fn remove_note_tag_single(&self, note_id: i64, tag: &str) -> CoreResult<()> {
        self.storage
            .remove_note_tag(note_id, tag, self.clock.now_ms())?;
        self.events.emit(DomainEvent::NoteUpdated { note_id });
        Ok(())
    }

    /// Bulk: add a tag to many notes.
    pub fn bulk_add_tag(&self, note_ids: &[i64], tag: &str) -> CoreResult<()> {
        let now = self.clock.now_ms();
        for &id in note_ids {
            self.storage.add_note_tag(id, tag, now)?;
            self.events.emit(DomainEvent::NoteUpdated { note_id: id });
        }
        Ok(())
    }

    /// Bulk: remove a tag from many notes.
    pub fn bulk_remove_tag(&self, note_ids: &[i64], tag: &str) -> CoreResult<()> {
        let now = self.clock.now_ms();
        for &id in note_ids {
            self.storage.remove_note_tag(id, tag, now)?;
            self.events.emit(DomainEvent::NoteUpdated { note_id: id });
        }
        Ok(())
    }

    /// Dump the full collection for export (`.apkg`/`.colpkg`).
    pub fn dump_collection(&self) -> CoreResult<CanonicalModel> {
        self.storage.dump_collection()
    }

    /// Browser rows for a set of note ids (search hits).
    pub fn notes_by_ids(&self, ids: &[i64]) -> CoreResult<Vec<NoteOverview>> {
        self.storage.notes_by_ids(ids)
    }

    /// Persist an answered card's new state + review log.
    /// Also buries siblings and, if lapses crossed the leech threshold, tags the
    /// note with "leech" and suspends the card. Returns `true` if a leech fired.
    pub fn apply_answer(
        &self,
        card_id: i64,
        note_id: i64,
        next: &CardState,
        due: i64,
        log: &Revlog,
        leech_threshold: u32,
    ) -> CoreResult<bool> {
        self.storage.apply_answer(card_id, next, due, log)?;
        // Sibling bury: keep siblings off the queue for the rest of the session.
        self.storage.bury_siblings(note_id, card_id)?;
        // Leech detection.
        let now_ms = self.clock.now_ms();
        let is_leech =
            leech_threshold > 0 && next.lapses > 0 && next.lapses.is_multiple_of(leech_threshold);
        if is_leech {
            self.storage.add_note_tag(note_id, "leech", now_ms)?;
            self.storage.suspend_cards(&[card_id])?;
            self.events.emit(DomainEvent::NoteUpdated { note_id });
        }
        self.events.emit(DomainEvent::CardAnswered { card_id });
        Ok(is_leech)
    }

    /// Suspend cards (sets `queue = -1`).
    pub fn suspend_cards(&self, card_ids: &[i64]) -> CoreResult<()> {
        self.storage.suspend_cards(card_ids)?;
        for &id in card_ids {
            self.events.emit(DomainEvent::CardAnswered { card_id: id });
        }
        Ok(())
    }

    /// Unsuspend cards (restores `queue = type`).
    pub fn unsuspend_cards(&self, card_ids: &[i64]) -> CoreResult<()> {
        self.storage.unsuspend_cards(card_ids)?;
        Ok(())
    }

    /// Manually bury cards (sets `queue = -2`).
    pub fn bury_cards(&self, card_ids: &[i64]) -> CoreResult<()> {
        self.storage.bury_cards(card_ids)?;
        for &id in card_ids {
            self.events.emit(DomainEvent::CardAnswered { card_id: id });
        }
        Ok(())
    }

    /// Set the flag (0–7) on a list of cards.
    pub fn set_card_flag(&self, card_ids: &[i64], flag: u8) -> CoreResult<()> {
        self.storage.set_card_flag(card_ids, flag)?;
        Ok(())
    }

    // ── M17: tag manager ─────────────────────────────────────────────────────

    pub fn list_tags(&self) -> CoreResult<Vec<String>> {
        self.storage.list_tags()
    }

    pub fn rename_tag(&self, old_tag: &str, new_tag: &str) -> CoreResult<u32> {
        let n = self
            .storage
            .rename_tag(old_tag, new_tag, self.clock.now_ms())?;
        self.events.emit(DomainEvent::SchemaChanged);
        Ok(n)
    }

    pub fn delete_tag(&self, tag: &str) -> CoreResult<u32> {
        let n = self.storage.delete_tag(tag, self.clock.now_ms())?;
        self.events.emit(DomainEvent::SchemaChanged);
        Ok(n)
    }

    pub fn merge_tags(&self, sources: Vec<String>, target: &str) -> CoreResult<()> {
        self.storage
            .merge_tags(&sources, target, self.clock.now_ms())?;
        self.events.emit(DomainEvent::SchemaChanged);
        Ok(())
    }

    // ── M17: filtered decks ───────────────────────────────────────────────────

    pub fn create_filtered_deck(
        &self,
        name: &str,
        search: &str,
        order: u8,
        limit: u32,
    ) -> CoreResult<DeckSummary> {
        let name = name.trim();
        if name.is_empty() {
            return Err(CoreError::Invalid("deck name is empty".into()));
        }
        let deck = self.storage.create_filtered_deck(
            name,
            search,
            order,
            limit,
            self.today(),
            self.clock.now_ms(),
        )?;
        let id = deck.id;
        self.events.emit(DomainEvent::DeckChanged { deck_id: id });
        Ok(DeckSummary::from(deck))
    }

    pub fn rebuild_filtered(&self, deck_id: i64) -> CoreResult<u32> {
        let n = self
            .storage
            .rebuild_filtered(deck_id, self.today(), self.clock.now_ms())?;
        self.events.emit(DomainEvent::DeckChanged { deck_id });
        Ok(n)
    }

    pub fn empty_filtered(&self, deck_id: i64) -> CoreResult<()> {
        self.storage.empty_filtered(deck_id, self.clock.now_ms())?;
        self.events.emit(DomainEvent::DeckChanged { deck_id });
        Ok(())
    }

    pub fn get_filtered_config(&self, deck_id: i64) -> CoreResult<Option<FilteredDeckConfig>> {
        self.storage.get_filtered_config(deck_id)
    }

    pub fn integrity_check(&self) -> CoreResult<Vec<String>> {
        self.storage.integrity_check()
    }

    pub fn optimize(&self) -> CoreResult<()> {
        self.storage.optimize()
    }

    pub fn note_media_refs(&self) -> CoreResult<Vec<String>> {
        self.storage.note_media_refs()
    }

    pub fn backup_db(&self, dest_path: &std::path::Path) -> CoreResult<()> {
        self.storage.backup_db(dest_path)
    }

    pub fn revlogs_for_optimize(
        &self,
        deck_id: Option<i64>,
    ) -> CoreResult<Vec<crate::model::Revlog>> {
        self.storage.revlogs_for_optimize(deck_id)
    }

    /// Shared handle to the event bus, for wiring external subscribers
    /// (e.g. the Tauri → webview bridge).
    pub fn events(&self) -> Arc<EventBus> {
        self.events.clone()
    }

    pub fn schema_version(&self) -> CoreResult<i64> {
        self.storage.schema_version()
    }

    pub fn list_decks(&self) -> CoreResult<Vec<Deck>> {
        self.storage.list_decks()
    }

    pub fn create_deck(&self, name: &str) -> CoreResult<Deck> {
        let name = name.trim();
        if name.is_empty() {
            return Err(CoreError::Invalid("deck name is empty".into()));
        }
        if self.storage.deck_by_name(name)?.is_some() {
            return Err(CoreError::Invalid(format!(
                "a deck named \"{name}\" already exists"
            )));
        }
        let deck = self.storage.create_deck(name, self.clock.now_ms())?;
        let id = deck.id;
        self.record_undo(format!("Create deck \"{name}\""), move |s, _now| {
            s.remove_deck(id).map(|_| ())
        });
        self.events.emit(DomainEvent::DeckChanged { deck_id: id });
        Ok(deck)
    }

    pub fn rename_deck(&self, id: i64, name: &str) -> CoreResult<()> {
        let name = name.trim();
        if name.is_empty() {
            return Err(CoreError::Invalid("deck name is empty".into()));
        }
        let old = self
            .storage
            .deck_by_id(id)?
            .ok_or_else(|| CoreError::NotFound(format!("deck {id}")))?;
        self.storage.rename_deck(id, name, self.clock.now_ms())?;
        let old_name = old.name;
        self.record_undo(format!("Rename deck to \"{name}\""), move |s, now| {
            s.rename_deck(id, &old_name, now)
        });
        self.events.emit(DomainEvent::DeckChanged { deck_id: id });
        Ok(())
    }

    pub fn remove_deck(&self, id: i64) -> CoreResult<()> {
        let deck = self
            .storage
            .deck_by_id(id)?
            .ok_or_else(|| CoreError::NotFound(format!("deck {id}")))?;
        // Return gathered cards before deleting a filtered deck.
        if deck.is_filtered {
            self.storage.empty_filtered(id, self.clock.now_ms())?;
        }
        // Cascades to sub-decks + their cards; the returned snapshot drives undo.
        let removed = self.storage.remove_deck(id)?;
        let description = format!("Delete deck \"{}\"", deck.name);
        self.record_undo(description, move |s, _now| s.restore_deck(&removed));
        self.events.emit(DomainEvent::DeckChanged { deck_id: id });
        Ok(())
    }

    /// Merge a parsed package (from `synapse-ankifmt`) into this collection.
    /// Import is not undoable via the per-op log (it is bulk and transactional);
    /// the pre-import backup is the recovery path (added in a later milestone).
    pub fn import(&self, model: &CanonicalModel) -> CoreResult<ImportSummary> {
        let summary = self.storage.import(model)?;
        self.events.emit(DomainEvent::SchemaChanged);
        Ok(summary)
    }

    /// Same as [`Collection::import`], but reports progress via `on_progress`
    /// as notes/cards are merged — for surfacing a live indicator on large imports.
    pub fn import_with_progress(
        &self,
        model: &CanonicalModel,
        on_progress: &mut dyn FnMut(u32, u32),
    ) -> CoreResult<ImportSummary> {
        let summary = self.storage.import_with_progress(model, on_progress)?;
        self.events.emit(DomainEvent::SchemaChanged);
        Ok(summary)
    }

    /// All note types with their ordered field names, for the Add Note picker.
    pub fn list_notetypes(&self) -> CoreResult<Vec<NotetypeSummary>> {
        let notetypes = self.storage.list_notetypes()?;
        let mut result = Vec::with_capacity(notetypes.len());
        for nt in notetypes {
            let fields = self.storage.fields_for_notetype(nt.id)?;
            result.push(NotetypeSummary {
                id: nt.id,
                name: nt.name,
                kind: nt.kind,
                field_names: fields.into_iter().map(|f| f.name).collect(),
            });
        }
        Ok(result)
    }

    /// Add a note to `deck_id`, generating cards from the notetype's templates.
    /// Returns the result DTO (`note_id` + `cards_added`).
    pub fn add_note(
        &self,
        notetype_id: i64,
        deck_id: i64,
        fields: &[String],
        tags: &[String],
    ) -> CoreResult<AddNoteResult> {
        self.storage
            .deck_by_id(deck_id)?
            .ok_or_else(|| CoreError::NotFound(format!("deck {deck_id}")))?;
        let (note_id, cards_added) = self.storage.add_note_with_cards(
            notetype_id,
            deck_id,
            fields,
            tags,
            self.clock.now_ms(),
        )?;
        self.events.emit(DomainEvent::NoteAdded { note_id });
        Ok(AddNoteResult {
            note_id,
            cards_added,
        })
    }

    // ── Note-type editor ──────────────────────────────────────────────────────

    pub fn get_notetype_detail(&self, notetype_id: i64) -> CoreResult<Option<NotetypeDetail>> {
        self.storage.get_notetype_detail(notetype_id)
    }

    pub fn create_notetype(&self, name: &str, kind: i64) -> CoreResult<NotetypeDetail> {
        let name = name.trim();
        if name.is_empty() {
            return Err(CoreError::Invalid("note type name is empty".into()));
        }
        let id = self
            .storage
            .create_notetype(name, kind, self.clock.now_ms())?;
        self.events.emit(DomainEvent::SchemaChanged);
        self.storage
            .get_notetype_detail(id)?
            .ok_or_else(|| CoreError::NotFound(format!("notetype {id}")))
    }

    pub fn delete_notetype(&self, notetype_id: i64) -> CoreResult<()> {
        self.storage
            .delete_notetype(notetype_id, self.clock.now_ms())?;
        self.events.emit(DomainEvent::SchemaChanged);
        Ok(())
    }

    pub fn rename_notetype(&self, notetype_id: i64, name: &str) -> CoreResult<()> {
        let name = name.trim();
        if name.is_empty() {
            return Err(CoreError::Invalid("note type name is empty".into()));
        }
        self.storage
            .rename_notetype(notetype_id, name, self.clock.now_ms())?;
        self.events.emit(DomainEvent::SchemaChanged);
        Ok(())
    }

    pub fn stock_notetype_names(&self) -> Vec<&'static str> {
        self.storage.stock_notetype_names()
    }

    pub fn add_stock_notetype(&self, index: usize) -> CoreResult<NotetypeDetail> {
        let id = self
            .storage
            .add_stock_notetype(index, self.clock.now_ms())?;
        self.events.emit(DomainEvent::SchemaChanged);
        self.storage
            .get_notetype_detail(id)?
            .ok_or_else(|| CoreError::NotFound(format!("notetype {id}")))
    }

    pub fn save_notetype_css(&self, notetype_id: i64, css: &str) -> CoreResult<()> {
        self.storage
            .save_notetype_css(notetype_id, css, self.clock.now_ms())?;
        self.events.emit(DomainEvent::SchemaChanged);
        Ok(())
    }

    pub fn add_field(&self, notetype_id: i64, name: &str) -> CoreResult<()> {
        let name = name.trim();
        if name.is_empty() {
            return Err(CoreError::Invalid("field name is empty".into()));
        }
        self.storage
            .add_field(notetype_id, name, self.clock.now_ms())?;
        self.events.emit(DomainEvent::SchemaChanged);
        Ok(())
    }

    pub fn check_field_remove(&self, notetype_id: i64, ord: i64) -> CoreResult<FieldRemoveWarning> {
        self.storage.check_field_remove(notetype_id, ord)
    }

    pub fn remove_field(&self, notetype_id: i64, ord: i64) -> CoreResult<()> {
        self.storage
            .remove_field(notetype_id, ord, self.clock.now_ms())?;
        self.events.emit(DomainEvent::SchemaChanged);
        Ok(())
    }

    pub fn rename_field(&self, notetype_id: i64, ord: i64, name: &str) -> CoreResult<()> {
        let name = name.trim();
        if name.is_empty() {
            return Err(CoreError::Invalid("field name is empty".into()));
        }
        self.storage
            .rename_field(notetype_id, ord, name, self.clock.now_ms())?;
        self.events.emit(DomainEvent::SchemaChanged);
        Ok(())
    }

    pub fn reorder_fields(&self, notetype_id: i64, new_order: Vec<i64>) -> CoreResult<()> {
        self.storage
            .reorder_fields(notetype_id, &new_order, self.clock.now_ms())?;
        self.events.emit(DomainEvent::SchemaChanged);
        Ok(())
    }

    pub fn add_template(
        &self,
        notetype_id: i64,
        name: &str,
        qfmt: &str,
        afmt: &str,
    ) -> CoreResult<()> {
        self.storage
            .add_template(notetype_id, name, qfmt, afmt, self.clock.now_ms())?;
        self.events.emit(DomainEvent::SchemaChanged);
        Ok(())
    }

    pub fn remove_template(&self, notetype_id: i64, ord: i64) -> CoreResult<()> {
        self.storage
            .remove_template(notetype_id, ord, self.clock.now_ms())?;
        self.events.emit(DomainEvent::SchemaChanged);
        Ok(())
    }

    pub fn save_template(
        &self,
        notetype_id: i64,
        ord: i64,
        name: &str,
        qfmt: &str,
        afmt: &str,
    ) -> CoreResult<()> {
        self.storage
            .save_template(notetype_id, ord, name, qfmt, afmt, self.clock.now_ms())?;
        self.events.emit(DomainEvent::SchemaChanged);
        Ok(())
    }

    /// Description of the next undoable operation, if any.
    pub fn undo_status(&self) -> Option<String> {
        self.undo.lock().unwrap().peek().map(str::to_owned)
    }

    /// Undo the most recent operation; returns its description.
    pub fn undo(&self) -> CoreResult<Option<String>> {
        let step = self.undo.lock().unwrap().pop();
        match step {
            None => Ok(None),
            Some(step) => {
                let description = step.description.clone();
                step.run(self.storage.as_ref(), self.clock.now_ms())?;
                self.events.emit(DomainEvent::SchemaChanged);
                Ok(Some(description))
            }
        }
    }

    fn record_undo(
        &self,
        description: impl Into<String>,
        action: impl FnOnce(&dyn Storage, i64) -> CoreResult<()> + Send + 'static,
    ) {
        self.undo
            .lock()
            .unwrap()
            .record(description, Box::new(action));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Field, Notetype, Template};
    use crate::ports::FixedClock;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Minimal in-memory `Storage` fake so the application layer can be tested
    /// without `synapse-db`. (synapse-db has its own SQLite-backed tests.)
    ///
    /// `queues`/`due_ms`/`due_counts` are test fixtures: unset entries fall
    /// back to empty/zero, matching the original all-stub behavior, so
    /// existing tests that never populate them are unaffected.
    #[derive(Default)]
    struct FakeStorage {
        decks: Mutex<Vec<Deck>>,
        next_id: AtomicUsize,
        queues: Mutex<std::collections::HashMap<i64, crate::ports::StudyQueue>>,
        due_ms: Mutex<std::collections::HashMap<i64, i64>>,
        due_counts: Mutex<std::collections::HashMap<i64, (u32, u32, u32)>>,
        last_answered: Mutex<std::collections::HashMap<i64, i64>>,
    }

    impl FakeStorage {
        fn new_id(&self) -> i64 {
            self.next_id.fetch_add(1, Ordering::SeqCst) as i64 + 1
        }
    }

    impl Storage for FakeStorage {
        fn schema_version(&self) -> CoreResult<i64> {
            Ok(1)
        }
        fn create_deck(&self, name: &str, now_ms: i64) -> CoreResult<Deck> {
            let deck = Deck {
                id: self.new_id(),
                name: name.to_string(),
                parent_id: None,
                config_id: self.new_id(),
                mod_ms: now_ms,
                usn: -1,
                collapsed: false,
                is_filtered: false,
            };
            self.decks.lock().unwrap().push(deck.clone());
            Ok(deck)
        }
        fn deck_by_id(&self, id: i64) -> CoreResult<Option<Deck>> {
            Ok(self
                .decks
                .lock()
                .unwrap()
                .iter()
                .find(|d| d.id == id)
                .cloned())
        }
        fn deck_by_name(&self, name: &str) -> CoreResult<Option<Deck>> {
            Ok(self
                .decks
                .lock()
                .unwrap()
                .iter()
                .find(|d| d.name == name)
                .cloned())
        }
        fn list_decks(&self) -> CoreResult<Vec<Deck>> {
            Ok(self.decks.lock().unwrap().clone())
        }
        fn rename_deck(&self, id: i64, name: &str, now_ms: i64) -> CoreResult<()> {
            let mut decks = self.decks.lock().unwrap();
            let deck = decks
                .iter_mut()
                .find(|d| d.id == id)
                .ok_or(CoreError::NotFound("deck".into()))?;
            deck.name = name.to_string();
            deck.mod_ms = now_ms;
            Ok(())
        }
        fn remove_deck(&self, id: i64) -> CoreResult<crate::ports::RemovedDeck> {
            let mut decks = self.decks.lock().unwrap();
            let removed: Vec<Deck> = decks.iter().filter(|d| d.id == id).cloned().collect();
            decks.retain(|d| d.id != id);
            Ok(crate::ports::RemovedDeck {
                decks: removed,
                cards: vec![],
                notes: vec![],
            })
        }
        fn restore_deck(&self, removed: &crate::ports::RemovedDeck) -> CoreResult<()> {
            self.decks
                .lock()
                .unwrap()
                .extend(removed.decks.iter().cloned());
            Ok(())
        }
        fn insert_deck(&self, deck: &Deck) -> CoreResult<()> {
            self.decks.lock().unwrap().push(deck.clone());
            Ok(())
        }
        fn import(&self, model: &CanonicalModel) -> CoreResult<ImportSummary> {
            let mut summary = ImportSummary::default();
            for deck in &model.decks {
                if self.deck_by_name(&deck.name)?.is_none() {
                    self.create_deck(&deck.name, deck.mod_ms)?;
                    summary.decks_added += 1;
                }
            }
            summary.notes_added = model.notes.len() as u32;
            Ok(summary)
        }
        fn ensure_collection(&self, _now_ms: i64) -> CoreResult<i64> {
            Ok(0)
        }
        fn study_queue(
            &self,
            deck_id: i64,
            _today: i32,
            _now_ms: i64,
            _today_end_ms: i64,
            _new_limit: u32,
            _review_limit: u32,
        ) -> CoreResult<crate::ports::StudyQueue> {
            Ok(self
                .queues
                .lock()
                .unwrap()
                .get(&deck_id)
                .cloned()
                .unwrap_or_default())
        }
        fn count_due_by_type(
            &self,
            deck_id: i64,
            _today: i32,
            _now_ms: i64,
            _today_end_ms: i64,
            _new_limit: u32,
            _review_limit: u32,
        ) -> CoreResult<(u32, u32, u32)> {
            Ok(self
                .due_counts
                .lock()
                .unwrap()
                .get(&deck_id)
                .copied()
                .unwrap_or((0, 0, 0)))
        }
        fn count_due(
            &self,
            deck_id: i64,
            today: i32,
            now_ms: i64,
            today_end_ms: i64,
            new_limit: u32,
            review_limit: u32,
        ) -> CoreResult<u32> {
            let (n, l, r) = self.count_due_by_type(
                deck_id,
                today,
                now_ms,
                today_end_ms,
                new_limit,
                review_limit,
            )?;
            Ok(n + l + r)
        }
        fn deck_due_counts(
            &self,
            _today: i32,
            _now_ms: i64,
            _today_end_ms: i64,
        ) -> CoreResult<std::collections::HashMap<i64, (u32, u32, u32)>> {
            Ok(self.due_counts.lock().unwrap().clone())
        }
        fn cards_due_ms(
            &self,
            card_ids: &[i64],
        ) -> CoreResult<std::collections::HashMap<i64, i64>> {
            let due = self.due_ms.lock().unwrap();
            Ok(card_ids
                .iter()
                .filter_map(|id| due.get(id).map(|&ms| (*id, ms)))
                .collect())
        }
        fn cards_last_answered(
            &self,
            card_ids: &[i64],
        ) -> CoreResult<std::collections::HashMap<i64, i64>> {
            let last = self.last_answered.lock().unwrap();
            Ok(card_ids
                .iter()
                .filter_map(|id| last.get(id).map(|&ms| (*id, ms)))
                .collect())
        }
        fn deck_limits(&self, _config_id: i64) -> CoreResult<(u32, u32)> {
            Ok((20, 200))
        }
        fn all_deck_limits(&self) -> CoreResult<std::collections::HashMap<i64, (u32, u32)>> {
            Ok(std::collections::HashMap::new())
        }
        fn today_studied(&self, _deck_id: i64, _today_start_ms: i64) -> CoreResult<(u32, u32)> {
            Ok((0, 0))
        }
        fn all_today_studied(
            &self,
            _today_start_ms: i64,
        ) -> CoreResult<std::collections::HashMap<i64, (u32, u32)>> {
            Ok(std::collections::HashMap::new())
        }
        fn set_deck_limits(
            &self,
            _config_id: i64,
            _new_per_day: u32,
            _rev_per_day: u32,
            _now_ms: i64,
        ) -> CoreResult<()> {
            Ok(())
        }
        fn get_deck_config(&self, _config_id: i64) -> CoreResult<crate::scheduling::SchedConfig> {
            Ok(crate::scheduling::SchedConfig::default())
        }
        fn set_deck_config(
            &self,
            _config_id: i64,
            _config: &crate::scheduling::SchedConfig,
            _now_ms: i64,
        ) -> CoreResult<()> {
            Ok(())
        }
        fn get_rollover_hour(&self) -> CoreResult<u8> {
            Ok(4)
        }
        fn set_rollover_hour(&self, _hour: u8, _now_ms: i64) -> CoreResult<()> {
            Ok(())
        }
        fn day_extra_new(&self, _deck_id: i64, _day: i32) -> CoreResult<u32> {
            Ok(0)
        }
        fn all_day_extra_new(&self, _day: i32) -> CoreResult<std::collections::HashMap<i64, u32>> {
            Ok(std::collections::HashMap::new())
        }
        fn set_day_extra_new(&self, _deck_id: i64, _day: i32, _extra_new: u32) -> CoreResult<()> {
            Ok(())
        }
        fn study_card(&self, _card_id: i64) -> CoreResult<Option<StudyCard>> {
            Ok(None)
        }
        fn apply_answer(
            &self,
            _card_id: i64,
            _next: &CardState,
            _due: i64,
            _log: &Revlog,
        ) -> CoreResult<()> {
            Ok(())
        }
        fn list_notes(&self, _query: Option<&str>, _limit: i64) -> CoreResult<Vec<NoteOverview>> {
            Ok(vec![])
        }
        fn note_detail(&self, _note_id: i64) -> CoreResult<Option<NoteDetail>> {
            Ok(None)
        }
        fn update_note(
            &self,
            _note_id: i64,
            _fields: &[String],
            _tags: &[String],
            _now_ms: i64,
        ) -> CoreResult<()> {
            Ok(())
        }
        #[allow(clippy::too_many_arguments)]
        fn stats(
            &self,
            _deck_ids: Option<&[i64]>,
            _days: Option<u32>,
            _tz_offset_minutes: i32,
            _rollover_hour: u8,
            _fsrs_weights: &[f64; 21],
            _retention_goal_pct: f64,
            _today: i32,
            _now_ms: i64,
            _created_ms: i64,
        ) -> CoreResult<crate::ipc::StatsDto> {
            Ok(crate::ipc::StatsDto::default())
        }
        fn index_rows(&self) -> CoreResult<Vec<NoteIndexRow>> {
            Ok(vec![])
        }
        fn notes_by_ids(&self, _ids: &[i64]) -> CoreResult<Vec<NoteOverview>> {
            Ok(vec![])
        }
        fn dump_collection(&self) -> CoreResult<CanonicalModel> {
            Ok(CanonicalModel::default())
        }
        fn list_notetypes(&self) -> CoreResult<Vec<Notetype>> {
            Ok(vec![])
        }
        fn fields_for_notetype(&self, _notetype_id: i64) -> CoreResult<Vec<Field>> {
            Ok(vec![])
        }
        fn templates_for_notetype(&self, _notetype_id: i64) -> CoreResult<Vec<Template>> {
            Ok(vec![])
        }
        fn add_note_with_cards(
            &self,
            _notetype_id: i64,
            _deck_id: i64,
            _fields: &[String],
            _tags: &[String],
            _now_ms: i64,
        ) -> CoreResult<(i64, u32)> {
            Ok((1, 0))
        }
        fn get_notetype_detail(&self, _notetype_id: i64) -> CoreResult<Option<NotetypeDetail>> {
            Ok(None)
        }
        fn create_notetype(&self, _name: &str, _kind: i64, _now_ms: i64) -> CoreResult<i64> {
            Ok(1)
        }
        fn stock_notetype_names(&self) -> Vec<&'static str> {
            vec![]
        }
        fn add_stock_notetype(&self, _index: usize, _now_ms: i64) -> CoreResult<i64> {
            Ok(1)
        }
        fn delete_notetype(&self, _notetype_id: i64, _now_ms: i64) -> CoreResult<()> {
            Ok(())
        }
        fn rename_notetype(&self, _notetype_id: i64, _name: &str, _now_ms: i64) -> CoreResult<()> {
            Ok(())
        }
        fn save_notetype_css(&self, _notetype_id: i64, _css: &str, _now_ms: i64) -> CoreResult<()> {
            Ok(())
        }
        fn add_field(&self, _notetype_id: i64, _name: &str, _now_ms: i64) -> CoreResult<()> {
            Ok(())
        }
        fn check_field_remove(
            &self,
            _notetype_id: i64,
            _ord: i64,
        ) -> CoreResult<FieldRemoveWarning> {
            Ok(FieldRemoveWarning {
                notes_with_content: 0,
            })
        }
        fn remove_field(&self, _notetype_id: i64, _ord: i64, _now_ms: i64) -> CoreResult<()> {
            Ok(())
        }
        fn rename_field(
            &self,
            _notetype_id: i64,
            _ord: i64,
            _name: &str,
            _now_ms: i64,
        ) -> CoreResult<()> {
            Ok(())
        }
        fn reorder_fields(
            &self,
            _notetype_id: i64,
            _new_order: &[i64],
            _now_ms: i64,
        ) -> CoreResult<()> {
            Ok(())
        }
        fn add_template(
            &self,
            _notetype_id: i64,
            _name: &str,
            _qfmt: &str,
            _afmt: &str,
            _now_ms: i64,
        ) -> CoreResult<()> {
            Ok(())
        }
        fn remove_template(&self, _notetype_id: i64, _ord: i64, _now_ms: i64) -> CoreResult<()> {
            Ok(())
        }
        fn save_template(
            &self,
            _notetype_id: i64,
            _ord: i64,
            _name: &str,
            _qfmt: &str,
            _afmt: &str,
            _now_ms: i64,
        ) -> CoreResult<()> {
            Ok(())
        }
        fn suspend_cards(&self, _card_ids: &[i64]) -> CoreResult<()> {
            Ok(())
        }
        fn unsuspend_cards(&self, _card_ids: &[i64]) -> CoreResult<()> {
            Ok(())
        }
        fn bury_cards(&self, _card_ids: &[i64]) -> CoreResult<()> {
            Ok(())
        }
        fn bury_siblings(&self, _note_id: i64, _answered_card_id: i64) -> CoreResult<()> {
            Ok(())
        }
        fn unbury_deck(&self, _deck_id: i64) -> CoreResult<()> {
            Ok(())
        }
        fn set_card_flag(&self, _card_ids: &[i64], _flag: u8) -> CoreResult<()> {
            Ok(())
        }
        fn add_note_tag(&self, _note_id: i64, _tag: &str, _now_ms: i64) -> CoreResult<()> {
            Ok(())
        }
        fn search_cards(
            &self,
            _q: &str,
            _today: i32,
            _now_ms: i64,
            _limit: i64,
            _offset: i64,
        ) -> CoreResult<Vec<crate::ipc::CardRow>> {
            Ok(vec![])
        }
        fn delete_notes(&self, _note_ids: &[i64], _now_ms: i64) -> CoreResult<()> {
            Ok(())
        }
        fn move_cards_to_deck(&self, _card_ids: &[i64], _deck_id: i64) -> CoreResult<()> {
            Ok(())
        }
        fn remove_note_tag(&self, _note_id: i64, _tag: &str, _now_ms: i64) -> CoreResult<()> {
            Ok(())
        }
        fn list_tags(&self) -> CoreResult<Vec<String>> {
            Ok(vec![])
        }
        fn rename_tag(&self, _old: &str, _new: &str, _now: i64) -> CoreResult<u32> {
            Ok(0)
        }
        fn delete_tag(&self, _tag: &str, _now: i64) -> CoreResult<u32> {
            Ok(0)
        }
        fn merge_tags(&self, _src: &[String], _tgt: &str, _now: i64) -> CoreResult<()> {
            Ok(())
        }
        fn create_filtered_deck(
            &self,
            _name: &str,
            _search: &str,
            _order: u8,
            _limit: u32,
            _today: i32,
            _now: i64,
        ) -> CoreResult<Deck> {
            Ok(Deck {
                id: 99,
                name: "filtered".into(),
                parent_id: None,
                config_id: 1,
                mod_ms: 0,
                usn: -1,
                collapsed: false,
                is_filtered: true,
            })
        }
        fn rebuild_filtered(&self, _id: i64, _today: i32, _now: i64) -> CoreResult<u32> {
            Ok(0)
        }
        fn empty_filtered(&self, _id: i64, _now: i64) -> CoreResult<()> {
            Ok(())
        }
        fn get_filtered_config(
            &self,
            _id: i64,
        ) -> CoreResult<Option<crate::ipc::FilteredDeckConfig>> {
            Ok(None)
        }
        fn integrity_check(&self) -> CoreResult<Vec<String>> {
            Ok(vec![])
        }
        fn optimize(&self) -> CoreResult<()> {
            Ok(())
        }
        fn note_media_refs(&self) -> CoreResult<Vec<String>> {
            Ok(vec![])
        }
        fn backup_db(&self, _dest: &std::path::Path) -> CoreResult<()> {
            Ok(())
        }
        fn revlogs_for_optimize(
            &self,
            _deck_id: Option<i64>,
        ) -> CoreResult<Vec<crate::model::Revlog>> {
            Ok(vec![])
        }
    }

    fn collection() -> Collection {
        Collection::new(
            Box::new(FakeStorage::default()),
            Arc::new(FixedClock(1_000)),
        )
    }

    #[test]
    fn prefer_new_interleaves_evenly() {
        // Fresh session, equal-sized streams: new goes first (tie → new).
        assert!(prefer_new(0, 10, 0, 10));
        // After one new and zero reviews, review is now behind → pick review.
        assert!(!prefer_new(1, 9, 0, 10));
        // Driving the merge to completion yields an even split (~half new).
        let (mut new_done, mut rev_done) = (0u32, 0u32);
        let (new_total, rev_total) = (10u32, 20u32);
        for _ in 0..(new_total + rev_total) {
            let new_rem = new_total - new_done;
            let rev_rem = rev_total - rev_done;
            if new_rem == 0 {
                rev_done += 1;
            } else if rev_rem == 0 || prefer_new(new_done, new_rem, rev_done, rev_rem) {
                new_done += 1;
            } else {
                rev_done += 1;
            }
        }
        assert_eq!((new_done, rev_done), (new_total, rev_total));
    }

    #[test]
    fn create_then_list() {
        let c = collection();
        c.create_deck("Spanish").unwrap();
        let decks = c.list_decks().unwrap();
        assert_eq!(decks.len(), 1);
        assert_eq!(decks[0].name, "Spanish");
    }

    #[test]
    fn rejects_blank_and_duplicate_names() {
        let c = collection();
        c.create_deck("Spanish").unwrap();
        assert!(c.create_deck("   ").is_err());
        assert!(c.create_deck("Spanish").is_err());
    }

    #[test]
    fn undo_reverses_create_rename_delete() {
        let c = collection();
        let deck = c.create_deck("Spanish").unwrap();

        c.rename_deck(deck.id, "Español").unwrap();
        assert_eq!(c.list_decks().unwrap()[0].name, "Español");

        // undo rename
        assert_eq!(
            c.undo().unwrap().as_deref(),
            Some("Rename deck to \"Español\"")
        );
        assert_eq!(c.list_decks().unwrap()[0].name, "Spanish");

        // undo create
        assert!(c.undo().unwrap().is_some());
        assert!(c.list_decks().unwrap().is_empty());

        // nothing left
        assert!(c.undo().unwrap().is_none());
    }

    #[test]
    fn import_creates_decks_and_counts_notes() {
        let c = collection();
        let model = CanonicalModel {
            decks: vec![Deck {
                id: 5,
                name: "Imported".into(),
                parent_id: None,
                config_id: 1,
                mod_ms: 0,
                usn: -1,
                collapsed: false,
                is_filtered: false,
            }],
            notes: vec![crate::model::Note {
                id: 1700000000000,
                guid: "abc".into(),
                notetype_id: 1,
                mod_ms: 0,
                usn: -1,
                tags: vec![],
                fields: vec!["Front".into(), "Back".into()],
                sort_field: Some("Front".into()),
                checksum: None,
            }],
            ..Default::default()
        };
        let summary = c.import(&model).unwrap();
        assert_eq!(summary.decks_added, 1);
        assert_eq!(summary.notes_added, 1);
        assert!(c.list_decks().unwrap().iter().any(|d| d.name == "Imported"));
    }

    #[test]
    fn undo_restores_deleted_deck() {
        let c = collection();
        let deck = c.create_deck("Spanish").unwrap();
        c.remove_deck(deck.id).unwrap();
        assert!(c.list_decks().unwrap().is_empty());
        c.undo().unwrap();
        assert_eq!(c.list_decks().unwrap()[0].name, "Spanish");
    }

    fn deck(id: i64, name: &str, parent_id: Option<i64>, is_filtered: bool) -> Deck {
        Deck {
            id,
            name: name.into(),
            parent_id,
            config_id: 1,
            mod_ms: 0,
            usn: -1,
            collapsed: false,
            is_filtered,
        }
    }

    #[test]
    fn list_decks_with_counts_rolls_up_subdecks() {
        let storage = FakeStorage::default();
        storage.decks.lock().unwrap().extend([
            deck(1, "Spanish", None, false),
            deck(2, "Spanish::Verbs", Some(1), false),
            deck(3, "Cram", Some(1), true), // filtered: excluded from parent rollup
        ]);
        {
            let mut counts = storage.due_counts.lock().unwrap();
            counts.insert(1, (2, 0, 0));
            counts.insert(2, (5, 1, 3));
            counts.insert(3, (9, 9, 9));
        }

        let c = Collection::new(Box::new(storage), Arc::new(FixedClock(1_000)));
        let by_id: std::collections::HashMap<i64, (u32, u32, u32)> = c
            .list_decks_with_counts()
            .unwrap()
            .into_iter()
            .map(|(d, counts)| (d.id, counts))
            .collect();

        assert_eq!(
            by_id[&1],
            (7, 1, 3),
            "parent rolls up its own + child's counts"
        );
        assert_eq!(by_id[&2], (5, 1, 3), "child counts are unaffected");
        assert_eq!(
            by_id[&3],
            (9, 9, 9),
            "filtered deck reports only its own counts"
        );
    }

    #[test]
    fn count_due_by_type_sums_subtree_excluding_filtered() {
        let storage = FakeStorage::default();
        storage.decks.lock().unwrap().extend([
            deck(1, "Spanish", None, false),
            deck(2, "Spanish::Verbs", Some(1), false),
            deck(3, "Cram", Some(1), true),
        ]);
        {
            let mut counts = storage.due_counts.lock().unwrap();
            counts.insert(1, (1, 0, 0));
            counts.insert(2, (2, 1, 1));
            counts.insert(3, (100, 100, 100));
        }

        let c = Collection::new(Box::new(storage), Arc::new(FixedClock(1_000)));
        assert_eq!(c.count_due_by_type(1).unwrap(), (3, 1, 1));
        assert_eq!(c.count_due(1).unwrap(), 5);
    }

    #[test]
    fn next_card_prefers_soonest_learning_across_subtree() {
        let storage = FakeStorage::default();
        storage.decks.lock().unwrap().extend([
            deck(1, "Spanish", None, false),
            deck(2, "Spanish::Verbs", Some(1), false),
        ]);
        {
            let mut queues = storage.queues.lock().unwrap();
            queues.insert(
                1,
                crate::ports::StudyQueue {
                    learning: vec![101],
                    ..Default::default()
                },
            );
            queues.insert(
                2,
                crate::ports::StudyQueue {
                    learning: vec![201],
                    ..Default::default()
                },
            );
        }
        {
            let mut due = storage.due_ms.lock().unwrap();
            due.insert(101, 5_000);
            due.insert(201, 1_000); // the child's card is due sooner
        }

        let c = Collection::new(Box::new(storage), Arc::new(FixedClock(1_000)));
        // Studying the parent surfaces the child's card because it's soonest due.
        assert_eq!(c.next_card_id(1).unwrap(), Some(201));
        // Studying the child directly only ever sees its own queue.
        assert_eq!(c.next_card_id(2).unwrap(), Some(201));
    }

    #[test]
    fn next_card_interleaves_new_and_review_across_subtree() {
        let storage = FakeStorage::default();
        storage.decks.lock().unwrap().extend([
            deck(1, "Spanish", None, false),
            deck(2, "Spanish::Verbs", Some(1), false),
        ]);
        {
            let mut queues = storage.queues.lock().unwrap();
            queues.insert(
                1,
                crate::ports::StudyQueue {
                    new: vec![11],
                    ..Default::default()
                },
            );
            queues.insert(
                2,
                crate::ports::StudyQueue {
                    review: vec![22],
                    ..Default::default()
                },
            );
        }

        let c = Collection::new(Box::new(storage), Arc::new(FixedClock(1_000)));
        // No cards studied yet on either stream: `prefer_new` ties toward new.
        assert_eq!(c.next_card_id(1).unwrap(), Some(11));
    }

    #[test]
    fn filtered_deck_studies_in_isolation() {
        let storage = FakeStorage::default();
        storage
            .decks
            .lock()
            .unwrap()
            .push(deck(1, "Cram", None, true));
        storage.queues.lock().unwrap().insert(
            1,
            crate::ports::StudyQueue {
                new: vec![11],
                ..Default::default()
            },
        );

        let c = Collection::new(Box::new(storage), Arc::new(FixedClock(1_000)));
        assert_eq!(c.next_card_id(1).unwrap(), Some(11));
    }

    #[test]
    fn learn_ahead_skips_in_grace_card_for_a_later_one() {
        let storage = FakeStorage::default();
        storage.decks.lock().unwrap().push(deck(1, "Spanish", None, false));
        storage.queues.lock().unwrap().insert(
            1,
            crate::ports::StudyQueue {
                learning_ahead: vec![101, 102], // 101 due sooner
                ..Default::default()
            },
        );
        storage.due_ms.lock().unwrap().extend([(101, 2_000), (102, 5_000)]);
        // 101 was just answered (in grace); 102 has never been answered.
        storage.last_answered.lock().unwrap().insert(101, 1_000);

        let c = Collection::new(Box::new(storage), Arc::new(FixedClock(1_000)));
        assert_eq!(c.next_card_id(1).unwrap(), Some(102));
    }

    #[test]
    fn learn_ahead_returns_sole_candidate_even_in_grace() {
        let storage = FakeStorage::default();
        storage.decks.lock().unwrap().push(deck(1, "Spanish", None, false));
        storage.queues.lock().unwrap().insert(
            1,
            crate::ports::StudyQueue {
                learning_ahead: vec![201],
                ..Default::default()
            },
        );
        storage.last_answered.lock().unwrap().insert(201, 1_000);

        let c = Collection::new(Box::new(storage), Arc::new(FixedClock(1_000)));
        // Grace never blocks the only card that can be shown.
        assert_eq!(c.next_card_id(1).unwrap(), Some(201));
    }

    #[test]
    fn learning_due_now_skips_in_grace_card_for_another_due_card() {
        let storage = FakeStorage::default();
        storage.decks.lock().unwrap().push(deck(1, "Spanish", None, false));
        storage.queues.lock().unwrap().insert(
            1,
            crate::ports::StudyQueue {
                learning: vec![301, 302],
                ..Default::default()
            },
        );
        storage.last_answered.lock().unwrap().insert(301, 1_000);

        let c = Collection::new(Box::new(storage), Arc::new(FixedClock(1_000)));
        assert_eq!(c.next_card_id(1).unwrap(), Some(302));
    }

    #[test]
    fn learning_due_now_in_grace_defers_to_new_card() {
        let storage = FakeStorage::default();
        storage.decks.lock().unwrap().push(deck(1, "Spanish", None, false));
        storage.queues.lock().unwrap().insert(
            1,
            crate::ports::StudyQueue {
                learning: vec![301],
                new: vec![11],
                ..Default::default()
            },
        );
        storage.last_answered.lock().unwrap().insert(301, 1_000);

        let c = Collection::new(Box::new(storage), Arc::new(FixedClock(1_000)));
        assert_eq!(c.next_card_id(1).unwrap(), Some(11));
    }

    #[test]
    fn learning_due_now_in_grace_is_shown_when_it_is_the_only_card() {
        let storage = FakeStorage::default();
        storage.decks.lock().unwrap().push(deck(1, "Spanish", None, false));
        storage.queues.lock().unwrap().insert(
            1,
            crate::ports::StudyQueue {
                learning: vec![301],
                ..Default::default()
            },
        );
        storage.last_answered.lock().unwrap().insert(301, 1_000);

        let c = Collection::new(Box::new(storage), Arc::new(FixedClock(1_000)));
        // Sole card in the whole deck: grace must not dead-end the session.
        assert_eq!(c.next_card_id(1).unwrap(), Some(301));
    }
}
