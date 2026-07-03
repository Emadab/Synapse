import { useState, useEffect } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { BookType, Plus, Trash2, ChevronUp, ChevronDown } from "lucide-react";
import { ScreenHeader } from "@/components/ScreenHeader";
import { EmptyState } from "@/components/EmptyState";
import { Button } from "@/components/ui/button";
import { CardFace } from "@/components/CardFace";
import { ipc, isTauri } from "@/lib/ipc";
import { queryKeys } from "@/lib/queryKeys";
import { useTheme } from "@/stores/theme";
import type { NotetypeDetail, TemplateSummary } from "@synapse/ipc-types";

type Tab = "fields" | "templates" | "styling";

export function NotetypeScreen() {
  const tauri = isTauri();
  const queryClient = useQueryClient();
  const night = useTheme((s) => s.resolved === "dark");

  const notetypes = useQuery({
    queryKey: queryKeys.notetypes,
    queryFn: () => ipc.listNotetypes(),
    enabled: tauri,
  });

  const stockNames = useQuery({
    queryKey: ["stockNotetypes"],
    queryFn: () => ipc.listStockNotetypes(),
    enabled: tauri,
    staleTime: Infinity,
  });

  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [activeTab, setActiveTab] = useState<Tab>("fields");
  const [selectedTemplateOrd, setSelectedTemplateOrd] = useState<number>(0);

  // Auto-select first notetype when list loads.
  useEffect(() => {
    if (selectedId === null && notetypes.data && notetypes.data.length > 0) {
      setSelectedId(notetypes.data[0].id);
    }
  }, [notetypes.data, selectedId]);

  const detail = useQuery({
    queryKey: queryKeys.notetype(selectedId!),
    queryFn: () => ipc.getNotetype(selectedId!),
    enabled: tauri && selectedId !== null,
    staleTime: 30_000,
  });

  const nt: NotetypeDetail | null = detail.data ?? null;

  // Preview query — debounced by template selection.
  const template: TemplateSummary | undefined = nt?.templates.find(
    (t) => Number(t.ord) === selectedTemplateOrd,
  );
  const sampleFields = nt?.fields.map(() => "") ?? [];
  const preview = useQuery({
    queryKey: ["preview", selectedId, selectedTemplateOrd, template?.qfmt, template?.afmt],
    queryFn: () => ipc.previewTemplate(selectedId!, selectedTemplateOrd, sampleFields),
    enabled: tauri && selectedId !== null && !!template,
    staleTime: 0,
  });

  function invalidate() {
    void queryClient.invalidateQueries({ queryKey: queryKeys.notetypes });
    if (selectedId !== null) {
      void queryClient.invalidateQueries({
        queryKey: queryKeys.notetype(selectedId),
      });
    }
  }

  // ── Notetype mutations ─────────────────────────────────────────────────────

  const createNotetype = useMutation({
    mutationFn: () => ipc.createNotetype("New Note Type", 0),
    onSuccess: (detail) => {
      invalidate();
      setSelectedId(detail.id);
    },
  });

  const addStockNotetype = useMutation({
    mutationFn: (index: number) => ipc.addStockNotetype(index),
    onSuccess: (detail) => {
      invalidate();
      setSelectedId(detail.id);
    },
  });

  const deleteNotetype = useMutation({
    mutationFn: (id: number) => ipc.deleteNotetype(id),
    onSuccess: () => {
      setSelectedId(null);
      invalidate();
    },
  });

  const renameNotetype = useMutation({
    mutationFn: ({ id, name }: { id: number; name: string }) => ipc.renameNotetype(id, name),
    onSuccess: invalidate,
  });

  const saveCss = useMutation({
    mutationFn: ({ id, css }: { id: number; css: string }) => ipc.saveNotetypeCss(id, css),
    onSuccess: invalidate,
  });

  const [cssDraft, setCssDraft] = useState("");
  useEffect(() => {
    setCssDraft(nt?.css ?? "");
  }, [nt?.id, nt?.css]);

  // ── Field mutations ────────────────────────────────────────────────────────

  const addField = useMutation({
    mutationFn: () => ipc.addField(selectedId!, "New Field"),
    onSuccess: invalidate,
  });

  const removeField = useMutation({
    mutationFn: async (ord: number) => {
      const warn = await ipc.checkFieldRemove(selectedId!, ord);
      if (
        warn.notes_with_content > 0 &&
        !confirm(`${warn.notes_with_content} note(s) have content in this field. Delete anyway?`)
      ) {
        return;
      }
      return ipc.removeField(selectedId!, ord);
    },
    onSuccess: invalidate,
  });

  const renameField = useMutation({
    mutationFn: ({ ord, name }: { ord: number; name: string }) =>
      ipc.renameField(selectedId!, ord, name),
    onSuccess: invalidate,
  });

  const moveField = useMutation({
    mutationFn: ({ ord, direction }: { ord: number; direction: -1 | 1 }) => {
      if (!nt) throw new Error("no notetype");
      const n = nt.fields.length;
      const newOrder = Array.from({ length: n }, (_, i) => i);
      const targetOrd = ord + direction;
      if (targetOrd < 0 || targetOrd >= n) throw new Error("out of range");
      newOrder[ord] = targetOrd;
      newOrder[targetOrd] = ord;
      return ipc.reorderFields(selectedId!, newOrder);
    },
    onSuccess: invalidate,
  });

  // ── Template mutations ─────────────────────────────────────────────────────

  const addTemplate = useMutation({
    mutationFn: () => ipc.addTemplate(selectedId!, "New Card", "{{Front}}", "{{Back}}"),
    onSuccess: invalidate,
  });

  const removeTemplate = useMutation({
    mutationFn: (ord: number) => ipc.removeTemplate(selectedId!, ord),
    onSuccess: () => {
      setSelectedTemplateOrd(0);
      invalidate();
    },
  });

  const saveTemplate = useMutation({
    mutationFn: ({
      ord,
      name,
      qfmt,
      afmt,
    }: {
      ord: number;
      name: string;
      qfmt: string;
      afmt: string;
    }) => ipc.saveTemplate(selectedId!, ord, name, qfmt, afmt),
    onSuccess: invalidate,
  });

  // Local state for the template editor (controlled inputs before saving).
  const [tmplName, setTmplName] = useState("");
  const [tmplQfmt, setTmplQfmt] = useState("");
  const [tmplAfmt, setTmplAfmt] = useState("");

  // Sync local state when selected template changes.
  useEffect(() => {
    if (template) {
      setTmplName(template.name);
      setTmplQfmt(template.qfmt);
      setTmplAfmt(template.afmt);
    }
  }, [template, selectedId]);

  if (!tauri) {
    return (
      <EmptyState
        icon={BookType}
        title="Run the desktop app"
        description="Note type management requires the Tauri backend."
      />
    );
  }

  return (
    <div className="flex h-full flex-col">
      <ScreenHeader
        title="Note Types"
        description="Manage fields and card templates for each note type."
      />

      <div className="flex min-h-0 flex-1">
        {/* ── Left panel: notetype list ─────────────────────────────────── */}
        <aside className="flex w-56 shrink-0 flex-col border-r border-border">
          <div className="flex items-center justify-between px-3 py-2">
            <span className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
              Note Types
            </span>
            <div className="relative">
              <select
                className="absolute inset-0 size-6 cursor-pointer opacity-0"
                value=""
                title="Add note type"
                onChange={(e) => {
                  const value = e.target.value;
                  if (value === "blank") {
                    createNotetype.mutate();
                  } else if (value) {
                    addStockNotetype.mutate(Number(value));
                  }
                  e.target.value = "";
                }}
              >
                <option value="" disabled>
                  Add note type…
                </option>
                <option value="blank">Blank note type</option>
                {(stockNames.data ?? []).map((name, i) => (
                  <option key={name} value={i}>
                    {name}
                  </option>
                ))}
              </select>
              <Button variant="ghost" size="icon" className="pointer-events-none size-6">
                <Plus className="size-3.5" />
              </Button>
            </div>
          </div>

          <div className="flex-1 overflow-auto">
            {(notetypes.data ?? []).map((nt) => (
              <button
                key={nt.id}
                onClick={() => {
                  setSelectedId(nt.id);
                  setSelectedTemplateOrd(0);
                  setActiveTab("fields");
                }}
                className={[
                  "w-full px-3 py-2 text-left text-sm transition-colors",
                  selectedId === nt.id
                    ? "bg-sidebar-accent font-medium"
                    : "text-sidebar-foreground hover:bg-sidebar-accent/60",
                ].join(" ")}
              >
                {nt.name}
                <span className="ml-1 text-xs text-muted-foreground">
                  ({Number(nt.kind) === 1 ? "Cloze" : "Standard"})
                </span>
              </button>
            ))}
          </div>
        </aside>

        {/* ── Right panel: editor ───────────────────────────────────────── */}
        {nt ? (
          <div className="flex min-w-0 flex-1 flex-col">
            {/* Header: name + delete */}
            <div className="flex items-center gap-3 border-b border-border px-5 py-3">
              <input
                className="flex-1 rounded border border-transparent bg-transparent px-2 py-1 text-base font-semibold outline-none hover:border-input focus:border-ring focus:ring-1 focus:ring-ring"
                value={nt.name}
                onChange={(e) => {
                  const name = e.target.value;
                  renameNotetype.mutate({ id: nt.id, name });
                }}
              />
              <Button
                variant="ghost"
                size="icon"
                className="text-destructive hover:text-destructive"
                title="Delete note type"
                onClick={() => {
                  if (confirm(`Delete "${nt.name}"? This fails if notes reference it.`)) {
                    deleteNotetype.mutate(nt.id);
                  }
                }}
              >
                <Trash2 className="size-4" />
              </Button>
            </div>

            {/* Tabs */}
            <div className="flex border-b border-border">
              {(["fields", "templates", "styling"] as Tab[]).map((tab) => (
                <button
                  key={tab}
                  onClick={() => setActiveTab(tab)}
                  className={[
                    "px-4 py-2 text-sm font-medium capitalize transition-colors",
                    activeTab === tab
                      ? "border-b-2 border-primary text-foreground"
                      : "text-muted-foreground hover:text-foreground",
                  ].join(" ")}
                >
                  {tab}
                </button>
              ))}
            </div>

            {/* Tab content */}
            <div className="flex min-h-0 flex-1 overflow-hidden">
              {activeTab === "fields" && (
                <div className="flex w-full flex-col gap-2 overflow-auto p-4">
                  {nt.fields.map((field) => {
                    const fieldOrd = Number(field.ord);
                    return (
                      <div
                        key={fieldOrd}
                        className="flex items-center gap-2 rounded-md border border-border bg-card px-3 py-2"
                      >
                        <div className="flex flex-col gap-0.5">
                          <button
                            onClick={() => moveField.mutate({ ord: fieldOrd, direction: -1 })}
                            disabled={fieldOrd === 0}
                            className="text-muted-foreground hover:text-foreground disabled:opacity-30"
                          >
                            <ChevronUp className="size-3.5" />
                          </button>
                          <button
                            onClick={() => moveField.mutate({ ord: fieldOrd, direction: 1 })}
                            disabled={fieldOrd === nt.fields.length - 1}
                            className="text-muted-foreground hover:text-foreground disabled:opacity-30"
                          >
                            <ChevronDown className="size-3.5" />
                          </button>
                        </div>

                        <input
                          className="flex-1 rounded border border-transparent bg-transparent px-1 py-0.5 text-sm outline-none hover:border-input focus:border-ring focus:ring-1 focus:ring-ring"
                          defaultValue={field.name}
                          onBlur={(e) => {
                            const name = e.target.value.trim();
                            if (name && name !== field.name) {
                              renameField.mutate({ ord: fieldOrd, name });
                            }
                          }}
                        />

                        <button
                          onClick={() => removeField.mutate(fieldOrd)}
                          className="text-muted-foreground hover:text-destructive"
                        >
                          <Trash2 className="size-3.5" />
                        </button>
                      </div>
                    );
                  })}

                  <Button
                    variant="outline"
                    size="sm"
                    className="mt-1 w-fit"
                    onClick={() => addField.mutate()}
                  >
                    <Plus className="mr-1.5 size-3.5" />
                    Add Field
                  </Button>
                </div>
              )}

              {activeTab === "templates" && (
                <div className="flex min-w-0 flex-1 overflow-hidden">
                  {/* Template selector */}
                  <div className="flex w-44 shrink-0 flex-col border-r border-border">
                    <div className="flex items-center justify-between px-3 py-2">
                      <span className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                        Cards
                      </span>
                      {Number(nt.kind) === 0 && (
                        <Button
                          variant="ghost"
                          size="icon"
                          className="size-6"
                          onClick={() => addTemplate.mutate()}
                          title="Add card template"
                        >
                          <Plus className="size-3.5" />
                        </Button>
                      )}
                    </div>
                    {nt.templates.map((t) => {
                      const tOrd = Number(t.ord);
                      return (
                        <button
                          key={tOrd}
                          onClick={() => setSelectedTemplateOrd(tOrd)}
                          className={[
                            "flex items-center justify-between px-3 py-2 text-left text-sm transition-colors",
                            selectedTemplateOrd === tOrd
                              ? "bg-sidebar-accent font-medium"
                              : "text-sidebar-foreground hover:bg-sidebar-accent/60",
                          ].join(" ")}
                        >
                          <span className="truncate">{t.name}</span>
                          {nt.templates.length > 1 && (
                            <button
                              onClick={(e) => {
                                e.stopPropagation();
                                if (confirm(`Remove card template "${t.name}"?`)) {
                                  removeTemplate.mutate(tOrd);
                                }
                              }}
                              className="ml-1 text-muted-foreground hover:text-destructive"
                            >
                              <Trash2 className="size-3" />
                            </button>
                          )}
                        </button>
                      );
                    })}
                  </div>

                  {/* Template editor + preview */}
                  {template && (
                    <div className="flex min-w-0 flex-1 flex-col overflow-hidden">
                      <div className="flex min-h-0 flex-1 overflow-hidden">
                        {/* Editor */}
                        <div className="flex w-1/2 flex-col gap-3 overflow-auto border-r border-border p-4">
                          <div className="space-y-1">
                            <label className="text-xs font-medium text-muted-foreground">
                              Name
                            </label>
                            <input
                              className="w-full rounded border border-input bg-background px-2 py-1 text-sm outline-none focus:ring-1 focus:ring-ring"
                              value={tmplName}
                              onChange={(e) => setTmplName(e.target.value)}
                            />
                          </div>

                          <div className="space-y-1">
                            <label className="text-xs font-medium text-muted-foreground">
                              Front template
                            </label>
                            <textarea
                              className="h-28 w-full resize-none rounded border border-input bg-background px-2 py-1 font-mono text-xs outline-none focus:ring-1 focus:ring-ring"
                              value={tmplQfmt}
                              onChange={(e) => setTmplQfmt(e.target.value)}
                            />
                          </div>

                          <div className="space-y-1">
                            <label className="text-xs font-medium text-muted-foreground">
                              Back template
                            </label>
                            <textarea
                              className="h-28 w-full resize-none rounded border border-input bg-background px-2 py-1 font-mono text-xs outline-none focus:ring-1 focus:ring-ring"
                              value={tmplAfmt}
                              onChange={(e) => setTmplAfmt(e.target.value)}
                            />
                          </div>

                          <Button
                            size="sm"
                            className="w-fit"
                            onClick={() =>
                              saveTemplate.mutate({
                                ord: Number(template.ord),
                                name: tmplName,
                                qfmt: tmplQfmt,
                                afmt: tmplAfmt,
                              })
                            }
                            disabled={saveTemplate.isPending}
                          >
                            {saveTemplate.isPending ? "Saving…" : "Save"}
                          </Button>
                        </div>

                        {/* Preview */}
                        <div className="flex w-1/2 flex-col overflow-auto p-4">
                          <span className="mb-2 text-xs font-medium text-muted-foreground">
                            Preview (empty fields)
                          </span>
                          <div className="mb-3 space-y-1">
                            <div className="rounded border border-border bg-card p-3">
                              <div className="mb-1 text-xs text-muted-foreground">Front</div>
                              <CardFace
                                html={preview.data?.question ?? ""}
                                css={nt.css}
                                tauri={tauri}
                                night={night}
                                className="text-sm"
                              />
                            </div>
                            <div className="rounded border border-border bg-card p-3">
                              <div className="mb-1 text-xs text-muted-foreground">Back</div>
                              <CardFace
                                html={preview.data?.answer ?? ""}
                                css={nt.css}
                                tauri={tauri}
                                night={night}
                                className="text-sm"
                              />
                            </div>
                          </div>
                        </div>
                      </div>
                    </div>
                  )}
                </div>
              )}

              {activeTab === "styling" && (
                <div className="flex min-w-0 flex-1 overflow-hidden">
                  <div className="flex w-1/2 flex-col gap-3 overflow-auto border-r border-border p-4">
                    <label className="text-xs font-medium text-muted-foreground">Card CSS</label>
                    <textarea
                      className="h-full min-h-64 w-full resize-none rounded border border-input bg-background px-2 py-1 font-mono text-xs outline-none focus:ring-1 focus:ring-ring"
                      value={cssDraft}
                      onChange={(e) => setCssDraft(e.target.value)}
                      spellCheck={false}
                    />
                    <Button
                      size="sm"
                      className="w-fit"
                      onClick={() => saveCss.mutate({ id: nt.id, css: cssDraft })}
                      disabled={saveCss.isPending}
                    >
                      {saveCss.isPending ? "Saving…" : "Save"}
                    </Button>
                  </div>
                  <div className="flex w-1/2 flex-col overflow-auto p-4">
                    <span className="mb-2 text-xs font-medium text-muted-foreground">
                      Preview (empty fields)
                    </span>
                    <div className="space-y-1">
                      <div className="rounded border border-border bg-card p-3">
                        <div className="mb-1 text-xs text-muted-foreground">Front</div>
                        <CardFace
                          html={preview.data?.question ?? ""}
                          css={cssDraft}
                          tauri={tauri}
                          night={night}
                          className="text-sm"
                        />
                      </div>
                      <div className="rounded border border-border bg-card p-3">
                        <div className="mb-1 text-xs text-muted-foreground">Back</div>
                        <CardFace
                          html={preview.data?.answer ?? ""}
                          css={cssDraft}
                          tauri={tauri}
                          night={night}
                          className="text-sm"
                        />
                      </div>
                    </div>
                  </div>
                </div>
              )}
            </div>
          </div>
        ) : (
          <div className="flex flex-1 items-center justify-center text-sm text-muted-foreground">
            {notetypes.isLoading ? "Loading…" : "Select a note type"}
          </div>
        )}
      </div>
    </div>
  );
}
