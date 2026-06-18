import { BarChart3 } from "lucide-react";
import { ScreenHeader } from "@/components/ScreenHeader";
import { EmptyState } from "@/components/EmptyState";

export function StatsScreen() {
  return (
    <div className="flex h-full flex-col">
      <ScreenHeader title="Statistics" description="Your learning, visualized." />
      <div className="flex-1">
        <EmptyState
          icon={BarChart3}
          title="No statistics yet"
          description="Review heatmaps, retention, time studied and forecasts arrive in milestone M8."
        />
      </div>
    </div>
  );
}
