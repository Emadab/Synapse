//! FSRS-5 weight optimizer.
//!
//! Fits the 19 FSRS-5 weights to a user's review history using Adam gradient
//! descent with numerical (central-difference) gradients. No external ML crates
//! required — the objective is a scalar log-loss computed by replaying the FSRS
//! forward simulation.
//!
//! # Algorithm
//! 1. Group revlog entries by card, sort by timestamp.
//! 2. Build a `Vec<Vec<Review>>` — one inner vec per card, each entry holding
//!    (elapsed_days_since_previous_review, rating 1-4).
//! 3. Evaluate cross-entropy loss by simulating FSRS forward for each sequence.
//! 4. Compute numerical gradient (central differences, h = 1e-5).
//! 5. Update weights via Adam (lr = 0.01, β₁ = 0.9, β₂ = 0.999).
//! 6. Clamp weights to per-index safe bounds after each step.
//! 7. Run 200 steps; return before/after metrics.
//!
//! Minimum 400 reviews required (Anki's threshold).

use synapse_core::model::Revlog;

const DECAY: f64 = -0.5;
const FACTOR: f64 = 19.0 / 81.0;

/// Minimum reviews to attempt optimization.
pub const MIN_REVIEWS: usize = 400;

/// Result of a completed optimization run.
#[derive(Debug, Clone)]
pub struct OptimizeResult {
    pub weights: [f64; 19],
    pub log_loss_before: f64,
    pub log_loss_after: f64,
    /// Number of (card, review) pairs used in training.
    pub review_count: usize,
    /// Number of card sequences.
    pub card_count: usize,
}

/// One review in a sequence.
#[derive(Debug, Clone, Copy)]
struct Review {
    elapsed_days: f64,
    rating: i64, // 1..=4
}

/// Build per-card review sequences from a flat revlog.
/// Filters to review_kind in {0, 1, 2} (learn / review / relearn).
/// Sequences with fewer than 2 reviews are dropped (nothing to predict).
fn build_sequences(revlogs: &[Revlog]) -> Vec<Vec<Review>> {
    use std::collections::HashMap;
    let mut by_card: HashMap<i64, Vec<&Revlog>> = HashMap::new();
    for rl in revlogs {
        if rl.review_kind <= 2 {
            by_card.entry(rl.card_id).or_default().push(rl);
        }
    }

    let mut sequences = Vec::new();
    for (_, mut entries) in by_card {
        entries.sort_by_key(|e| e.id);
        if entries.len() < 2 {
            continue;
        }
        let mut seq = Vec::with_capacity(entries.len());
        let mut prev_ms = entries[0].id;
        seq.push(Review { elapsed_days: 0.0, rating: entries[0].ease.clamp(1, 4) });
        for e in &entries[1..] {
            let elapsed = ((e.id - prev_ms) as f64 / 86_400_000.0).max(0.0);
            seq.push(Review { elapsed_days: elapsed, rating: e.ease.clamp(1, 4) });
            prev_ms = e.id;
        }
        sequences.push(seq);
    }
    sequences
}

// ---------------------------------------------------------------------------
// FSRS forward simulation (parameterised by `w`)
// ---------------------------------------------------------------------------

fn initial_stability(w: &[f64; 19], g: i64) -> f64 {
    w[(g - 1) as usize].max(0.1)
}

fn initial_difficulty(w: &[f64; 19], g: i64) -> f64 {
    (w[4] - (w[5] * (g - 1) as f64).exp() + 1.0).clamp(1.0, 10.0)
}

fn difficulty_easy_anchor(w: &[f64; 19]) -> f64 {
    initial_difficulty(w, 4)
}

fn next_difficulty(w: &[f64; 19], d: f64, g: i64) -> f64 {
    let delta = -w[6] * (g - 3) as f64;
    let damped = d + delta * (10.0 - d) / 9.0;
    (w[7] * difficulty_easy_anchor(w) + (1.0 - w[7]) * damped).clamp(1.0, 10.0)
}

fn retrievability(elapsed: f64, stability: f64) -> f64 {
    (1.0 + FACTOR * elapsed / stability.max(0.1)).powf(DECAY)
}

fn success_stability(w: &[f64; 19], d: f64, s: f64, r: f64, g: i64) -> f64 {
    let hard_penalty = if g == 2 { w[15] } else { 1.0 };
    let easy_bonus = if g == 4 { w[16] } else { 1.0 };
    let growth = (w[8].exp())
        * (11.0 - d)
        * s.powf(-w[9])
        * ((w[10] * (1.0 - r)).exp() - 1.0)
        * hard_penalty
        * easy_bonus;
    (s * (1.0 + growth)).max(0.1)
}

