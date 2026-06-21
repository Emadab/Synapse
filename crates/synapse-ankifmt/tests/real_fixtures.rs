//! Any real `.apkg`/`.colpkg` dropped into the repo's `fixtures/` directory is
//! picked up here: it must either parse to a non-empty model, or fail with the
//! known "v18 not yet supported" error. This is how real-world exports join the
//! test corpus without code changes.

use std::path::PathBuf;

use synapse_ankifmt::{read_package, write_apkg};
use synapse_core::model::{CanonicalModel, Card, Deck, Field, Note, Notetype, Revlog, Template};

#[test]
fn real_fixtures_parse_or_are_known_unsupported() {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let Ok(entries) = std::fs::read_dir(&fixtures) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "apkg" && ext != "colpkg" {
            continue;
        }

        match read_package(&path, None) {
            Ok((model, _)) => {
                assert!(
                    !model.is_empty(),
                    "fixture {path:?} parsed to an empty model"
                );
            }
            Err(error) => {
                let message = error.to_string();
                assert!(
                    message.contains("v18") || message.contains("supported"),
                    "fixture {path:?} failed unexpectedly: {message}"
                );
            }
        }
    }
}

#[test]
fn write_then_read_round_trips() {
    let now = 1_700_000_000_000i64;
    let model = CanonicalModel {
        decks: vec![Deck {
            id: 1,
            name: "Spanish".into(),
            parent_id: None,
            config_id: 1,
            mod_ms: now,
            usn: -1,
            collapsed: false,
            is_filtered: false,
        }],
        notetypes: vec![Notetype {
            id: 10,
            name: "Basic".into(),
            kind: 0,
            mod_ms: now,
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
            id: now,
            guid: "abc123".into(),
            notetype_id: 10,
            mod_ms: now,
            usn: -1,
            tags: vec!["vocab".into()],
            fields: vec!["hola".into(), "hello".into()],
            sort_field: Some("hola".into()),
            checksum: None,
        }],
        cards: vec![Card {
            id: now + 1,
            note_id: now,
            deck_id: 1,
            ord: 0,
            mod_ms: now,
            usn: -1,
            ctype: 0,
            queue: 0,
            due: 1,
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
            id: now + 2,
            card_id: now + 1,
            usn: -1,
            ease: 3,
            interval: 1,
            last_interval: -60,
            ease_factor: 2500,
            taken_ms: 5000,
            review_kind: 0,
        }],
        ..Default::default()
    };

    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("test.apkg");
    let media_count = write_apkg(&model, &out, None).expect("write_apkg failed");
    assert_eq!(media_count, 0);

    // Re-read the exported file.
    let (re_read, _) = read_package(&out, None).expect("re-read failed");
    assert_eq!(re_read.decks.len(), 1);
    assert_eq!(re_read.decks[0].name, "Spanish");
    assert_eq!(re_read.notetypes.len(), 1);
    assert_eq!(re_read.notetypes[0].name, "Basic");
    assert_eq!(re_read.notes.len(), 1);
    assert_eq!(re_read.notes[0].fields, vec!["hola", "hello"]);
    assert_eq!(re_read.notes[0].tags, vec!["vocab"]);
    assert_eq!(re_read.cards.len(), 1);
    assert_eq!(re_read.revlog.len(), 1);
    assert_eq!(re_read.revlog[0].ease, 3);
}
