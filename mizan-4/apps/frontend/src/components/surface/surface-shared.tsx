/**
 * Shared "surface" primitives — the world-class look-and-feel that the
 * 12 asset-class panel pages established, generalised so every other
 * screen, settings tab, sheet, dialog, and alert can wear the same skin.
 *
 * Re-exports the panel pieces so a screen only needs one import to opt
 * in to the whole grammar:
 *
 *   import { SurfaceHero, SurfaceCard, goldLadderClass, statusToToneClass }
 *     from "@/components/surface/surface-shared";
 *
 * The panel module continues to own the deep primitives (donut, bars,
 * holdings list). This file adds the lighter shells (`SurfaceHero`,
 * `SurfaceCard`, `SheetSurfaceHeader`, `ConfirmDialog`, `StatusDot`) and
 * the colour helpers (`goldLadderClass`, `statusToToneClass`).
 */
import type { ReactNode } from "react";

import {
  goldLadder,
  PanelEmpty,
} from "@/components/panels/panel-shared";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  Button,
} from "@mizan/ui";
import { Icons } from "@mizan/ui/components/ui/icons";
import { formatAmount } from "@mizan/ui/lib/utils";

export { goldLadder, PanelEmpty };

/* ────────────────────────────────────────────────────────────────────
 * SurfaceHero
 *
 * Same visual contract as `PanelHero` — icon + uppercase eyebrow,
 * tabular value, meta line, optional rounded "+ Add" — but rendered as
 * a plain `<header>` so it composes outside `PanelShell` (settings
 * pages, modal bodies, dashboard sections).
 *
 * `value` is optional: when omitted, the hero shows just the eyebrow
 * + meta (used by status / settings heroes that have no headline
 * number).
 */
export function SurfaceHero({
  icon: Icon,
  eyebrow,
  value,
  baseCurrency,
  meta,
  empty = false,
  onAction,
  actionLabel = "Add",
  actionIcon: ActionIcon = Icons.Plus,
  className = "",
}: {
  icon: typeof Icons.TrendingUp;
  eyebrow: string;
  value?: number;
  baseCurrency?: string;
  meta: string;
  empty?: boolean;
  onAction?: () => void;
  actionLabel?: string;
  actionIcon?: typeof Icons.Plus;
  className?: string;
}) {
  const hasValue = typeof value === "number" && typeof baseCurrency === "string";
  return (
    <header
      className={`flex items-start justify-between gap-4 pb-2 ${className}`}
    >
      <div className="min-w-0 flex-1 space-y-2">
        <div className="text-muted-foreground flex items-center gap-2 text-xs font-semibold uppercase tracking-wider">
          <Icon className="h-3.5 w-3.5" />
          {eyebrow}
        </div>
        {hasValue ? (
          <div className="flex flex-col gap-1">
            <div className="text-foreground font-serif text-3xl font-semibold tabular-nums md:text-4xl">
              {empty ? "—" : formatAmount(value, baseCurrency)}
            </div>
            <p className="text-muted-foreground text-sm">{meta}</p>
          </div>
        ) : (
          <p className="text-foreground text-2xl font-semibold leading-tight md:text-3xl">
            {meta}
          </p>
        )}
      </div>
      {onAction && (
        <button
          type="button"
          onClick={onAction}
          aria-label={`${actionLabel} ${eyebrow}`}
          className="bg-foreground text-background hover:bg-foreground/90 focus-visible:ring-ring inline-flex h-9 shrink-0 items-center gap-1.5 rounded-full pl-3 pr-4 text-[13px] font-semibold transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-offset-2"
        >
          <ActionIcon className="h-4 w-4" />
          {actionLabel}
        </button>
      )}
    </header>
  );
}

/* ────────────────────────────────────────────────────────────────────
 * SurfaceCard
 *
 * The single canonical card shell — `rounded-2xl border bg-card` with
 * an optional eyebrow header and right-side action slot. Replaces the
 * mix of `<Card>` + `<CardHeader>` + ad-hoc `<section>` patterns that
 * accumulated across the app.
 */
