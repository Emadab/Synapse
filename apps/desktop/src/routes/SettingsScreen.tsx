import { useState, useEffect, useRef } from "react";
import { useQuery, useMutation } from "@tanstack/react-query";
import { ScreenHeader } from "@/components/ScreenHeader";
import { Button } from "@/components/ui/button";
import { useTheme, type Theme } from "@/stores/theme";
import { ipc, isTauri, pickAndExportPackage, errorMessage } from "@/lib/ipc";
import type { BackupInfo, PluginInfo } from "@synapse/ipc-types";
import { pluginHost, type PluginToast } from "@/lib/pluginHost";
import { open } from "@tauri-apps/plugin-dialog";

function MaintenanceSection() {
  const [backupMsg, setBackupMsg] = useState<string>("");
  const [integrityResult, setIntegrityResult] = useState<string[]>([]);
  const [optimizeMsg, setOptimizeMsg] = useState<string>("");
  const [mediaResult, setMediaResult] = useState<{
    orphan_files: string[];
    missing_files: string[];
  } | null>(null);
  const [deleteOrphanConfirm, setDeleteOrphanConfirm] = useState(false);
  const [restoreConfirm, setRestoreConfirm] = useState<string | null>(null);

  const backupsQuery = useQuery({ queryKey: ["backups"], queryFn: ipc.listBackups });

  const backupMut = useMutation({
    mutationFn: ipc.createBackup,
    onSuccess: (info) => {
      setBackupMsg(`Backup created: ${info.name} (${fmt_size(info.size_bytes)})`);
      void backupsQuery.refetch();
    },
    onError: (e) => setBackupMsg(errorMessage(e)),
  });

  const restoreMut = useMutation({
    mutationFn: (name: string) => ipc.restoreBackup(name),
    onSuccess: () => {
      setRestoreConfirm(null);
      setBackupMsg("Backup restored. Restart Synapse to apply.");
    },
    onError: (e) => {
      setRestoreConfirm(null);
      setBackupMsg(errorMessage(e));
    },
  });

  const deleteBackupMut = useMutation({
    mutationFn: (name: string) => ipc.deleteBackup(name),
    onSuccess: () => void backupsQuery.refetch(),
    onError: (e) => setBackupMsg(errorMessage(e)),
  });

  const integrityMut = useMutation({
    mutationFn: ipc.checkIntegrity,
    onSuccess: (errs) => setIntegrityResult(errs),
    onError: (e) => setIntegrityResult([errorMessage(e)]),
  });

  const optimizeMut = useMutation({
    mutationFn: ipc.optimizeDb,
    onSuccess: () => setOptimizeMsg("Database compacted and optimized."),
    onError: (e) => setOptimizeMsg(errorMessage(e)),
  });

  const mediaMut = useMutation({
    mutationFn: ipc.checkMedia,
    onSuccess: (r) => setMediaResult(r),
    onError: (e) => setMediaResult({ orphan_files: [errorMessage(e)], missing_files: [] }),
  });

  const deleteOrphanMut = useMutation({
    mutationFn: (files: string[]) => ipc.deleteOrphanMedia(files),
    onSuccess: (count) => {
      setDeleteOrphanConfirm(false);
      setMediaResult((prev) => (prev ? { ...prev, orphan_files: [] } : prev));
      setOptimizeMsg(`Deleted ${count} orphan file${count === 1 ? "" : "s"}.`);
    },
    onError: (e) => {
      setDeleteOrphanConfirm(false);
      setOptimizeMsg(errorMessage(e));
    },
  });

  const busy =
    backupMut.isPending ||
    restoreMut.isPending ||
    deleteBackupMut.isPending ||
    integrityMut.isPending ||
    optimizeMut.isPending ||
    mediaMut.isPending ||
    deleteOrphanMut.isPending;

  return (
    <section className="mt-8 max-w-xl space-y-6">
      <div>
        <h2 className="text-sm font-medium">Maintenance</h2>
        <p className="text-sm text-muted-foreground">Backups, integrity, and media cleanup.</p>
      </div>

      {/* Backups */}
      <div className="space-y-2">
        <div className="flex items-center gap-2">
          <Button size="sm" variant="outline" disabled={busy} onClick={() => backupMut.mutate()}>
            {backupMut.isPending ? "Backing up…" : "Backup now"}
          </Button>
          {backupMsg && <span className="text-sm text-muted-foreground">{backupMsg}</span>}
        </div>
        {(backupsQuery.data ?? []).length > 0 && (
          <ul className="rounded-lg border border-border divide-y divide-border text-sm">
            {(backupsQuery.data ?? []).map((b: BackupInfo) => (
              <li key={b.name} className="flex items-center justify-between px-3 py-2 gap-2">
                <span className="text-muted-foreground tabular-nums text-xs">
                  {fmt_date(b.created_ms)}
                </span>
                <span className="text-xs text-muted-foreground">{fmt_size(b.size_bytes)}</span>
                <Button
                  size="sm"
                  variant="ghost"
                  className="h-6 px-2 text-xs text-destructive"
                  disabled={busy}
                  onClick={() => setRestoreConfirm(b.name)}
                >
                  Restore
                </Button>
                <Button
                  size="sm"
                  variant="ghost"
                  className="h-6 px-2 text-xs text-muted-foreground hover:text-destructive"
                  disabled={busy}
                  onClick={() => deleteBackupMut.mutate(b.name)}
                >
                  Delete
                </Button>
              </li>
            ))}
          </ul>
        )}
        {restoreConfirm && (
          <div className="rounded-lg border border-destructive/50 bg-destructive/10 p-3 space-y-2 text-sm">
            <p className="font-medium text-destructive">Restore backup "{restoreConfirm}"?</p>
            <p className="text-muted-foreground text-xs">
              This overwrites your current collection. Restart the app after restoring. This cannot
              be undone.
            </p>
            <div className="flex gap-2">
              <Button
                size="sm"
                variant="destructive"
                disabled={restoreMut.isPending}
                onClick={() => restoreMut.mutate(restoreConfirm)}
              >
                {restoreMut.isPending ? "Restoring…" : "Yes, restore"}
              </Button>
              <Button size="sm" variant="outline" onClick={() => setRestoreConfirm(null)}>
                Cancel
              </Button>
            </div>
          </div>
        )}
      </div>

      {/* Integrity */}
      <div className="space-y-2">
        <div className="flex items-center gap-2">
          <Button size="sm" variant="outline" disabled={busy} onClick={() => integrityMut.mutate()}>
            {integrityMut.isPending ? "Checking…" : "Check integrity"}
          </Button>
          {integrityResult.length === 0 && integrityMut.isSuccess && (
            <span className="text-sm text-green-600 dark:text-green-400">Database is healthy.</span>
          )}
        </div>
        {integrityResult.length > 0 && (
          <ul className="text-sm text-destructive space-y-0.5">
            {integrityResult.map((e, i) => (
              <li key={i}>{e}</li>
            ))}
          </ul>
        )}
      </div>

      {/* Optimize */}
      <div className="flex items-center gap-2">
        <Button size="sm" variant="outline" disabled={busy} onClick={() => optimizeMut.mutate()}>
          {optimizeMut.isPending ? "Optimizing…" : "Optimize database"}
        </Button>
        {optimizeMsg && <span className="text-sm text-muted-foreground">{optimizeMsg}</span>}
      </div>

      {/* Media */}
      <div className="space-y-2">
        <div className="flex items-center gap-2">
          <Button size="sm" variant="outline" disabled={busy} onClick={() => mediaMut.mutate()}>
            {mediaMut.isPending ? "Scanning…" : "Check media"}
          </Button>
          {mediaResult &&
            mediaResult.orphan_files.length === 0 &&
            mediaResult.missing_files.length === 0 && (
              <span className="text-sm text-green-600 dark:text-green-400">
                All media files are consistent.
              </span>
            )}
        </div>
        {mediaResult &&
          (mediaResult.orphan_files.length > 0 || mediaResult.missing_files.length > 0) && (
            <div className="space-y-2 text-sm">
              {mediaResult.orphan_files.length > 0 && (
                <div className="space-y-2">
                  <p className="font-medium text-amber-600 dark:text-amber-400">
                    Orphan files ({mediaResult.orphan_files.length}) — on disk but not used:
                  </p>
                  <ul className="text-xs text-muted-foreground pl-3 space-y-0.5">
                    {mediaResult.orphan_files.slice(0, 20).map((f) => (
                      <li key={f}>{f}</li>
                    ))}
                    {mediaResult.orphan_files.length > 20 && (
                      <li>…and {mediaResult.orphan_files.length - 20} more</li>
                    )}
                  </ul>
                  {!deleteOrphanConfirm ? (
                    <Button
                      size="sm"
                      variant="outline"
                      className="text-destructive border-destructive/40 hover:bg-destructive/10"
                      disabled={busy}
                      onClick={() => setDeleteOrphanConfirm(true)}
                    >
                      Delete orphan files…
                    </Button>
                  ) : (
                    <div className="rounded-lg border border-destructive/50 bg-destructive/10 p-3 space-y-2 text-sm">
                      <p className="font-medium text-destructive">
                        Delete {mediaResult.orphan_files.length} orphan file
                        {mediaResult.orphan_files.length === 1 ? "" : "s"}?
                      </p>
                      <p className="text-xs text-muted-foreground">This cannot be undone.</p>
                      <div className="flex gap-2">
                        <Button
                          size="sm"
                          variant="destructive"
                          disabled={deleteOrphanMut.isPending}
                          onClick={() => deleteOrphanMut.mutate(mediaResult.orphan_files)}
                        >
                          {deleteOrphanMut.isPending ? "Deleting…" : "Yes, delete"}
                        </Button>
                        <Button
                          size="sm"
                          variant="outline"
                          onClick={() => setDeleteOrphanConfirm(false)}
                        >
                          Cancel
                        </Button>
                      </div>
                    </div>
                  )}
                </div>
              )}
              {mediaResult.missing_files.length > 0 && (
                <div>
                  <p className="font-medium text-destructive">
                    Missing files ({mediaResult.missing_files.length}) — referenced in notes but not
                    on disk:
                  </p>
                  <ul className="text-xs text-muted-foreground pl-3 space-y-0.5">
                    {mediaResult.missing_files.slice(0, 20).map((f) => (
                      <li key={f}>{f}</li>
                    ))}
                    {mediaResult.missing_files.length > 20 && (
                      <li>…and {mediaResult.missing_files.length - 20} more</li>
                    )}
                  </ul>
                </div>
              )}
            </div>
          )}
      </div>
    </section>
  );
}

