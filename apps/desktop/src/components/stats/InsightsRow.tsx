import { Flame, Sparkles, TrendingUp } from "lucide-react";
import type { DayCount, HourlyStat } from "@synapse/ipc-types";

const WEEKDAYS = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

function bestHour(hourly: HourlyStat[]): string | null {
  const candidates = hourly.filter((h) => h.total >= 20);
  if (candidates.length === 0) return null;
  const best = candidates.reduce((a, b) => (b.passed / b.total > a.passed / a.total ? b : a));
  const hour12 = best.hour % 12 === 0 ? 12 : best.hour % 12;
  const ampm = best.hour < 12 ? "am" : "pm";
  return `${hour12}${ampm}`;
}

function busiestWeekday(reviews: DayCount[], day0Ms: number): string | null {
  if (reviews.length === 0) return null;
  const totals = new Array(7).fill(0);
  for (const d of reviews) {
    const weekday = new Date(day0Ms + d.day * 86_400_000).getDay();
    totals[weekday] += d.count;
  }
  const maxIdx = totals.indexOf(Math.max(...totals));
  return WEEKDAYS[maxIdx];
}

function Insight({ icon: Icon, label, value }: { icon: typeof Flame; label: string; value: string }) {
  return (
    <div className="flex items-center gap-2 rounded-lg border border-border bg-card px-3 py-2">
      <Icon className="size-4 text-primary" />
      <span className="text-xs text-muted-foreground">{label}</span>
      <span className="text-sm font-semibold">{value}</span>
    </div>
  );
}

export function InsightsRow({
  reviews,
  hourly,
  day0Ms,
  longestStreak,
}: {
  reviews: DayCount[];
  hourly: HourlyStat[];
  day0Ms: number;
  longestStreak: number;
}) {
  const hour = bestHour(hourly);
  const weekday = busiestWeekday(reviews, day0Ms);

  if (!hour && !weekday && longestStreak === 0) return null;

  return (
    <div className="flex flex-wrap gap-3">
      {hour ? <Insight icon={Sparkles} label="Best hour" value={hour} /> : null}
      {weekday ? <Insight icon={TrendingUp} label="Busiest day" value={weekday} /> : null}
      {longestStreak > 0 ? (
        <Insight icon={Flame} label="Longest streak" value={`${longestStreak} days`} />
      ) : null}
    </div>
  );
}
