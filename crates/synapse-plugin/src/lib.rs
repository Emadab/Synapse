//! Plugin host: manifest loading, capability enforcement, install/enable/disable.
//!
//! Plugins execute in the frontend (sandboxed Workers). This crate manages
//! filesystem state under `<app-data>/plugins/` and enforces that only declared
//! capabilities are surfaced by the bridge preamble injected before plugin code.
//!
//! Architecture:
//!   `<app-data>/plugins/installed/<plugin-id>/`  ← plugin files (manifest + entry)
//!   `<app-data>/plugins/state.json`              ← `{ enabled: string[] }`

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

// ── Error ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum PluginError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("plugin not found: {0}")]
    NotFound(String),
    #[error("invalid manifest: {0}")]
    InvalidManifest(String),
}

pub type PluginResult<T> = Result<T, PluginError>;

// ── Capability ────────────────────────────────────────────────────────────────

/// Capabilities a plugin may declare in its `plugin.json`. The bridge preamble
/// injected before plugin code exposes only the APIs matching declared capabilities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Capability {
    #[serde(rename = "read:notes")]
    ReadNotes,
    #[serde(rename = "write:cards")]
    WriteCards,
    #[serde(rename = "read:decks")]
    ReadDecks,
    #[serde(rename = "ui:panel")]
    UiPanel,
    #[serde(rename = "ui:command")]
    UiCommand,
    #[serde(rename = "ui:review-button")]
    UiReviewButton,
    #[serde(rename = "net:fetch")]
    NetFetch,
    #[serde(rename = "events:listen")]
    EventsListen,
    #[serde(rename = "ai:card-hint")]
    AiCardHint,
}

impl Capability {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ReadNotes => "read:notes",
            Self::WriteCards => "write:cards",
            Self::ReadDecks => "read:decks",
            Self::UiPanel => "ui:panel",
            Self::UiCommand => "ui:command",
            Self::UiReviewButton => "ui:review-button",
            Self::NetFetch => "net:fetch",
            Self::EventsListen => "events:listen",
            Self::AiCardHint => "ai:card-hint",
        }
    }
}

// ── Manifest ──────────────────────────────────────────────────────────────────

/// Parsed `plugin.json` manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    /// Entry-point JS file path relative to the plugin root.
    pub entry: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(rename = "apiVersion")]
    pub api_version: String,
    #[serde(default)]
    pub permissions: Vec<Capability>,
}

impl PluginManifest {
    pub fn from_dir(dir: &Path) -> PluginResult<Self> {
        let path = dir.join("plugin.json");
        if !path.is_file() {
            return Err(PluginError::NotFound(path.display().to_string()));
        }
        let raw = std::fs::read_to_string(&path)?;
        let m: PluginManifest = serde_json::from_str(&raw)?;
        if m.id.is_empty() {
            return Err(PluginError::InvalidManifest("id is empty".into()));
        }
        if m.entry.is_empty() {
            return Err(PluginError::InvalidManifest("entry is empty".into()));
        }
        Ok(m)
    }

    pub fn permission_strings(&self) -> Vec<String> {
        self.permissions.iter().map(|c| c.as_str().to_string()).collect()
    }
}

// ── Persisted state ───────────────────────────────────────────────────────────

#[derive(Debug, Default, Serialize, Deserialize)]
struct PluginState {
    enabled: Vec<String>,
}

// ── PluginRecord ──────────────────────────────────────────────────────────────

/// A loaded, registered plugin with its on-disk location.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginRecord {
    pub manifest: PluginManifest,
    pub enabled: bool,
    pub install_path: PathBuf,
}

// ── PluginManager ─────────────────────────────────────────────────────────────

/// Manages the `<app-data>/plugins/` directory tree.
/// Thread-safe: holds only `PathBuf` fields; all I/O is synchronous.
pub struct PluginManager {
    installed_dir: PathBuf,
    state_path: PathBuf,
}

impl PluginManager {
    pub fn new(app_data_dir: &Path) -> Self {
        let base = app_data_dir.join("plugins");
        Self {
            installed_dir: base.join("installed"),
            state_path: base.join("state.json"),
        }
    }

