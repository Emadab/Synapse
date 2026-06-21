import { useEffect, useState } from "react";
import { Command } from "cmdk";
import { useNavigate } from "@tanstack/react-router";
import {
  BarChart3,
  BookOpen,
  Download,
  Layers,
  Moon,
  Plus,
  Search,
  Settings,
  Sun,
  Upload,
} from "lucide-react";
import { useTheme } from "@/stores/theme";
import { isTauri, pickAndImportPackage, pickAndExportPackage } from "@/lib/ipc";
import { useQueryClient } from "@tanstack/react-query";
import { queryKeys } from "@/lib/queryKeys";
import { Kbd } from "@/components/Kbd";

/**
 * Keyboard-first command palette (⌘K / Ctrl+K).
 * Groups: Navigation · Decks · Data · Theme.
 * Plugin commands register against the same list via the extension registry (M9).
 */
export function CommandPalette() {
  const [open, setOpen] = useState(false);
  const navigate = useNavigate();
  const { resolved, setTheme } = useTheme();
  const queryClient = useQueryClient();
  const tauri = isTauri();

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key.toLowerCase() === "k" && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        setOpen((v) => !v);
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, []);

  const close = () => setOpen(false);

  const go = (to: string) => {
    close();
    void navigate({ to });
  };

  const itemClass =
    "flex cursor-pointer items-center gap-2.5 rounded-md px-2 py-2 text-sm text-foreground outline-none aria-selected:bg-accent aria-selected:text-accent-foreground [&_svg]:size-4 [&_svg]:text-muted-foreground";

  const shortcutClass = "ml-auto text-xs text-muted-foreground";

  return (
    <Command.Dialog
      open={open}
      onOpenChange={setOpen}
      label="Command palette"
      overlayClassName="fixed inset-0 z-50 bg-black/40 backdrop-blur-sm data-[state=open]:animate-in data-[state=open]:fade-in-0"
      contentClassName="fixed left-1/2 top-[18%] z-50 w-full max-w-lg -translate-x-1/2 overflow-hidden rounded-xl border border-border bg-popover text-popover-foreground shadow-2xl data-[state=open]:animate-fade-in"
    >
      <Command.Input
        placeholder="Type a command or search…"
        className="w-full border-b border-border bg-transparent px-4 py-3.5 text-sm outline-none placeholder:text-muted-foreground"
      />
      <Command.List className="max-h-80 overflow-y-auto p-2 [&_[cmdk-group-heading]]:px-2 [&_[cmdk-group-heading]]:py-1.5 [&_[cmdk-group-heading]]:text-xs [&_[cmdk-group-heading]]:font-medium [&_[cmdk-group-heading]]:text-muted-foreground">
        <Command.Empty className="py-6 text-center text-sm text-muted-foreground">
          No results found.
        </Command.Empty>

        <Command.Group heading="Navigation">
          <Command.Item className={itemClass} onSelect={() => go("/")}>
            <Layers /> Decks
          </Command.Item>
          <Command.Item className={itemClass} onSelect={() => go("/study")}>
            <BookOpen /> Study
            <span className={shortcutClass}>
              <Kbd>S</Kbd>
            </span>
          </Command.Item>
          <Command.Item className={itemClass} onSelect={() => go("/browse")}>
            <Search /> Browse cards
            <span className={shortcutClass}>
              <Kbd>B</Kbd>
            </span>
          </Command.Item>
          <Command.Item className={itemClass} onSelect={() => go("/stats")}>
            <BarChart3 /> Statistics
          </Command.Item>
          <Command.Item className={itemClass} onSelect={() => go("/settings")}>
            <Settings /> Settings
          </Command.Item>
        </Command.Group>

        {tauri && (
          <Command.Group heading="Decks">
            <Command.Item
              className={itemClass}
              onSelect={() => {
                close();
                void navigate({ to: "/" });
              }}
            >
              <Plus /> New deck
            </Command.Item>
          </Command.Group>
        )}

        {tauri && (
          <Command.Group heading="Data">
            <Command.Item
              className={itemClass}
              onSelect={async () => {
                close();
                try {
                  const summary = await pickAndImportPackage();
                  if (summary) {
                    void queryClient.invalidateQueries({ queryKey: queryKeys.decks });
                  }
                } catch {
                  // errors surfaced in the source screen
                }
              }}
            >
              <Download /> Import .apkg / .colpkg
            </Command.Item>
            <Command.Item
              className={itemClass}
              onSelect={async () => {
                close();
                await pickAndExportPackage();
              }}
            >
              <Upload /> Export collection as .apkg
            </Command.Item>
          </Command.Group>
        )}

        <Command.Group heading="Theme">
          <Command.Item
            className={itemClass}
            onSelect={() => {
              setTheme(resolved === "dark" ? "light" : "dark");
              close();
            }}
          >
            {resolved === "dark" ? <Sun /> : <Moon />} Toggle light / dark
          </Command.Item>
        </Command.Group>
      </Command.List>
    </Command.Dialog>
  );
}