fn forget_stability(w: &[f64; 19], d: f64, s: f64, r: f64) -> f64 {
    let sf = w[11] * d.powf(-w[12]) * ((s + 1.0).powf(w[13]) - 1.0) * (w[14] * (1.0 - r)).exp();
    sf.clamp(0.1, s)
}

// ---------------------------------------------------------------------------
// Loss function
// ---------------------------------------------------------------------------

/// Binary cross-entropy loss averaged over all (card, review) prediction pairs.
fn compute_loss(w: &[f64; 19], sequences: &[Vec<Review>]) -> f64 {
    const EPS: f64 = 1e-9;
    let mut total_loss = 0.0f64;
    let mut count = 0usize;

    for seq in sequences {
        let first = seq[0];
        let g0 = first.rating;
        let mut s = initial_stability(w, g0);
        let mut d = initial_difficulty(w, g0);

        for review in &seq[1..] {
            let g = review.rating;
            let r = retrievability(review.elapsed_days, s);
            let y = if g >= 2 { 1.0f64 } else { 0.0f64 };
            total_loss -= y * r.clamp(EPS, 1.0 - EPS).ln()
                + (1.0 - y) * (1.0 - r).clamp(EPS, 1.0 - EPS).ln();
            count += 1;

            let new_s = if g == 1 {
                forget_stability(w, d, s, r)
            } else {
                success_stability(w, d, s, r, g)
            };
            let new_d = next_difficulty(w, d, g);
            s = new_s;
            d = new_d;
        }
    }

    if count == 0 { f64::INFINITY } else { total_loss / count as f64 }
}

// ---------------------------------------------------------------------------
// Numerical gradient (central differences)
// ---------------------------------------------------------------------------

fn compute_gradient(w: &[f64; 19], sequences: &[Vec<Review>]) -> [f64; 19] {
    const H: f64 = 1e-5;
    let mut grad = [0.0f64; 19];
    let mut wm = *w;
    let mut wp = *w;
    for i in 0..19 {
        wm[i] = w[i] - H;
        wp[i] = w[i] + H;
        grad[i] = (compute_loss(&wp, sequences) - compute_loss(&wm, sequences)) / (2.0 * H);
        wm[i] = w[i];
        wp[i] = w[i];
    }
    grad
}

// ---------------------------------------------------------------------------
// Weight bounds (clamp after each Adam step)
// ---------------------------------------------------------------------------

const BOUNDS: [(f64, f64); 19] = [
    (0.1, 40.0),   // w0  initial stability Again
    (0.1, 40.0),   // w1  initial stability Hard
    (0.1, 40.0),   // w2  initial stability Good
    (0.1, 40.0),   // w3  initial stability Easy
    (1.0, 10.0),   // w4  initial difficulty anchor
    (0.01, 5.0),   // w5  difficulty exp scale
    (0.01, 5.0),   // w6  difficulty delta
    (0.0, 1.0),    // w7  mean-reversion factor
    (0.0, 5.0),    // w8  stability growth (exponentiated)
    (0.0, 3.0),    // w9  stability decay
    (0.01, 5.0),   // w10 success R coeff
    (0.01, 5.0),   // w11 lapse stability base
    (0.01, 3.0),   // w12 lapse D exponent
    (0.01, 3.0),   // w13 lapse S+1 exponent
    (0.01, 5.0),   // w14 lapse R coeff
    (0.01, 1.5),   // w15 hard penalty
    (1.0, 5.0),    // w16 easy bonus
    (0.01, 5.0),   // w17 (unused in current formula; kept in array)
    (0.01, 5.0),   // w18 (unused in current formula; kept in array)
];

fn clamp_weights(w: &mut [f64; 19]) {
    for (i, wi) in w.iter_mut().enumerate() {
        let (lo, hi) = BOUNDS[i];
        *wi = wi.clamp(lo, hi);
    }
}

// ---------------------------------------------------------------------------
// Adam optimizer
// ---------------------------------------------------------------------------

