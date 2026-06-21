//! Card lifecycle IPC commands: suspend, bury, flag.

use synapse_core::ipc::{IpcError, IpcErrorKind};
use synapse_core::Collection;
use tauri::State;

type IpcResult<T> = Result<T, IpcError>;

fn ipc_err(msg: impl std::fmt::Display) -> IpcError {
    IpcError {
        kind: IpcErrorKind::Storage,
        message: msg.to_string(),
    }
}

#[tauri::command]
pub fn suspend_cards(collection: State<'_, Collection>, card_ids: Vec<i64>) -> IpcResult<()> {
    collection.suspend_cards(&card_ids).map_err(ipc_err)
}

#[tauri::command]
pub fn unsuspend_cards(collection: State<'_, Collection>, card_ids: Vec<i64>) -> IpcResult<()> {
    collection.unsuspend_cards(&card_ids).map_err(ipc_err)
}

#[tauri::command]
pub fn bury_cards(collection: State<'_, Collection>, card_ids: Vec<i64>) -> IpcResult<()> {
    collection.bury_cards(&card_ids).map_err(ipc_err)
}

#[tauri::command]
pub fn set_card_flag(
    collection: State<'_, Collection>,
    card_ids: Vec<i64>,
    flag: u8,
) -> IpcResult<()> {
    if flag > 7 {
        return Err(IpcError {
            kind: IpcErrorKind::Invalid,
            message: format!("flag {flag} out of range 0–7"),
        });
    }
    collection.set_card_flag(&card_ids, flag).map_err(ipc_err)
}
