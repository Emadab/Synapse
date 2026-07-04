import {
  Bar,
  BarChart,
  CartesianGrid,
  Legend,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import type { AnswerButtons } from "@synapse/ipc-types";
import {
  axisProps,
  categorical,
  gridProps,
  tooltipCursor,
  tooltipItemStyle,
  tooltipLabelStyle,
  tooltipStyle,
} from "./chartTheme";
import { ExportButton } from "./ExportButton";
import { answeredQuery, type AnswerPhase } from "./statQuery";

const LABELS = ["Again", "Hard", "Good", "Easy"];

export function AnswerButtonsChart({
  buttons,
  onDrill,
  deckName,
  rangeDays,
}: {
  buttons: AnswerButtons;
  onDrill: (query: string) => void;
  deckName: string | null;
  rangeDays: number | null;
}) {
  const data = LABELS.map((label, i) => ({
    label,
    ease: i + 1,
    Learning: buttons.learning[i] ?? 0,
    Young: buttons.young[i] ?? 0,
    Mature: buttons.mature[i] ?? 0,
  }));

  const drillPhase = (phase: AnswerPhase) => (d: { payload?: unknown }) => {
    const payload = d.payload as (typeof data)[number];
    onDrill(answeredQuery(phase, payload.ease, rangeDays, deckName));
  };

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
          <Tooltip
            contentStyle={tooltipStyle}
            itemStyle={tooltipItemStyle}
            labelStyle={tooltipLabelStyle}
            cursor={tooltipCursor}
          />
          <Legend wrapperStyle={{ fontSize: 12 }} />
          <Bar
            dataKey="Learning"
            fill={categorical[2]}
            radius={[3, 3, 0, 0]}
            cursor="pointer"
            onClick={drillPhase("learning")}
          />
          <Bar
            dataKey="Young"
            fill={categorical[0]}
            radius={[3, 3, 0, 0]}
            cursor="pointer"
            onClick={drillPhase("young")}
          />
          <Bar
            dataKey="Mature"
            fill={categorical[1]}
            radius={[3, 3, 0, 0]}
            cursor="pointer"
            onClick={drillPhase("mature")}
          />
        </BarChart>
      </ResponsiveContainer>
    </div>
  );
}
