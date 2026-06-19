//! Transactional merge of a [`CanonicalModel`] into the open collection.
//!
//! Matching rules (idempotent on re-import):
//! - **deck configs** by id (`INSERT OR IGNORE`)
//! - **decks** by full name (get-or-create); parent links fixed in a second pass
//! - **notetypes** by name; fields/templates inserted only for new notetypes
//! - **notes** by `guid` (update if present, else insert)
//! - **cards** by `(note_id, ord)`
//! - **revlog** by id (`INSERT OR IGNORE`)
//!
//! Source ids are remapped through `*_map` tables so foreign keys stay valid in
//! our id space (deck/notetype/note/card ids are regenerated; revlog ids are
//! preserved because they are unique review timestamps).

use std::collections::HashMap;

use rusqlite::{params, OptionalExtension, Transaction};
use synapse_core::error::{CoreError, CoreResult};
use synapse_core::model::{CanonicalModel, ImportSummary};

fn err(e: impl std::fmt::Display) -> CoreError {
    CoreError::Storage(e.to_string())
}

pub(crate) fn import(tx: &Transaction<'_>, model: &CanonicalModel) -> CoreResult<ImportSummary> {
    let mut summary = ImportSummary::default();

    // 1. Deck options groups.
    for cfg in &model.deck_configs {
        tx.execute(
            r#"INSERT OR IGNORE INTO deck_config (id, name, "mod", usn, config) VALUES (?1, ?2, ?3, ?4, ?5)"#,
            params![cfg.id, cfg.name, cfg.mod_ms, cfg.usn, cfg.config_json],
        )
        .map_err(err)?;
    }

    // 2. Decks — get-or-create by name.
    let mut deck_map: HashMap<i64, i64> = HashMap::new();
    for deck in &model.decks {
        let existing: Option<i64> = tx
            .query_row("SELECT id FROM decks WHERE name = ?1", [&deck.name], |r| {
                r.get(0)
            })
            .optional()
            .map_err(err)?;
        let target = match existing {
            Some(id) => id,
            None => {
                let config_id = resolve_config(tx, deck.config_id)?;
                tx.execute(
                    r#"INSERT INTO decks (name, parent_id, config_id, "mod", usn, collapsed, is_filtered)
                       VALUES (?1, NULL, ?2, ?3, ?4, ?5, ?6)"#,
                    params![
                        deck.name,
                        config_id,
                        deck.mod_ms,
                        deck.usn,
                        deck.collapsed as i64,
                        deck.is_filtered as i64
                    ],
                )
                .map_err(err)?;
                summary.decks_added += 1;
                tx.last_insert_rowid()
            }
        };
        deck_map.insert(deck.id, target);
    }
    // 2b. Resolve parent links now that every deck row exists.
    for deck in &model.decks {
        if let Some((parent, _)) = deck.name.rsplit_once("::") {
            tx.execute(
                "UPDATE decks SET parent_id = (SELECT id FROM decks WHERE name = ?1) WHERE name = ?2",
                params![parent, deck.name],
            )
            .map_err(err)?;
        }
    }

    // 3. Note types — get-or-create by name; new ones get their fields/templates.
    let mut nt_map: HashMap<i64, i64> = HashMap::new();
    for nt in &model.notetypes {
        let existing: Option<i64> = tx
            .query_row(
                "SELECT id FROM notetypes WHERE name = ?1",
                [&nt.name],
                |r| r.get(0),
            )
            .optional()
            .map_err(err)?;
        let target = match existing {
            Some(id) => id,
            None => {
                tx.execute(
                    r#"INSERT INTO notetypes (name, kind, "mod", usn, config) VALUES (?1, ?2, ?3, ?4, ?5)"#,
                    params![nt.name, nt.kind, nt.mod_ms, nt.usn, nt.config_json],
                )
                .map_err(err)?;
                summary.notetypes_added += 1;
                let id = tx.last_insert_rowid();
                for f in model.fields.iter().filter(|f| f.notetype_id == nt.id) {
                    tx.execute(
                        "INSERT INTO fields (notetype_id, ord, name, config) VALUES (?1, ?2, ?3, ?4)",
                        params![id, f.ord, f.name, f.config_json],
                    )
                    .map_err(err)?;
                }
                for t in model.templates.iter().filter(|t| t.notetype_id == nt.id) {
                    tx.execute(
                        "INSERT INTO templates (notetype_id, ord, name, qfmt, afmt, config) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                        params![id, t.ord, t.name, t.qfmt, t.afmt, t.config_json],
                    )
                    .map_err(err)?;
                }
                id
            }
        };
        nt_map.insert(nt.id, target);
    }

    // 4. Notes — dedup by guid.
    let mut note_map: HashMap<i64, i64> = HashMap::new();
    for note in &model.notes {
        let Some(&notetype_id) = nt_map.get(&note.notetype_id) else {
            continue;
        };
        let fields = note.fields.join("\u{1f}");
        let tags = if note.tags.is_empty() {
            String::new()
        } else {
            format!(" {} ", note.tags.join(" "))
        };
        let sort = note
            .sort_field
            .clone()
            .or_else(|| note.fields.first().cloned());

        let existing: Option<i64> = tx
            .query_row("SELECT id FROM notes WHERE guid = ?1", [&note.guid], |r| {
                r.get(0)
            })
            .optional()
            .map_err(err)?;
        match existing {
            Some(id) => {
                tx.execute(
                    r#"UPDATE notes SET "mod" = ?1, tags = ?2, fields = ?3, sort_field = ?4, checksum = ?5, usn = -1
                       WHERE id = ?6 AND ?1 >= "mod""#,
                    params![note.mod_ms, tags, fields, sort, note.checksum, id],
                )
                .map_err(err)?;
                summary.notes_updated += 1;
                note_map.insert(note.id, id);
            }
            None => {
                tx.execute(
                    r#"INSERT INTO notes (guid, notetype_id, "mod", usn, tags, fields, sort_field, checksum)
                       VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
                    params![note.guid, notetype_id, note.mod_ms, note.usn, tags, fields, sort, note.checksum],
                )
                .map_err(err)?;
                summary.notes_added += 1;
                note_map.insert(note.id, tx.last_insert_rowid());
            }
        }
    }

    // 5. Cards — dedup by (note_id, ord).
    let mut card_map: HashMap<i64, i64> = HashMap::new();
    for card in &model.cards {
        let Some(&note_id) = note_map.get(&card.note_id) else {
            continue;
        };
        let deck_id = deck_map.get(&card.deck_id).copied().unwrap_or(1);
        let existing: Option<i64> = tx
            .query_row(
                "SELECT id FROM cards WHERE note_id = ?1 AND ord = ?2",
                params![note_id, card.ord],
                |r| r.get(0),
            )
            .optional()
            .map_err(err)?;
        let target = match existing {
            Some(id) => id,
            None => {
                tx.execute(
                    r#"INSERT INTO cards
                       (note_id, deck_id, ord, "mod", usn, type, queue, due, interval, ease_factor,
                        reps, lapses, remaining, original_due, original_deck_id, flags,
                        fsrs_stability, fsrs_difficulty, fsrs_last_review, data)
                       VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)"#,
                    params![
                        note_id,
                        deck_id,
                        card.ord,
                        card.mod_ms,
                        card.usn,
                        card.ctype,
                        card.queue,
                        card.due,
                        card.interval,
                        card.ease_factor,
                        card.reps,
                        card.lapses,
                        card.remaining,
                        card.original_due,
                        card.original_deck_id,
                        card.flags,
                        card.fsrs_stability,
                        card.fsrs_difficulty,
                        card.fsrs_last_review,
                        card.data,
                    ],
                )
                .map_err(err)?;
                summary.cards_added += 1;
                tx.last_insert_rowid()
            }
        };
        card_map.insert(card.id, target);
    }

    // 6. Review history — preserve ids (unique timestamps); dedup via PK.
    for rev in &model.revlog {
        let Some(&card_id) = card_map.get(&rev.card_id) else {
            continue;
        };
        let changed = tx
            .execute(
                r#"INSERT OR IGNORE INTO revlog
                   (id, card_id, usn, ease, interval, last_interval, ease_factor, taken_ms, review_kind)
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"#,
                params![
                    rev.id,
                    card_id,
                    rev.usn,
                    rev.ease,
                    rev.interval,
                    rev.last_interval,
                    rev.ease_factor,
                    rev.taken_ms,
                    rev.review_kind
                ],
            )
            .map_err(err)?;
        summary.revlog_added += changed as u32;
    }

    Ok(summary)
}

