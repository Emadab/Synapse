import { useEffect, useRef } from "react";
import { animate, motion, useMotionValue, useReducedMotion, useTransform } from "framer-motion";
import type { StatsDto } from "@synapse/ipc-types";

function CountUp({ value, format }: { value: number; format?: (n: number) => string }) {
  const reduced = useReducedMotion();
  const mv = useMotionValue(0);
  const text = useTransform(mv, (v) => (format ? format(v) : Math.round(v).toLocaleString()));
  const prev = useRef(0);

  useEffect(() => {
    if (reduced) {
      mv.set(value);
      prev.current = value;
      return;
    }
    const controls = animate(prev.current, value, {
      duration: 0.6,
      ease: [0.2, 0, 0, 1],
      onUpdate: (v) => mv.set(v),
    });
    prev.current = value;
    return () => controls.stop();
  }, [value, mv, reduced]);

  return <motion.span>{text}</motion.span>;
}

function StatCard({
  label,
  value,
  format,
}: {
  label: string;
  value: number;
  format?: (n: number) => string;
}) {
  return (
    <div className="rounded-xl border border-border bg-card p-4">
      <div className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
        {label}
      </div>
      <div className="mt-1 text-2xl font-semibold tabular-nums">
        <CountUp value={value} format={format} />
      </div>
    </div>
  );
}

function formatTime(n: number): string {
  const minutes = Math.round(n / 60000);
  return minutes >= 60 ? `${(minutes / 60).toFixed(1)}h` : `${minutes}m`;
}

export function StatCards({ stats, streak }: { stats: StatsDto; streak: number }) {
  const avgSec = stats.total_reviews > 0 ? stats.total_time_ms / stats.total_reviews / 1000 : 0;
  return (
    <div className="grid grid-cols-2 gap-4 md:grid-cols-3 lg:grid-cols-6">
      <StatCard label="Reviews" value={stats.total_reviews} />
      <StatCard label="Retention" value={stats.retention_pct} format={(n) => `${n.toFixed(0)}%`} />
      <StatCard label="Streak" value={streak} format={(n) => `${Math.round(n)}🔥`} />
      <StatCard label="Days studied" value={stats.studied_days} />
      <StatCard label="Time" value={stats.total_time_ms} format={formatTime} />
      <StatCard label="Avg / card" value={avgSec} format={(n) => `${n.toFixed(1)}s`} />
    </div>
  );
}
