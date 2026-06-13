/**
 * Settings row primitives — the canonical `rounded-2xl border bg-card`
 * list with divide-y rows for settings pages that previously rolled
 * their own row grammar (Market Data, AI Providers, Addons, Taxonomies).
 *
 * Two pieces:
 *
 *   <SettingsRowList>           the container — rounded-2xl + divide-y
 *     <SettingsRow ... />       a non-expandable row (left icon + text,
 *                               right control)
 *     <SettingsCollapsibleRow>  a row that opens an inset body underneath
 *       <body>
 *     </SettingsCollapsibleRow>
 *   </SettingsRowList>
 */
import { useState, type ReactNode } from "react";

import { Icons } from "@mizan/ui/components/ui/icons";

export function SettingsRowList({
  children,
  ariaLabel,
  className = "",
}: {
  children: ReactNode;
  ariaLabel?: string;
  className?: string;
}) {
  return (
    <section
      aria-label={ariaLabel}
      className={`bg-card overflow-hidden rounded-2xl border ${className}`}
    >
      <ul role="list" className="divide-border divide-y">
        {children}
      </ul>
    </section>
  );
}

export function SettingsRow({
  icon: Icon,
  iconNode,
  title,
  subtitle,
  right,
  onClick,
  ariaLabel,
}: {
  icon?: typeof Icons.TrendingUp;
  /** Custom node instead of an icon (e.g. brand logo). */
  iconNode?: ReactNode;
  title: ReactNode;
  subtitle?: ReactNode;
  right?: ReactNode;
  onClick?: () => void;
  ariaLabel?: string;
}) {
  const Tag = onClick ? "button" : "div";
  const baseClasses =
    "group flex w-full items-center gap-3 px-5 py-3.5 text-left transition-colors";
  const interactive = onClick
    ? "hover:bg-muted/40 focus-visible:bg-muted/40 focus:outline-none"
    : "";
  return (
    <li>
      <Tag
        type={onClick ? "button" : undefined}
        onClick={onClick}
        aria-label={ariaLabel}
        className={`${baseClasses} ${interactive}`}
      >
        {(Icon || iconNode) && (
          <span className="bg-muted/60 text-foreground/80 grid h-9 w-9 shrink-0 place-items-center rounded-md">
            {iconNode ?? (Icon ? <Icon className="h-4 w-4" /> : null)}
          </span>
        )}
        <span className="min-w-0 flex-1">
          <span className="text-foreground block truncate text-[13px] font-medium">
            {title}
          </span>
          {subtitle && (
            <span className="text-muted-foreground block truncate text-[11px]">
              {subtitle}
            </span>
          )}
        </span>
        {right && <span className="shrink-0">{right}</span>}
      </Tag>
    </li>
  );
}

export function SettingsCollapsibleRow({
  icon: Icon,
  iconNode,
  title,
  subtitle,
  right,
  open: controlledOpen,
  defaultOpen = false,
  onOpenChange,
  children,
}: {
  icon?: typeof Icons.TrendingUp;
  iconNode?: ReactNode;
  title: ReactNode;
  subtitle?: ReactNode;
  right?: ReactNode;
  open?: boolean;
  defaultOpen?: boolean;
  onOpenChange?: (open: boolean) => void;
  children: ReactNode;
}) {
  const [uncontrolledOpen, setUncontrolledOpen] = useState(defaultOpen);
  const isControlled = typeof controlledOpen === "boolean";
  const isOpen = isControlled ? controlledOpen : uncontrolledOpen;
  const toggle = () => {
    const next = !isOpen;
    if (!isControlled) setUncontrolledOpen(next);
    onOpenChange?.(next);
  };

  return (
    <li>
      <button
        type="button"
        onClick={toggle}
        aria-expanded={isOpen}
        className="hover:bg-muted/40 focus-visible:bg-muted/40 group flex w-full items-center gap-3 px-5 py-3.5 text-left transition-colors focus:outline-none"
      >
        {(Icon || iconNode) && (
          <span className="bg-muted/60 text-foreground/80 grid h-9 w-9 shrink-0 place-items-center rounded-md">
            {iconNode ?? (Icon ? <Icon className="h-4 w-4" /> : null)}
          </span>
        )}
        <span className="min-w-0 flex-1">
          <span className="text-foreground block truncate text-[13px] font-medium">
            {title}
          </span>
          {subtitle && (
            <span className="text-muted-foreground block truncate text-[11px]">
              {subtitle}
            </span>
          )}
        </span>
        {right && <span className="shrink-0">{right}</span>}
        <Icons.ChevronDown
          className={`text-muted-foreground/60 h-4 w-4 shrink-0 transition-transform ${
            isOpen ? "rotate-180" : ""
          }`}
          aria-hidden="true"
        />
      </button>
      {isOpen && (
        <div className="bg-muted/20 border-border/60 border-t px-5 py-4">
          {children}
        </div>
      )}
    </li>
  );
}
