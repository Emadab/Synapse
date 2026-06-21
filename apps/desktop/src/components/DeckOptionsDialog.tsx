import { useEffect, useRef, useState } from "react";
import { useMutation } from "@tanstack/react-query";
import { motion } from "framer-motion";
import { Button } from "@/components/ui/button";
import { ipc, errorMessage } from "@/lib/ipc";
import type { DeckConfig, FsrsOptimizeResult } from "@synapse/ipc-types";
import { useFocusTrap } from "@/hooks/useFocusTrap";
import { scaleIn } from "@/lib/motion";

type Tab = "general" | "new" | "reviews" | "lapses" | "fsrs";

function NumField({
  label,
  value,
  onChange,
  min,
  max,
  step,
}: {
  label: string;
  value: number;
  onChange: (v: number) => void;
  min?: number;
  max?: number;
  step?: number;
}) {
  return (
    <label className="flex items-center justify-between gap-4 py-1">
      <span className="text-sm text-muted-foreground">{label}</span>
      <input
        type="number"
        min={min}
        max={max}
        step={step ?? 1}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        className="h-7 w-28 rounded border border-input bg-background px-2 text-sm outline-none focus:ring-1 focus:ring-ring"
      />
    </label>
  );
}

function StepsField({
  label,
  value,
  onChange,
}: {
  label: string;
  value: number[];
  onChange: (v: number[]) => void;
}) {
  const text = value.join(" ");
  return (
    <label className="flex flex-col gap-1 py-1">
      <span className="text-sm text-muted-foreground">{label} (minutes, space-separated)</span>
      <input
        type="text"
        defaultValue={text}
        key={text}
        onBlur={(e) => {
          const parsed = e.target.value
            .trim()
            .split(/\s+/)
            .map(Number)
            .filter((n) => Number.isFinite(n) && n > 0);
          if (parsed.length > 0) onChange(parsed);
        }}
        className="h-7 rounded border border-input bg-background px-2 text-sm outline-none focus:ring-1 focus:ring-ring"
      />
    </label>
  );
}

interface Props {
  deckId: number;
  deckName: string;
  onClose: () => void;
  onSaved: () => void;
}

