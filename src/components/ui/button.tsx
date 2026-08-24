import { type ButtonHTMLAttributes, forwardRef } from "react";
import { cn } from "@/lib/utils";

type Variant = "primary" | "secondary" | "ghost" | "danger" | "quiet";
type Size = "sm" | "md" | "lg" | "icon";

const variants: Record<Variant, string> = {
  primary: "bg-accent text-accent-fg hover:opacity-90 disabled:opacity-40",
  secondary: "bg-bg-subtle text-fg border border-border hover:bg-bg-hover",
  ghost: "text-fg-muted hover:bg-bg-hover hover:text-fg",
  danger: "bg-danger text-fg hover:opacity-90",
  quiet: "bg-transparent text-fg-muted hover:text-fg hover:bg-bg-hover",
};

const sizes: Record<Size, string> = {
  sm: "h-8 px-3 text-xs rounded-sm",
  md: "h-10 px-4 text-sm rounded-md",
  lg: "h-11 px-5 text-sm rounded-md",
  icon: "size-10 rounded-md",
};

export const Button = forwardRef<
  HTMLButtonElement,
  ButtonHTMLAttributes<HTMLButtonElement> & { variant?: Variant; size?: Size }
>(function Button(
  { className, variant = "secondary", size = "md", type = "button", ...props },
  ref,
) {
  return (
    <button
      ref={ref}
      type={type}
      className={cn(
        "inline-flex items-center justify-center gap-2 font-medium transition-opacity duration-150 active:scale-[0.98] disabled:pointer-events-none",
        variants[variant],
        sizes[size],
        className,
      )}
      {...props}
    />
  );
});
