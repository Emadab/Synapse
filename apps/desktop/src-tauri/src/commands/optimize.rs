use synapse_core::{
    collection::Collection,
    ipc::{FsrsOptimizeResult, IpcError, IpcErrorKind},
    scheduling::FSRS5_DEFAULT_WEIGHTS,
};
use synapse_scheduler::optimizer;
use tauri::State;

type IpcResult<T> = Result<T, IpcError>;

fn sched_err(msg: impl std::fmt::Display) -> IpcError {
    IpcError { kind: IpcErrorKind::Scheduler, message: msg.to_string() }
}

/// Run the FSRS weight optimizer on the collection's review history.
///
/// `deck_id` — scope to one deck (`Some(id)`) or the full collection (`None`).
/// Requires at least 400 review entries; returns an error otherwise.
#[tauri::command]
pub async fn optimize_fsrs(
    deck_id: Option<i64>,
    col: State<'_, Collection>,
) -> IpcResult<FsrsOptimizeResult> {
    // Get current weights (starting point for optimization).
    let initial_weights: [f64; 19] = match deck_id {
        Some(id) => {
            let cfg = col.get_deck_config(id).map_err(IpcError::from)?;
            let mut arr = FSRS5_DEFAULT_WEIGHTS;
            if cfg.fsrs_weights.len() == 19 {
                arr.copy_from_slice(&cfg.fsrs_weights);
            }
            arr
        }
        None => FSRS5_DEFAULT_WEIGHTS,
    };

    // Load revlogs (potentially many; do not block the async runtime with heavy work).
    let revlogs = col.revlogs_for_optimize(deck_id).map_err(IpcError::from)?;

    // Optimizer is CPU-bound (~100ms–2s). Tauri async commands run on a thread
    // pool so this won't block the main/UI thread.
    let result = optimizer::optimize(&revlogs, &initial_weights).map_err(sched_err)?;

    Ok(FsrsOptimizeResult {
        weights: result.weights.to_vec(),
        log_loss_before: result.log_loss_before,
        log_loss_after: result.log_loss_after,
        review_count: result.review_count,
        card_count: result.card_count,
    })
}

/// Persist fitted FSRS weights to a deck's scheduling config.
#[tauri::command]
pub async fn apply_fsrs_weights(
    deck_id: i64,
    weights: Vec<f64>,
    col: State<'_, Collection>,
) -> IpcResult<()> {
    if weights.len() != 19 {
        return Err(IpcError {
            kind: IpcErrorKind::Invalid,
            message: format!("expected 19 weights, got {}", weights.len()),
        });
    }
    let mut cfg = col.get_deck_config(deck_id).map_err(IpcError::from)?;
    cfg.fsrs_weights = weights;
    col.set_deck_config(&cfg).map_err(IpcError::from)
}
