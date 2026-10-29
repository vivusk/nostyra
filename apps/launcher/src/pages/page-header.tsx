import type { ReactNode } from "react";

interface PageHeaderProps {
  title: string;
  subtitle?: ReactNode;
}

/** Page title block shared by every page (mockup: Sora 36 px + muted subtitle). */
export function PageHeader({ title, subtitle }: PageHeaderProps) {
  return (
    <header className="flex flex-col gap-1.5">
      <h1 className="m-0 font-display text-4xl font-bold tracking-[-0.02em]">{title}</h1>
      {subtitle && <p className="m-0 text-sm text-muted">{subtitle}</p>}
    </header>
  );
}
