# ADR-002: Dual scheduler — SM-2 and FSRS-5, switchable per deck

**Status:** Accepted  
**Date:** 2024-11-15

## Context

Anki has used SM-2 for decades; most imported collections have SM-2 history. FSRS-5 is measurably better for long-term retention but requires learning a new mental model. Users migrating from Anki should not be forced to switch.

## Decision

Implement both SM-2 and FSRS-5 in `synapse-scheduler`. Each deck's `deck_config` JSON records `algorithm: "sm2" | "fsrs"` and the full per-algorithm parameters (steps, ease, FSRS weights, etc.). At answer time `collection.rs` reads the deck's config and dispatches to the correct algorithm.

FSRS-5 weights are the published defaults; the optimizer (M20) can fit per-user weights from `revlog`.

## Consequences

- Deck-options UI must expose both algorithm tabs; switching algorithms is non-destructive (parameters stored separately).
- Interval preview on answer buttons must recalculate when algorithm changes.
- SM-2 and FSRS scheduling golden-vector tests live in `crates/synapse-scheduler/tests/`.
- Anki `.apkg` import maps existing `ease_factor` + `interval` to SM-2 state; FSRS state columns (`fsrs_stability`, `fsrs_difficulty`, `fsrs_last_review`) are nullable and populated by the optimizer.
