import { useCallback, useEffect, useState } from "react";
import { Link, Outlet, useMatchRoute } from "@tanstack/react-router";
import { AnimatePresence, motion } from "framer-motion";
import {
  BarChart3,
  BookType,
  ChevronLeft,
  ChevronRight,
  Layers,
  PlusCircle,
  Search,
  Settings,
  X,
} from "lucide-react";
import { CommandPalette } from "@/components/CommandPalette";
import { KeyboardShortcutsDialog } from "@/components/KeyboardShortcutsDialog";
import { Kbd } from "@/components/Kbd";
import { useCoreEvents } from "@/lib/useCoreEvents";
import { useUi } from "@/stores/ui";
import { TitleBar } from "@/components/TitleBar";
import { ease } from "@/lib/motion";

const CMDK_TIP_KEY = "synapse-tip-cmdk";

function CommandPaletteTip() {
  const [dismissed, setDismissed] = useState(() => localStorage.getItem(CMDK_TIP_KEY) === "1");
  const paletteOpenSignal = useUi((s) => s.paletteOpenSignal);

  useEffect(() => {
    if (paletteOpenSignal > 0) {
      localStorage.setItem(CMDK_TIP_KEY, "1");
      setDismissed(true);
    }
  }, [paletteOpenSignal]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key.toLowerCase() === "k" && (e.metaKey || e.ctrlKey)) {
        localStorage.setItem(CMDK_TIP_KEY, "1");
        setDismissed(true);
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, []);

  if (dismissed) return null;

  return (
    <motion.div
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, y: 8 }}
      transition={{ duration: 0.18, ease }}
      className="glass-panel fixed bottom-4 right-4 z-40 flex items-center gap-2 rounded-full border px-3 py-1.5 shadow-lg"
    >
      <span className="text-xs text-muted-foreground">Press</span>
      <Kbd>⌘K</Kbd>
      <span className="text-xs text-muted-foreground">to jump anywhere</span>
      <button
        type="button"
        aria-label="Dismiss tip"
        onClick={() => {
          localStorage.setItem(CMDK_TIP_KEY, "1");
          setDismissed(true);
        }}
        className="ml-1 text-muted-foreground hover:text-foreground"
      >
        <X className="size-3.5" />
      </button>
    </motion.div>
  );
}

const nav: {
  to: string;
  label: string;
  icon: React.ElementType;
  exact: boolean;
  alsoMatch?: string;
}[] = [
  { to: "/", label: "Decks", icon: Layers, exact: true, alsoMatch: "/study/$deckId" },
  { to: "/browse", label: "Browse", icon: Search, exact: false },
  { to: "/add", label: "Add", icon: PlusCircle, exact: false },
  { to: "/notetypes", label: "Note Types", icon: BookType, exact: false },
  { to: "/stats", label: "Stats", icon: BarChart3, exact: false },
];

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
  alsoMatch,
}: {
  to: string;
  label: string;
  icon: React.ElementType;
  exact: boolean;
  compact: boolean;
  alsoMatch?: string;
}) {
  const matchRoute = useMatchRoute();
  const isActive =
    !!matchRoute({ to, fuzzy: !exact }) ||
    (!!alsoMatch && !!matchRoute({ to: alsoMatch, fuzzy: true }));

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
}: {
  compact: boolean;
  mode: SidebarMode;
  toggleMode: () => void;
}) {
  const matchRoute = useMatchRoute();
  const settingsActive = !!matchRoute({ to: "/settings", fuzzy: true });

  return (
    <>
      {/* Wordmark + collapse toggle (logo itself already shown in TitleBar) */}
      <div
        className={`flex h-12 shrink-0 items-center border-b border-border/60 ${compact ? "justify-center px-2" : "justify-between px-4"}`}
      >
        {!compact && (
          <span className="text-[13px] font-semibold tracking-tight text-muted-foreground">
            Synapse
          </span>
        )}
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
        {nav.map(({ to, label, icon, exact, alsoMatch }) => (
          <NavLink
            key={to}
            to={to}
            label={label}
            icon={icon}
            exact={exact}
            compact={compact}
            alsoMatch={alsoMatch}
          />
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
  const [shortcutsOpen, setShortcutsOpen] = useState(false);
  const [mode, setMode] = useState<SidebarMode>(readMode);
  const [sidebarWidth, setSidebarWidth] = useState(readWidth);
  const [hoverOpen, setHoverOpen] = useState(false);
  const [isResizing, setIsResizing] = useState(false);
  const matchRoute = useMatchRoute();
  const isStudy = !!matchRoute({ to: "/study/$deckId", fuzzy: true });
  const focusMode = useUi((s) => s.focusMode);
  const setFocusMode = useUi((s) => s.setFocusMode);
  const openPalette = useUi((s) => s.openPalette);
  const inFocus = focusMode && isStudy;
  useCoreEvents();

  // Never leave focus mode "on" once the user navigates away from study.
  useEffect(() => {
    if (!isStudy && focusMode) setFocusMode(false);
  }, [isStudy, focusMode, setFocusMode]);

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
      <TitleBar
        onToggleSidebar={toggleMode}
        showSidebarDock={mode === "hover" && !inFocus}
        onOpenPalette={openPalette}
      />

      <a
        href="#main-content"
        className="sr-only focus:not-sr-only focus:absolute focus:left-2 focus:top-2 focus:z-[100] focus:rounded focus:bg-primary focus:px-3 focus:py-1.5 focus:text-sm focus:font-medium focus:text-primary-foreground"
      >
        Skip to content
      </a>

      <div className="relative flex min-h-0 flex-1">
        <AnimatePresence initial={false}>
          {!inFocus &&
            (mode === "docked" ? (
              <motion.aside
                key="docked-sidebar"
                style={{ width: sidebarWidth }}
                initial={{ width: 0, opacity: 0 }}
                animate={{ width: sidebarWidth, opacity: 1 }}
                exit={{ width: 0, opacity: 0 }}
                transition={{ duration: isResizing ? 0 : 0.15, ease }}
                className="glass-chrome relative flex shrink-0 flex-col border-r"
              >
                <SidebarContent compact={iconOnly} mode={mode} toggleMode={toggleMode} />
                {/* Drag handle */}
                <div
                  onMouseDown={handleDragStart}
                  className="absolute right-0 top-0 h-full w-1 cursor-col-resize transition-colors hover:bg-primary/30 active:bg-primary/50"
                />
              </motion.aside>
            ) : (
              /* Hover mode: trigger strip + animated overlay */
              <motion.div key="hover-trigger" exit={{ opacity: 0 }}>
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
                      className="glass-chrome absolute left-0 top-0 z-50 flex h-full flex-col border-r shadow-xl"
                      style={{ width: Math.max(sidebarWidth, DEFAULT_WIDTH) }}
                      onMouseLeave={() => setHoverOpen(false)}
                    >
                      <SidebarContent compact={false} mode={mode} toggleMode={toggleMode} />
                    </motion.aside>
                  )}
                </AnimatePresence>
              </motion.div>
            ))}
        </AnimatePresence>

        <div className="flex min-w-0 flex-1 flex-col">
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
      <AnimatePresence>{!inFocus && <CommandPaletteTip />}</AnimatePresence>
    </div>
  );
}
