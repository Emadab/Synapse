//! Media ingestion for the note editor: paste/insert an image or audio file
//! and get back a filename to reference in field HTML (`<img src="…">`,
//! `[sound:…]`). Files are content-addressed with a suffix so identical
//! pastes always resolve to the same on-disk file (no duplicate copies) and
//! same-named-but-different content never collides.

use std::hash::{Hash, Hasher};
use std::path::PathBuf;

use synapse_core::ipc::{IpcError, IpcErrorKind};
use tauri::{AppHandle, Manager};

type IpcResult<T> = Result<T, IpcError>;

fn media_dir(app: &AppHandle) -> Result<PathBuf, IpcError> {
    app.path()
        .app_data_dir()
        .map(|p| p.join("collection.media"))
        .map_err(map_io)
}

fn map_io<E: std::fmt::Display>(e: E) -> IpcError {
    IpcError {
        kind: IpcErrorKind::Storage,
        message: e.to_string(),
    }
}

fn hash_bytes(bytes: &[u8]) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

fn write_content_addressed(dir: &std::path::Path, bytes: &[u8], filename: &str) -> IpcResult<String> {
    let path = std::path::Path::new(filename);
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("media");
    let ext = path.extension().and_then(|e| e.to_str());

    let hash = hash_bytes(bytes);
    let final_name = match ext {
        Some(ext) => format!("{stem}-{hash:016x}.{ext}"),
        None => format!("{stem}-{hash:016x}"),
    };

    let final_path = dir.join(&final_name);
    if !final_path.exists() {
        std::fs::write(&final_path, bytes).map_err(map_io)?;
    }
    Ok(final_name)
}

/// Save a pasted media file (e.g. from clipboard). `filename` is the
/// suggested name; the actual on-disk name is content-addressed —
/// `<stem>-<hash>.<ext>` — so re-saving identical bytes is a no-op and never
/// overwrites unrelated content.
#[tauri::command]
pub fn save_media(app: AppHandle, bytes: Vec<u8>, filename: String) -> IpcResult<String> {
    let dir = media_dir(&app)?;
    std::fs::create_dir_all(&dir).map_err(map_io)?;
    write_content_addressed(&dir, &bytes, &filename)
}

/// Save a media file picked from disk via a file dialog. Reads the file at
/// `source_path` and stores it the same content-addressed way as
/// [`save_media`].
#[tauri::command]
pub fn save_media_from_path(app: AppHandle, source_path: String) -> IpcResult<String> {
    let dir = media_dir(&app)?;
    std::fs::create_dir_all(&dir).map_err(map_io)?;
    let bytes = std::fs::read(&source_path).map_err(map_io)?;
    let filename = std::path::Path::new(&source_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("media")
        .to_string();
    write_content_addressed(&dir, &bytes, &filename)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_bytes_reuse_the_same_filename() {
        let dir = std::env::temp_dir().join(format!("synapse-media-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let a = write_content_addressed(&dir, b"hello", "photo.png").unwrap();
        let b = write_content_addressed(&dir, b"hello", "photo.png").unwrap();
        assert_eq!(a, b, "identical content maps to the same file, no duplicate write");

        let c = write_content_addressed(&dir, b"different", "photo.png").unwrap();
        assert_ne!(a, c, "different content under the same suggested name gets a distinct file");

        assert!(a.starts_with("photo-") && a.ends_with(".png"));

        std::fs::remove_dir_all(&dir).ok();
    }
}
