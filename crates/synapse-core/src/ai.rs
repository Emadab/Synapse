//! AI extension points — architecture only; no provider ships in MVP.
//!
//! `synapse-core` defines the `AIProvider` trait and its value types. A future
//! milestone wires in a concrete provider (OpenAI-compatible, local LLM, …)
//! behind this interface so the application layer and any plugin are decoupled
//! from the choice of provider.
//!
//! Extension points (post-MVP):
//! - Card explanation / hint generation
//! - Note-content suggestions (fill Back from Front)
//! - Deck-topic summarisation
//! - Custom scheduler feedback signal

use crate::error::CoreResult;

/// A text completion request to an AI provider.
#[derive(Debug, Clone)]
pub struct AITextRequest {
    /// System-level instructions (provider-agnostic).
    pub system: Option<String>,
    /// User prompt / content.
    pub prompt: String,
    /// Maximum tokens to generate. `None` = provider default.
    pub max_tokens: Option<u32>,
    /// Temperature 0.0–2.0. `None` = provider default.
    pub temperature: Option<f32>,
}

/// A text completion response from an AI provider.
#[derive(Debug, Clone)]
pub struct AITextResponse {
    /// Generated text.
    pub text: String,
    /// Token counts, if the provider reports them.
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
}

/// Trait for AI text generation. Implement this to wire in any LLM backend.
///
/// Kept out of `ports.rs` intentionally — AI is an optional capability, not a
/// required port. The application layer checks `Option<Arc<dyn AIProvider>>`.
pub trait AIProvider: Send + Sync {
    /// Generate a text completion. Must be non-blocking (the implementation is
    /// expected to wrap async internally if needed).
    fn complete(&self, request: &AITextRequest) -> CoreResult<AITextResponse>;

    /// A human-readable name for this provider (e.g. `"Claude"`, `"Ollama"`).
    fn name(&self) -> &str;
}

/// Predefined extension-point ids. Plugins declare which they implement;
/// the registry dispatches to the first registered provider for each.
pub mod extension_point {
    /// Generate a hint for a card's front side. Input: front HTML. Output: hint text.
    pub const CARD_HINT: &str = "ai.card_hint";
    /// Suggest content for a note's Back field given its Front. Output: HTML.
    pub const NOTE_FILL_BACK: &str = "ai.note_fill_back";
    /// Summarise a deck's topic from its note content. Output: plain text.
    pub const DECK_SUMMARY: &str = "ai.deck_summary";
}
