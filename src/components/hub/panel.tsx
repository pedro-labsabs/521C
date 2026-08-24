import type { ReactNode } from "react";
import { cn } from "@/lib/utils";

export function Panel({
  title,
  description,
  children,
  className,
  action,
}: {
  title?: string;
  description?: string;
  children: ReactNode;
  className?: string;
  action?: ReactNode;
}) {
  return (
    <section
      className={cn(
        "rounded-xl border border-border bg-bg-elevated p-4 shadow-[var(--shadow-panel)] sm:p-5",
        className,
      )}
    >
      {(title || action) && (
        <header className="mb-4 flex items-start justify-between gap-3">
          <div>
            {title && <h2 className="text-sm font-semibold tracking-tight">{title}</h2>}
            {description && <p className="mt-1 text-xs text-fg-muted">{description}</p>}
          </div>
          {action}
        </header>
      )}
      {children}
    </section>
  );
}

export function Toggle({
  checked,
  onChange,
  label,
  disabled,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
  label?: string;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={cn(
        "relative h-6 w-11 rounded-full transition-colors duration-150",
        checked ? "bg-accent" : "bg-bg-hover",
        disabled && "opacity-40",
      )}
    >
      <span
        className={cn(
          "absolute top-0.5 size-5 rounded-full bg-bg-subtle transition-transform duration-150",
          checked ? "translate-x-5" : "translate-x-0.5",
        )}
      />
      {label && <span className="sr-only">{label}</span>}
    </button>
  );
}

export function Segmented<T extends string>({
  value,
  onChange,
  options,
}: {
  value: T;
  onChange: (v: T) => void;
  options: { id: T; label: string; disabled?: boolean }[];
}) {
  return (
    <div className="flex flex-wrap gap-1 rounded-lg bg-bg p-1">
      {options.map((opt) => (
        <button
          key={opt.id}
          type="button"
          disabled={opt.disabled}
          onClick={() => onChange(opt.id)}
          className={cn(
            "rounded-md px-3 py-1.5 text-xs font-medium transition-colors duration-150",
            value === opt.id ? "bg-bg-subtle text-fg shadow-sm" : "text-fg-muted hover:text-fg",
            opt.disabled && "opacity-40",
          )}
        >
          {opt.label}
        </button>
      ))}
    </div>
  );
}
