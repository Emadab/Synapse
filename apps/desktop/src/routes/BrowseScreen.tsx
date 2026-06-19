import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Save, Search } from "lucide-react";
import type { NoteField } from "@synapse/ipc-types";
import { ScreenHeader } from "@/components/ScreenHeader";
import { EmptyState } from "@/components/EmptyState";
import { Button } from "@/components/ui/button";
import { FieldEditor } from "@/components/FieldEditor";
import { ipc, isTauri } from "@/lib/ipc";

const plain = (html: string) => html.replace(/<[^>]*>/g, "");

export function BrowseScreen() {
  const tauri = isTauri();
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<number | null>(null);

  const notes = useQuery({
    queryKey: ["notes", query],
    queryFn: () => ipc.listNotes(query || undefined),
    enabled: tauri,
  });

  return (
    <div className="flex h-full flex-col">
      <ScreenHeader title="Browse" description="Search and edit your notes." />
      <div className="flex min-h-0 flex-1">
        <aside className="flex w-80 shrink-0 flex-col border-r border-border">
          <div className="relative border-b border-border p-2">
            <Search className="absolute left-4 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
            <input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Search notes…"
              className="h-9 w-full rounded-md bg-secondary pl-9 pr-3 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
            />
          </div>
          <ul className="min-h-0 flex-1 overflow-auto">
            {(notes.data ?? []).map((note) => (
              <li key={note.note_id}>
                <button
                  onClick={() => setSelected(note.note_id)}
                  className={`flex w-full flex-col items-start gap-0.5 border-b border-border px-3 py-2.5 text-left transition-colors hover:bg-accent/50 ${
                    selected === note.note_id ? "bg-accent" : ""
                  }`}
                >
                  <span className="line-clamp-1 text-sm font-medium">
                    {plain(note.sort_field) || "(empty)"}
                  </span>
                  {note.tags.length > 0 ? (
                    <span className="line-clamp-1 text-xs text-muted-foreground">
                      {note.tags.join(" ")}
                    </span>
                  ) : null}
                </button>
              </li>
            ))}
          </ul>
        </aside>

        <section className="min-w-0 flex-1 overflow-auto">
          {selected === null ? (
            <EmptyState
              icon={Search}
              title={tauri ? "Select a note" : "Run the desktop app"}
              description={
                tauri
                  ? "Pick a note on the left to edit its fields and tags."
                  : "The browser runs against the Rust core over Tauri. Launch with `pnpm dev`."
              }
            />
          ) : (
            <NoteEditorPanel key={selected} noteId={selected} />
          )}
        </section>
      </div>
    </div>
  );
}

function NoteEditorPanel({ noteId }: { noteId: number }) {
  const queryClient = useQueryClient();
  const note = useQuery({ queryKey: ["note", noteId], queryFn: () => ipc.getNote(noteId) });

  const [fields, setFields] = useState<NoteField[]>([]);
  const [tags, setTags] = useState("");

  useEffect(() => {
    if (note.data) {
      setFields(note.data.fields);
      setTags(note.data.tags.join(" "));
    }
  }, [note.data]);

  const save = useMutation({
    mutationFn: () =>
      ipc.saveNote(
        noteId,
        fields.map((f) => f.value),
        tags.split(/\s+/).filter(Boolean),
      ),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["notes"] });
      void queryClient.invalidateQueries({ queryKey: ["note", noteId] });
    },
  });

  if (!note.data) {
    return <div className="p-8 text-sm text-muted-foreground">Loading…</div>;
  }

  return (
    <div className="mx-auto max-w-2xl space-y-4 p-8">
      <div className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
        {note.data.notetype_name}
      </div>

      {fields.map((field, index) => (
        <div key={`${noteId}:${field.name}`} className="space-y-1.5">
          <label className="text-sm font-medium">{field.name}</label>
          <FieldEditor
            value={field.value}
            onChange={(html) =>
              setFields((current) =>
                current.map((f, i) => (i === index ? { ...f, value: html } : f)),
              )
            }
          />
        </div>
      ))}

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
        <Button onClick={() => save.mutate()} disabled={save.isPending}>
          <Save /> {save.isPending ? "Saving…" : "Save"}
        </Button>
        {save.isSuccess ? <span className="text-sm text-muted-foreground">Saved.</span> : null}
      </div>
    </div>
  );
}
