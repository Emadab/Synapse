# Synapse User Guide

## Getting started

1. Download the installer for your platform from the [releases page](https://github.com/synapse-srs/synapse/releases).
2. Run the installer and launch Synapse.
3. On first launch the **Default** deck is ready. Create more decks from the **Decks** screen.

---

## Importing a collection

Synapse reads Anki `.apkg` and `.colpkg` files (schema v11 and v18).

1. Open the **Decks** screen.
2. Click **Import .apkg**.
3. Select your file. Notes are merged by GUID so re-importing is safe.

---

## Studying

1. Click **Study** next to a deck, or navigate to the **Study** screen.
2. Read the question, then press **Space** or **Enter** (or click **Show answer**).
3. Rate the card: **Again / Hard / Good / Easy** (keys **1–4**).

### Card actions during study

| Action | Key |
|--------|-----|
| Show answer | Space / Enter |
| Again | 1 |
| Hard | 2 |
| Good | 3 |
| Easy | 4 |
| Replay audio | R |
| Suspend card | S |
| Bury card | B |
| Flag | F |
| Keyboard shortcuts | ? |

### Daily limits

Synapse enforces a per-deck new-card limit and review limit. Change them in **Deck Options → General**.

---

## Adding and editing notes

1. Navigate to **Add** in the sidebar.
2. Choose a note type from the dropdown.
3. Fill in the fields (supports basic HTML; audio/image via drag-drop).
4. Click **Add**.

To edit an existing note, find it in the **Browse** screen and click the row.

---

## Note types and templates

Navigate to **Note Types** to:
- Create a new note type with custom fields.
- Edit card templates (question / answer format using `{{FieldName}}` syntax).
- Preview how a template renders with sample data.

Template syntax follows Anki conventions: `{{Field}}`, `{{#Field}}...{{/Field}}` (conditional), `{{^Field}}...{{/Field}}` (negation), `{{cloze:Field}}` for cloze note types.

---

## Deck options

Click the **⋯** menu next to a deck → **Options**.

| Tab | Settings |
|-----|----------|
| General | Daily new and review limits |
| New cards | Learning steps, graduating interval, starting ease |
| Reviews | Max interval, ease bonus, hard interval |
| Lapses | Relearning steps, minimum interval, leech threshold |
| FSRS | Enable FSRS, set weights and target retention |

### Switching to FSRS

Set **Algorithm** to **FSRS** in the FSRS tab. Use **Optimize** (after accumulating review history) to fit weights to your memory.

---

## Browser

The **Browse** screen shows all notes and cards. Use the search bar to filter:

| Query | Meaning |
|-------|---------|
| `deck:Spanish` | Cards in the Spanish deck |
| `tag:grammar` | Notes tagged "grammar" |
| `is:due` | Cards due today |
| `is:suspended` | Suspended cards |
| `flag:1` | Flagged red |
| `prop:ivl>30` | Interval > 30 days |
| `-tag:easy` | Negate with `-` |

Select multiple rows to bulk-suspend, bulk-set flag, bulk-change deck, or delete.

---

## Filtered decks (custom study)

1. Browse screen → **Create filtered deck** (or from the Decks screen custom-study menu).
2. Enter a search query and card limit.
3. Cards matching the query are gathered; study proceeds normally.
4. **Empty** returns cards to their original deck with original scheduling intact.

---

## Statistics

The **Stats** screen shows:
- Today's studied counts (new / learning / review)
- Calendar heatmap
- Forecast (cards due per day)
- Card maturity distribution
- FSRS metrics (retrievability, difficulty distribution)

---

## Settings

| Section | What it controls |
|---------|-----------------|
| Appearance | Light / Dark / System theme |
| Scheduling | Default algorithm preference |
| Export | Export full collection as `.apkg` |
| Updates | Check for a newer version |
| Maintenance | Backup, restore, integrity check, optimize DB, media check |
| Plugins | Install, enable, disable sandboxed plugin scripts |

### Keyboard shortcuts reference

Press **?** anywhere in the app (outside a text field) to open the keyboard shortcuts dialog.

---

## Backups and restore

Synapse creates automatic backups on a schedule. To restore:

1. Settings → Maintenance → **Restore backup**.
2. Select a backup from the list.
3. Confirm. A pre-restore backup is created automatically before overwriting.

---

## Plugins

Plugins are sandboxed JavaScript files with a `manifest.json` declaring their capabilities. They cannot access your filesystem or the network beyond what Synapse exposes through the plugin API.

1. Settings → Plugins → **Install plugin…**
2. Select a directory containing `manifest.json` and an entry JS file.
3. Enable the plugin with the toggle.
4. Plugin commands appear under **Plugin commands**; run them with **Run**.

---

## Exporting

Settings → **Export .apkg** — exports your full collection as an Anki-compatible `.apkg` file readable by Anki 2.1+.

---

## Troubleshooting

- **Import fails:** Check that the file is a valid `.apkg`/`.colpkg`. Corrupt zips are rejected with an error message.
- **Audio doesn't play:** Ensure media was included in the `.apkg` (Anki's "include media" option).
- **App won't start:** Check `Help → Open Log Folder` for the latest log file.
- **Study queue shows 0:** Daily limits may be exhausted or all cards are suspended. Check **Browse** for the deck.
