/** Shimmer placeholder matching a panel's height, shown while stats are loading. */
export function PanelSkeleton({ height = 200 }: { height?: number }) {
  return (
    <div className="w-full animate-pulse rounded-lg bg-secondary" style={{ height }} aria-hidden />
  );
}
