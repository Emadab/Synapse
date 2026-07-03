import { useMemo } from "react";
import { useQuery } from "@tanstack/react-query";
import { Flame, Play } from "lucide-react";
import type { DeckSummary } from "@synapse/ipc-types";
import { Button } from "@/components/ui/button";
import { ipc, isTauri } from "@/lib/ipc";
import { queryKeys } from "@/lib/queryKeys";
import { computeStreaks, YearHeatmap } from "@/components/stats/YearHeatmap";
import { PanelSkeleton } from "@/components/stats/PanelSkeleton";

const HERO_RANGE_DAYS = 90;
const HERO_HEATMAP_WEEKS = 12;

function Sparkline({ values }: { values: number[] }) {
  const max = Math.max(1, ...values);
  const w = 120;
  const h = 28;
  const step = values.length > 1 ? w / (values.length - 1) : w;
  const points = values.map((v, i) => `${i * step},${h - (v / max) * h}`).join(" ");

  return (
    <svg width={w} height={h} viewBox={`0 0 ${w} ${h}`} className="overflow-visible">
      <polyline
        points={points}
        fill="none"
        stroke="hsl(var(--primary))"
        strokeWidth={1.5}
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

/**
 * Compact "today" strip for the home screen's Hero layout: due total + study
 * CTA, streak, a 12-week activity heatmap, and a 7-day reviews sparkline.
 * Reuses the same stats IPC query as StatsScreen (same query key → shares cache).
 */
export function HomeHero({
  decks,
  onStudy,
}: {
  decks: DeckSummary[];
  onStudy: (deck: DeckSummary) => void;
}) {
  const tauri = isTauri();
  const statsQuery = useQuery({
    queryKey: queryKeys.stats(null, HERO_RANGE_DAYS),
    queryFn: () => ipc.getStats(null, HERO_RANGE_DAYS),
    enabled: tauri,
  });

  const dueTotal = decks.reduce((sum, d) => sum + d.new_count + d.learning_count + d.review_count, 0);
  const nextDueDeck = decks.find((d) => d.new_count + d.learning_count + d.review_count > 0);

  const stats = statsQuery.data;
  const today = useMemo(
    () => (stats ? Math.floor((Date.now() - stats.day0_ms) / 86_400_000) : 0),
    [stats],
  );
  const streak = useMemo(
    () => (stats ? computeStreaks(stats.reviews, today).current : 0),
    [stats, today],
  );
  const last7 = useMemo(() => {
    if (!stats) return [];
    const counts = new Map(stats.reviews.map((d) => [d.day, d.count]));
    return Array.from({ length: 7 }, (_, i) => counts.get(today - (6 - i)) ?? 0);
  }, [stats, today]);

  if (!tauri) return null;

  return (
    <div className="mx-8 mt-4 rounded-xl border border-border bg-card p-5">
      {!stats ? (
        <PanelSkeleton height={80} />
      ) : (
        <div className="flex flex-wrap items-center justify-between gap-6">
          <div className="flex items-center gap-6">
            <div>
              <p className="text-2xl font-semibold tnum">{dueTotal}</p>
              <p className="text-xs text-muted-foreground">due today</p>
            </div>
            <div className="flex items-center gap-1.5">
              <Flame className="size-4 text-orange-500" />
              <div>
                <p className="text-lg font-semibold tnum leading-none">{streak}</p>
                <p className="text-xs text-muted-foreground">day streak</p>
              </div>
            </div>
            <div>
              <Sparkline values={last7} />
              <p className="mt-1 text-xs text-muted-foreground">last 7 days</p>
            </div>
            <div className="hidden lg:block">
              <YearHeatmap
                reviews={stats.reviews}
                today={today}
                day0Ms={stats.day0_ms}
                weeks={HERO_HEATMAP_WEEKS}
              />
            </div>
          </div>

          <Button
            disabled={!nextDueDeck}
            onClick={() => nextDueDeck && onStudy(nextDueDeck)}
            title={nextDueDeck ? `Study ${nextDueDeck.name}` : "Nothing due"}
          >
            <Play className="size-4" /> Study now
          </Button>
        </div>
      )}
    </div>
  );
}
