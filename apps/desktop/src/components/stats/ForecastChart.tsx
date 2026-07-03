import {
  Bar,
  BarChart,
  CartesianGrid,
  Cell,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import type { DayCount } from "@synapse/ipc-types";
import {
  axisProps,
  categorical,
  gridProps,
  tooltipItemStyle,
  tooltipLabelStyle,
  tooltipStyle,
} from "./chartTheme";
import { ExportButton } from "./ExportButton";

const FORECAST_DAYS = 30;
const BACKLOG_COLOR = "hsl(1 74% 59%)"; // status-adjacent red, distinct from the categorical due-bar color

export function ForecastChart({
  forecast,
  backlogCount,
}: {
  forecast: DayCount[];
  backlogCount: number;
}) {
  const map = new Map(forecast.map((d) => [d.day, d.count]));
  const data = [
    { label: "overdue", count: backlogCount, isBacklog: true },
    ...Array.from({ length: FORECAST_DAYS + 1 }, (_, i) => ({
      label: i === 0 ? "today" : `+${i}`,
      count: map.get(i) ?? 0,
      isBacklog: false,
    })),
  ];

  const dueTotal = data.reduce((sum, d) => sum + d.count, 0);

  return (
    <div className="relative">
      <div className="absolute right-0 top-0">
        <ExportButton filename="forecast" rows={data} />
      </div>
      <p className="mb-2 text-xs text-muted-foreground">
        <span className="font-medium text-foreground">{dueTotal.toLocaleString()}</span> cards due
        in the next 30 days
        {backlogCount > 0 ? (
          <>
            {" "}
            (<span className="font-medium text-foreground">
              {backlogCount.toLocaleString()}
            </span>{" "}
            overdue)
          </>
        ) : null}
      </p>
      <ResponsiveContainer width="100%" height={200}>
        <BarChart data={data} margin={{ top: 4, right: 4, bottom: 0, left: -20 }}>
          <CartesianGrid {...gridProps} />
          <XAxis dataKey="label" {...axisProps} interval={4} />
          <YAxis allowDecimals={false} {...axisProps} />
          <Tooltip
            contentStyle={tooltipStyle}
            itemStyle={tooltipItemStyle}
            labelStyle={tooltipLabelStyle}
          />
          <Bar dataKey="count" radius={[3, 3, 0, 0]}>
            {data.map((d) => (
              <Cell key={d.label} fill={d.isBacklog ? BACKLOG_COLOR : categorical[0]} />
            ))}
          </Bar>
        </BarChart>
      </ResponsiveContainer>
    </div>
  );
}
