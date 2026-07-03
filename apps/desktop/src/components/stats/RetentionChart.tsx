import {
  CartesianGrid,
  Legend,
  Line,
  LineChart,
  ReferenceLine,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import type { RetentionWeek } from "@synapse/ipc-types";
import { axisProps, categorical, gridProps, tooltipItemStyle, tooltipLabelStyle, tooltipStyle } from "./chartTheme";
import { ExportButton } from "./ExportButton";

const MIN_SAMPLE = 5;

export function RetentionChart({
  weekly,
  goalPct,
  day0Ms,
}: {
  weekly: RetentionWeek[];
  goalPct: number;
  day0Ms: number;
}) {
  const data = weekly.map((w) => ({
    label: new Date(day0Ms + w.week_index * 7 * 86_400_000).toLocaleDateString(undefined, {
      month: "short",
      day: "numeric",
    }),
    young: w.young_total >= MIN_SAMPLE ? (w.young_passed / w.young_total) * 100 : null,
    mature: w.mature_total >= MIN_SAMPLE ? (w.mature_passed / w.mature_total) * 100 : null,
  }));

  if (weekly.length === 0) {
    return <p className="text-sm text-muted-foreground">Not enough review history yet.</p>;
  }

  return (
    <div className="relative">
      <div className="absolute right-0 top-0">
        <ExportButton
          filename="retention-weekly"
          rows={weekly.map((w) => ({ ...w }))}
        />
      </div>
      <ResponsiveContainer width="100%" height={220}>
        <LineChart data={data} margin={{ top: 4, right: 4, bottom: 0, left: -20 }}>
          <CartesianGrid {...gridProps} />
          <XAxis dataKey="label" {...axisProps} interval="preserveStartEnd" />
          <YAxis domain={[0, 100]} {...axisProps} />
          <ReferenceLine
            y={goalPct}
            stroke="hsl(var(--muted-foreground))"
            strokeDasharray="4 4"
            label={{ value: `Goal ${goalPct.toFixed(0)}%`, fontSize: 10, fill: "hsl(var(--muted-foreground))" }}
          />
          <Tooltip
            contentStyle={tooltipStyle}
            itemStyle={tooltipItemStyle}
            labelStyle={tooltipLabelStyle}
            formatter={(v) => (typeof v === "number" ? `${v.toFixed(1)}%` : v)}
          />
          <Legend wrapperStyle={{ fontSize: 12 }} />
          <Line
            type="monotone"
            dataKey="young"
            name="Young"
            stroke={categorical[0]}
            strokeWidth={2}
            dot={false}
            connectNulls
          />
          <Line
            type="monotone"
            dataKey="mature"
            name="Mature"
            stroke={categorical[1]}
            strokeWidth={2}
            dot={false}
            connectNulls
          />
        </LineChart>
      </ResponsiveContainer>
    </div>
  );
}
