//! Undo log.
//!
//! Each mutating use-case records an *inverse* action that, given storage and
//! the current time, reverses the change. This is separate from the event bus:
//! events are for notification, the undo log is for reversal. Redo (replaying
//! forward actions popped during undo) is a later-milestone extension.

use crate::error::CoreResult;
use crate::ports::Storage;

/// An inverse action: given storage and "now", undo one operation.
pub type UndoAction = Box<dyn FnOnce(&dyn Storage, i64) -> CoreResult<()> + Send>;

pub struct UndoStep {
    pub description: String,
    pub(crate) action: UndoAction,
}

impl UndoStep {
    pub(crate) fn run(self, storage: &dyn Storage, now_ms: i64) -> CoreResult<()> {
        (self.action)(storage, now_ms)
    }
}

/// LIFO stack of reversible operations.
#[derive(Default)]
pub struct UndoLog {
    steps: Vec<UndoStep>,
}

impl UndoLog {
    pub fn record(&mut self, description: impl Into<String>, action: UndoAction) {
        self.steps.push(UndoStep {
            description: description.into(),
            action,
        });
    }

    /// Description of the operation that would be undone next, if any.
    pub fn peek(&self) -> Option<&str> {
        self.steps.last().map(|s| s.description.as_str())
    }

    pub fn pop(&mut self) -> Option<UndoStep> {
        self.steps.pop()
    }

    pub fn len(&self) -> usize {
        self.steps.len()
    }

    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }
}
