import type { DayCount } from "@synapse/ipc-types";
import { sequential } from "./chartTheme";

const DEFAULT_WEEKS = 52;
const MONTH_NAMES = [
  "Jan",
  "Feb",
  "Mar",
  "Apr",
  "May",
  "Jun",
  "Jul",
  "Aug",
  "Sep",
  "Oct",
  "Nov",
  "Dec",
];

/** Current + longest consecutive-day streaks, counting back from `today` (collection-relative day). */
export function computeStreaks(
  reviews: DayCount[],
  today: number,
): { current: number; longest: number } {
  const days = new Set(reviews.filter((d) => d.count > 0).map((d) => d.day));

  let current = 0;
  // A streak is "alive" if today or yesterday was studied (allows for not-yet-studied-today).
  let cursor = days.has(today) ? today : today - 1;
  while (days.has(cursor)) {
    current++;
    cursor--;
  }

  let longest = 0;
  let run = 0;
  const sorted = [...days].sort((a, b) => a - b);
  for (let i = 0; i < sorted.length; i++) {
    if (i > 0 && sorted[i] === sorted[i - 1] + 1) {
      run++;
    } else {
      run = 1;
    }
    longest = Math.max(longest, run);
  }

  return { current, longest };
}

function heatColor(count: number): string {
  if (count === 0) return "hsl(var(--secondary))";
  if (count < 3) return sequential[0];
  if (count < 6) return sequential[1];
  if (count < 12) return sequential[2];
  if (count < 20) return sequential[3];
  return sequential[4];
}

export function YearHeatmap({
  reviews,
  today,
  day0Ms,
  weeks = DEFAULT_WEEKS,
}: {
  reviews: DayCount[];
  today: number;
  day0Ms: number;
  /** Window width in weeks — defaults to a full year; pass a smaller value for compact use. */
  weeks?: number;
}) {
  const counts = new Map(reviews.map((d) => [d.day, d.count]));
  const days = weeks * 7;
  const start = today - (days - 1);
  const cells = Array.from({ length: days }, (_, i) => {
    const day = start + i;
    return { day, count: counts.get(day) ?? 0 };
  });

  const dateOf = (day: number) => new Date(day0Ms + day * 86_400_000);

  // Month labels: mark the first column (week) whose Monday falls in a new month.
  const weekCount = Math.ceil(days / 7);
  const monthLabels: { week: number; label: string }[] = [];
  let lastMonth = -1;
  for (let w = 0; w < weekCount; w++) {
    const day = start + w * 7;
    const m = dateOf(day).getMonth();
    if (m !== lastMonth) {
      monthLabels.push({ week: w, label: MONTH_NAMES[m] });
      lastMonth = m;
    }
  }

  return (
    <div className="w-full">
      <div
        className="mb-1 grid text-[10px] text-muted-foreground"
        style={{ gridTemplateColumns: `repeat(${weekCount}, minmax(0, 1fr))` }}
      >
        {Array.from({ length: weekCount }, (_, w) => {
          const label = monthLabels.find((m) => m.week === w)?.label;
          return <span key={w}>{label}</span>;
        })}
      </div>
      <div
        className="grid gap-[2px]"
        style={{
          gridTemplateColumns: `repeat(${weekCount}, minmax(0, 1fr))`,
          gridTemplateRows: "repeat(7, minmax(0, 1fr))",
          gridAutoFlow: "column",
        }}
      >
        {cells.map((cell) => (
          <div
            key={cell.day}
            title={`${cell.count} review${cell.count === 1 ? "" : "s"} on ${dateOf(cell.day).toLocaleDateString()}`}
            className="aspect-square w-full rounded-sm transition-transform hover:scale-125"
            style={{
              backgroundColor: heatColor(cell.count),
              outline: cell.day === today ? "1.5px solid hsl(var(--foreground))" : undefined,
              outlineOffset: cell.day === today ? "-1.5px" : undefined,
            }}
          />
        ))}
      </div>
    </div>
  );
}
