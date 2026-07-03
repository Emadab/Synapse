import { Bar, BarChart, CartesianGrid, Cell, ResponsiveContainer, Tooltip, XAxis, YAxis } from "recharts";
import type { HourlyStat } from "@synapse/ipc-types";
import { gridProps, sequential, tooltipItemStyle, tooltipLabelStyle, tooltipStyle } from "./chartTheme";
import { ExportButton } from "./ExportButton";

// Higher-contrast tick style than the shared `axisProps` — this chart's bars
// span a light sequential ramp, so muted-foreground reads too faint beside them.
const hourAxisProps = {
  tick: { fontSize: 11, fill: "hsl(var(--foreground))" },
  stroke: "hsl(var(--border))",
  tickLine: false,
} as const;

/** Bucket a 0-100 pass rate into one of the 5 sequential-ramp steps. */
function passRateColor(passRate: number | null): string {
  if (passRate === null) return "hsl(var(--secondary))";
  const idx = Math.min(4, Math.floor(passRate / 20));
  return sequential[idx];
}

export function HourlyChart({ hourly }: { hourly: HourlyStat[] }) {
  const byHour = new Map(hourly.map((h) => [h.hour, h]));
  const data = Array.from({ length: 24 }, (_, hour) => {
    const h = byHour.get(hour);
    const passRate = h && h.total > 0 ? (h.passed / h.total) * 100 : null;
    return { hour, label: `${hour}`, total: h?.total ?? 0, passRate };
  });

  const total = data.reduce((sum, d) => sum + d.total, 0);
  if (total === 0) {
    return <p className="text-sm text-muted-foreground">No reviews in this range.</p>;
  }

  return (
    <div className="relative">
      <div className="absolute right-0 top-0 flex items-center gap-2">
        <div className="flex items-center gap-1 text-[10px] font-medium text-foreground">
          <span>Low pass %</span>
          <div className="flex overflow-hidden rounded-sm ring-1 ring-border">
            {sequential.map((c) => (
              <span key={c} className="h-2 w-3" style={{ backgroundColor: c }} />
            ))}
          </div>
          <span>High</span>
        </div>
        <ExportButton filename="hourly-reviews" rows={data} />
      </div>
      <ResponsiveContainer width="100%" height={220}>
        <BarChart data={data} margin={{ top: 20, right: 4, bottom: 0, left: -20 }}>
          <CartesianGrid {...gridProps} />
          <XAxis dataKey="label" {...hourAxisProps} interval={1} />
          <YAxis allowDecimals={false} {...hourAxisProps} />
          <Tooltip
            contentStyle={tooltipStyle}
            itemStyle={tooltipItemStyle}
            labelStyle={tooltipLabelStyle}
            formatter={(value, _name, item) => {
              const rate = (item?.payload as { passRate: number | null } | undefined)?.passRate ?? null;
              return [`${value} reviews${rate !== null ? `, ${rate.toFixed(0)}% passed` : ""}`, "Hour"];
            }}
          />
          <Bar dataKey="total" radius={[3, 3, 0, 0]}>
            {data.map((d) => (
              <Cell key={d.hour} fill={passRateColor(d.passRate)} />
            ))}
          </Bar>
        </BarChart>
      </ResponsiveContainer>
    </div>
  );
}