fn adam_step(
    w: &mut [f64; 19],
    m: &mut [f64; 19],
    v: &mut [f64; 19],
    grad: &[f64; 19],
    t: u32,
) {
    const LR: f64 = 0.01;
    const B1: f64 = 0.9;
    const B2: f64 = 0.999;
    const EPS: f64 = 1e-8;

    let b1t = B1.powi(t as i32);
    let b2t = B2.powi(t as i32);

    for i in 0..19 {
        m[i] = B1 * m[i] + (1.0 - B1) * grad[i];
        v[i] = B2 * v[i] + (1.0 - B2) * grad[i] * grad[i];
        let m_hat = m[i] / (1.0 - b1t);
        let v_hat = v[i] / (1.0 - b2t);
        w[i] -= LR * m_hat / (v_hat.sqrt() + EPS);
    }
    clamp_weights(w);
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Fit FSRS-5 weights from review history.
///
/// `initial_weights` is the starting point (typically the current deck weights
/// or the FSRS-5 defaults). Returns an error if the history is too short.
pub fn optimize(
    revlogs: &[Revlog],
    initial_weights: &[f64; 19],
) -> Result<OptimizeResult, String> {
    let sequences = build_sequences(revlogs);
    let review_count: usize = sequences.iter().map(|s| s.len()).sum();

    if review_count < MIN_REVIEWS {
        return Err(format!(
            "insufficient review history: {review_count} reviews (minimum {MIN_REVIEWS})"
        ));
    }

    let card_count = sequences.len();
    let loss_before = compute_loss(initial_weights, &sequences);

    let mut w = *initial_weights;
    let mut m = [0.0f64; 19];
    let mut v = [0.0f64; 19];

    for t in 1u32..=200 {
        let grad = compute_gradient(&w, &sequences);
        adam_step(&mut w, &mut m, &mut v, &grad, t);
    }

    let loss_after = compute_loss(&w, &sequences);

    Ok(OptimizeResult {
        weights: w,
        log_loss_before: loss_before,
        log_loss_after: loss_after,
        review_count,
        card_count,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use synapse_core::scheduling::FSRS5_DEFAULT_WEIGHTS;

    fn synthetic_revlog() -> Vec<Revlog> {
        // Simulate 50 cards, each with 10 reviews, alternating Good/Easy.
        let mut logs = Vec::new();
        let mut ts = 1_700_000_000_000i64; // base timestamp ms
        for card_id in 1..=50i64 {
            let mut prev_ts = ts;
            for review in 0..10u32 {
                let rating = if review % 5 == 0 { 1 } else { 3 }; // mix in some Again
                let id = prev_ts + 86_400_000 * (review as i64 + 1);
                logs.push(Revlog {
                    id,
                    card_id,
                    usn: -1,
                    ease: rating,
                    interval: 1,
                    last_interval: 0,
                    ease_factor: 2500,
                    taken_ms: 5000,
                    review_kind: 1,
                });
                prev_ts = id;
            }
            ts += 10; // offset each card slightly
        }
        logs
    }

    #[test]
    fn optimizer_converges_below_initial_loss() {
        let logs = synthetic_revlog();
        // Need at least MIN_REVIEWS = 400 reviews; 50*10 = 500 ✓
        let result = optimize(&logs, &FSRS5_DEFAULT_WEIGHTS).expect("optimize failed");
        assert!(
            result.log_loss_after <= result.log_loss_before,
            "loss did not decrease: before={:.4} after={:.4}",
            result.log_loss_before,
            result.log_loss_after
        );
        assert_eq!(result.card_count, 50);
        assert!(result.review_count >= 400);
    }

    #[test]
    fn optimizer_rejects_short_history() {
        // Only 10 reviews — below threshold.
        let logs: Vec<Revlog> = (0..10)
            .map(|i| Revlog {
                id: 1_700_000_000_000 + i * 86_400_000,
                card_id: i,
                usn: -1,
                ease: 3,
                interval: 1,
                last_interval: 0,
                ease_factor: 2500,
                taken_ms: 5000,
                review_kind: 1,
            })
            .collect();
        let result = optimize(&logs, &FSRS5_DEFAULT_WEIGHTS);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("insufficient"));
    }

    #[test]
    fn weights_stay_within_bounds_after_optimize() {
        let logs = synthetic_revlog();
        let result = optimize(&logs, &FSRS5_DEFAULT_WEIGHTS).unwrap();
        for (i, (&w, &(lo, hi))) in result.weights.iter().zip(BOUNDS.iter()).enumerate() {
            assert!(
                w >= lo && w <= hi,
                "weight {i} = {w} out of bounds [{lo}, {hi}]"
            );
        }
    }
}
