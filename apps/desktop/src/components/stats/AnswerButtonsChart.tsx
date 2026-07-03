import { Bar, BarChart, CartesianGrid, Legend, ResponsiveContainer, Tooltip, XAxis, YAxis } from "recharts";
import type { AnswerButtons } from "@synapse/ipc-types";
import { axisProps, categorical, gridProps, tooltipItemStyle, tooltipLabelStyle, tooltipStyle } from "./chartTheme";
import { ExportButton } from "./ExportButton";

const LABELS = ["Again", "Hard", "Good", "Easy"];

export function AnswerButtonsChart({ buttons }: { buttons: AnswerButtons }) {
  const data = LABELS.map((label, i) => ({
    label,
    Learning: buttons.learning[i] ?? 0,
    Young: buttons.young[i] ?? 0,
    Mature: buttons.mature[i] ?? 0,
  }));

  const total = data.reduce((sum, d) => sum + d.Learning + d.Young + d.Mature, 0);
  if (total === 0) {
    return <p className="text-sm text-muted-foreground">No answers in this range.</p>;
  }

  return (
    <div className="relative">
      <div className="absolute right-0 top-0">
        <ExportButton filename="answer-buttons" rows={data} />
      </div>
      <ResponsiveContainer width="100%" height={220}>
        <BarChart data={data} margin={{ top: 4, right: 4, bottom: 0, left: -20 }}>
          <CartesianGrid {...gridProps} />
          <XAxis dataKey="label" {...axisProps} />
          <YAxis allowDecimals={false} {...axisProps} />
          <Tooltip contentStyle={tooltipStyle} itemStyle={tooltipItemStyle} labelStyle={tooltipLabelStyle} />
          <Legend wrapperStyle={{ fontSize: 12 }} />
          <Bar dataKey="Learning" fill={categorical[2]} radius={[3, 3, 0, 0]} />
          <Bar dataKey="Young" fill={categorical[0]} radius={[3, 3, 0, 0]} />
          <Bar dataKey="Mature" fill={categorical[1]} radius={[3, 3, 0, 0]} />
        </BarChart>
      </ResponsiveContainer>
    </div>
  );
}
