import { Cell, Legend, Pie, PieChart, ResponsiveContainer, Tooltip } from "recharts";
import type { StatsDto } from "@synapse/ipc-types";
import { categorical, tooltipItemStyle, tooltipLabelStyle, tooltipStyle } from "./chartTheme";

const MATURITY_COLORS: Record<string, string> = {
  New: categorical[0],
  Learning: categorical[2],
  Young: categorical[1],
  Mature: categorical[3],
  Suspended: "hsl(var(--muted-foreground))",
};

export function MaturityDonut({ stats }: { stats: StatsDto }) {
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
    <ResponsiveContainer width="100%" height={220}>
      <PieChart>
        <Pie
          data={data}
          dataKey="value"
          nameKey="name"
          innerRadius={45}
          outerRadius={75}
          paddingAngle={2}
          stroke="none"
        >
          {data.map((entry) => (
            <Cell key={entry.name} fill={MATURITY_COLORS[entry.name]} />
          ))}
        </Pie>
        <Tooltip contentStyle={tooltipStyle} itemStyle={tooltipItemStyle} labelStyle={tooltipLabelStyle} />
        <Legend wrapperStyle={{ fontSize: 12 }} />
      </PieChart>
    </ResponsiveContainer>
  );
}
