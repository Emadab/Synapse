import { Download } from "lucide-react";

/** Downloads `rows` (already flattened to plain objects) as a CSV file. */
export function ExportButton({ filename, rows }: { filename: string; rows: Record<string, unknown>[] }) {
  const handleExport = () => {
    if (rows.length === 0) return;
    const headers = Object.keys(rows[0]);
    const csv = [
      headers.join(","),
      ...rows.map((row) =>
        headers.map((h) => JSON.stringify(row[h] ?? "")).join(","),
      ),
    ].join("\n");
    const blob = new Blob([csv], { type: "text/csv;charset=utf-8;" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `${filename}.csv`;
    a.click();
    URL.revokeObjectURL(url);
  };

  return (
    <button
      type="button"
      onClick={handleExport}
      disabled={rows.length === 0}
      className="inline-flex items-center gap-1 rounded-md p-1 text-muted-foreground transition-colors hover:bg-secondary hover:text-foreground disabled:pointer-events-none disabled:opacity-40"
      title="Export as CSV"
      aria-label="Export as CSV"
    >
      <Download className="size-3.5" />
    </button>
  );
}
