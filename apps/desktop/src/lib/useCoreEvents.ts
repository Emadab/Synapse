import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { useQueryClient } from "@tanstack/react-query";
import { ipc, isTauri } from "@/lib/ipc";
import { queryKeys } from "@/lib/queryKeys";

/**
 * Subscribe to core domain events (re-emitted by the Tauri shell) and map them
 * to TanStack Query cache invalidations. This is how the UI stays live without
 * polling — the Rust core is the source of truth. Mount once in the AppShell.
 */
export function useCoreEvents() {
  const queryClient = useQueryClient();

  useEffect(() => {
    if (!isTauri()) return;
    // Rust core has no OS timezone access; the day-rollover boundary is
    // evaluated in local time using this offset, sent once per session.
    void ipc.setLocalOffset(-new Date().getTimezoneOffset());
  }, []);

  useEffect(() => {
    if (!isTauri()) return;

    const unlisten = listen<string>("synapse://event", ({ payload }) => {
      if (payload === "deck-changed" || payload === "schema-changed") {
        void queryClient.invalidateQueries({ queryKey: queryKeys.decks });
      }
      if (payload === "notes-changed" || payload === "schema-changed") {
        void queryClient.invalidateQueries({ queryKey: ["notes"] });
        void queryClient.invalidateQueries({ queryKey: ["notetypes"] });
      }
      if (
        payload === "card-answered" ||
        payload === "notes-changed" ||
        payload === "schema-changed"
      ) {
        void queryClient.invalidateQueries({ queryKey: ["stats"] });
      }
    });

    return () => {
      void unlisten.then((dispose) => dispose());
    };
  }, [queryClient]);
}
