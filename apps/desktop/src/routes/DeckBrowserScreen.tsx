import { useQuery } from "@tanstack/react-query";
import { Download, Layers } from "lucide-react";
import { ScreenHeader } from "@/components/ScreenHeader";
import { EmptyState } from "@/components/EmptyState";
import { Button } from "@/components/ui/button";
import { ipc, isTauri } from "@/lib/ipc";
import { queryKeys } from "@/lib/queryKeys";

export function DeckBrowserScreen() {
  // Exercises the full IPC + ts-rs pipeline when running under Tauri.
  const { data: app } = useQuery({
    queryKey: queryKeys.appInfo,
    queryFn: ipc.appInfo,
    enabled: isTauri(),
  });

  return (
    <div className="flex h-full flex-col">
      <ScreenHeader
        title="Decks"
        description="Your collection. Import an Anki deck to get started."
        actions={
          <Button disabled>
            <Download /> Import .apkg
          </Button>
        }
      />
      <div className="relative flex-1">
        <EmptyState
          icon={Layers}
          title="No decks yet"
          description="Import an .apkg or .colpkg to bring your Anki decks into Synapse. Deck import lands in milestone M2."
        />
        {app ? (
          <div className="absolute bottom-4 right-6 text-xs text-muted-foreground">
            {app.name} v{app.version} · Tauri v{app.tauri_version}
          </div>
        ) : null}
      </div>
    </div>
  );
}
