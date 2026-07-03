import { useMemo, type ReactNode } from "react";
import { keepPreviousData, useQuery } from "@tanstack/react-query";
import { useNavigate, useSearch } from "@tanstack/react-router";
import { BarChart3 } from "lucide-react";
import { motion } from "framer-motion";
import type { StatsDto } from "@synapse/ipc-types";
import { ScreenHeader } from "@/components/ScreenHeader";
import { EmptyState } from "@/components/EmptyState";
import { ipc, isTauri } from "@/lib/ipc";
import { queryKeys } from "@/lib/queryKeys";
import { listItem, staggerList } from "@/lib/motion";
import { AnswerButtonsChart } from "@/components/stats/AnswerButtonsChart";
import { DeckTable } from "@/components/stats/DeckTable";
import { ForecastChart } from "@/components/stats/ForecastChart";
import { FsrsPanels } from "@/components/stats/FsrsPanels";
import { HourlyChart } from "@/components/stats/HourlyChart";
import { InsightsRow } from "@/components/stats/InsightsRow";
import { MaturityDonut } from "@/components/stats/MaturityDonut";
import { PanelSkeleton } from "@/components/stats/PanelSkeleton";
import { RetentionChart } from "@/components/stats/RetentionChart";
import { RANGE_DAYS, StatsFilters, type StatsRange } from "@/components/stats/StatsFilters";
import { StatCards } from "@/components/stats/StatCards";
import { computeStreaks, YearHeatmap } from "@/components/stats/YearHeatmap";

export function StatsScreen() {
  const tauri = isTauri();
  const search = useSearch({ from: "/stats" });
  const navigate = useNavigate({ from: "/stats" });

  const deckId = search.deck ?? null;
  const range: StatsRange = search.range ?? "all";
  const days = RANGE_DAYS[range];

  const setDeckId = (id: number | null) => {
    void navigate({ search: (prev) => ({ ...prev, deck: id ?? undefined }) });
  };
  const setRange = (r: StatsRange) => {
    void navigate({ search: (prev) => ({ ...prev, range: r === "all" ? undefined : r }) });
  };

  const decksQuery = useQuery({
    queryKey: queryKeys.decks,
    queryFn: ipc.listDecks,
    enabled: tauri,
  });
  const statsQuery = useQuery({
    queryKey: queryKeys.stats(deckId, days),
    queryFn: () => ipc.getStats(deckId, days),
    enabled: tauri,
    placeholderData: keepPreviousData,
  });

  return (
    <div className="flex h-full flex-col">
      <ScreenHeader title="Statistics" description="Your learning, visualized." />
      <div className="min-h-0 flex-1 overflow-auto p-8">
        {!tauri ? (
          <EmptyState
            icon={BarChart3}
            title="Run the desktop app"
            description="Statistics are computed by the Rust core over Tauri. Launch with `pnpm dev`."
          />
        ) : (
          <div className="mx-auto max-w-5xl space-y-6">
            <StatsFilters
              decks={decksQuery.data ?? []}
              deckId={deckId}
              range={range}
              onDeckChange={setDeckId}
              onRangeChange={setRange}
            />
            {statsQuery.data ? (
              <Dashboard stats={statsQuery.data} onSelectDeck={setDeckId} />
            ) : (
              <div className="space-y-6">
                <PanelSkeleton height={88} />
                <PanelSkeleton height={140} />
                <PanelSkeleton height={220} />
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

function Dashboard({
  stats,
  onSelectDeck,
}: {
  stats: StatsDto;
  onSelectDeck: (deckId: number) => void;
}) {
  const today = useMemo(
    () => Math.floor((Date.now() - stats.day0_ms) / 86_400_000),
    [stats.day0_ms],
  );
  const streaks = useMemo(() => computeStreaks(stats.reviews, today), [stats.reviews, today]);

  return (
    <motion.div variants={staggerList} initial="hidden" animate="show" className="space-y-6">
      <motion.div variants={listItem}>
        <StatCards stats={stats} streak={streaks.current} />
      </motion.div>

      <motion.div variants={listItem}>
        <InsightsRow
          reviews={stats.reviews}
          hourly={stats.hourly}
          day0Ms={stats.day0_ms}
          longestStreak={streaks.longest}
        />
      </motion.div>

      <motion.div variants={listItem}>
        <Panel title="Review activity">
          <YearHeatmap reviews={stats.reviews} today={today} day0Ms={stats.day0_ms} />
        </Panel>
      </motion.div>

      <motion.div variants={listItem}>
        <Panel title="Retention over time">
          <RetentionChart
            weekly={stats.retention_weekly}
            goalPct={stats.retention_goal_pct}
            day0Ms={stats.day0_ms}
          />
        </Panel>
      </motion.div>

      <div className="grid gap-6 md:grid-cols-2">
        <motion.div variants={listItem}>
          <Panel title="Answer buttons">
            <AnswerButtonsChart buttons={stats.answer_buttons} />
          </Panel>
        </motion.div>
        <motion.div variants={listItem}>
          <Panel title="Reviews by hour">
            <HourlyChart hourly={stats.hourly} />
          </Panel>
        </motion.div>
      </div>

      <div className="grid gap-6 md:grid-cols-2">
        <motion.div variants={listItem}>
          <Panel title="Forecast (next 30 days)">
            <ForecastChart forecast={stats.forecast} backlogCount={stats.backlog_count} />
          </Panel>
        </motion.div>
        <motion.div variants={listItem}>
          <Panel title="Card maturity">
            <MaturityDonut stats={stats} />
          </Panel>
        </motion.div>
      </div>

      <motion.div variants={listItem}>
        <Panel title="FSRS memory model">
          <FsrsPanels fsrs={stats.fsrs} />
        </Panel>
      </motion.div>

      {stats.deck_stats.length > 0 ? (
        <motion.div variants={listItem}>
          <Panel title="Decks">
            <DeckTable deckStats={stats.deck_stats} onSelectDeck={onSelectDeck} />
          </Panel>
        </motion.div>
      ) : null}
    </motion.div>
  );
}

function Panel({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="rounded-xl border border-border bg-card p-5">
      <h2 className="mb-4 text-sm font-medium">{title}</h2>
      {children}
    </section>
  );
}