function PluginManagerSection() {
  const [toast, setToast] = useState<string | null>(null);
  const [loadingIds, setLoadingIds] = useState<Set<string>>(new Set());
  const [commands, setCommands] = useState(() => [...pluginHost.commands]);
  const [runResult, setRunResult] = useState<string | null>(null);
  const unsubRef = useRef<(() => void) | null>(null);

  const pluginsQuery = useQuery({ queryKey: ["plugins"], queryFn: ipc.listPlugins });

  // Subscribe to plugin toasts
  useEffect(() => {
    const unsub = pluginHost.onToast((t: PluginToast) => {
      setToast(t.message);
      setTimeout(() => setToast(null), 4000);
    });
    unsubRef.current = unsub;
    return unsub;
  }, []);

  const enableMut = useMutation({
    mutationFn: async (plugin: PluginInfo) => {
      await ipc.enablePlugin(plugin.id);
      const code = await ipc.getPluginEntry(plugin.id);
      pluginHost.load(plugin.id, plugin.permissions, code);
      setCommands([...pluginHost.commands]);
    },
    onSuccess: () => void pluginsQuery.refetch(),
    onError: (e) => setToast(errorMessage(e)),
  });

  const disableMut = useMutation({
    mutationFn: async (plugin: PluginInfo) => {
      await ipc.disablePlugin(plugin.id);
      pluginHost.unload(plugin.id);
      setCommands([...pluginHost.commands]);
    },
    onSuccess: () => void pluginsQuery.refetch(),
    onError: (e) => setToast(errorMessage(e)),
  });

  const installMut = useMutation({
    mutationFn: async () => {
      const selected = await open({ directory: true, title: "Select plugin folder" });
      if (typeof selected !== "string") return;
      return ipc.installPlugin(selected);
    },
    onSuccess: () => void pluginsQuery.refetch(),
    onError: (e) => setToast(errorMessage(e)),
  });

  function markLoading(id: string, loading: boolean) {
    setLoadingIds((prev) => {
      const next = new Set(prev);
      if (loading) {
        next.add(id);
      } else {
        next.delete(id);
      }
      return next;
    });
  }

  const plugins = pluginsQuery.data ?? [];

  return (
    <section className="mt-8 max-w-xl space-y-4">
      <div>
        <h2 className="text-sm font-medium">Plugins</h2>
        <p className="text-sm text-muted-foreground">
          Extend Synapse with sandboxed plugin scripts.
        </p>
      </div>

      {toast && (
        <div className="rounded-md bg-primary/10 px-3 py-2 text-sm text-primary border border-primary/20">
          {toast}
        </div>
      )}

      {plugins.length === 0 && !pluginsQuery.isLoading && (
        <p className="text-sm text-muted-foreground">No plugins installed.</p>
      )}

      {plugins.length > 0 && (
        <ul className="divide-y divide-border rounded-lg border border-border text-sm">
          {plugins.map((p) => (
            <li key={p.id} className="flex flex-col gap-1.5 px-3 py-3">
              <div className="flex items-center justify-between gap-2">
                <div>
                  <span className="font-medium">{p.name}</span>
                  <span className="ml-2 text-xs text-muted-foreground">v{p.version}</span>
                  {p.author && (
                    <span className="ml-2 text-xs text-muted-foreground">by {p.author}</span>
                  )}
                </div>
                <Button
                  size="sm"
                  variant={p.enabled ? "default" : "outline"}
                  disabled={loadingIds.has(p.id)}
                  onClick={async () => {
                    markLoading(p.id, true);
                    try {
                      if (p.enabled) {
                        await disableMut.mutateAsync(p);
                      } else {
                        await enableMut.mutateAsync(p);
                      }
                    } finally {
                      markLoading(p.id, false);
                    }
                  }}
                >
                  {loadingIds.has(p.id) ? "…" : p.enabled ? "Enabled" : "Enable"}
                </Button>
              </div>
              {p.description && <p className="text-xs text-muted-foreground">{p.description}</p>}
              {p.permissions.length > 0 && (
                <div className="flex flex-wrap gap-1">
                  {p.permissions.map((perm) => (
                    <span
                      key={perm}
                      className="rounded-full bg-secondary px-2 py-0.5 text-[10px] font-medium text-secondary-foreground"
                    >
                      {perm}
                    </span>
                  ))}
                </div>
              )}
            </li>
          ))}
        </ul>
      )}

      <Button
        size="sm"
        variant="outline"
        disabled={installMut.isPending}
        onClick={() => installMut.mutate()}
      >
        {installMut.isPending ? "Installing…" : "Install plugin…"}
      </Button>

      {/* Plugin commands contributed by enabled plugins */}
      {commands.length > 0 && (
        <div className="space-y-2">
          <p className="text-xs font-medium text-muted-foreground">Plugin commands</p>
          <ul className="divide-y divide-border rounded-lg border border-border text-sm">
            {commands.map((cmd) => (
              <li key={cmd.id} className="flex items-center justify-between gap-2 px-3 py-2">
                <span>{cmd.title}</span>
                <Button
                  size="sm"
                  variant="ghost"
                  className="h-6 px-2 text-xs"
                  onClick={() => {
                    pluginHost.invokeCommand(cmd.id);
                    setRunResult(`Invoked: ${cmd.title}`);
                    setTimeout(() => setRunResult(null), 3000);
                  }}
                >
                  Run
                </Button>
              </li>
            ))}
          </ul>
          {runResult && <p className="text-xs text-muted-foreground">{runResult}</p>}
        </div>
      )}
    </section>
  );
}

