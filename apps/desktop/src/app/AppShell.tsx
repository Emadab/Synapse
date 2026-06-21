import { useEffect, useState } from "react";
import { Link, Outlet } from "@tanstack/react-router";
import { motion } from "framer-motion";
import {
  BarChart3,
  BookOpen,
  BookType,
  Command as CommandIcon,
  Layers,
  Moon,
  PlusCircle,
  Search,
  Settings,
  Sun,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { CommandPalette } from "@/components/CommandPalette";
import { KeyboardShortcutsDialog } from "@/components/KeyboardShortcutsDialog";
import { Kbd } from "@/components/Kbd";
import { useCoreEvents } from "@/lib/useCoreEvents";
import { useTheme } from "@/stores/theme";

const nav = [
  { to: "/", label: "Decks", icon: Layers, exact: true },
  { to: "/study", label: "Study", icon: BookOpen, exact: false },
  { to: "/browse", label: "Browse", icon: Search, exact: false },
  { to: "/add", label: "Add", icon: PlusCircle, exact: false },
  { to: "/notetypes", label: "Note Types", icon: BookType, exact: false },
  { to: "/stats", label: "Stats", icon: BarChart3, exact: false },
  { to: "/settings", label: "Settings", icon: Settings, exact: false },
] as const;

/** Root layout: persistent sidebar + header, routed content in the outlet. */
export function AppShell() {
  const { resolved, setTheme } = useTheme();
  const [shortcutsOpen, setShortcutsOpen] = useState(false);
  useCoreEvents();

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (
        e.key === "?" &&
        !e.metaKey &&
        !e.ctrlKey &&
        !(e.target instanceof HTMLInputElement) &&
        !(e.target instanceof HTMLTextAreaElement)
      ) {
        e.preventDefault();
        setShortcutsOpen((v) => !v);
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, []);

  return (
    <div className="flex h-screen overflow-hidden bg-background text-foreground">
      {/* Skip-to-content: invisible until focused by keyboard navigation */}
      <a
        href="#main-content"
        className="sr-only focus:not-sr-only focus:absolute focus:left-2 focus:top-2 focus:z-[100] focus:rounded focus:bg-primary focus:px-3 focus:py-1.5 focus:text-sm focus:font-medium focus:text-primary-foreground"
      >
        Skip to content
      </a>

      <aside className="flex w-60 shrink-0 flex-col border-r border-border bg-sidebar">
        <div className="flex h-14 items-center gap-2.5 px-4">
          <div className="flex size-7 items-center justify-center rounded-md bg-primary text-sm font-bold text-primary-foreground">
            S
          </div>
          <span className="text-sm font-semibold tracking-tight">Synapse</span>
        </div>

        <nav aria-label="Main navigation" className="flex-1 space-y-1 px-3 py-2">
          {nav.map(({ to, label, icon: Icon, exact }) => (
            <Link
              key={to}
              to={to}
              activeOptions={{ exact }}
              className="group flex items-center gap-2.5 rounded-md px-3 py-2 text-sm font-medium transition-colors"
              activeProps={{ className: "bg-sidebar-accent text-foreground" }}
              inactiveProps={{
                className:
                  "text-sidebar-foreground hover:bg-sidebar-accent/60 hover:text-foreground",
              }}
            >
              <Icon className="size-4" />
              {label}
            </Link>
          ))}
        </nav>

        <div className="px-4 py-3 text-xs text-muted-foreground">v0.1.0 · MVP</div>
      </aside>

      <div className="flex min-w-0 flex-1 flex-col">
        <header className="flex h-14 shrink-0 items-center justify-between border-b border-border px-5">
          <div className="flex items-center gap-2 text-sm text-muted-foreground">
            <CommandIcon className="size-3.5" />
            <span>Press</span>
            <Kbd>⌘K</Kbd>
            <span>for commands</span>
          </div>
          <Button
            variant="ghost"
            size="icon"
            aria-label="Toggle theme"
            onClick={() => setTheme(resolved === "dark" ? "light" : "dark")}
          >
            {resolved === "dark" ? <Sun className="size-4" /> : <Moon className="size-4" />}
          </Button>
        </header>

        <main id="main-content" className="min-h-0 flex-1 overflow-auto">
          <motion.div
            initial={{ opacity: 0, y: 4 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.18, ease: "easeOut" }}
            className="h-full"
          >
            <Outlet />
          </motion.div>
        </main>
      </div>

      <CommandPalette />
      {shortcutsOpen && (
        <KeyboardShortcutsDialog onClose={() => setShortcutsOpen(false)} />
      )}
    </div>
  );
}
