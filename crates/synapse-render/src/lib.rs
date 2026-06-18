//! # synapse-render
//!
//! Turns a note + a card template into the HTML shown during study. Supports
//! Anki's templating: `{{Field}}`, `{{#Field}}…{{/Field}}` conditionals,
//! `{{cloze:Field}}`, `{{type:Field}}`, special fields, and LaTeX delimiters.
//! Pure and UI-free, so it is reused by desktop, mobile and export preview.
//!
//! Renderer lands in M4 (study mode).
