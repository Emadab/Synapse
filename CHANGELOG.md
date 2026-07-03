# Changelog

All notable changes to Synapse are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versions follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.1.0-beta.3] - 2026-07-04

Diff: [beta.2...beta.3](https://github.com/Emadab/Synapse/compare/v0.1.0-beta.2...v0.1.0-beta.3)

### Added

- Statistics dashboard rebuild: filters, retention trend, FSRS memory-model panels, review heatmap, 30-day forecast
- Full Anki-compatible template engine (conditional sections, filters, cloze) with a modern card design and rich note editor
- Study focus mode — `F` to enter, `Esc` to exit — with a session progress bar and distraction-free HUD
- Customizable home screen: list / grid / hero layouts for the deck browser, with a "due today" hero strip (streak, activity heatmap, 7-day sparkline)
- Sleeker, glassy application shell — consolidated title bar / header chrome into two slim layers, cooler accent palette
- Configurable day-rollover hour for new-card limits
- "Increase today's new limit" control and a revamped deck picker
- Async .apkg/.colpkg import with live progress and bulk-preload merge

### Changed

- Decks and Study merged into a single page; subdeck counts now roll up into their parents

### Fixed

- Stats queries moved off the main thread (previously could freeze the UI while computing)
- Startup backup/index/plugin work moved off the main thread
- Deck options number fields can now be cleared while typing instead of snapping back
- Browse screen's select-all checkbox now shows an indeterminate state correctly
- Stale new-card counts after a day rollover
- Study keyboard shortcuts (`s`/`b`/`r`) no longer fire while typing into the type-answer field

## [0.1.0-beta.2] - 2026-07-01

### Fixed

- Patched a transitive RCE advisory in the e2e devDependency chain (`serialize-javascript`)
- Release and e2e CI workflow fixes

## [0.1.0-beta.1] - 2026-06-24

### Added

- M21: Plugin runtime — sandboxed Worker-based JS plugins with capability enforcement
- M20: FSRS weight optimizer — Adam gradient descent on review history, per-deck or full-collection scope
- M19: Backups, integrity check, DB optimize, media consistency scan, panic hook, auto-backup on startup
- M18: KaTeX LaTeX rendering, audio autoplay + replay (R key), dark-mode image inversion, cloze CSS
- M17: Tag manager (rename/delete/merge), filtered decks (custom study)
- M16: Advanced browser search, bulk operations, card-level browser table
- M15: Suspend/bury/flags, leech detection, sibling bury
- M14: Full deck options (algorithm, steps, FSRS weights, retention)
- M13: Note-type and template editor with live preview
- M12: Add Note UI, card generation, template rendering
- Full FSRS-5 and SM-2 schedulers, switchable per deck
- Bidirectional .apkg/.colpkg import/export (Anki schema v11/v18)
- Statistics dashboards
