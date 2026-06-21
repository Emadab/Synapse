import { useCallback, useEffect, useState } from "react";
import { Link, Outlet, useMatchRoute } from "@tanstack/react-router";
import { AnimatePresence, motion } from "framer-motion";
import {
  BarChart3,
  BookOpen,
  BookType,
  ChevronLeft,
  ChevronRight,
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
import { TitleBar } from "@/components/TitleBar";
import { ease } from "@/lib/motion";

const nav = [
  { to: "/", label: "Decks", icon: Layers, exact: true },
  { to: "/study", label: "Study", icon: BookOpen, exact: false },
  { to: "/browse", label: "Browse", icon: Search, exact: false },
  { to: "/add", label: "Add", icon: PlusCircle, exact: false },
  { to: "/notetypes", label: "Note Types", icon: BookType, exact: false },
  { to: "/stats", label: "Stats", icon: BarChart3, exact: false },
] as const;

type SidebarMode = "docked" | "hover";

const ICON_ONLY_THRESHOLD = 96;
const MIN_WIDTH = 44;
const MAX_WIDTH = 360;
const DEFAULT_WIDTH = 240;

function readMode(): SidebarMode {
  return (localStorage.getItem("sb-mode") as SidebarMode) ?? "docked";
}
function readWidth(): number {
  return Number(localStorage.getItem("sb-width")) || DEFAULT_WIDTH;
}

function NavLink({
  to,
  label,
  icon: Icon,
  exact,
  compact,
}: {
  to: string;
  label: string;
  icon: React.ElementType;
  exact: boolean;
  compact: boolean;
}) {
  const matchRoute = useMatchRoute();
  const isActive = !!matchRoute({ to, fuzzy: !exact });

  return (
    <Link
      to={to}
      activeOptions={{ exact }}
      title={compact ? label : undefined}
      className={`relative flex items-center rounded-md py-2 text-sm font-medium transition-colors ${compact ? "justify-center px-2" : "gap-2.5 px-3"}`}
      activeProps={{ className: "text-foreground" }}
      inactiveProps={{
        className: "text-sidebar-foreground hover:bg-sidebar-accent/60 hover:text-foreground",
      }}
    >
      {isActive && (
        <motion.div
          layoutId="nav-pill"
          className="absolute inset-0 rounded-md bg-sidebar-accent"
          transition={{ duration: 0.18, ease }}
        />
      )}
      <span className={`relative z-10 flex items-center ${compact ? "" : "gap-2.5"}`}>
        <Icon className="size-4 shrink-0" />
        {!compact && label}
      </span>
    </Link>
  );
}

function SidebarContent({
  compact,
  mode,
  toggleMode,
  resolved,
}: {
  compact: boolean;
  mode: SidebarMode;
  toggleMode: () => void;
  resolved: string;
}) {
  const matchRoute = useMatchRoute();
  const settingsActive = !!matchRoute({ to: "/settings", fuzzy: true });

  return (
    <>
      {/* Logo + collapse toggle */}
      <div
        className={`flex h-14 shrink-0 items-center border-b border-border ${compact ? "justify-center px-2" : "justify-between px-4"}`}
      >
        <div className={`flex items-center ${compact ? "" : "gap-2.5"}`}>
          <img
            src={
              resolved === "dark"
                ? "/logos/synapse-icon-mono-white.png"
                : "/logos/synapse-icon-mono-black.png"
            }
            alt="Synapse"
            className="size-7 shrink-0"
          />
          {!compact && <span className="text-sm font-semibold tracking-tight">Synapse</span>}
        </div>
        {!compact && (
          <button
            onClick={toggleMode}
            title={mode === "docked" ? "Hover mode (Alt+Z)" : "Dock sidebar (Alt+Z)"}
            className="rounded p-1 text-sidebar-foreground transition-colors hover:bg-sidebar-accent/60 hover:text-foreground"
          >
            {mode === "docked" ? (
              <ChevronLeft className="size-4" />
            ) : (
              <ChevronRight className="size-4" />
            )}
          </button>
        )}
      </div>

      {/* Nav items */}
      <nav aria-label="Main navigation" className="flex-1 space-y-0.5 px-2 py-2">
        {nav.map(({ to, label, icon, exact }) => (
          <NavLink key={to} to={to} label={label} icon={icon} exact={exact} compact={compact} />
        ))}
      </nav>

      {/* Bottom: Settings + version */}
      <div className="mt-auto space-y-0.5 border-t border-border px-2 py-2">
        <Link
          to="/settings"
          activeOptions={{ exact: false }}
          title={compact ? "Settings" : undefined}
          className={`relative flex items-center rounded-md py-2 text-sm font-medium transition-colors ${compact ? "justify-center px-2" : "gap-2.5 px-3"}`}
          activeProps={{ className: "text-foreground" }}
          inactiveProps={{
            className: "text-sidebar-foreground hover:bg-sidebar-accent/60 hover:text-foreground",
          }}
        >
          {settingsActive && (
            <motion.div
              layoutId="nav-pill"
              className="absolute inset-0 rounded-md bg-sidebar-accent"
              transition={{ duration: 0.18, ease }}
            />
          )}
          <span className={`relative z-10 flex items-center ${compact ? "" : "gap-2.5"}`}>
            <Settings className="size-4 shrink-0" />
            {!compact && "Settings"}
          </span>
        </Link>
      </div>
    </>
  );
}

export function AppShell() {
  const { resolved, setTheme } = useTheme();
  const [shortcutsOpen, setShortcutsOpen] = useState(false);
  const [mode, setMode] = useState<SidebarMode>(readMode);
  const [sidebarWidth, setSidebarWidth] = useState(readWidth);
  const [hoverOpen, setHoverOpen] = useState(false);
  const [isResizing, setIsResizing] = useState(false);
  useCoreEvents();

  useEffect(() => {
    localStorage.setItem("sb-mode", mode);
    if (mode === "hover") setHoverOpen(false);
  }, [mode]);

  useEffect(() => {
    localStorage.setItem("sb-width", String(sidebarWidth));
  }, [sidebarWidth]);

  const toggleMode = useCallback(() => {
    setMode((m) => (m === "docked" ? "hover" : "docked"));
  }, []);

  const handleDragStart = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      setIsResizing(true);
      const startX = e.clientX;
      const startWidth = sidebarWidth;

      const onMove = (ev: MouseEvent) => {
        const next = startWidth + (ev.clientX - startX);
        if (next < MIN_WIDTH) {
          setMode("hover");
          setIsResizing(false);
          document.removeEventListener("mousemove", onMove);
          document.removeEventListener("mouseup", onUp);
          return;
        }
        setSidebarWidth(Math.min(next, MAX_WIDTH));
      };

      const onUp = () => {
        setIsResizing(false);
        document.removeEventListener("mousemove", onMove);
        document.removeEventListener("mouseup", onUp);
      };

      document.addEventListener("mousemove", onMove);
      document.addEventListener("mouseup", onUp);
    },
    [sidebarWidth],
  );

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.altKey && e.key === "z") {
        e.preventDefault();
        toggleMode();
        return;
      }
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
  }, [toggleMode]);

  const iconOnly = mode === "docked" && sidebarWidth < ICON_ONLY_THRESHOLD;

  return (
    <div
      className={`flex h-screen flex-col overflow-hidden bg-background text-foreground ${isResizing ? "select-none cursor-col-resize" : ""}`}
    >
      <TitleBar />

      <a
        href="#main-content"
        className="sr-only focus:not-sr-only focus:absolute focus:left-2 focus:top-2 focus:z-[100] focus:rounded focus:bg-primary focus:px-3 focus:py-1.5 focus:text-sm focus:font-medium focus:text-primary-foreground"
      >
        Skip to content
      </a>

      <div className="relative flex min-h-0 flex-1">
        {mode === "docked" ? (
          <aside
            style={{ width: sidebarWidth }}
            className={`relative flex shrink-0 flex-col bg-sidebar ${isResizing ? "" : "transition-[width] duration-150 ease-out"}`}
          >
            <SidebarContent
              compact={iconOnly}
              mode={mode}
              toggleMode={toggleMode}
              resolved={resolved}
            />
            {/* Drag handle */}
            <div
              onMouseDown={handleDragStart}
              className="absolute right-0 top-0 h-full w-1 cursor-col-resize transition-colors hover:bg-primary/30 active:bg-primary/50"
            />
          </aside>
        ) : (
          /* Hover mode: trigger strip + animated overlay */
          <>
            <div
              className="absolute left-0 top-0 z-40 h-full w-3"
              onMouseEnter={() => setHoverOpen(true)}
            />
            <AnimatePresence>
              {hoverOpen && (
                <motion.aside
                  key="hover-sidebar"
                  initial={{ x: "-100%", opacity: 0 }}
                  animate={{ x: 0, opacity: 1 }}
                  exit={{ x: "-100%", opacity: 0 }}
                  transition={{ duration: 0.2, ease }}
                  className="absolute left-0 top-0 z-50 flex h-full flex-col bg-sidebar shadow-xl"
                  style={{ width: Math.max(sidebarWidth, DEFAULT_WIDTH) }}
                  onMouseLeave={() => setHoverOpen(false)}
                >
                  <SidebarContent
                    compact={false}
                    mode={mode}
                    toggleMode={toggleMode}
                    resolved={resolved}
                  />
                </motion.aside>
              )}
            </AnimatePresence>
          </>
        )}

        <div className="flex min-w-0 flex-1 flex-col border-l border-border">
          <header className="flex h-14 shrink-0 items-center justify-between border-b border-border px-5">
            <div className="flex items-center gap-2 text-sm text-muted-foreground">
              {mode === "hover" && (
                <button
                  onClick={toggleMode}
                  title="Dock sidebar (Alt+Z)"
                  className="mr-1 rounded p-1 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
                >
                  <ChevronRight className="size-4" />
                </button>
              )}
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
              transition={{ duration: 0.18, ease }}
              className="h-full"
            >
              <Outlet />
            </motion.div>
          </main>
        </div>
      </div>

      <CommandPalette />
      {shortcutsOpen && <KeyboardShortcutsDialog onClose={() => setShortcutsOpen(false)} />}
    </div>
  );
}