const UPDATE_MANIFEST_URL =
  "https://github.com/synapse-srs/synapse/releases/latest/download/latest.json";

interface UpdateManifest {
  version: string;
  notes?: string;
  pub_date?: string;
}

function UpdateSection() {
  const [checking, setChecking] = useState(false);
  const [result, setResult] = useState<
    | { kind: "up-to-date" }
    | { kind: "available"; version: string; notes?: string }
    | { kind: "error"; message: string }
    | null
  >(null);

  const appInfoQuery = useQuery({ queryKey: ["appInfo"], queryFn: ipc.appInfo });

  async function checkForUpdate() {
    setChecking(true);
    setResult(null);
    try {
      const res = await fetch(UPDATE_MANIFEST_URL);
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const manifest: UpdateManifest = await res.json();
      const current = appInfoQuery.data?.version ?? "0.0.0";
      if (manifest.version && manifest.version !== current) {
        setResult({ kind: "available", version: manifest.version, notes: manifest.notes });
      } else {
        setResult({ kind: "up-to-date" });
      }
    } catch (e) {
      setResult({ kind: "error", message: String(e) });
    } finally {
      setChecking(false);
    }
  }

  return (
    <section className="mt-8 max-w-xl space-y-3">
      <div>
        <h2 className="text-sm font-medium">Updates</h2>
        <p className="text-sm text-muted-foreground">
          Current version: <span className="font-mono">{appInfoQuery.data?.version ?? "…"}</span>
        </p>
      </div>
      <Button size="sm" variant="outline" disabled={checking} onClick={checkForUpdate}>
        {checking ? "Checking…" : "Check for updates"}
      </Button>
      {result?.kind === "up-to-date" && (
        <p className="text-sm text-green-600 dark:text-green-400">Synapse is up to date.</p>
      )}
      {result?.kind === "available" && (
        <div className="rounded-lg border border-primary/30 bg-primary/5 p-3 space-y-2 text-sm">
          <p className="font-medium">
            Version <span className="font-mono">{result.version}</span> is available.
          </p>
          {result.notes && <p className="text-xs text-muted-foreground">{result.notes}</p>}
          <a
            href={`https://github.com/synapse-srs/synapse/releases/tag/v${result.version}`}
            target="_blank"
            rel="noreferrer"
            className="inline-block text-xs text-primary underline underline-offset-2"
          >
            Download from GitHub →
          </a>
        </div>
      )}
      {result?.kind === "error" && <p className="text-sm text-destructive">{result.message}</p>}
    </section>
  );
}

