use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::OnceLock,
};

use regex::Regex;
use rusqlite::{Connection, DatabaseName};
use synapse_core::error::{CoreError, CoreResult};

fn err(e: impl std::fmt::Display) -> CoreError {
    CoreError::Storage(e.to_string())
}

/// Hot-copy the main database to `dest_path` using SQLite's online backup API.
/// Safe to call while the DB is in active use.
pub fn backup_db(conn: &Connection, dest_path: &Path) -> CoreResult<()> {
    conn.backup(DatabaseName::Main, dest_path, None)
        .map_err(err)
}

/// Run `PRAGMA integrity_check`. Returns empty vec on a healthy database,
/// or a list of error strings otherwise.
pub fn integrity_check(conn: &Connection) -> CoreResult<Vec<String>> {
    let mut stmt = conn
        .prepare("PRAGMA integrity_check")
        .map_err(err)?;
    let rows = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(err)?;
    let mut results = Vec::new();
    for row in rows {
        let s = row.map_err(err)?;
        if s != "ok" {
            results.push(s);
        }
    }
    Ok(results)
}

/// Run `PRAGMA optimize` then `VACUUM` to compact and tune the database.
pub fn optimize(conn: &Connection) -> CoreResult<()> {
    conn.execute_batch("PRAGMA optimize; VACUUM;").map_err(err)
}

/// Extract media filenames referenced in note fields.
/// Matches `<img src="name">` and `[sound:name]` patterns.
pub fn note_media_refs(conn: &Connection) -> CoreResult<Vec<String>> {
    static RE_IMG: OnceLock<Regex> = OnceLock::new();
    static RE_SND: OnceLock<Regex> = OnceLock::new();
    let re_img = RE_IMG.get_or_init(|| Regex::new(r#"(?i)<img\b[^>]*\ssrc="([^":/][^"]*)""#).unwrap());
    let re_snd = RE_SND.get_or_init(|| Regex::new(r"\[sound:([^\]]+)\]").unwrap());

    let mut stmt = conn.prepare("SELECT fields FROM notes").map_err(err)?;
    let mut refs: Vec<String> = Vec::new();
    let rows = stmt.query_map([], |r| r.get::<_, String>(0)).map_err(err)?;
    for row in rows {
        let flds = row.map_err(err)?;
        for cap in re_img.captures_iter(&flds) {
            refs.push(cap[1].to_string());
        }
        for cap in re_snd.captures_iter(&flds) {
            refs.push(cap[1].to_string());
        }
    }
    refs.sort_unstable();
    refs.dedup();
    Ok(refs)
}

/// Create a `.zip` archive at `zip_path` containing:
/// - `collection.sqlite` (the hot-backup copy at `db_backup`)
/// - all files under `media_dir/` (non-recursive, skipping dirs)
///
/// Returns the archive size in bytes.
pub fn create_zip(db_backup: &Path, media_dir: &Path, zip_path: &Path) -> CoreResult<u64> {
    let file = std::fs::File::create(zip_path).map_err(err)?;
    let mut zip = zip::ZipWriter::new(file);
    let opts: zip::write::FileOptions<()> = zip::write::FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    // Add the DB backup.
    zip.start_file("collection.sqlite", opts).map_err(err)?;
    let mut db_bytes = Vec::new();
    std::fs::File::open(db_backup)
        .map_err(err)?
        .read_to_end(&mut db_bytes)
        .map_err(err)?;
    zip.write_all(&db_bytes).map_err(err)?;

    // Add media files (flat, non-recursive).
    if media_dir.is_dir() {
        for entry in std::fs::read_dir(media_dir).map_err(err)? {
            let entry = entry.map_err(err)?;
            if entry.file_type().map_err(err)?.is_file() {
                let name = entry.file_name().to_string_lossy().into_owned();
                zip.start_file(format!("media/{name}"), opts).map_err(err)?;
                let mut bytes = Vec::new();
                std::fs::File::open(entry.path())
                    .map_err(err)?
                    .read_to_end(&mut bytes)
                    .map_err(err)?;
                zip.write_all(&bytes).map_err(err)?;
            }
        }
    }
    zip.finish().map_err(err)?;

    let size = std::fs::metadata(zip_path).map_err(err)?.len();
    Ok(size)
}

/// Validate a SQLite file with `PRAGMA integrity_check`. Returns errors, empty = ok.
pub fn validate_sqlite_file(path: &Path) -> CoreResult<Vec<String>> {
    let conn = Connection::open(path).map_err(err)?;
    integrity_check(&conn)
}

/// Extract `collection.sqlite` from a backup zip to `dest_path`.
/// Returns an error if the zip doesn't contain the expected file.
pub fn extract_db_from_zip(zip_path: &Path, dest_path: &Path) -> CoreResult<()> {
    let file = std::fs::File::open(zip_path).map_err(err)?;
    let mut archive = zip::ZipArchive::new(file).map_err(err)?;
    let mut entry = archive
        .by_name("collection.sqlite")
        .map_err(|_| CoreError::Storage("backup zip missing collection.sqlite".into()))?;
    let mut out = std::fs::File::create(dest_path).map_err(err)?;
    std::io::copy(&mut entry, &mut out).map_err(err)?;
    Ok(())
}

/// List `.zip` files in `backup_dir`, sorted newest-first.
/// Returns `(filename, modified_ms, size_bytes)`.
pub fn list_zips(backup_dir: &Path) -> CoreResult<Vec<(String, i64, u64)>> {
    if !backup_dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut entries: Vec<(String, i64, u64)> = std::fs::read_dir(backup_dir)
        .map_err(err)?
        .filter_map(|e| {
            let e = e.ok()?;
            let name = e.file_name().to_string_lossy().into_owned();
            if !name.ends_with(".zip") {
                return None;
            }
            let meta = e.metadata().ok()?;
            let size = meta.len();
            let modified = meta
                .modified()
                .ok()?
                .duration_since(std::time::UNIX_EPOCH)
                .ok()
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0);
            Some((name, modified, size))
        })
        .collect();
    entries.sort_by_key(|e| -(e.1)); // newest first (negate ms timestamp)
    Ok(entries)
}

/// Delete oldest zips in `backup_dir` keeping at most `keep` files.
pub fn rotate_backups(backup_dir: &Path, keep: usize) -> CoreResult<()> {
    let mut entries = list_zips(backup_dir)?;
    // entries is newest-first; truncate to keep newest `keep`.
    if entries.len() <= keep {
        return Ok(());
    }
    entries.drain(..keep); // keep the first `keep` (newest)
    for (name, _, _) in entries {
        let _ = std::fs::remove_file(backup_dir.join(name));
    }
    Ok(())
}

/// Full paths helper.
pub fn backup_path(backup_dir: &Path, name: &str) -> PathBuf {
    backup_dir.join(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::SqliteStorage;

    fn make_db() -> SqliteStorage {
        SqliteStorage::open_in_memory().unwrap()
    }

    #[test]
    fn integrity_check_clean_db_returns_ok() {
        let s = make_db();
        let conn = s.lock();
        let errs = integrity_check(&conn).unwrap();
        assert!(errs.is_empty(), "unexpected errors: {errs:?}");
    }

    #[test]
    fn optimize_runs_without_error() {
        let s = make_db();
        let conn = s.lock();
        optimize(&conn).unwrap();
    }

    #[test]
    fn note_media_refs_empty_collection() {
        let s = make_db();
        let conn = s.lock();
        let refs = note_media_refs(&conn).unwrap();
        assert!(refs.is_empty());
    }
}
