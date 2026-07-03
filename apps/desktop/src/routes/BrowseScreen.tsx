import { useEffect, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  ChevronDown,
  ChevronUp,
  Flag,
  Layers,
  MinusCircle,
  Pencil,
  Save,
  Search,
  SkipForward,
  Tag,
  Trash2,
  X,
} from "lucide-react";
import type { CardRow, NoteField } from "@synapse/ipc-types";
import { ScreenHeader } from "@/components/ScreenHeader";
import { EmptyState } from "@/components/EmptyState";
import { Button } from "@/components/ui/button";
import { FieldEditor } from "@/components/FieldEditor";
import { ipc, isTauri } from "@/lib/ipc";

// ── helpers ──────────────────────────────────────────────────────────────────

const plain = (html: string) => html.replace(/<[^>]*>/g, "");

const QUEUE_LABELS: Record<number, { text: string; cls: string }> = {
  "-3": { text: "Sibling", cls: "text-muted-foreground" },
  "-2": { text: "Buried", cls: "text-amber-600 dark:text-amber-400" },
  "-1": { text: "Suspended", cls: "text-red-500" },
  0: { text: "New", cls: "text-blue-600 dark:text-blue-400" },
  1: { text: "Learn", cls: "text-amber-600 dark:text-amber-400" },
  2: { text: "Review", cls: "text-green-600 dark:text-green-400" },
};

const FLAG_COLORS: Record<number, string> = {
  0: "text-muted-foreground",
  1: "text-red-500",
  2: "text-orange-500",
  3: "text-green-500",
  4: "text-blue-500",
};

type SortKey = "sort_field" | "deck" | "queue" | "due" | "lapses" | "interval";

function sortRows(rows: CardRow[], key: SortKey, asc: boolean): CardRow[] {
  return [...rows].sort((a, b) => {
    let va: string | number = a[key];
    let vb: string | number = b[key];
    if (typeof va === "string") va = va.toLowerCase();
    if (typeof vb === "string") vb = vb.toLowerCase();
    return asc ? (va < vb ? -1 : va > vb ? 1 : 0) : va > vb ? -1 : va < vb ? 1 : 0;
  });
}

const QUERY_SYNTAX = `Syntax help:
  is:new  is:review  is:learn  is:due
  is:suspended  is:buried
  flag:1  flag:2  flag:3  flag:4
  deck:name   deck:parent*
  tag:word
  note:notetype
  added:7  (last N days)
  prop:ivl>5   prop:lapses>=3   prop:reps<10
  prop:due>0   prop:ease>200
  -is:new  (negate with -)
  word or phrase  (OR connector)`;

// ── tag sidebar ───────────────────────────────────────────────────────────────

