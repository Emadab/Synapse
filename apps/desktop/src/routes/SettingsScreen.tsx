import { useState } from "react";
import { ScreenHeader } from "@/components/ScreenHeader";
import { Button } from "@/components/ui/button";
import { useTheme, type Theme } from "@/stores/theme";
import { isTauri, pickAndExportPackage, errorMessage } from "@/lib/ipc";

const themes: { value: Theme; label: string }[] = [
  { value: "light", label: "Light" },
  { value: "dark", label: "Dark" },
  { value: "system", label: "System" },
];

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
            SM-2 and FSRS will be selectable per deck. Wired up in milestone M3.
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
      </div>
    </div>
  );
}
