import { useEffect, useState } from "react";
import { Command } from "cmdk";
import { useNavigate } from "@tanstack/react-router";
import { BarChart3, BookOpen, Layers, Moon, Search, Settings, Sun } from "lucide-react";
import { useTheme } from "@/stores/theme";

/**
 * Keyboard-first command palette (⌘K / Ctrl+K). M0 ships navigation + theme;
 * later milestones register import/export/study/suspend actions and a plugin
 * command provider against the same list.
 */
export function CommandPalette() {
  const [open, setOpen] = useState(false);
  const navigate = useNavigate();
  const { resolved, setTheme } = useTheme();

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key.toLowerCase() === "k" && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        setOpen((value) => !value);
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, []);

  const itemClass =
    "flex cursor-pointer items-center gap-2.5 rounded-md px-2 py-2 text-sm text-foreground outline-none aria-selected:bg-accent aria-selected:text-accent-foreground [&_svg]:size-4 [&_svg]:text-muted-foreground";

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
          <Command.Item
            className={itemClass}
            onSelect={() => (setOpen(false), navigate({ to: "/" }))}
          >
            <Layers /> Decks
          </Command.Item>
          <Command.Item
            className={itemClass}
            onSelect={() => (setOpen(false), navigate({ to: "/study" }))}
          >
            <BookOpen /> Study
          </Command.Item>
          <Command.Item
            className={itemClass}
            onSelect={() => (setOpen(false), navigate({ to: "/browse" }))}
          >
            <Search /> Browse cards
          </Command.Item>
          <Command.Item
            className={itemClass}
            onSelect={() => (setOpen(false), navigate({ to: "/stats" }))}
          >
            <BarChart3 /> Statistics
          </Command.Item>
          <Command.Item
            className={itemClass}
            onSelect={() => (setOpen(false), navigate({ to: "/settings" }))}
          >
            <Settings /> Settings
          </Command.Item>
        </Command.Group>

        <Command.Group heading="Theme">
          <Command.Item
            className={itemClass}
            onSelect={() => {
              setTheme(resolved === "dark" ? "light" : "dark");
              setOpen(false);
            }}
          >
            {resolved === "dark" ? <Sun /> : <Moon />} Toggle light / dark
          </Command.Item>
        </Command.Group>
      </Command.List>
    </Command.Dialog>
  );
}