    fn load_state(&self) -> PluginState {
        std::fs::read_to_string(&self.state_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn save_state(&self, state: &PluginState) -> PluginResult<()> {
        if let Some(parent) = self.state_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&self.state_path, serde_json::to_string_pretty(state)?)?;
        Ok(())
    }

    /// List all installed plugins with their enabled state, sorted by name.
    pub fn list(&self) -> PluginResult<Vec<PluginRecord>> {
        let state = self.load_state();
        let mut records = Vec::new();
        let Ok(entries) = std::fs::read_dir(&self.installed_dir) else {
            return Ok(vec![]);
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            match PluginManifest::from_dir(&path) {
                Ok(manifest) => {
                    let enabled = state.enabled.contains(&manifest.id);
                    records.push(PluginRecord { enabled, install_path: path, manifest });
                }
                Err(e) => {
                    tracing::warn!("skipping plugin dir {:?}: {e}", path);
                }
            }
        }
        records.sort_by(|a, b| a.manifest.name.cmp(&b.manifest.name));
        Ok(records)
    }

    /// Install a plugin by copying `src_dir` into the managed directory.
    /// Returns the new `PluginRecord` (disabled by default).
    pub fn install(&self, src_dir: &Path) -> PluginResult<PluginRecord> {
        let manifest = PluginManifest::from_dir(src_dir)?;
        let dest = self.installed_dir.join(&manifest.id);
        std::fs::create_dir_all(&dest)?;
        copy_dir_all(src_dir, &dest)?;
        Ok(PluginRecord { enabled: false, install_path: dest, manifest })
    }

    /// Install the bundled word-count sample plugin (no-op if already installed).
    pub fn ensure_sample(&self) -> PluginResult<()> {
        let dest = self.installed_dir.join("synapse-word-count");
        if dest.is_dir() {
            return Ok(());
        }
        std::fs::create_dir_all(&dest)?;
        std::fs::write(dest.join("plugin.json"), SAMPLE_MANIFEST)?;
        std::fs::write(dest.join("index.js"), SAMPLE_ENTRY)?;
        Ok(())
    }

    pub fn enable(&self, id: &str) -> PluginResult<()> {
        let mut state = self.load_state();
        if !state.enabled.iter().any(|s| s == id) {
            state.enabled.push(id.to_string());
        }
        self.save_state(&state)
    }

    pub fn disable(&self, id: &str) -> PluginResult<()> {
        let mut state = self.load_state();
        state.enabled.retain(|s| s != id);
        self.save_state(&state)
    }

    /// Read the entry-point JS source for a plugin so the host can inject it
    /// into a sandboxed Worker (with the bridge preamble prepended).
    pub fn read_entry(&self, id: &str) -> PluginResult<String> {
        let records = self.list()?;
        let record = records
            .into_iter()
            .find(|r| r.manifest.id == id)
            .ok_or_else(|| PluginError::NotFound(id.to_string()))?;
        let entry_path = record.install_path.join(&record.manifest.entry);
        Ok(std::fs::read_to_string(&entry_path)?)
    }
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)?.flatten() {
        let ty = entry.file_type()?;
        let dest = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dest)?;
        } else {
            std::fs::copy(entry.path(), dest)?;
        }
    }
    Ok(())
}

// ── Sample plugin ─────────────────────────────────────────────────────────────

const SAMPLE_MANIFEST: &str = r#"{
  "id": "synapse-word-count",
  "name": "Word Count",
  "version": "1.0.0",
  "entry": "index.js",
  "description": "Sample plugin — counts words in a text field and registers a command.",
  "author": "Synapse",
  "apiVersion": "0.1.0",
  "permissions": ["ui:command"]
}"#;

const SAMPLE_ENTRY: &str = r#"// Word Count sample plugin (runs in a sandboxed Worker).
// SynapsePlugin is provided by the host bridge preamble.

SynapsePlugin.registerCommand(
  "synapse-word-count.hello",
  "Word Count: Hello from plugin"
);

SynapsePlugin.onCommand("synapse-word-count.hello", function () {
  SynapsePlugin.showToast("Word Count plugin is active and running in a sandboxed Worker!");
});
"#;

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write_plugin_dir(parent: &Path, manifest: &str) -> PathBuf {
        let dir = parent.join("test-plugin");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("plugin.json"), manifest).unwrap();
        fs::write(dir.join("index.js"), "SynapsePlugin.registerCommand('x', 'X');").unwrap();
        dir
    }

    const MANIFEST_JSON: &str = r#"{
        "id": "test-plugin",
        "name": "Test Plugin",
        "version": "1.2.3",
        "entry": "index.js",
        "description": "A test plugin",
        "apiVersion": "0.1.0",
        "permissions": ["ui:command", "events:listen"]
    }"#;

    #[test]
    fn manifest_parses_capabilities() {
        let tmp = TempDir::new().unwrap();
        let dir = write_plugin_dir(tmp.path(), MANIFEST_JSON);
        let m = PluginManifest::from_dir(&dir).unwrap();
        assert_eq!(m.id, "test-plugin");
        assert_eq!(m.version, "1.2.3");
        assert_eq!(
            m.permissions,
            vec![Capability::UiCommand, Capability::EventsListen]
        );
        assert_eq!(
            m.permission_strings(),
            vec!["ui:command", "events:listen"]
        );
    }

    #[test]
    fn install_enable_disable_persists() {
        let tmp = TempDir::new().unwrap();
        let src = write_plugin_dir(tmp.path(), MANIFEST_JSON);
        let app_dir = tmp.path().join("appdata");
        let mgr = PluginManager::new(&app_dir);

        let record = mgr.install(&src).unwrap();
        assert_eq!(record.manifest.id, "test-plugin");
        assert!(!record.enabled);

        // Enable → list shows enabled
        mgr.enable("test-plugin").unwrap();
        let records = mgr.list().unwrap();
        assert!(records.iter().any(|r| r.manifest.id == "test-plugin" && r.enabled));

        // Disable → back to disabled
        mgr.disable("test-plugin").unwrap();
        let records = mgr.list().unwrap();
        assert!(records.iter().any(|r| r.manifest.id == "test-plugin" && !r.enabled));
    }

    #[test]
    fn install_copies_files_and_entry_readable() {
        let tmp = TempDir::new().unwrap();
        let src = write_plugin_dir(tmp.path(), MANIFEST_JSON);
        let app_dir = tmp.path().join("appdata");
        let mgr = PluginManager::new(&app_dir);
        mgr.install(&src).unwrap();

        // Files copied
        let dest = app_dir.join("plugins/installed/test-plugin");
        assert!(dest.join("plugin.json").exists());
        assert!(dest.join("index.js").exists());

        // Entry readable
        let entry = mgr.read_entry("test-plugin").unwrap();
        assert!(!entry.is_empty());
    }
}