const themes: { value: Theme; label: string }[] = [
  { value: "light", label: "Light" },
  { value: "dark", label: "Dark" },
  { value: "system", label: "System" },
];

function fmt_size(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function fmt_date(ms: number): string {
  return new Date(ms).toLocaleString();
}

export function SettingsScreen() {
  const { theme, setTheme } = useTheme();
  const tauri = isTauri();
  const [exportState, setExportState] = useState<"idle" | "busy" | "done" | string>("idle");

  return (
    <div className="flex h-full flex-col">
      <ScreenHeader title="Settings" description="Preferences and appearance." />
      <div className="flex-1 overflow-auto px-8 py-6">
        <section className="max-w-xl space-y-3">
          <div>
            <h2 className="text-sm font-medium">Appearance</h2>
            <p className="text-sm text-muted-foreground">Choose how Synapse looks.</p>
          </div>
          <div className="flex gap-2">
            {themes.map((option) => (
              <Button
                key={option.value}
                variant={theme === option.value ? "default" : "outline"}
                size="sm"
                onClick={() => setTheme(option.value)}
              >
                {option.label}
              </Button>
            ))}
          </div>
        </section>

        <section className="mt-8 max-w-xl space-y-1">
          <h2 className="text-sm font-medium">Scheduling</h2>
          <p className="text-sm text-muted-foreground">
            SM-2 and FSRS are both implemented and switchable per deck via deck options.
          </p>
        </section>

        {tauri && (
          <section className="mt-8 max-w-xl space-y-3">
            <div>
              <h2 className="text-sm font-medium">Export</h2>
              <p className="text-sm text-muted-foreground">
                Export your full collection as an Anki-compatible .apkg file.
              </p>
            </div>
            <div className="flex items-center gap-3">
              <Button
                variant="outline"
                size="sm"
                disabled={exportState === "busy"}
                onClick={async () => {
                  setExportState("busy");
                  try {
                    const count = await pickAndExportPackage();
                    setExportState(count === null ? "idle" : "done");
                  } catch (e) {
                    setExportState(errorMessage(e));
                  }
                }}
              >
                {exportState === "busy" ? "Exporting…" : "Export .apkg"}
              </Button>
              {exportState === "done" && (
                <span className="text-sm text-muted-foreground">Exported.</span>
              )}
              {exportState !== "idle" && exportState !== "busy" && exportState !== "done" && (
                <span className="text-sm text-destructive">{exportState}</span>
              )}
            </div>
          </section>
        )}

        {tauri && <MaintenanceSection />}
        {tauri && <PluginManagerSection />}
        {tauri && <UpdateSection />}
      </div>
    </div>
  );
}
