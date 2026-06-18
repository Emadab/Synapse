import { Search } from "lucide-react";
import { ScreenHeader } from "@/components/ScreenHeader";
import { EmptyState } from "@/components/EmptyState";

export function BrowseScreen() {
  return (
    <div className="flex h-full flex-col">
      <ScreenHeader title="Browse" description="Search and edit your cards and notes." />
      <div className="flex-1">
        <EmptyState
          icon={Search}
          title="Card browser"
          description="A virtualized browser with fuzzy search, filters and inline editing arrives in milestones M5 and M7 (Tantivy search)."
        />
      </div>
    </div>
  );
}
