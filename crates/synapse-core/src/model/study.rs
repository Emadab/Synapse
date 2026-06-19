//! Data the study loop needs for one card: everything to render it plus its
//! scheduling state. Rendering (HTML) and the scheduling decision happen in
//! outer layers; core only assembles and persists this.

use crate::scheduling::CardState;

/// Inputs for rendering a card's HTML (consumed by `synapse-render`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardRender {
    /// (field name, value) in note order.
    pub fields: Vec<(String, String)>,
    pub qfmt: String,
    pub afmt: String,
    pub is_cloze: bool,
    pub card_ord: u16,
}

/// One card queued for study: identity, deck, render inputs, scheduling state.
#[derive(Debug, Clone, PartialEq)]
pub struct StudyCard {
    pub id: i64,
    pub deck_id: i64,
    pub render: CardRender,
    pub state: CardState,
}
