use std::path::PathBuf;

use synapse_core::{
    collection::Collection,
    ipc::{BackupInfo, IpcError, IpcErrorKind, MediaReport},
};
use synapse_db::backup;
use tauri::{AppHandle, Manager, State};

type IpcResult<T> = Result<T, IpcError>;

const KEEP_BACKUPS: usize = 20;

fn backup_dir(app: &AppHandle) -> Result<PathBuf, IpcError> {
    app.path()
        .app_data_dir()
        .map(|p| p.join("backups"))
        .map_err(|e| IpcError {
            kind: IpcErrorKind::Storage,
            message: e.to_string(),
        })
}

fn map_io<E: std::fmt::Display>(e: E) -> IpcError {
    IpcError {
        kind: IpcErrorKind::Storage,
        message: e.to_string(),
    }
}

/// Create a backup zip and return its metadata.
#[tauri::command]
pub async fn create_backup(
    app: AppHandle,
    col: State<'_, Collection>,
) -> IpcResult<BackupInfo> {
    let dir = backup_dir(&app)?;
    std::fs::create_dir_all(&dir).map_err(map_io)?;

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let name = format!("{now_ms}.zip");
    let zip_path = dir.join(&name);

    // Hot-copy the DB to a temp file first so we can zip while the DB is live.
    let tmp = dir.join(format!("{now_ms}.tmp.sqlite"));
    col.backup_db(&tmp).map_err(IpcError::from)?;

    let media_dir = app
        .path()
        .app_data_dir()
        .map(|p| p.join("collection.media"))
        .map_err(map_io)?;

    let size_bytes = backup::create_zip(&tmp, &media_dir, &zip_path)
        .map_err(IpcError::from)?;
    let _ = std::fs::remove_file(&tmp);

    backup::rotate_backups(&dir, KEEP_BACKUPS).map_err(IpcError::from)?;

    Ok(BackupInfo { name, created_ms: now_ms, size_bytes: size_bytes as i64 })
}

/// List all existing backups, newest first.
#[tauri::command]
pub async fn list_backups(app: AppHandle) -> IpcResult<Vec<BackupInfo>> {
    let dir = backup_dir(&app)?;
    let entries = backup::list_zips(&dir).map_err(IpcError::from)?;
    Ok(entries
        .into_iter()
        .map(|(name, created_ms, size_bytes)| BackupInfo {
            name,
            created_ms,
            size_bytes: size_bytes as i64,
        })
        .collect())
}

/// Restore a backup by name (zip filename).
///
/// **WARNING**: Overwrites the live database file. The app must be restarted
/// after this command returns for changes to take effect. The caller must
/// obtain explicit user confirmation before invoking this command.
#[tauri::command]
pub async fn restore_backup(
    app: AppHandle,
    name: String,
) -> IpcResult<()> {
    let dir = backup_dir(&app)?;
    let zip_path = dir.join(&name);
    if !zip_path.is_file() {
        return Err(IpcError {
            kind: IpcErrorKind::NotFound,
            message: format!("backup '{name}' not found"),
        });
    }

    let data_dir = app.path().app_data_dir().map_err(map_io)?;
    let db_path = data_dir.join("collection.sqlite");

    // Extract to a temp file first so we can validate before overwriting.
    let tmp = data_dir.join("collection.restore.tmp");
    backup::extract_db_from_zip(&zip_path, &tmp).map_err(IpcError::from)?;

    // Validate the extracted DB.
    {
        let errors = backup::validate_sqlite_file(&tmp).map_err(IpcError::from)?;
        if !errors.is_empty() {
            let _ = std::fs::remove_file(&tmp);
            return Err(IpcError {
                kind: IpcErrorKind::Format,
                message: format!("backup integrity check failed: {}", errors.join("; ")),
            });
        }
    }

    // Overwrite the live DB file. The app must restart to pick up the change.
    std::fs::rename(&tmp, &db_path).map_err(map_io)?;
    Ok(())
}

/// Run `PRAGMA integrity_check`. Returns empty list when healthy.
#[tauri::command]
pub async fn check_integrity(col: State<'_, Collection>) -> IpcResult<Vec<String>> {
    col.integrity_check().map_err(IpcError::from)
}

/// Run `PRAGMA optimize; VACUUM` to compact and tune the database.
#[tauri::command]
pub async fn optimize_db(col: State<'_, Collection>) -> IpcResult<()> {
    col.optimize().map_err(IpcError::from)
}

/// Scan media references in notes against files on disk.
/// Returns orphan files (on disk but unreferenced) and missing files
/// (referenced in notes but absent from disk).
#[tauri::command]
pub async fn check_media(
    app: AppHandle,
    col: State<'_, Collection>,
) -> IpcResult<MediaReport> {
    let media_dir = app
        .path()
        .app_data_dir()
        .map(|p| p.join("collection.media"))
        .map_err(map_io)?;

    let refs = col.note_media_refs().map_err(IpcError::from)?;

    let disk_files: std::collections::HashSet<String> = if media_dir.is_dir() {
        std::fs::read_dir(&media_dir)
            .map_err(map_io)?
            .filter_map(|e| {
                let e = e.ok()?;
                if e.file_type().ok()?.is_file() {
                    Some(e.file_name().to_string_lossy().into_owned())
                } else {
                    None
                }
            })
            .collect()
    } else {
        std::collections::HashSet::new()
    };

    let ref_set: std::collections::HashSet<String> = refs.into_iter().collect();

    let orphan_files = disk_files
        .difference(&ref_set)
        .cloned()
        .collect::<Vec<_>>()
        .tap_sort();
    let missing_files = ref_set
        .difference(&disk_files)
        .cloned()
        .collect::<Vec<_>>()
        .tap_sort();

    Ok(MediaReport { orphan_files, missing_files })
}

trait TapSort {
    fn tap_sort(self) -> Self;
}
impl TapSort for Vec<String> {
    fn tap_sort(mut self) -> Self {
        self.sort_unstable();
        self
    }
}
