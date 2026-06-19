//! End-to-end: synthesize a v11 .apkg, read it into the canonical model, and
//! import it into a fresh SQLite collection.

use synapse_ankifmt::{read_package, testkit};
use synapse_core::ports::Storage;
use synapse_db::SqliteStorage;

#[test]
fn reads_v11_package_and_imports() {
    let dir = tempfile::tempdir().unwrap();
    let apkg = dir.path().join("sample.apkg");
    testkit::write_sample_v11_apkg(&apkg).unwrap();

    let media_dir = dir.path().join("media");
    let (model, media) = read_package(&apkg, Some(&media_dir)).unwrap();

    assert_eq!(model.notetypes.len(), 1);
    assert_eq!(model.notetypes[0].name, "Basic");
    let nt_id = model.notetypes[0].id;
    assert_eq!(
        model
            .fields
            .iter()
            .filter(|f| f.notetype_id == nt_id)
            .count(),
        2
    );
    assert_eq!(
        model
            .templates
            .iter()
            .filter(|t| t.notetype_id == nt_id)
            .count(),
        1
    );
    assert_eq!(model.notes.len(), 2);
    assert_eq!(model.cards.len(), 2);
    assert_eq!(model.revlog.len(), 1);
    assert!(model.decks.iter().any(|d| d.name == "Spanish::Verbs"));

    assert_eq!(media, 1, "one media file extracted");
    assert!(media_dir.join("hola.png").exists());

    // Full pipeline: import the parsed model into a fresh collection.
    let storage = SqliteStorage::open_in_memory().unwrap();
    let summary = storage.import(&model).unwrap();
    assert_eq!(summary.notes_added, 2);
    assert_eq!(summary.cards_added, 2);
    assert_eq!(summary.revlog_added, 1);
    assert!(summary.decks_added >= 2);
    assert_eq!(summary.notetypes_added, 1);
}

#[test]
fn reads_v18_package_and_imports() {
    let dir = tempfile::tempdir().unwrap();
    let apkg = dir.path().join("sample-v18.apkg");
    testkit::write_sample_v18_apkg(&apkg).unwrap();

    let media_dir = dir.path().join("media");
    let (model, media) = read_package(&apkg, Some(&media_dir)).unwrap();

    assert_eq!(model.notetypes.len(), 1);
    assert_eq!(model.notetypes[0].name, "Basic");
    let nt_id = model.notetypes[0].id;
    assert_eq!(
        model
            .fields
            .iter()
            .filter(|f| f.notetype_id == nt_id)
            .count(),
        2
    );

    // The template q/a formats come from the decoded protobuf config.
    let template = model
        .templates
        .iter()
        .find(|t| t.notetype_id == nt_id)
        .unwrap();
    assert_eq!(template.qfmt, "{{Front}}");
    assert!(template.afmt.contains("{{Back}}"));

    assert_eq!(model.notes.len(), 2);
    assert_eq!(model.cards.len(), 2);
    assert!(model.decks.iter().any(|d| d.name == "Spanish::Verbs"));

    assert_eq!(media, 1, "media extracted via the protobuf map");
    assert!(media_dir.join("hola.png").exists());

    let storage = SqliteStorage::open_in_memory().unwrap();
    let summary = storage.import(&model).unwrap();
    assert_eq!(summary.notes_added, 2);
    assert_eq!(summary.cards_added, 2);
}
