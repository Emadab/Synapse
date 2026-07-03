import type { ReactNode } from "react";

interface ScreenHeaderProps {
  title: string;
  description?: ReactNode;
  actions?: ReactNode;
}

/** Slim sticky page header used at the top of each route. */
export function ScreenHeader({ title, description, actions }: ScreenHeaderProps) {
  return (
    <div className="glass-panel sticky top-0 z-20 flex h-12 shrink-0 items-center justify-between gap-4 border-b px-6">
      <div className="flex min-w-0 items-baseline gap-3">
        <h1 className="shrink-0 text-[15px] font-semibold tracking-tight">{title}</h1>
        {description ? (
          <span className="hidden truncate text-[13px] text-muted-foreground md:inline">
            {description}
          </span>
        ) : null}
      </div>
      {actions ? <div className="flex shrink-0 items-center gap-2">{actions}</div> : null}
    </div>
  );
}
