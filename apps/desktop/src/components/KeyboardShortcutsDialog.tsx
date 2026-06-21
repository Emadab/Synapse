import { useEffect, useRef } from "react";
import { Kbd } from "@/components/Kbd";
import { useFocusTrap } from "@/hooks/useFocusTrap";

const SECTIONS: { heading: string; rows: [string, string][] }[] = [
  {
    heading: "Global",
    rows: [
      ["Open command palette", "⌘K / Ctrl+K"],
      ["Keyboard shortcuts", "?"],
    ],
  },
  {
    heading: "Study",
    rows: [
      ["Show answer", "Space / Enter"],
      ["Again (rating 1)", "1"],
      ["Hard (rating 2)", "2"],
      ["Good (rating 3)", "3"],
      ["Easy (rating 4)", "4"],
      ["Replay audio", "R"],
      ["Suspend card", "S"],
      ["Bury card", "B"],
    ],
  },
  {
    heading: "Browse",
    rows: [
      ["Search / filter", "Type in search box"],
      ["Select all", "⌘A / Ctrl+A"],
      ["Delete selected notes", "⌫"],
    ],
  },
  {
    heading: "Navigation",
    rows: [
      ["Decks", "G then D"],
      ["Study", "G then S"],
      ["Browse", "G then B"],
      ["Settings", "G then ,"],
    ],
  },
];

interface Props {
  onClose: () => void;
}

export function KeyboardShortcutsDialog({ onClose }: Props) {
  const dialogRef = useRef<HTMLDivElement>(null);
  useFocusTrap(dialogRef, true);

  // Close on Escape.
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [onClose]);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 backdrop-blur-sm"
      onClick={(e) => e.target === e.currentTarget && onClose()}
      aria-hidden="true"
    >
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby="shortcuts-dialog-title"
        className="flex max-h-[80vh] w-full max-w-md flex-col overflow-hidden rounded-xl border border-border bg-background shadow-xl"
      >
        <div className="flex items-center justify-between border-b border-border px-5 py-3">
          <h2 id="shortcuts-dialog-title" className="text-sm font-semibold">
            Keyboard shortcuts
          </h2>
          <button
            onClick={onClose}
            aria-label="Close dialog"
            className="text-muted-foreground hover:text-foreground"
          >
            ✕
          </button>
        </div>

        <div className="flex-1 overflow-auto px-5 py-4 space-y-5">
          {SECTIONS.map((section) => (
            <section key={section.heading}>
              <h3 className="mb-2 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                {section.heading}
              </h3>
              <table className="w-full text-sm" role="presentation">
                <tbody>
                  {section.rows.map(([action, key]) => (
                    <tr key={action} className="border-b border-border last:border-0">
                      <td className="py-1.5 text-foreground">{action}</td>
                      <td className="py-1.5 text-right">
                        <Kbd>{key}</Kbd>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </section>
          ))}
        </div>
      </div>
    </div>
  );
}
