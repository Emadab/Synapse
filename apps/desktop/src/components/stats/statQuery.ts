/**
 * Maps a clicked stats-chart bucket to an Anki-style query string for
 * `ipc.searchCards`, so drilling into a bar jumps to Browse pre-filtered to
 * (approximately) the cards behind it. Mirrors the exact predicates used by
 * the Rust stats aggregates in `crates/synapse-db/src/stats.rs`.
 */

const STABILITY_EDGES = [1, 7, 21, 90, 180, 365];

function withDeck(query: string, deckName: string | null): string {
  return deckName ? `deck:"${deckName}*" ${query}` : query;
}

export type MaturityBucket = "new" | "learning" | "young" | "mature" | "suspended";

export function maturityQuery(bucket: MaturityBucket, deckName: string | null): string {
  const base: Record<MaturityBucket, string> = {
    new: "is:new",
    learning: "is:learn",
    young: "is:review prop:ivl<21",
    mature: "is:review prop:ivl>=21",
    suspended: "is:suspended",
  };
  return withDeck(base[bucket], deckName);
}

/** `dayOffset` is `null` for the "overdue" backlog bar. */
export function forecastQuery(
  dayOffset: number | null,
  today: number,
  deckName: string | null,
): string {
  const base =
    dayOffset === null ? `is:review prop:due<${today}` : `is:review prop:due=${today + dayOffset}`;
  return withDeck(base, deckName);
}

/** `bucketIndex` into the 7 stability buckets: <1d, 1-7d, 7-21d, 21-90d, 90-180d, 180-365d, 365d+. */
export function stabilityQuery(bucketIndex: number, deckName: string | null): string {
  const lo = bucketIndex === 0 ? null : STABILITY_EDGES[bucketIndex - 1];
  const hi = bucketIndex < STABILITY_EDGES.length ? STABILITY_EDGES[bucketIndex] : null;
  const parts = ["-is:suspended", "-is:buried"];
  if (lo !== null) parts.push(`prop:stability>=${lo}`);
  if (hi !== null) parts.push(`prop:stability<${hi}`);
  return withDeck(parts.join(" "), deckName);
}

/** `bucketIndex` 0-9, labels "1".."10". */
export function difficultyQuery(bucketIndex: number, deckName: string | null): string {
  const lo = bucketIndex + 1;
  const parts = ["-is:suspended", "-is:buried", `prop:difficulty>=${lo}`];
  if (bucketIndex < 9) parts.push(`prop:difficulty<${bucketIndex + 2}`);
  return withDeck(parts.join(" "), deckName);
}

export type AnswerPhase = "learning" | "young" | "mature";

export function answeredQuery(
  phase: AnswerPhase,
  ease: number,
  rangeDays: number | null,
  deckName: string | null,
): string {
  const suffix = rangeDays !== null ? `:${rangeDays}` : "";
  return withDeck(`answered:${phase}:${ease}${suffix}`, deckName);
}
