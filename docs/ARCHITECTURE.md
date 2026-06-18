# Synapse — System Architecture

> A beautiful, modern spaced-repetition desktop app, **fully bidirectionally
> compatible with Anki**. Tauri + React/TS frontend over a UI-agnostic Rust core.

## Why this shape

Millions rely on Anki but dislike its dated UI, clumsy browser and weak
editing/search. The opportunity is **not** a prettier clone and **not** a new
SRS algorithm — it is to wrap Anki's proven scheduling data model in a
Linear/Obsidian-grade experience while guaranteeing lossless interop.

Three load-bearing decisions let a team build this for years without a rewrite:

1. A **canonical, normalized internal schema** that is a _superset_ of Anki's,
   with import/export adapters — we are never bound to Anki's on-disk format.
2. A **platform-agnostic Rust core** (`synapse-core`) with zero Tauri/React
   deps, so the same engine powers desktop today and mobile/CLI later.
3. **Clean-architecture dependency rules** enforced at the crate boundary.

### Locked decisions

- **Anki compatibility:** support **both** legacy schema **v11** (decks/models/
  conf as JSON blobs in `col`) and modern **v18** (table-based), detected on
  read. Export to **.apkg v2 (zlib)**, **.apkg v3 / .colpkg (zstd)**. Full
  fidelity for tags, deck hierarchy, templates, HTML/CSS/JS, media, review
  history, learning state and FSRS memory state.
- **Sync:** architected, not implemented in MVP. The `SyncProvider` trait plus
  local change tracking (`usn`/`mod`/`graves`) keep the collection sync-ready.
- **Scheduling:** **SM-2 and FSRS both ship**, switchable per deck, behind one
  `Scheduler` trait.

## 1. Monorepo & folder structure

Cargo workspace (Rust) + pnpm workspace (TS) in one repo. The Tauri shell is a
thin adapter; all logic lives in reusable crates.

```
crates/        synapse-{core,db,scheduler,ankifmt,search,media,render,plugin}
apps/desktop/  src-tauri/ (Rust shell)  +  src/ (React frontend)
packages/      ipc-types (generated), ui-tokens, plugin-sdk
fixtures/      real .apkg/.colpkg + golden scheduler vectors
docs/          this file + ADRs
```

Features are vertical slices (study, browser, editor…), every Rust module is a
small single-responsibility crate, and Tauri appears only in `apps/desktop`.

## 2. Module boundaries (clean architecture)

Four rings; **dependencies point inward only**, enforced by the crate graph.
`synapse-core` defines _ports_ (traits) and depends on none of the impls:

- `Storage` → `synapse-db` (SQLite)
- `MediaStore` → `synapse-media`
- `Scheduler` → `synapse-scheduler` (SM-2, FSRS)
- `SyncProvider` → unimplemented in MVP (architected)
- `Clock` → injectable time (real at runtime, fixed in tests)

**Hard rule:** `synapse-core`, `synapse-scheduler`, `synapse-render` have no
dependency on Tauri, rusqlite or any UI. That is what makes mobile/CLI reuse
free (§13).

## 3. Database schema

Synapse stores a **canonical, normalized (v18-style) schema** as the source of
truth; import/export adapters translate to/from Anki v11 (JSON-blob) and v18
(tabular). The schema is a _superset_, so nothing is lost on round-trip:
Anki-specific scheduling fields are preserved verbatim, and Synapse-only fields
live in JSON `data` columns Anki ignores. SQLite, WAL mode; `synapse-db` owns
all SQL + a versioned migration runner.

Core tables: `collection` (singleton: crt/mod/scm/usn + global config JSON),
`deck_config`, `decks` (full `name`, denormalized `parent_id`, `config_id`),
`notetypes` + `fields` + `templates`, `notes` (guid, notetype_id, tags, fields,
sort_field, checksum), `cards` (type/queue/due/ivl/factor/reps/lapses/left +
`fsrs_stability`/`fsrs_difficulty` + verbatim `data` JSON), `revlog` (full
review history), `tags`, `graves` (deletion tombstones), `media` (filesystem is
the store; table indexes checksums for dedup/cleanup).

Fidelity & sync-readiness: every mutable row carries `usn` + `mod`; deletions
write a `graves` row — the entire substrate sync needs. FSRS state is stored
both as typed columns (fast queries) and preserved in `cards.data` for byte-
faithful v18 round-trip. Full-text search lives in Tantivy, not SQLite.

## 4. Review engine (`synapse-scheduler`)

A **pure** crate: deterministic functions over plain data + an injected `Clock`,
no IO. The application layer loads cards, hands them to the scheduler, persists
results.

```rust
pub trait Scheduler {
    fn build_queue(&self, ctx: &SchedContext) -> Queue;
    fn answer_buttons(&self, card: &Card, ctx: &SchedContext) -> [Interval; 4];
    fn answer(&self, card: &Card, rating: Rating, ctx: &SchedContext) -> AnswerOutcome;
    fn algorithm(&self) -> Algorithm; // Sm2 | Fsrs
}
```

`Sm2Scheduler` is Anki-faithful (learning/review/relearn queues, steps, ease,
fuzz, leeches, day cutoff via `Clock`). `FsrsScheduler` computes stability/
difficulty from 19 per-deck weights + desired retention. Selection is per-deck
config; switching is a config write + queue rebuild (both states always
persist). Validated by golden vectors generated from Anki (§12).

## 5. Import / export (`synapse-ankifmt`)

Pipeline, transactional (all-or-nothing staging txn):

