import type { ReactNode } from "react";
import { useQuery } from "@tanstack/react-query";
import { BarChart3 } from "lucide-react";
import {
  Bar,
  BarChart,
  Cell,
  Pie,
  PieChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import type { StatsDto } from "@synapse/ipc-types";
import { ScreenHeader } from "@/components/ScreenHeader";
import { EmptyState } from "@/components/EmptyState";
import { ipc, isTauri } from "@/lib/ipc";
import { queryKeys } from "@/lib/queryKeys";

const HEATMAP_WEEKS = 18;
const FORECAST_DAYS = 30;
const MATURITY_COLORS = ["#6366f1", "#f59e0b", "#22c55e", "#16a34a", "#94a3b8"];

export function StatsScreen() {
  const tauri = isTauri();
  const stats = useQuery({ queryKey: queryKeys.stats, queryFn: ipc.getStats, enabled: tauri });

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
        ) : stats.data ? (
          <Dashboard stats={stats.data} />
        ) : (
          <p className="text-sm text-muted-foreground">Loading…</p>
        )}
      </div>
    </div>
  );
}

function Dashboard({ stats }: { stats: StatsDto }) {
  const minutes = Math.round(stats.total_time_ms / 60000);
  return (
    <div className="mx-auto max-w-4xl space-y-8">
      <div className="grid grid-cols-2 gap-4 md:grid-cols-4">
        <StatCard label="Reviews" value={stats.total_reviews.toLocaleString()} />
        <StatCard label="Retention (30d)" value={`${stats.retention_pct.toFixed(0)}%`} />
        <StatCard label="Days studied" value={String(stats.studied_days)} />
        <StatCard
          label="Time"
          value={minutes >= 60 ? `${(minutes / 60).toFixed(1)}h` : `${minutes}m`}
        />
      </div>

      <Panel title="Review activity">
        <Heatmap reviews={stats.reviews} />
      </Panel>

      <div className="grid gap-8 md:grid-cols-2">
        <Panel title="Forecast (due, next 30 days)">
          <Forecast forecast={stats.forecast} />
        </Panel>
        <Panel title="Card maturity">
          <Maturity stats={stats} />
        </Panel>
      </div>
    </div>
  );
}

function StatCard({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-xl border border-border bg-card p-4">
      <div className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
        {label}
      </div>
      <div className="mt-1 text-2xl font-semibold tabular-nums">{value}</div>
    </div>
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

function heatColor(count: number): string {
  if (count === 0) return "hsl(var(--secondary))";
  if (count < 3) return "hsl(var(--primary) / 0.35)";
  if (count < 6) return "hsl(var(--primary) / 0.6)";
  if (count < 12) return "hsl(var(--primary) / 0.8)";
  return "hsl(var(--primary))";
}

function Heatmap({ reviews }: { reviews: StatsDto["reviews"] }) {
  const counts = new Map(reviews.map((d) => [d.day, d.count]));
  const days = HEATMAP_WEEKS * 7;
  const todayDay = Math.floor(Date.now() / 86_400_000);
  const start = todayDay - (days - 1);
  const cells = Array.from({ length: days }, (_, i) => ({
    day: start + i,
    count: counts.get(start + i) ?? 0,
  }));

  return (
    <div className="grid grid-flow-col grid-rows-7 gap-1">
      {cells.map((cell) => (
        <div
          key={cell.day}
          title={`${cell.count} reviews on ${new Date(cell.day * 86_400_000).toLocaleDateString()}`}
          className="size-3 rounded-sm"
          style={{ backgroundColor: heatColor(cell.count) }}
        />
      ))}
    </div>
  );
}

function Forecast({ forecast }: { forecast: StatsDto["forecast"] }) {
  const map = new Map(forecast.map((d) => [d.day, d.count]));
  const data = Array.from({ length: FORECAST_DAYS + 1 }, (_, i) => ({
    label: i === 0 ? "now" : `+${i}`,
    count: map.get(i) ?? 0,
  }));
  return (
    <ResponsiveContainer width="100%" height={200}>
      <BarChart data={data} margin={{ top: 4, right: 4, bottom: 0, left: -20 }}>
        <XAxis
          dataKey="label"
          tick={{ fontSize: 10 }}
          interval={4}
          stroke="hsl(var(--muted-foreground))"
        />
        <YAxis
          allowDecimals={false}
          tick={{ fontSize: 10 }}
          stroke="hsl(var(--muted-foreground))"
        />
        <Tooltip
          contentStyle={{
            background: "hsl(var(--popover))",
            border: "1px solid hsl(var(--border))",
            borderRadius: 8,
            fontSize: 12,
          }}
        />
        <Bar dataKey="count" fill="hsl(var(--primary))" radius={[3, 3, 0, 0]} />
      </BarChart>
    </ResponsiveContainer>
  );
}

function Maturity({ stats }: { stats: StatsDto }) {
  const data = [
    { name: "New", value: stats.new_count },
    { name: "Learning", value: stats.learning_count },
    { name: "Young", value: stats.young_count },
    { name: "Mature", value: stats.mature_count },
    { name: "Suspended", value: stats.suspended_count },
  ].filter((d) => d.value > 0);

  if (data.length === 0) {
    return <p className="text-sm text-muted-foreground">No cards yet.</p>;
  }

  return (
    <ResponsiveContainer width="100%" height={200}>
      <PieChart>
        <Pie
          data={data}
          dataKey="value"
          nameKey="name"
          innerRadius={45}
          outerRadius={75}
          paddingAngle={2}
        >
          {data.map((entry, index) => (
            <Cell key={entry.name} fill={MATURITY_COLORS[index % MATURITY_COLORS.length]} />
          ))}
        </Pie>
        <Tooltip
          contentStyle={{
            background: "hsl(var(--popover))",
            border: "1px solid hsl(var(--border))",
            borderRadius: 8,
            fontSize: 12,
          }}
        />
      </PieChart>
    </ResponsiveContainer>
  );
}
