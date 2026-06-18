import { BookOpen } from "lucide-react";
import { ScreenHeader } from "@/components/ScreenHeader";
import { EmptyState } from "@/components/EmptyState";

export function StudyScreen() {
  return (
    <div className="flex h-full flex-col">
      <ScreenHeader title="Study" description="Review your due cards." />
      <div className="flex-1">
        <EmptyState
          icon={BookOpen}
          title="Nothing to study"
          description="Once you import or create decks, your due cards appear here. Study mode (queues, card rendering, answer grading) lands in milestone M4."
        />
      </div>
    </div>
  );
}