function TagSidebar({ onFilter }: { onFilter: (tag: string) => void }) {
  const queryClient = useQueryClient();
  const tagsQuery = useQuery({ queryKey: ["tags"], queryFn: ipc.listTags });
  const tags = tagsQuery.data ?? [];

  const [renaming, setRenaming] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState("");

  const renameMut = useMutation({
    mutationFn: ({ old, next }: { old: string; next: string }) => ipc.renameTag(old, next),
    onSuccess: () => {
      setRenaming(null);
      void queryClient.invalidateQueries({ queryKey: ["tags"] });
      void queryClient.invalidateQueries({ queryKey: ["cards"] });
    },
  });

  const deleteMut = useMutation({
    mutationFn: (tag: string) => ipc.deleteTag(tag),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["tags"] });
      void queryClient.invalidateQueries({ queryKey: ["cards"] });
    },
  });

  return (
    <aside className="w-44 shrink-0 overflow-y-auto border-r border-border bg-secondary/20">
      <div className="px-3 py-2 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
        Tags
      </div>
      {tags.length === 0 ? (
        <p className="px-3 py-2 text-xs text-muted-foreground">No tags</p>
      ) : (
        <ul>
          {tags.map((tag) => (
            <li key={tag} className="group flex items-center gap-1 px-2 py-0.5">
              {renaming === tag ? (
                <form
                  className="flex flex-1 items-center gap-1"
                  onSubmit={(e) => {
                    e.preventDefault();
                    if (renameValue.trim() && renameValue.trim() !== tag) {
                      renameMut.mutate({ old: tag, next: renameValue.trim() });
                    } else {
                      setRenaming(null);
                    }
                  }}
                >
                  <input
                    autoFocus
                    value={renameValue}
                    onChange={(e) => setRenameValue(e.target.value)}
                    className="h-6 flex-1 rounded border border-input bg-background px-1.5 text-xs outline-none focus:ring-1 focus:ring-ring"
                    onKeyDown={(e) => {
                      if (e.key === "Escape") setRenaming(null);
                    }}
                  />
                  <button type="submit" className="text-primary hover:opacity-70">
                    <Save className="size-3" />
                  </button>
                  <button
                    type="button"
                    onClick={() => setRenaming(null)}
                    className="text-muted-foreground hover:opacity-70"
                  >
                    <X className="size-3" />
                  </button>
                </form>
              ) : (
                <>
                  <button
                    className="flex-1 truncate text-left text-xs hover:text-foreground text-muted-foreground"
                    onClick={() => onFilter(tag)}
                    title={`Search tag:${tag}`}
                  >
                    {tag}
                  </button>
                  <button
                    className="hidden group-hover:inline-flex text-muted-foreground hover:text-foreground"
                    onClick={() => {
                      setRenaming(tag);
                      setRenameValue(tag);
                    }}
                    title="Rename"
                  >
                    <Pencil className="size-3" />
                  </button>
                  <button
                    className="hidden group-hover:inline-flex text-muted-foreground hover:text-destructive"
                    onClick={() => {
                      if (confirm(`Delete tag "${tag}" from all notes?`)) {
                        deleteMut.mutate(tag);
                      }
                    }}
                    title="Delete"
                  >
                    <Trash2 className="size-3" />
                  </button>
                </>
              )}
            </li>
          ))}
        </ul>
      )}
    </aside>
  );
}

// ── main component ────────────────────────────────────────────────────────────