export function DeckOptionsDialog({ deckId, deckName, onClose, onSaved }: Props) {
  const [tab, setTab] = useState<Tab>("general");
  const [cfg, setCfg] = useState<DeckConfig | null>(null);
  const [loadErr, setLoadErr] = useState<string | null>(null);
  const [optimizeResult, setOptimizeResult] = useState<FsrsOptimizeResult | null>(null);
  const [optimizeError, setOptimizeError] = useState<string | null>(null);
  const dialogRef = useRef<HTMLDivElement>(null);
  const titleId = `deck-options-title-${deckId}`;
  useFocusTrap(dialogRef, true);

  const optimizeMut = useMutation({
    mutationFn: () => ipc.optimizeFsrs(deckId),
    onSuccess: (r) => {
      setOptimizeResult(r);
      setOptimizeError(null);
    },
    onError: (e) => {
      setOptimizeError(errorMessage(e));
      setOptimizeResult(null);
    },
  });

  useEffect(() => {
    ipc
      .getDeckConfig(deckId)
      .then((c) => setCfg(c))
      .catch((e) => setLoadErr(String(e)));
  }, [deckId]);

  const saveMut = useMutation({
    mutationFn: (c: DeckConfig) => ipc.setDeckConfig(c),
    onSuccess: () => {
      onSaved();
      onClose();
    },
  });

  function set<K extends keyof DeckConfig>(key: K, val: DeckConfig[K]) {
    setCfg((prev) => (prev ? { ...prev, [key]: val } : prev));
  }

  const tabs: { key: Tab; label: string }[] = [
    { key: "general", label: "General" },
    { key: "new", label: "New cards" },
    { key: "reviews", label: "Reviews" },
    { key: "lapses", label: "Lapses" },
    { key: "fsrs", label: "FSRS" },
  ];

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 backdrop-blur-sm"
      onClick={(e) => e.target === e.currentTarget && onClose()}
      aria-hidden="true"
    >
      <motion.div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        variants={scaleIn}
        initial="hidden"
        animate="show"
        exit="exit"
        className="flex w-[560px] flex-col overflow-hidden rounded-xl border border-border bg-background shadow-xl"
      >
        {/* Header */}
        <div className="flex items-center justify-between border-b border-border px-5 py-3">
          <h2 id={titleId} className="text-sm font-semibold">Options — {deckName}</h2>
          <button
            onClick={onClose}
            aria-label="Close dialog"
            className="text-muted-foreground hover:text-foreground"
          >
            ✕
          </button>
        </div>

        {loadErr ? (
          <p className="px-5 py-4 text-sm text-destructive">{loadErr}</p>
        ) : !cfg ? (
          <p className="px-5 py-4 text-sm text-muted-foreground">Loading…</p>
        ) : (
          <>
            {/* Tab bar */}
            <div className="flex border-b border-border">
              {tabs.map((t) => (
                <button
                  key={t.key}
                  onClick={() => setTab(t.key)}
                  className={[
                    "px-4 py-2 text-xs font-medium transition-colors",
                    tab === t.key
                      ? "border-b-2 border-primary text-foreground"
                      : "text-muted-foreground hover:text-foreground",
                  ].join(" ")}
                >
                  {t.label}
                </button>
              ))}
            </div>

            {/* Tab content */}
            <div className="flex-1 overflow-auto px-5 py-4">
              {tab === "general" && (
                <div className="space-y-1">
                  <p className="mb-3 text-xs text-muted-foreground">
                    Algorithm:{" "}
                    <span className="font-medium text-foreground">
                      {cfg.algorithm === "fsrs" ? "FSRS-5" : "SM-2"}
                    </span>
                  </p>
                  <div className="mb-4 flex gap-2">
                    <Button
                      size="sm"
                      variant={cfg.algorithm === "sm2" ? "default" : "outline"}
                      onClick={() => set("algorithm", "sm2")}
                    >
                      SM-2
                    </Button>
                    <Button
                      size="sm"
                      variant={cfg.algorithm === "fsrs" ? "default" : "outline"}
                      onClick={() => set("algorithm", "fsrs")}
                    >
                      FSRS-5
                    </Button>
                  </div>
                  <NumField
                    label="New cards / day"
                    value={cfg.new_per_day}
                    onChange={(v) => set("new_per_day", v)}
                    min={0}
                    max={9999}
                  />
                  <NumField
                    label="Maximum reviews / day"
                    value={cfg.review_per_day}
                    onChange={(v) => set("review_per_day", v)}
                    min={0}
                    max={9999}
                  />
                </div>
              )}

              {tab === "new" && (
                <div className="space-y-1">
                  <StepsField
                    label="Learning steps"
                    value={cfg.learning_steps_min}
                    onChange={(v) => set("learning_steps_min", v)}
                  />
                  <NumField
                    label="Graduating interval (days)"
                    value={cfg.graduating_interval_days}
                    onChange={(v) => set("graduating_interval_days", v)}
                    min={1}
                  />
                  <NumField
                    label="Easy interval (days)"
                    value={cfg.easy_interval_days}
                    onChange={(v) => set("easy_interval_days", v)}
                    min={1}
                  />
                  <NumField
                    label="Starting ease (‰, e.g. 2500 = 250%)"
                    value={cfg.starting_ease_milli}
                    onChange={(v) => set("starting_ease_milli", v)}
                    min={1300}
                    max={9999}
                  />
                </div>
              )}

              {tab === "reviews" && (
                <div className="space-y-1">
                  <NumField
                    label="Easy bonus"
                    value={cfg.easy_bonus}
                    onChange={(v) => set("easy_bonus", v)}
                    min={1.0}
                    max={5.0}
                    step={0.1}
                  />
                  <NumField
                    label="Hard interval factor"
                    value={cfg.hard_interval_factor}
                    onChange={(v) => set("hard_interval_factor", v)}
                    min={0.5}
                    max={3.0}
                    step={0.1}
                  />
                  <NumField
                    label="Interval modifier"
                    value={cfg.interval_modifier}
                    onChange={(v) => set("interval_modifier", v)}
                    min={0.01}
                    max={9.99}
                    step={0.01}
                  />
                  <NumField
                    label="Maximum interval (days)"
                    value={cfg.maximum_interval_days}
                    onChange={(v) => set("maximum_interval_days", v)}
                    min={1}
                  />
                </div>
              )}

              {tab === "lapses" && (
                <div className="space-y-1">
                  <StepsField
                    label="Relearning steps"
                    value={cfg.relearning_steps_min}
                    onChange={(v) => set("relearning_steps_min", v)}
                  />
                  <NumField
                    label="New interval (fraction of old, e.g. 0.0)"
                    value={cfg.lapse_interval_factor}
                    onChange={(v) => set("lapse_interval_factor", v)}
                    min={0.0}
                    max={1.0}
                    step={0.01}
                  />
                  <NumField
                    label="Minimum interval (days)"
                    value={cfg.minimum_interval_days}
                    onChange={(v) => set("minimum_interval_days", v)}
                    min={1}
                  />
                  <NumField
                    label="Leech threshold (lapses)"
                    value={cfg.leech_threshold}
                    onChange={(v) => set("leech_threshold", v)}
                    min={1}
                    max={99}
                  />
                </div>
              )}

              {tab === "fsrs" && (
                <div className="space-y-3">
                  <NumField
                    label="Desired retention (0.50–0.99)"
                    value={cfg.desired_retention}
                    onChange={(v) => set("desired_retention", v)}
                    min={0.5}
                    max={0.99}
                    step={0.01}
                  />
                  <div className="space-y-1">
                    <span className="text-xs text-muted-foreground">
                      FSRS-6 weights (21 values, comma or space separated)
                    </span>
                    <textarea
                      className="h-24 w-full resize-none rounded border border-input bg-background px-2 py-1 font-mono text-xs outline-none focus:ring-1 focus:ring-ring"
                      defaultValue={cfg.fsrs_weights.join(", ")}
                      key={cfg.fsrs_weights.join(",")}
                      onBlur={(e) => {
                        const parsed = e.target.value
                          .trim()
                          .split(/[\s,]+/)
                          .map(Number)
                          .filter((n) => Number.isFinite(n));
                        if (parsed.length === 21) {
                          set("fsrs_weights", parsed);
                        }
                      }}
                    />
                    <p className="text-xs text-muted-foreground">
                      Must be exactly 21 values. Invalid input is ignored.
                    </p>
                  </div>

                  {/* Optimize */}
                  <div className="space-y-2 rounded-lg border border-border p-3">
                    <p className="text-xs font-medium">Optimizer</p>
                    <p className="text-xs text-muted-foreground">
                      Fit weights to this deck's review history (minimum 400 reviews).
                    </p>
                    <Button
                      size="sm"
                      variant="outline"
                      disabled={optimizeMut.isPending}
                      onClick={() => optimizeMut.mutate()}
                    >
                      {optimizeMut.isPending ? "Optimizing…" : "Optimize FSRS weights"}
                    </Button>
                    {optimizeError && (
                      <p className="text-xs text-destructive">{optimizeError}</p>
                    )}
                    {optimizeResult && (
                      <div className="space-y-1.5 text-xs">
                        <p className="text-muted-foreground">
                          Trained on{" "}
                          <strong>{optimizeResult.review_count}</strong> reviews across{" "}
                          <strong>{optimizeResult.card_count}</strong> cards.
                        </p>
                        <p>
                          Log-loss:{" "}
                          <span className="text-muted-foreground">
                            {optimizeResult.log_loss_before.toFixed(4)}
                          </span>{" "}
                          →{" "}
                          <span
                            className={
                              optimizeResult.log_loss_after < optimizeResult.log_loss_before
                                ? "text-green-600 dark:text-green-400 font-semibold"
                                : "text-destructive"
                            }
                          >
                            {optimizeResult.log_loss_after.toFixed(4)}
                          </span>
                        </p>
                        <div className="flex gap-2">
                          <Button
                            size="sm"
                            variant="default"
                            onClick={() => {
                              if (cfg) {
                                set("fsrs_weights", optimizeResult.weights);
                                setOptimizeResult(null);
                              }
                            }}
                          >
                            Apply weights
                          </Button>
                          <Button
                            size="sm"
                            variant="ghost"
                            onClick={() => setOptimizeResult(null)}
                          >
                            Discard
                          </Button>
                        </div>
                      </div>
                    )}
                  </div>
                </div>
              )}
            </div>

            {/* Footer */}
            {saveMut.isError && (
              <p className="px-5 text-xs text-destructive">
                {String((saveMut.error as { message?: string })?.message ?? saveMut.error)}
              </p>
            )}
            <div className="flex justify-end gap-2 border-t border-border px-5 py-3">
              <Button variant="ghost" size="sm" onClick={onClose}>
                Cancel
              </Button>
              <Button
                size="sm"
                onClick={() => saveMut.mutate(cfg)}
                disabled={saveMut.isPending}
              >
                {saveMut.isPending ? "Saving…" : "Save"}
              </Button>
            </div>
          </>
        )}
      </motion.div>
    </div>
  );
}
