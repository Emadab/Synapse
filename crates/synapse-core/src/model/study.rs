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
    /// Space-joined note tags.
    pub tags: String,
    /// Full deck path (e.g. `Spanish::Verbs`), for `{{Deck}}`.
    pub deck: String,
    /// Leaf deck name (e.g. `Verbs`), for `{{Subdeck}}`.
    pub subdeck: String,
    /// Notetype name, for `{{Type}}`.
    pub notetype: String,
    /// Card template display name, for `{{Card}}`.
    pub card_name: String,
    /// 0 = no flag, 1-7 = flag color, for `{{CardFlag}}`.
    pub flag: u8,
    /// Notetype's custom card CSS (`Notetype.config_json.css`), applied
    /// scoped to the card face at study time.
    pub css: String,
    /// Image-occlusion notetype's `"hideAllGuessOne"` / `"hideOneGuessOne"` mode.
    pub occlusion_mode: String,
}

/// One card queued for study: identity, deck, render inputs, scheduling state.
#[derive(Debug, Clone, PartialEq)]
pub struct StudyCard {
    pub id: i64,
    pub note_id: i64,
    pub deck_id: i64,
    pub render: CardRender,
    pub state: CardState,
}
