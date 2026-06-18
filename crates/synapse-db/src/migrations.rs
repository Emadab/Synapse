//! Migration runner. Uses SQLite's `user_version` pragma as the schema
//! revision and applies any pending [`schema::MIGRATIONS`] inside a transaction.

use rusqlite::Connection;
use synapse_core::error::{CoreError, CoreResult};

use crate::schema::MIGRATIONS;

fn user_version(conn: &Connection) -> CoreResult<i64> {
    conn.query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|e| CoreError::Storage(e.to_string()))
}

/// Apply every migration newer than the database's current `user_version`.
/// Idempotent: running again on an up-to-date database is a no-op.
pub fn run(conn: &mut Connection) -> CoreResult<()> {
    let current = user_version(conn)?;
    let target = MIGRATIONS.len() as i64;

    for version in current..target {
        let sql = MIGRATIONS[version as usize];
        let tx = conn
            .transaction()
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        tx.execute_batch(sql)
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        // PRAGMA can't be parameterised; the value is a trusted loop counter.
        tx.pragma_update(None, "user_version", version + 1)
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        tx.commit().map_err(|e| CoreError::Storage(e.to_string()))?;
    }
    Ok(())
}
