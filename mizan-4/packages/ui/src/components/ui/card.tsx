import * as React from "react";

import { cn } from "../../lib/utils";

/**
 * Card primitive — PR-DENSITY-4 unified design language.
 *
 * One container, opt-in variants. Used to feel like every screen had a
 * slightly different card — different padding, different border, different
 * elevation. Now there's one primitive and a handful of variant +
 * padding props.
 *
 * Variants:
 *
 *   - `default`      bg-card + soft border + small shadow. The 95% case.
 *   - `elevated`     more reach on shadow + lifts subtly on hover. Use
 *                    for hero / focal cards (Net Worth strip, Today's
 *                    Signal, the headline panel on a detail view).
 *   - `bordered`     no shadow, slightly stronger border. For dense
 *                    grids of cards where shadow stacking creates noise
 *                    (e.g. the 12 asset class tiles use their own bare
 *                    container, but anything else dense uses this).
 *   - `interactive`  hover: bg-muted/30 + border highlight + 2px lift +
 *                    cursor-pointer. Use when the whole card is the
 *                    tap target (sidebar Goals/Health/Zakat cards).
 *   - `alert`        left accent border in semantic color (default
 *                    success). Pair with `accentColor` prop for
 *                    "warning" / "destructive" / "info" / "success".
 *
 * Padding:
 *
 *   - `none`  no padding (caller controls).
 *   - `sm`    p-3 (12px) for dense surfaces.
 *   - `md`    p-4 (16px) — the new default for most cards.
 *   - `lg`    p-5 (20px) for spacious / hero cards.
 *
 * All props are optional — `<Card>` without props matches the prior
 * default behavior (rounded-xl, bg-card, soft border, shadow-sm).
 *
 * Existing CardHeader / CardContent / CardTitle / CardDescription /
 * CardFooter still ship for callers using the shadcn composition
 * pattern. New surfaces should prefer passing `padding` to the Card
 * directly and rendering children flat.
 */

export type CardVariant =
  | "default"
  | "elevated"
  | "bordered"
  | "interactive"
  | "alert";
export type CardPadding = "none" | "sm" | "md" | "lg";
export type CardAccentColor = "success" | "warning" | "destructive" | "info";

const VARIANT_CLASSES: Record<CardVariant, string> = {
  default: "bg-card text-card-foreground border-border/70 shadow-sm",
  elevated:
    "bg-card text-card-foreground border-border/60 shadow-md hover:shadow-lg transition-shadow",
  bordered: "bg-card text-card-foreground border-border",
  interactive:
    "bg-card text-card-foreground border-border/70 shadow-sm cursor-pointer transition-[background-color,border-color,transform,box-shadow] duration-150 ease-out hover:bg-muted/30 hover:border-border hover:-translate-y-0.5 hover:shadow-md active:translate-y-0 active:scale-[0.99] motion-reduce:hover:translate-y-0 motion-reduce:hover:shadow-none motion-reduce:active:scale-100",
  alert: "bg-card text-card-foreground border-border/70 shadow-sm border-l-4",
};

const ACCENT_BORDER: Record<CardAccentColor, string> = {
  success: "border-l-success",
  warning: "border-l-warning",
  destructive: "border-l-destructive",
  info: "border-l-primary",
};

const PADDING_CLASSES: Record<CardPadding, string> = {
  none: "",
  sm: "p-3",
  md: "p-4",
  lg: "p-5",
};

interface CardProps extends React.HTMLAttributes<HTMLDivElement> {
  variant?: CardVariant;
  padding?: CardPadding;
  /** Only honored when `variant === "alert"`. Defaults to `success`. */
  accentColor?: CardAccentColor;
}

const Card = React.forwardRef<HTMLDivElement, CardProps>(
  (
    {
      className,
      variant = "default",
      padding,
      accentColor = "success",
      ...props
    },
    ref,
  ) => (
    <div
      ref={ref}
      className={cn(
        "rounded-xl border",
        VARIANT_CLASSES[variant],
        variant === "alert" ? ACCENT_BORDER[accentColor] : "",
        padding ? PADDING_CLASSES[padding] : "",
        className,
      )}
      data-card-variant={variant}
      {...props}
    />
  ),
);
Card.displayName = "Card";

const CardHeader = React.forwardRef<HTMLDivElement, React.HTMLAttributes<HTMLDivElement>>(
  ({ className, ...props }, ref) => (
    <div ref={ref} className={cn("flex flex-col space-y-1.5 p-6", className)} {...props} />
  ),
);
CardHeader.displayName = "CardHeader";

const CardTitle = React.forwardRef<HTMLDivElement, React.HTMLAttributes<HTMLDivElement>>(
  ({ className, ...props }, ref) => (
    <div ref={ref} className={cn("text-xl font-semibold leading-none tracking-tight", className)} {...props} />
  ),
);
CardTitle.displayName = "CardTitle";

const CardDescription = React.forwardRef<HTMLDivElement, React.HTMLAttributes<HTMLDivElement>>(
  ({ className, ...props }, ref) => (
    <div ref={ref} className={cn("text-muted-foreground text-sm", className)} {...props} />
  ),
);
CardDescription.displayName = "CardDescription";

const CardContent = React.forwardRef<HTMLDivElement, React.HTMLAttributes<HTMLDivElement>>(
  ({ className, ...props }, ref) => <div ref={ref} className={cn("p-6 pt-0", className)} {...props} />,
);
CardContent.displayName = "CardContent";

const CardFooter = React.forwardRef<HTMLDivElement, React.HTMLAttributes<HTMLDivElement>>(
  ({ className, ...props }, ref) => (
    <div ref={ref} className={cn("flex items-center p-6 pt-0", className)} {...props} />
  ),
);
CardFooter.displayName = "CardFooter";

export { Card, CardHeader, CardFooter, CardTitle, CardDescription, CardContent };
