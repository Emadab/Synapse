import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Copy, Minus, Square, X } from "lucide-react";
import { Logo } from "./Logo";

const isMac =
  typeof navigator !== "undefined" && /Mac/i.test(navigator.platform || navigator.userAgent);

// Stable reference — never changes for the lifetime of the app.
const appWindow = getCurrentWindow();

export function TitleBar() {
  const [isMaximized, setIsMaximized] = useState(false);

  useEffect(() => {
    if (isMac) return;
    let unlisten: (() => void) | undefined;
    appWindow.isMaximized().then(setIsMaximized);
    appWindow
      .onResized(() => appWindow.isMaximized().then(setIsMaximized))
      .then((fn) => {
        unlisten = fn;
      });
    return () => unlisten?.();
  }, []);

  return (
    <div
      data-tauri-drag-region
      className={`flex h-9 shrink-0 items-center justify-between border-b border-border bg-sidebar select-none pr-1 ${isMac ? "pl-[78px]" : "pl-3"}`}
    >
      <div className="pointer-events-none">
        <Logo size={16} />
      </div>

      {!isMac && (
        <div className="flex items-center gap-0.5">
          <button
            type="button"
            onClick={() => appWindow.minimize()}
            aria-label="Minimize"
            className="flex h-7 w-9 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
          >
            <Minus className="h-3.5 w-3.5" />
          </button>
          <button
            type="button"
            onClick={() => appWindow.toggleMaximize()}
            aria-label={isMaximized ? "Restore" : "Maximize"}
            className="flex h-7 w-9 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
          >
            {isMaximized ? (
              <Copy className="h-3.5 w-3.5 -scale-x-100" />
            ) : (
              <Square className="h-3.5 w-3.5" />
            )}
          </button>
          <button
            type="button"
            onClick={() => appWindow.close()}
            aria-label="Close"
            className="flex h-7 w-9 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-destructive hover:text-destructive-foreground"
          >
            <X className="h-5 w-5" strokeWidth={1.75} />
          </button>
        </div>
      )}
    </div>
  );
}
