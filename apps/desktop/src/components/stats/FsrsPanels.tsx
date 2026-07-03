import { Bar, BarChart, CartesianGrid, ResponsiveContainer, Tooltip, XAxis, YAxis } from "recharts";
import { Brain } from "lucide-react";
import type { FsrsStats } from "@synapse/ipc-types";
import { EmptyState } from "@/components/EmptyState";
import { axisProps, gridProps, sequential, tooltipItemStyle, tooltipLabelStyle, tooltipStyle } from "./chartTheme";

const STABILITY_LABELS = ["<1d", "1-7d", "7-21d", "21-90d", "90-180d", "180-365d", "365d+"];

export function FsrsPanels({ fsrs }: { fsrs: FsrsStats }) {
  if (fsrs.card_count === 0) {
    return (
      <EmptyState
        icon={Brain}
        title="No FSRS cards yet"
        description="Switch a deck to the FSRS scheduler in its options to see memory-model insights here."
      />
    );
  }

  const stabilityData = fsrs.stability_buckets.map((count, i) => ({ label: STABILITY_LABELS[i], count }));
  const difficultyData = fsrs.difficulty_buckets.map((count, i) => ({ label: `${i + 1}`, count }));

  return (
    <div className="space-y-6">
      {fsrs.avg_retrievability !== null ? (
        <div className="flex items-center gap-3 rounded-lg border border-border bg-secondary/40 p-3">
          <span className="text-xs text-muted-foreground">Avg. retrievability now</span>
          <span className="text-lg font-semibold tabular-nums">
            {fsrs.avg_retrievability?.toFixed(0)}%
          </span>
        </div>
      ) : null}
      <div>
        <h3 className="mb-2 text-xs font-medium text-muted-foreground">Stability distribution</h3>
        <ResponsiveContainer width="100%" height={160}>
          <BarChart data={stabilityData} margin={{ top: 4, right: 4, bottom: 0, left: -20 }}>
            <CartesianGrid {...gridProps} />
            <XAxis dataKey="label" {...axisProps} />
            <YAxis allowDecimals={false} {...axisProps} />
            <Tooltip contentStyle={tooltipStyle} itemStyle={tooltipItemStyle} labelStyle={tooltipLabelStyle} />
            <Bar dataKey="count" fill={sequential[3]} radius={[3, 3, 0, 0]} />
          </BarChart>
        </ResponsiveContainer>
      </div>
      <div>
        <h3 className="mb-2 text-xs font-medium text-muted-foreground">Difficulty distribution</h3>
        <ResponsiveContainer width="100%" height={160}>
          <BarChart data={difficultyData} margin={{ top: 4, right: 4, bottom: 0, left: -20 }}>
            <CartesianGrid {...gridProps} />
            <XAxis dataKey="label" {...axisProps} />
            <YAxis allowDecimals={false} {...axisProps} />
            <Tooltip contentStyle={tooltipStyle} itemStyle={tooltipItemStyle} labelStyle={tooltipLabelStyle} />
            <Bar dataKey="count" fill={sequential[3]} radius={[3, 3, 0, 0]} />
          </BarChart>
        </ResponsiveContainer>
      </div>
    </div>
  );
}
