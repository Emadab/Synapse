import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { PlusCircle } from "lucide-react";
import { ScreenHeader } from "@/components/ScreenHeader";
import { EmptyState } from "@/components/EmptyState";
import { Button } from "@/components/ui/button";
import { FieldEditor } from "@/components/FieldEditor";
import { ipc, isTauri } from "@/lib/ipc";

export function AddScreen() {
  const tauri = isTauri();
  const queryClient = useQueryClient();

  const notetypes = useQuery({
    queryKey: ["notetypes"],
    queryFn: () => ipc.listNotetypes(),
    enabled: tauri,
  });

  const decks = useQuery({
    queryKey: ["decks"],
    queryFn: () => ipc.listDecks(),
    enabled: tauri,
  });

  const [notetypeId, setNotetypeId] = useState<number | null>(null);
  const [deckId, setDeckId] = useState<number | null>(null);
  const [fields, setFields] = useState<string[]>([]);
  const [tags, setTags] = useState("");
  const [lastResult, setLastResult] = useState<string | null>(null);

  // When notetypes load, select the first one and initialise fields.
  const selectedNotetype =
    notetypes.data?.find((nt) => nt.id === notetypeId) ?? notetypes.data?.[0] ?? null;

  if (notetypeId === null && selectedNotetype) {
    setNotetypeId(selectedNotetype.id);
    setFields(selectedNotetype.field_names.map(() => ""));
  }

  if (deckId === null && decks.data && decks.data.length > 0) {
    setDeckId(decks.data[0].id);
  }

  const handleNotetypeChange = (id: number) => {
    const nt = notetypes.data?.find((n) => n.id === id);
    setNotetypeId(id);
    setFields(nt ? nt.field_names.map(() => "") : []);
    setLastResult(null);
  };

  const add = useMutation({
    mutationFn: () => {
      if (notetypeId === null || deckId === null) throw new Error("Select a note type and deck.");
      return ipc.addNote(notetypeId, deckId, fields, tags.split(/\s+/).filter(Boolean));
    },
    onSuccess: (result) => {
      setLastResult(`Added — ${result.cards_added} card${result.cards_added === 1 ? "" : "s"} created.`);
      // Reset fields for the next note, keep notetype + deck selected.
      if (selectedNotetype) {
        setFields(selectedNotetype.field_names.map(() => ""));
      }
      setTags("");
      void queryClient.invalidateQueries({ queryKey: ["notes"] });
    },
  });

  if (!tauri) {
    return (
      <EmptyState
        icon={PlusCircle}
        title="Run the desktop app"
        description="Add Note requires the Tauri backend. Launch with `pnpm dev`."
      />
    );
  }

  return (
    <div className="flex h-full flex-col">
      <ScreenHeader title="Add Note" description="Create a new note and generate its study cards." />

      <div className="mx-auto w-full max-w-2xl space-y-5 p-8">
        {/* Notetype + Deck pickers */}
        <div className="flex gap-4">
          <div className="flex-1 space-y-1.5">
            <label className="text-sm font-medium">Note type</label>
            <select
              value={notetypeId ?? ""}
              onChange={(e) => handleNotetypeChange(Number(e.target.value))}
              className="h-9 w-full rounded-md border border-input bg-background px-3 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
            >
              {(notetypes.data ?? []).map((nt) => (
                <option key={nt.id} value={nt.id}>
                  {nt.name}
                </option>
              ))}
            </select>
          </div>

          <div className="flex-1 space-y-1.5">
            <label className="text-sm font-medium">Deck</label>
            <select
              value={deckId ?? ""}
              onChange={(e) => setDeckId(Number(e.target.value))}
              className="h-9 w-full rounded-md border border-input bg-background px-3 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
            >
              {(decks.data ?? []).map((d) => (
                <option key={d.id} value={d.id}>
                  {d.name}
                </option>
              ))}
            </select>
          </div>
        </div>

        {/* Dynamic fields */}
        {(selectedNotetype?.field_names ?? []).map((name: string, index: number) => (
          <div key={`${notetypeId}:${name}`} className="space-y-1.5">
            <label className="text-sm font-medium">{name}</label>
            <FieldEditor
              value={fields[index] ?? ""}
              onChange={(html) =>
                setFields((current) => {
                  const next = [...current];
                  next[index] = html;
                  return next;
                })
              }
            />
          </div>
        ))}

        {/* Tags */}
        <div className="space-y-1.5">
          <label className="text-sm font-medium">Tags</label>
          <input
            value={tags}
            onChange={(e) => setTags(e.target.value)}
            placeholder="space separated"
            className="h-9 w-full rounded-md border border-input bg-background px-3 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
          />
        </div>

        <div className="flex items-center gap-3">
          <Button onClick={() => add.mutate()} disabled={add.isPending}>
            <PlusCircle className="mr-1.5 size-4" />
            {add.isPending ? "Adding…" : "Add Note"}
          </Button>
          {lastResult && <span className="text-sm text-muted-foreground">{lastResult}</span>}
          {add.isError && (
            <span className="text-sm text-destructive">
              {String((add.error as Error).message)}
            </span>
          )}
        </div>
      </div>
    </div>
  );
}
