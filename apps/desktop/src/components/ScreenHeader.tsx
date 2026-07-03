import type { ReactNode } from "react";

interface ScreenHeaderProps {
  title: string;
  description?: ReactNode;
  actions?: ReactNode;
}

/** Consistent page header used at the top of each route. */
export function ScreenHeader({ title, description, actions }: ScreenHeaderProps) {
  return (
    <div className="flex items-start justify-between gap-4 border-b border-border px-8 py-6">
      <div className="space-y-1">
        <h1 className="text-xl font-semibold tracking-tight">{title}</h1>
        {description ? <p className="text-sm text-muted-foreground">{description}</p> : null}
      </div>
      {actions ? <div className="flex shrink-0 items-center gap-2">{actions}</div> : null}
    </div>
  );
}