export function BrowseScreen() {
  const tauri = isTauri();
  const queryClient = useQueryClient();

  const [query, setQuery] = useState("");
  const [submitted, setSubmitted] = useState("");
  const [sortKey, setSortKey] = useState<SortKey>("sort_field");
  const [sortAsc, setSortAsc] = useState(true);
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [lastClickIdx, setLastClickIdx] = useState<number | null>(null);
  const [editNoteId, setEditNoteId] = useState<number | null>(null);
  const [showSyntax, setShowSyntax] = useState(false);
  const [bulkTag, setBulkTag] = useState("");
  const [bulkAction, setBulkAction] = useState<null | "addTag" | "removeTag" | "moveDeck">(null);
  const [deckInput, setDeckInput] = useState("");
  const bulkRef = useRef<HTMLDivElement>(null);

  const cardsQuery = useQuery({
    queryKey: ["cards", submitted],
    queryFn: () => ipc.searchCards(submitted),
    enabled: tauri,
  });

  const rows = cardsQuery.data ?? [];
  const sorted = sortRows(rows, sortKey, sortAsc);

  const invalidate = () => queryClient.invalidateQueries({ queryKey: ["cards"] });

  const deleteMut = useMutation({
    mutationFn: (noteIds: number[]) => ipc.deleteNotes(noteIds),
    onSuccess: () => {
      setSelected(new Set());
      void invalidate();
    },
  });

  const suspendMut = useMutation({
    mutationFn: (cardIds: number[]) => ipc.suspendCards(cardIds),
    onSuccess: () => {
      setSelected(new Set());
      void invalidate();
    },
  });

  const buryMut = useMutation({
    mutationFn: (cardIds: number[]) => ipc.buryCards(cardIds),
    onSuccess: () => {
      setSelected(new Set());
      void invalidate();
    },
  });

  const addTagMut = useMutation({
    mutationFn: ({ noteIds, tag }: { noteIds: number[]; tag: string }) =>
      ipc.bulkAddTag(noteIds, tag),
    onSuccess: () => {
      setBulkTag("");
      setBulkAction(null);
      void invalidate();
    },
  });

  const removeTagMut = useMutation({
    mutationFn: ({ noteIds, tag }: { noteIds: number[]; tag: string }) =>
      ipc.bulkRemoveTag(noteIds, tag),
    onSuccess: () => {
      setBulkTag("");
      setBulkAction(null);
      void invalidate();
    },
  });

  const moveDeckMut = useMutation({
    mutationFn: async ({ cardIds, deckName }: { cardIds: number[]; deckName: string }) => {
      const decks = await ipc.listDecks();
      const deck = decks.find((d) => d.name === deckName);
      if (!deck) throw new Error(`Deck "${deckName}" not found`);
      return ipc.moveCardsToDeck(cardIds, deck.id);
    },
    onSuccess: () => {
      setDeckInput("");
      setBulkAction(null);
      void invalidate();
    },
  });

  // Keyboard: Ctrl+A selects all.
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === "a" && sorted.length > 0) {
        e.preventDefault();
        setSelected(new Set(sorted.map((r) => r.card_id)));
      }
      if (e.key === "Escape") {
        setSelected(new Set());
        setBulkAction(null);
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [sorted]);

  function toggleSort(key: SortKey) {
    if (sortKey === key) setSortAsc((a) => !a);
    else {
      setSortKey(key);
      setSortAsc(true);
    }
  }

  function handleRowClick(row: CardRow, idx: number, e: React.MouseEvent) {
    const id = row.card_id;
    if (e.shiftKey && lastClickIdx !== null) {
      const lo = Math.min(lastClickIdx, idx);
      const hi = Math.max(lastClickIdx, idx);
      setSelected((prev) => {
        const next = new Set(prev);
        for (let i = lo; i <= hi; i++) next.add(sorted[i].card_id);
        return next;
      });
    } else if (e.ctrlKey || e.metaKey) {
      setSelected((prev) => {
        const next = new Set(prev);
        if (next.has(id)) next.delete(id);
        else next.add(id);
        return next;
      });
    } else {
      setSelected(new Set([id]));
      setEditNoteId(row.note_id);
    }
    setLastClickIdx(idx);
  }

  const selArray = [...selected];
  const selectedRows = sorted.filter((r) => selected.has(r.card_id));
  const selectedNoteIds = [...new Set(selectedRows.map((r) => r.note_id))];

  function SortIcon({ k }: { k: SortKey }) {
    if (sortKey !== k) return null;
    return sortAsc ? <ChevronUp className="size-3" /> : <ChevronDown className="size-3" />;
  }

  function Th({ label, k }: { label: string; k: SortKey }) {
    return (
      <th
        className="cursor-pointer select-none whitespace-nowrap border-b border-border px-3 py-2 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground hover:text-foreground"
        onClick={() => toggleSort(k)}
      >
        <span className="inline-flex items-center gap-1">
          {label}
          <SortIcon k={k} />
        </span>
      </th>
    );
  }

  return (
    <div className="flex h-full flex-col">
      <ScreenHeader title="Browse" description="Search and edit cards with Anki-style queries." />

      {/* Search bar */}
      <div className="flex items-center gap-2 border-b border-border px-4 py-2">
        <div className="relative flex-1">
          <Search className="absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") setSubmitted(query);
            }}
            placeholder={`Search cards… (Enter to run)  e.g. is:due tag:verb`}
            className="h-9 w-full rounded-md bg-secondary pl-9 pr-3 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
          />
        </div>
        <Button size="sm" onClick={() => setSubmitted(query)}>
          Search
        </Button>
        <Button
          variant="ghost"
          size="sm"
          onClick={() => setShowSyntax((v) => !v)}
          title="Query syntax help"
        >
          ?
        </Button>
        <span className="text-xs text-muted-foreground tabular-nums">
          {rows.length} {rows.length === 1 ? "card" : "cards"}
        </span>
      </div>

      {showSyntax && (
        <pre className="border-b border-border bg-secondary/60 px-5 py-3 text-xs text-muted-foreground">
          {QUERY_SYNTAX}
        </pre>
      )}

      <div className="flex min-h-0 flex-1">
        {/* Tag sidebar */}
        {tauri && (
          <TagSidebar
            onFilter={(tag) => {
              const q = `tag:${tag}`;
              setQuery(q);
              setSubmitted(q);
            }}
          />
        )}
        {/* Table */}
        <div className="min-w-0 flex-1 overflow-auto">
          {!tauri ? (
            <EmptyState
              icon={Layers}
              title="Run the desktop app"
              description="The browser queries the Rust core over Tauri. Launch with `pnpm dev`."
            />
          ) : rows.length === 0 && submitted !== "" ? (
            <EmptyState
              icon={Search}
              title="No results"
              description="Try a different query or clear the search to show all cards."
            />
          ) : (
            <table className="w-full border-collapse text-sm">
              <thead className="sticky top-0 bg-background z-10">
                <tr>
                  <th className="w-8 border-b border-border px-2 py-2">
                    <input
                      type="checkbox"
                      ref={(el) => {
                        if (el)
                          el.indeterminate = selArray.length > 0 && selArray.length < sorted.length;
                      }}
                      checked={selArray.length === sorted.length && sorted.length > 0}
                      onChange={(e) =>
                        setSelected(
                          e.target.checked ? new Set(sorted.map((r) => r.card_id)) : new Set(),
                        )
                      }
                      className="cursor-pointer"
                    />
                  </th>
                  <th className="w-5 border-b border-border px-1 py-2" />
                  <Th label="Front" k="sort_field" />
                  <Th label="Deck" k="deck" />
                  <Th label="Status" k="queue" />
                  <Th label="Due" k="due" />
                  <Th label="Interval" k="interval" />
                  <Th label="Lapses" k="lapses" />
                  <th className="border-b border-border px-3 py-2 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    Tags
                  </th>
                </tr>
              </thead>
              <tbody>
                {sorted.map((row, idx) => {
                  const isSelected = selected.has(row.card_id);
                  const qInfo = QUEUE_LABELS[row.queue] ?? { text: String(row.queue), cls: "" };
                  return (
                    <tr
                      key={row.card_id}
                      onClick={(e) => handleRowClick(row, idx, e)}
                      className={`cursor-pointer border-b border-border/50 hover:bg-accent/40 ${
                        isSelected ? "bg-accent" : ""
                      }`}
                    >
                      <td className="px-2 py-1.5 text-center" onClick={(e) => e.stopPropagation()}>
                        <input
                          type="checkbox"
                          checked={isSelected}
                          onChange={(e) => {
                            setSelected((prev) => {
                              const next = new Set(prev);
                              if (e.target.checked) next.add(row.card_id);
                              else next.delete(row.card_id);
                              return next;
                            });
                          }}
                          className="cursor-pointer"
                        />
                      </td>
                      <td className="px-1 py-1.5 text-center">
                        {row.flags > 0 && (
                          <Flag className={`size-3.5 ${FLAG_COLORS[row.flags] ?? ""}`} />
                        )}
                      </td>
                      <td className="max-w-xs truncate px-3 py-1.5 font-medium">
                        {plain(row.sort_field) || "(empty)"}
                      </td>
                      <td className="whitespace-nowrap px-3 py-1.5 text-muted-foreground">
                        {row.deck}
                      </td>
                      <td className={`whitespace-nowrap px-3 py-1.5 font-medium ${qInfo.cls}`}>
                        {qInfo.text}
                      </td>
                      <td className="whitespace-nowrap px-3 py-1.5 text-muted-foreground tabular-nums">
                        {row.queue === 0
                          ? `pos ${row.due}`
                          : row.queue === 1
                            ? "soon"
                            : `${row.due}d`}
                      </td>
                      <td className="whitespace-nowrap px-3 py-1.5 text-muted-foreground tabular-nums">
                        {row.interval > 0 ? `${row.interval}d` : "—"}
                      </td>
                      <td className="whitespace-nowrap px-3 py-1.5 text-muted-foreground tabular-nums">
                        {row.lapses}
                      </td>
                      <td className="max-w-xs px-3 py-1.5">
                        <span className="line-clamp-1 text-xs text-muted-foreground">
                          {row.tags.join(" ")}
                        </span>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          )}
        </div>

        {/* Editor panel */}
        {editNoteId !== null && selected.size === 1 && (
          <aside className="w-80 shrink-0 overflow-auto border-l border-border">
            <NoteEditorPanel
              key={editNoteId}
              noteId={editNoteId}
              onSaved={() => void invalidate()}
            />
          </aside>
        )}
      </div>

      {/* Bulk action bar */}
      {selArray.length > 0 && (
        <div
          ref={bulkRef}
          className="flex flex-wrap items-center gap-2 border-t border-border bg-secondary/60 px-4 py-2"
        >
          <span className="text-sm font-medium">{selArray.length} selected</span>
          <Button
            variant="outline"
            size="sm"
            disabled={suspendMut.isPending}
            onClick={() => suspendMut.mutate(selArray)}
          >
            <MinusCircle className="size-3.5" /> Suspend
          </Button>
          <Button
            variant="outline"
            size="sm"
            disabled={buryMut.isPending}
            onClick={() => buryMut.mutate(selArray)}
          >
            <SkipForward className="size-3.5" /> Bury
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={() => setBulkAction(bulkAction === "addTag" ? null : "addTag")}
          >
            <Tag className="size-3.5" /> Add tag
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={() => setBulkAction(bulkAction === "removeTag" ? null : "removeTag")}
          >
            <Tag className="size-3.5" /> Remove tag
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={() => setBulkAction(bulkAction === "moveDeck" ? null : "moveDeck")}
          >
            <Layers className="size-3.5" /> Move to deck
          </Button>
          <Button
            variant="outline"
            size="sm"
            className="text-destructive"
            disabled={deleteMut.isPending}
            onClick={() => {
              if (confirm(`Delete ${selectedNoteIds.length} note(s)? This cannot be undone.`)) {
                deleteMut.mutate(selectedNoteIds);
              }
            }}
          >
            <Trash2 className="size-3.5" /> Delete
          </Button>
          <Button
            variant="ghost"
            size="sm"
            onClick={() => {
              setSelected(new Set());
              setBulkAction(null);
            }}
          >
            Clear
          </Button>

          {(bulkAction === "addTag" || bulkAction === "removeTag") && (
            <form
              className="flex items-center gap-1.5"
              onSubmit={(e) => {
                e.preventDefault();
                if (!bulkTag.trim()) return;
                if (bulkAction === "addTag") {
                  addTagMut.mutate({ noteIds: selectedNoteIds, tag: bulkTag.trim() });
                } else {
                  removeTagMut.mutate({ noteIds: selectedNoteIds, tag: bulkTag.trim() });
                }
              }}
            >
              <input
                autoFocus
                value={bulkTag}
                onChange={(e) => setBulkTag(e.target.value)}
                placeholder="tag name"
                className="h-7 w-32 rounded border border-input bg-background px-2 text-xs outline-none focus:ring-1 focus:ring-ring"
              />
              <Button type="submit" size="sm" className="h-7 text-xs">
                {bulkAction === "addTag" ? "Add" : "Remove"}
              </Button>
            </form>
          )}

          {bulkAction === "moveDeck" && (
            <form
              className="flex items-center gap-1.5"
              onSubmit={(e) => {
                e.preventDefault();
                if (!deckInput.trim()) return;
                moveDeckMut.mutate({ cardIds: selArray, deckName: deckInput.trim() });
              }}
            >
              <input
                autoFocus
                value={deckInput}
                onChange={(e) => setDeckInput(e.target.value)}
                placeholder="Deck name"
                className="h-7 w-40 rounded border border-input bg-background px-2 text-xs outline-none focus:ring-1 focus:ring-ring"
              />
              <Button
                type="submit"
                size="sm"
                className="h-7 text-xs"
                disabled={moveDeckMut.isPending}
              >
                Move
              </Button>
              {moveDeckMut.isError && (
                <span className="text-xs text-destructive">
                  {String((moveDeckMut.error as Error).message)}
                </span>
              )}
            </form>
          )}
        </div>
      )}
    </div>
  );
}

// ── note editor panel ─────────────────────────────────────────────────────────

function NoteEditorPanel({ noteId, onSaved }: { noteId: number; onSaved: () => void }) {
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
      void queryClient.invalidateQueries({ queryKey: ["note", noteId] });
      onSaved();
    },
  });

  if (!note.data) {
    return <div className="p-6 text-sm text-muted-foreground">Loading…</div>;
  }

  return (
    <div className="space-y-4 p-5">
      <div className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
        {note.data.notetype_name}
      </div>

      {fields.map((field, index) => (
        <div key={`${noteId}:${field.name}`} className="space-y-1.5">
          <label className="text-sm font-medium">{field.name}</label>
          <FieldEditor
            value={field.value}
            otherFieldsHtml={fields.filter((_, i) => i !== index).map((f) => f.value)}
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
          className="h-8 w-full rounded-md border border-input bg-background px-3 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
        />
      </div>

      <div className="flex items-center gap-3">
        <Button size="sm" onClick={() => save.mutate()} disabled={save.isPending}>
          <Save className="size-3.5" /> {save.isPending ? "Saving…" : "Save"}
        </Button>
        {save.isSuccess ? <span className="text-xs text-muted-foreground">Saved.</span> : null}
      </div>
    </div>
  );
}