export function SurfaceCard({
  title,
  description,
  action,
  icon: Icon,
  children,
  padded = true,
  className = "",
  ariaLabel,
}: {
  title?: string;
  description?: string;
  action?: ReactNode;
  icon?: typeof Icons.TrendingUp;
  children: ReactNode;
  /** Pad the body. Turn off when the body is a borderless list. */
  padded?: boolean;
  className?: string;
  ariaLabel?: string;
}) {
  const showHeader = !!(title || action);
  return (
    <section
      aria-label={ariaLabel ?? title}
      className={`bg-card overflow-hidden rounded-2xl border ${className}`}
    >
      {showHeader && (
        <header className="flex items-center justify-between gap-3 border-b px-5 py-4">
          <div className="min-w-0">
            {title && (
              <h2 className="text-muted-foreground flex items-center gap-2 text-xs font-semibold uppercase tracking-wider">
                {Icon && <Icon className="h-3.5 w-3.5" />}
                {title}
              </h2>
            )}
            {description && (
              <p className="text-muted-foreground mt-1 text-[12px]">
                {description}
              </p>
            )}
          </div>
          {action && <div className="shrink-0">{action}</div>}
        </header>
      )}
      <div className={padded ? "p-5" : ""}>{children}</div>
    </section>
  );
}

/* ────────────────────────────────────────────────────────────────────
 * SheetSurfaceHeader
 *
 * Shared eyebrow + icon + title + subtitle block for `<SheetHeader>` /
 * `<DialogHeader>` bodies. Designed to drop in *inside* the primitive
 * header, so callers keep full control of open/close state.
 *
 *   <SheetHeader>
 *     <SheetSurfaceHeader
 *       icon={Icons.PiggyBank}
 *       eyebrow="Account"
 *       title="Edit Singapore CPF"
 *       subtitle="Sub-account splits and contribution limits"
 *     />
 *   </SheetHeader>
 */
export function SheetSurfaceHeader({
  icon: Icon,
  eyebrow,
  title,
  subtitle,
}: {
  icon?: typeof Icons.TrendingUp;
  eyebrow?: string;
  title: string;
  subtitle?: string;
}) {
  return (
    <div className="space-y-2">
      {(eyebrow || Icon) && (
        <div className="text-muted-foreground flex items-center gap-2 text-xs font-semibold uppercase tracking-wider">
          {Icon && <Icon className="h-3.5 w-3.5" />}
          {eyebrow}
        </div>
      )}
      <h2 className="text-foreground text-lg font-semibold leading-tight md:text-xl">
        {title}
      </h2>
      {subtitle && (
        <p className="text-muted-foreground text-[13px]">{subtitle}</p>
      )}
    </div>
  );
}

/* ────────────────────────────────────────────────────────────────────
 * ConfirmDialog
 *
 * Wraps `<AlertDialog>` in the shared header pattern so destructive
 * confirms (delete activity, cancel import, refresh quotes) stop
 * looking like raw browser prompts.
 */