fn resolve_config(tx: &Transaction<'_>, source_config_id: i64) -> CoreResult<i64> {
    let exists: Option<i64> = tx
        .query_row(
            "SELECT id FROM deck_config WHERE id = ?1",
            [source_config_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(err)?;
    Ok(exists.unwrap_or(1))
}

#[cfg(test)]
mod tests {
    use crate::storage::SqliteStorage;
    use synapse_core::model::{
        CanonicalModel, Card, Deck, DeckConfig, Field, Note, Notetype, Revlog, Template,
    };
    use synapse_core::ports::Storage;

    fn deck(id: i64, name: &str) -> Deck {
        Deck {
            id,
            name: name.into(),
            parent_id: None,
            config_id: 1,
            mod_ms: 0,
            usn: -1,
            collapsed: false,
            is_filtered: false,
        }
    }

    fn sample_model() -> CanonicalModel {
        CanonicalModel {
            deck_configs: vec![DeckConfig {
                id: 1,
                name: "Default".into(),
                mod_ms: 0,
                usn: 0,
                config_json: "{}".into(),
            }],
            decks: vec![
                deck(1, "Default"),
                deck(2, "Spanish"),
                deck(3, "Spanish::Verbs"),
            ],
            notetypes: vec![Notetype {
                id: 10,
                name: "Basic".into(),
                kind: 0,
                mod_ms: 0,
                usn: -1,
                config_json: "{}".into(),
            }],
            fields: vec![
                Field {
                    notetype_id: 10,
                    ord: 0,
                    name: "Front".into(),
                    config_json: "{}".into(),
                },
                Field {
                    notetype_id: 10,
                    ord: 1,
                    name: "Back".into(),
                    config_json: "{}".into(),
                },
            ],
            templates: vec![Template {
                notetype_id: 10,
                ord: 0,
                name: "Card 1".into(),
                qfmt: "{{Front}}".into(),
                afmt: "{{FrontSide}}<hr>{{Back}}".into(),
                config_json: "{}".into(),
            }],
            notes: vec![Note {
                id: 100,
                guid: "g1".into(),
                notetype_id: 10,
                mod_ms: 0,
                usn: -1,
                tags: vec!["spanish".into()],
                fields: vec!["hola".into(), "hello".into()],
                sort_field: Some("hola".into()),
                checksum: Some(123),
            }],
            cards: vec![Card {
                id: 1000,
                note_id: 100,
                deck_id: 3,
                ord: 0,
                mod_ms: 0,
                usn: -1,
                ctype: 0,
                queue: 0,
                due: 0,
                interval: 0,
                ease_factor: 2500,
                reps: 0,
                lapses: 0,
                remaining: 0,
                original_due: 0,
                original_deck_id: 0,
                flags: 0,
                fsrs_stability: None,
                fsrs_difficulty: None,
                fsrs_last_review: None,
                data: None,
            }],
            revlog: vec![Revlog {
                id: 5000,
                card_id: 1000,
                usn: -1,
                ease: 3,
                interval: 1,
                last_interval: 0,
                ease_factor: 2500,
                taken_ms: 1200,
                review_kind: 0,
            }],
        }
    }

    #[test]
    fn imports_links_parents_and_is_idempotent() {
        let storage = SqliteStorage::open_in_memory().unwrap();
        let model = sample_model();

        let first = storage.import(&model).unwrap();
        assert_eq!(
            first.decks_added, 2,
            "Default is seeded; Spanish + Spanish::Verbs are new"
        );
        assert_eq!(first.notetypes_added, 1);
        assert_eq!(first.notes_added, 1);
        assert_eq!(first.cards_added, 1);
        assert_eq!(first.revlog_added, 1);

        let verbs = storage.deck_by_name("Spanish::Verbs").unwrap().unwrap();
        let spanish = storage.deck_by_name("Spanish").unwrap().unwrap();
        assert_eq!(
            verbs.parent_id,
            Some(spanish.id),
            "sub-deck links to its parent"
        );

        // Re-importing the same package adds nothing new (guid/name dedup).
        let second = storage.import(&model).unwrap();
        assert_eq!(second.notes_added, 0);
        assert_eq!(second.cards_added, 0);
        assert_eq!(second.revlog_added, 0);
        assert_eq!(second.decks_added, 0);
        assert_eq!(second.notes_updated, 1);
    }
}