```
unzip → detect container (.apkg/.colpkg) → detect schema (col.ver: v11|v18)
      → SchemaReader (V11Reader JSON-blob | V18Reader tables) → CanonicalModel
      → MergeStrategy (match notes by guid, notetypes by signature, decks by name)
      → staging txn → commit ;  media deduped by checksum, refs rewritten
```

Export reverses it: `V2Writer` (zlib `collection.anki2`), `V3Writer` (zstd
`.anki21b` + protobuf meta), `.colpkg` (whole collection + media). Round-trip
(import → export → re-import = no-op diff) is a test gate.

## 6. Event system (`synapse-core::events`)

A typed domain event bus. Core mutations emit `DomainEvent`s _after_ commit;
subscribers include the search-index updater, stats invalidator, plugin host,
and a Tauri bridge that re-emits to the webview. The frontend maps events →
TanStack Query cache invalidation (live UI, no polling). Undo/redo uses a
separate command + inverse log in the application layer.

## 7. Plugin API (`synapse-plugin` + `packages/plugin-sdk`) — architecture only

Manifest + capability model (`read:notes`, `write:cards`, `ui:panel`,
`net:fetch`, …); the host grants only what is declared. Two surfaces: UI plugins
in a sandboxed worker/iframe (`registerCommand`, `addSidebarPanel`,
`addReviewButton`, `registerCardRenderer`, `onEvent`), and core hooks in a
sandboxed JS/Wasm runtime reacting to events. `@synapse/plugin` types are the
stable public contract. The execution sandbox is post-MVP.

## 8. State management

Rust core is the single source of truth; the frontend caches.

- **TanStack Query** = all server state (queues, cards, decks, stats, search).
  Keys centralized; mutations rely on event-driven invalidation.
- **Zustand** = ephemeral UI state only (theme, palette open, selection, draft).
- **Command layer** (`lib/ipc.ts`) = a typed wrapper over Tauri `invoke`; the UI
  never calls `invoke` directly.
- **Type safety**: Rust structs derive `ts-rs` → `packages/ipc-types`; a Rust
  schema change breaks the TS build.

## 9. UI component hierarchy

`AppShell` (TitleBar, Sidebar deck tree, CommandPalette ⌘K, `<Outlet/>`) →
routes: DeckBrowser, StudyView, CardBrowser, Editor, Statistics, Settings.
Tiers: shadcn primitives (`components/ui`) → shared composites (`components`) →
feature slices (`features/*`) → thin route shells (`routes`). Tokens in
`packages/ui-tokens`; Framer Motion (interruptible, never blocks input);
accessibility via Radix; respects `prefers-reduced-motion`.

## 10. Routing

TanStack Router (type-safe, pairs with TanStack Query, typed search params for
browser filters). Routes: `/`, `/study/$deckId`, `/browse`, `/edit/$noteId`,
`/stats[/$deckId]`, `/settings/*`.

## 11. Error handling

Rust: one `thiserror` enum per crate composing into `CoreError`; the IPC
boundary maps it to a typed serialisable error union. No `unwrap()` in library
code. Frontend: typed `Result` from the IPC client, per-route ErrorBoundary,
toasts for recoverable errors, recovery dialog for fatal (DB locked/corrupt →
backup-and-repair). Every destructive op runs in a transaction with an automatic
pre-op backup. Observability via `tracing` → rolling log.

## 12. Testing strategy

| Layer          | Tool                                 | What                                                                                |
| -------------- | ------------------------------------ | ----------------------------------------------------------------------------------- |
| Scheduler      | Rust unit + golden vectors           | SM-2/FSRS vs Anki-generated vectors; deterministic via injected `Clock` + fixed RNG |
| Import/export  | Rust integration                     | Real fixtures; round-trip no-op diff; media remap                                   |
| Core use-cases | Rust integration                     | In-memory SQLite; events, undo/redo, rollback                                       |
| Render         | Rust unit                            | Cloze, nested fields, conditionals, LaTeX                                           |
| Component      | Vitest + Testing Library             | UI with mocked IPC                                                                  |
| E2E            | Playwright (Tauri webview)           | import → study → rate → export                                                      |
| Lint/format    | ESLint + Prettier + clippy + rustfmt | CI gate                                                                             |

## 13. Future mobile compatibility

`synapse-core` and the other logic crates are platform-agnostic Rust (zero UI
deps) and compile for iOS/Android unchanged. Tauri v2 mobile reuses the same
core + command layer, swapping only the shell; alternatively the core can be
wrapped via UniFFI for a fully native UI. A CLI reusing the core doubles as a
scripting/test harness. Sync, when built, is one core-level trait shared by all
platforms.

## Milestone roadmap

- **M0** Scaffold: workspace, crates, Tauri shell, CI/lint, ts-rs pipeline,
  AppShell + routing + theme + empty screens.
- **M1** Core + DB: canonical schema, migrations, `Collection`, ports, event
  bus, undo log, command layer, IPC types.
- **M2** Import: v11 + v18 readers, media import, merge.
- **M3** Scheduler: SM-2 + FSRS behind the trait, golden vectors, per-deck switch.
- **M4** Study mode: queue build, card renderer, answer bar, shortcuts.
- **M5** Browser + Editor: virtualized browser, TipTap editor, decks, tags.
- **M6** Export: v2/v3 `.apkg` + `.colpkg`, round-trip gate.
- **M7** Search: Tantivy index + query parser.
- **M8** Statistics: heatmap, retention, forecast, time, difficulty.
- **M9** Polish + plugin/AI architecture: command palette, themes, plugin SDK +
  extension registry, AI extension points (no impl).

Deferred by decision (not gaps): sync implementation, plugin runtime sandbox,
AI provider integration.