export function ConfirmDialog({
  open,
  onOpenChange,
  icon: Icon = Icons.AlertTriangle,
  eyebrow,
  title,
  description,
  confirmLabel,
  cancelLabel = "Cancel",
  tone = "default",
  onConfirm,
  busy = false,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  icon?: typeof Icons.AlertTriangle;
  eyebrow?: string;
  title: string;
  description?: string;
  confirmLabel: string;
  cancelLabel?: string;
  tone?: "default" | "destructive";
  onConfirm: () => void | Promise<void>;
  busy?: boolean;
}) {
  return (
    <AlertDialog open={open} onOpenChange={onOpenChange}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <div className="flex items-start gap-3">
            <span
              className={`grid h-10 w-10 shrink-0 place-items-center rounded-full border ${
                tone === "destructive"
                  ? "border-destructive/30 bg-destructive/10 text-destructive"
                  : "border-foreground/10 bg-muted/60 text-foreground/70"
              }`}
            >
              <Icon className="h-4 w-4" aria-hidden="true" />
            </span>
            <div className="min-w-0 flex-1 space-y-1.5">
              {eyebrow && (
                <span className="text-muted-foreground block text-[11px] font-semibold uppercase tracking-wider">
                  {eyebrow}
                </span>
              )}
              <AlertDialogTitle className="text-foreground text-base font-semibold leading-tight">
                {title}
              </AlertDialogTitle>
              {description && (
                <AlertDialogDescription className="text-muted-foreground text-[13px]">
                  {description}
                </AlertDialogDescription>
              )}
            </div>
          </div>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel disabled={busy}>{cancelLabel}</AlertDialogCancel>
          <AlertDialogAction asChild>
            <Button
              variant={tone === "destructive" ? "destructive" : "default"}
              onClick={() => void onConfirm()}
              disabled={busy}
            >
              {confirmLabel}
            </Button>
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}

/* ────────────────────────────────────────────────────────────────────
 * StatusDot + tone helpers
 *
 * The Mizan tone scale, used by Health, IssueDetailSheet, activity
 * warning rows, the dashboard's status tiles, etc.
 *
 *   info / ok   → muted gold-cream (low emphasis)
 *   notice      → soft mid-gold
 *   warn        → deep gold-bronze (replaces the harsh orange usage)
 *   critical    → reserved destructive red — sparingly
 */
export type StatusTone = "info" | "ok" | "notice" | "warn" | "critical";

const TONE_CLASSES: Record<
  StatusTone,
  { dot: string; bg: string; border: string; text: string }
> = {
  info: {
    dot: "bg-muted-foreground/50",
    bg: "bg-muted/40",
    border: "border-muted-foreground/20",
    text: "text-muted-foreground",
  },
  ok: {
    dot: "bg-emerald-600/60",
    bg: "bg-emerald-50/40 dark:bg-emerald-950/20",
    border: "border-emerald-600/20",
    text: "text-emerald-700 dark:text-emerald-400",
  },
  notice: {
    dot: "",
    bg: "",
    border: "",
    text: "text-foreground/80",
  },
  warn: {
    dot: "",
    bg: "",
    border: "",
    text: "text-foreground/90",
  },
  critical: {
    dot: "bg-destructive",
    bg: "bg-destructive/10",
    border: "border-destructive/30",
    text: "text-destructive",
  },
};

/**
 * Returns the canonical Tailwind classes for a status tone. Use these
 * everywhere (dots, chips, alert cards) so the palette stays consistent.
 *
 * `notice` and `warn` resolve to gold-ladder stops at runtime because
 * those colours are HSL (not Tailwind classes) — the helper returns the
 * dot/bg/border via inline style instead. See `statusToToneStyle`.
 */
export function statusToToneClass(tone: StatusTone) {
  return TONE_CLASSES[tone];
}

/**
 * For `notice` / `warn` tones whose ideal accent is mid- and deep-gold,
 * we return inline `style` props (HSL values from the gold ladder) so
 * we don't fight Tailwind on a custom palette.
 */
export function statusToToneStyle(
  tone: StatusTone,
): { dotStyle?: React.CSSProperties; chipStyle?: React.CSSProperties } {
  if (tone === "notice") {
    return {
      dotStyle: { backgroundColor: goldLadder(2) },
      chipStyle: {
        backgroundColor: `${goldLadder(2)}22`,
        borderColor: `${goldLadder(2)}55`,
        color: goldLadder(6),
      },
    };
  }
  if (tone === "warn") {
    return {
      dotStyle: { backgroundColor: goldLadder(5) },
      chipStyle: {
        backgroundColor: `${goldLadder(5)}1a`,
        borderColor: `${goldLadder(5)}55`,
        color: goldLadder(7),
      },
    };
  }
  return {};
}

export function StatusDot({
  tone,
  className = "",
}: {
  tone: StatusTone;
  className?: string;
}) {
  const classes = statusToToneClass(tone);
  const { dotStyle } = statusToToneStyle(tone);
  return (
    <span
      aria-hidden="true"
      className={`inline-block h-2 w-2 shrink-0 rounded-full ${classes.dot} ${className}`}
      style={dotStyle}
    />
  );
}

/* ────────────────────────────────────────────────────────────────────
 * StatusChip
 *
 * Compact pill (dot + label) used in lists and tables to flag a status
 * without screaming.
 */
export function StatusChip({
  tone,
  children,
  className = "",
}: {
  tone: StatusTone;
  children: ReactNode;
  className?: string;
}) {
  const classes = statusToToneClass(tone);
  const { chipStyle } = statusToToneStyle(tone);
  const hasTwClasses = classes.bg || classes.border || classes.text;
  return (
    <span
      className={`inline-flex items-center gap-1.5 rounded-full border px-2 py-0.5 text-[11px] font-medium ${hasTwClasses ? `${classes.bg} ${classes.border} ${classes.text}` : ""} ${className}`}
      style={chipStyle}
    >
      <StatusDot tone={tone} />
      {children}
    </span>
  );
}
