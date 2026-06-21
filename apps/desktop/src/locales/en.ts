/**
 * English locale strings. Keys are dot-separated namespaces.
 * Add a new locale file (e.g. `fr.ts`) with the same keys to support more languages.
 * Use `{{var}}` placeholders for interpolation.
 */
export const en = {
  // App shell
  "app.skipToContent": "Skip to content",
  "app.nav.label": "Main navigation",
  "app.version": "v{{version}}",
  "app.toggleTheme": "Toggle theme",
  "app.shortcuts.hint": "Press",
  "app.shortcuts.key": "⌘K",
  "app.shortcuts.hint2": "for commands",

  // Navigation
  "nav.decks": "Decks",
  "nav.study": "Study",
  "nav.browse": "Browse",
  "nav.add": "Add",
  "nav.notetypes": "Note Types",
  "nav.stats": "Stats",
  "nav.settings": "Settings",

  // Deck browser
  "decks.empty.title": "No decks yet",
  "decks.empty.description": "Create your first deck to start studying.",
  "decks.newDeck": "New deck",
  "decks.import": "Import .apkg",
  "decks.options": "Options",
  "decks.study": "Study",

  // Study screen
  "study.pickDeck": "Pick a deck to review.",
  "study.showAnswer": "Show answer",
  "study.sessionCap.prefix": "Study at most",
  "study.sessionCap.suffix": "cards this session (0 = no limit)",
  "study.done.title": "Session complete",
  "study.done.description":
    "You've studied {{count}} cards this session. Come back later to keep going.",
  "study.backToDecks": "Back to decks",
  "study.suspend": "Suspend",
  "study.bury": "Bury",
  "study.flag": "Flag",
  "study.replay": "Replay",
  "study.rating.again": "Again",
  "study.rating.hard": "Hard",
  "study.rating.good": "Good",
  "study.rating.easy": "Easy",

  // Settings
  "settings.title": "Settings",
  "settings.description": "Preferences and appearance.",
  "settings.appearance.title": "Appearance",
  "settings.appearance.description": "Choose how Synapse looks.",
  "settings.theme.light": "Light",
  "settings.theme.dark": "Dark",
  "settings.theme.system": "System",
  "settings.scheduling.title": "Scheduling",
  "settings.scheduling.description":
    "SM-2 and FSRS are both implemented and switchable per deck via deck options.",
  "settings.export.title": "Export",
  "settings.export.description": "Export your full collection as an Anki-compatible .apkg file.",
  "settings.export.button": "Export .apkg",
  "settings.export.busy": "Exporting…",
  "settings.export.done": "Exported.",
  "settings.updates.title": "Updates",
  "settings.updates.checkButton": "Check for updates",
  "settings.updates.checking": "Checking…",
  "settings.updates.upToDate": "Synapse is up to date.",
  "settings.updates.available": "Version {{version}} is available.",
  "settings.updates.downloadLink": "Download from GitHub →",

  // Maintenance
  "maintenance.title": "Maintenance",
  "maintenance.description": "Backups, integrity, and media cleanup.",
  "maintenance.backup.button": "Backup now",
  "maintenance.backup.busy": "Backing up…",
  "maintenance.integrity.button": "Check integrity",
  "maintenance.integrity.busy": "Checking…",
  "maintenance.integrity.healthy": "Database is healthy.",
  "maintenance.optimize.button": "Optimize database",
  "maintenance.optimize.busy": "Optimizing…",
  "maintenance.optimize.done": "Database compacted and optimized.",
  "maintenance.media.button": "Check media",
  "maintenance.media.busy": "Scanning…",
  "maintenance.media.healthy": "All media files are consistent.",

  // Plugins
  "plugins.title": "Plugins",
  "plugins.description": "Extend Synapse with sandboxed plugin scripts.",
  "plugins.empty": "No plugins installed.",
  "plugins.install": "Install plugin…",
  "plugins.installing": "Installing…",
  "plugins.commands.title": "Plugin commands",
  "plugins.run": "Run",

  // Keyboard shortcuts dialog
  "shortcuts.title": "Keyboard shortcuts",
  "shortcuts.close": "Close dialog",

  // Errors
  "error.unknown": "An unknown error occurred.",
  "error.notOpen": "Collection is not open.",
} as const;

export type LocaleKey = keyof typeof en;
