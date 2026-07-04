import type { ReactNode } from "react";
import { clsx } from "clsx";

export function Toolbar({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <div className={clsx("flex flex-wrap items-center gap-2", className)} role="toolbar">
      {children}
    </div>
  );
}
