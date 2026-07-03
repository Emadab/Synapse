//! Canonical schema as an ordered list of migrations. The migration runner
//! applies any not yet reflected in the database's `user_version`. The schema
//! is a *superset* of Anki's so import/export round-trips losslessly; see
//! `docs/ARCHITECTURE.md` §3.

/// `MIGRATIONS[i]` upgrades the database from `user_version == i` to `i + 1`.
/// Never edit a shipped migration — append a new one.
pub const MIGRATIONS: &[&str] = &[
    // v0 -> v1: initial canonical schema + seed (default deck config + Default deck).
    r#"
    CREATE TABLE collection (
        id         INTEGER PRIMARY KEY CHECK (id = 1),
        created    INTEGER NOT NULL,
        modified   INTEGER NOT NULL,
        schema_mod INTEGER NOT NULL,
        anki_ver   INTEGER NOT NULL DEFAULT 18,
        usn        INTEGER NOT NULL DEFAULT 0,
        last_sync  INTEGER NOT NULL DEFAULT 0,
        config     TEXT NOT NULL DEFAULT '{}'
    );

    CREATE TABLE deck_config (
        id     INTEGER PRIMARY KEY,
        name   TEXT NOT NULL,
        "mod"  INTEGER NOT NULL,
        usn    INTEGER NOT NULL DEFAULT 0,
        config TEXT NOT NULL DEFAULT '{}'
    );

    CREATE TABLE decks (
        id          INTEGER PRIMARY KEY,
        name        TEXT NOT NULL UNIQUE,
        parent_id   INTEGER REFERENCES decks(id),
        config_id   INTEGER NOT NULL REFERENCES deck_config(id),
        "mod"       INTEGER NOT NULL,
        usn         INTEGER NOT NULL DEFAULT 0,
        collapsed   INTEGER NOT NULL DEFAULT 0,
        is_filtered INTEGER NOT NULL DEFAULT 0,
        common      TEXT NOT NULL DEFAULT '{}',
        filtered    TEXT
    );
    CREATE INDEX idx_decks_parent ON decks(parent_id);

    CREATE TABLE notetypes (
        id     INTEGER PRIMARY KEY,
        name   TEXT NOT NULL,
        kind   INTEGER NOT NULL DEFAULT 0,
        "mod"  INTEGER NOT NULL,
        usn    INTEGER NOT NULL DEFAULT 0,
        config TEXT NOT NULL DEFAULT '{}',
        data   TEXT
    );
    CREATE TABLE fields (
        notetype_id INTEGER NOT NULL REFERENCES notetypes(id) ON DELETE CASCADE,
        ord         INTEGER NOT NULL,
        name        TEXT NOT NULL,
        config      TEXT NOT NULL DEFAULT '{}',
        PRIMARY KEY (notetype_id, ord)
    );
    CREATE TABLE templates (
        notetype_id INTEGER NOT NULL REFERENCES notetypes(id) ON DELETE CASCADE,
        ord         INTEGER NOT NULL,
        name        TEXT NOT NULL,
        qfmt        TEXT NOT NULL,
        afmt        TEXT NOT NULL,
        config      TEXT NOT NULL DEFAULT '{}',
        PRIMARY KEY (notetype_id, ord)
    );

    CREATE TABLE notes (
        id          INTEGER PRIMARY KEY,
        guid        TEXT NOT NULL,
        notetype_id INTEGER NOT NULL REFERENCES notetypes(id),
        "mod"       INTEGER NOT NULL,
        usn         INTEGER NOT NULL DEFAULT 0,
        tags        TEXT NOT NULL DEFAULT '',
        fields      TEXT NOT NULL,
        sort_field  TEXT,
        checksum    INTEGER,
        data        TEXT
    );
    CREATE INDEX idx_notes_guid ON notes(guid);
    CREATE INDEX idx_notes_csum ON notes(checksum);

    CREATE TABLE cards (
        id               INTEGER PRIMARY KEY,
        note_id          INTEGER NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
        deck_id          INTEGER NOT NULL REFERENCES decks(id),
        ord              INTEGER NOT NULL,
        "mod"            INTEGER NOT NULL,
        usn              INTEGER NOT NULL DEFAULT 0,
        type             INTEGER NOT NULL DEFAULT 0,
        queue            INTEGER NOT NULL DEFAULT 0,
        due              INTEGER NOT NULL DEFAULT 0,
        interval         INTEGER NOT NULL DEFAULT 0,
        ease_factor      INTEGER NOT NULL DEFAULT 0,
        reps             INTEGER NOT NULL DEFAULT 0,
        lapses           INTEGER NOT NULL DEFAULT 0,
        remaining        INTEGER NOT NULL DEFAULT 0,
        original_due     INTEGER NOT NULL DEFAULT 0,
        original_deck_id INTEGER NOT NULL DEFAULT 0,
        flags            INTEGER NOT NULL DEFAULT 0,
        fsrs_stability   REAL,
        fsrs_difficulty  REAL,
        fsrs_last_review INTEGER,
        data             TEXT
    );
    CREATE INDEX idx_cards_note ON cards(note_id);
    CREATE INDEX idx_cards_sched ON cards(deck_id, queue, due);

    CREATE TABLE revlog (
        id            INTEGER PRIMARY KEY,
        card_id       INTEGER NOT NULL,
        usn           INTEGER NOT NULL DEFAULT 0,
        ease          INTEGER NOT NULL,
        interval      INTEGER NOT NULL,
        last_interval INTEGER NOT NULL,
        ease_factor   INTEGER NOT NULL,
        taken_ms      INTEGER NOT NULL,
        review_kind   INTEGER NOT NULL DEFAULT 0
    );
    CREATE INDEX idx_revlog_card ON revlog(card_id);

    CREATE TABLE tags (
        name     TEXT PRIMARY KEY,
        usn      INTEGER NOT NULL DEFAULT 0,
        expanded INTEGER NOT NULL DEFAULT 0
    );

    CREATE TABLE graves (
        oid  INTEGER NOT NULL,
        type INTEGER NOT NULL,
        usn  INTEGER NOT NULL DEFAULT -1,
        PRIMARY KEY (oid, type)
    );

    CREATE TABLE media (
        filename TEXT PRIMARY KEY,
        checksum TEXT NOT NULL,
        size     INTEGER NOT NULL,
        usn      INTEGER NOT NULL DEFAULT 0,
        mtime    INTEGER NOT NULL
    );

    -- Seed: a default options group and the Default deck (Anki convention).
    INSERT INTO deck_config (id, name, "mod", usn, config)
        VALUES (1, 'Default', 0, 0, '{}');
    INSERT INTO decks (id, name, parent_id, config_id, "mod", usn)
        VALUES (1, 'Default', NULL, 1, 0, 0);
    "#,
    // v1 -> v2: per-deck per-day "increase today's new card limit" override.
    // Keyed by (deck_id, day) where `day` is the collection-relative day number
    // (Collection::today()), so it naturally stops applying at day rollover.
    r#"
    CREATE TABLE day_limit_overrides (
        deck_id   INTEGER NOT NULL REFERENCES decks(id) ON DELETE CASCADE,
        day       INTEGER NOT NULL,
        extra_new INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY (deck_id, day)
    );
    "#,
];

/// Grave record types (matches Anki: card, note, deck).
pub mod grave_kind {
    pub const CARD: i64 = 0;
    pub const NOTE: i64 = 1;
    pub const DECK: i64 = 2;
}
