import { Link, Outlet } from "@tanstack/react-router";
import { motion } from "framer-motion";
import {
  BarChart3,
  BookOpen,
  Command as CommandIcon,
  Layers,
  Moon,
  Search,
  Settings,
  Sun,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { CommandPalette } from "@/components/CommandPalette";
import { Kbd } from "@/components/Kbd";
import { useTheme } from "@/stores/theme";

const nav = [
  { to: "/", label: "Decks", icon: Layers, exact: true },
  { to: "/study", label: "Study", icon: BookOpen, exact: false },
  { to: "/browse", label: "Browse", icon: Search, exact: false },
  { to: "/stats", label: "Stats", icon: BarChart3, exact: false },
  { to: "/settings", label: "Settings", icon: Settings, exact: false },
] as const;

/** Root layout: persistent sidebar + header, routed content in the outlet. */
export function AppShell() {
  const { resolved, setTheme } = useTheme();

  return (
    <div className="flex h-screen overflow-hidden bg-background text-foreground">
      <aside className="flex w-60 shrink-0 flex-col border-r border-border bg-sidebar">
        <div className="flex h-14 items-center gap-2.5 px-4">
          <div className="flex size-7 items-center justify-center rounded-md bg-primary text-sm font-bold text-primary-foreground">
            S
          </div>
          <span className="text-sm font-semibold tracking-tight">Synapse</span>
        </div>

        <nav className="flex-1 space-y-1 px-3 py-2">
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

        <main className="min-h-0 flex-1 overflow-auto">
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
    </div>
  );
}
