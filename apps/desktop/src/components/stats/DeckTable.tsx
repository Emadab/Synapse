import { useMemo, useState } from "react";
import { ChevronDown, ChevronRight } from "lucide-react";
import type { DeckStat } from "@synapse/ipc-types";
import { cn } from "@/lib/utils";
import { ExportButton } from "./ExportButton";

type SortKey = "name" | "total_cards" | "due_today" | "new_count" | "retention_pct" | "reviews_7d";

interface DeckNode extends DeckStat {
  children: DeckNode[];
  rollup: DeckStat;
}

function buildTree(rows: DeckStat[]): DeckNode[] {
  const byId = new Map<number, DeckNode>();
  for (const r of rows) {
    byId.set(r.deck_id, { ...r, children: [], rollup: { ...r } });
  }
  const roots: DeckNode[] = [];
  for (const node of byId.values()) {
    if (node.parent_id !== null && byId.has(node.parent_id)) {
      byId.get(node.parent_id)!.children.push(node);
    } else {
      roots.push(node);
    }
  }

  // Roll up totals/new/due from children into every ancestor; retention stays
  // per-deck (averaging percentages across decks of different sizes is misleading).
  function rollup(node: DeckNode): DeckStat {
    let total = node.total_cards;
    let due = node.due_today;
    let newCount = node.new_count;
    let reviews7d = node.reviews_7d;
    for (const child of node.children) {
      const r = rollup(child);
      total += r.total_cards;
      due += r.due_today;
      newCount += r.new_count;
      reviews7d += r.reviews_7d;
    }
    node.rollup = {
      ...node,
      total_cards: total,
      due_today: due,
      new_count: newCount,
      reviews_7d: reviews7d,
    };
    return node.rollup;
  }
  for (const root of roots) rollup(root);

  const sortByName = (a: DeckNode, b: DeckNode) => a.name.localeCompare(b.name);
  const sortTree = (nodes: DeckNode[]) => {
    nodes.sort(sortByName);
    for (const n of nodes) sortTree(n.children);
  };
  sortTree(roots);
  return roots;
}

function leafLabel(name: string): string {
  const parts = name.split("::");
  return parts[parts.length - 1];
}

export function DeckTable({
  deckStats,
  onSelectDeck,
}: {
  deckStats: DeckStat[];
  onSelectDeck: (deckId: number) => void;
}) {
  const tree = useMemo(() => buildTree(deckStats), [deckStats]);
  const [collapsed, setCollapsed] = useState<Set<number>>(new Set());
  const [sortKey, setSortKey] = useState<SortKey>("name");
  const [sortDesc, setSortDesc] = useState(false);

  const toggle = (id: number) => {
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const cycleSort = (key: SortKey) => {
    if (sortKey === key) setSortDesc((d) => !d);
    else {
      setSortKey(key);
      setSortDesc(false);
    }
  };

  const compareByKey = (a: DeckNode, b: DeckNode) => {
    const av = sortKey === "name" ? a.name : a.rollup[sortKey];
    const bv = sortKey === "name" ? b.name : b.rollup[sortKey];
    const cmp =
      typeof av === "string" ? av.localeCompare(bv as string) : (av as number) - (bv as number);
    return sortDesc ? -cmp : cmp;
  };

  const sorted = useMemo(() => {
    function sortSiblings(nodes: DeckNode[]): DeckNode[] {
      return [...nodes]
        .sort(compareByKey)
        .map((n) => ({ ...n, children: sortSiblings(n.children) }));
    }
    return sortSiblings(tree);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tree, sortKey, sortDesc]);

  const rows: { node: DeckNode; depth: number }[] = [];
  function flatten(nodes: DeckNode[], depth: number) {
    for (const n of nodes) {
      rows.push({ node: n, depth });
      if (!collapsed.has(n.deck_id)) flatten(n.children, depth + 1);
    }
  }
  flatten(sorted, 0);

  const columns: { key: SortKey; label: string }[] = [
    { key: "name", label: "Deck" },
    { key: "total_cards", label: "Cards" },
    { key: "due_today", label: "Due" },
    { key: "new_count", label: "New" },
    { key: "retention_pct", label: "Retention" },
    { key: "reviews_7d", label: "Reviews (7d)" },
  ];

  return (
    <div className="relative">
      <div className="absolute right-0 top-0">
        <ExportButton filename="deck-stats" rows={deckStats.map((d) => ({ ...d }))} />
      </div>
      <table className="w-full text-sm">
        <thead>
          <tr className="border-b border-border text-left text-xs text-muted-foreground">
            {columns.map((col) => (
              <th
                key={col.key}
                className={cn(
                  "cursor-pointer select-none py-2 pr-3 font-medium hover:text-foreground",
                  col.key !== "name" && "text-right",
                )}
                onClick={() => cycleSort(col.key)}
              >
                {col.label}
                {sortKey === col.key ? (sortDesc ? " ↓" : " ↑") : null}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map(({ node, depth }) => (
            <tr
              key={node.deck_id}
              className="cursor-pointer border-b border-border/50 last:border-0 hover:bg-secondary/40"
              onClick={() => onSelectDeck(node.deck_id)}
            >
              <td className="py-1.5 pr-3" style={{ paddingLeft: depth * 16 }}>
                <span className="inline-flex items-center gap-1">
                  {node.children.length > 0 ? (
                    <button
                      type="button"
                      onClick={(e) => {
                        e.stopPropagation();
                        toggle(node.deck_id);
                      }}
                      className="text-muted-foreground hover:text-foreground"
                    >
                      {collapsed.has(node.deck_id) ? (
                        <ChevronRight className="size-3.5" />
                      ) : (
                        <ChevronDown className="size-3.5" />
                      )}
                    </button>
                  ) : (
                    <span className="w-3.5" />
                  )}
                  {leafLabel(node.name)}
                </span>
              </td>
              <td className="py-1.5 pr-3 text-right tabular-nums">{node.rollup.total_cards}</td>
              <td className="py-1.5 pr-3 text-right tabular-nums">{node.rollup.due_today}</td>
              <td className="py-1.5 pr-3 text-right tabular-nums">{node.rollup.new_count}</td>
              <td className="py-1.5 pr-3 text-right tabular-nums">
                {node.total_cards > 0 ? `${node.retention_pct.toFixed(0)}%` : "—"}
              </td>
              <td className="py-1.5 pr-3 text-right tabular-nums">{node.rollup.reviews_7d}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
