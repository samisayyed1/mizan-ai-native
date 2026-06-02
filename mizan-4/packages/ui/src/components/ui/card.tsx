import * as React from "react";

import { cn } from "../../lib/utils";

// Enterprise UX-7 base look. The previous default was
// `shadow-xs rounded-lg border` — utility-grade, the typical "out of the
// box shadcn card" look that signals MVP rather than premium.
//
// Bumped to:
//   * rounded-xl  (12px corners; Apple/Notion/Linear all use 10-14px on
//                  major content surfaces; lg=8px reads cheap on hover)
//   * shadow-sm   (slightly more reach so cards have actual elevation
//                  against the page background — was virtually invisible)
//   * border-border/70 (softer single-pixel border instead of the hard
//                       full-opacity --border colour; the hard border was
//                       fighting the shadow and making cards look "stuck
//                       on" rather than floating)
//
// Caller can still override via className, so the dashboard's special
// cards (e.g. PortfolioHealthCard with `border-primary/20`) keep their
// brand emphasis.
const Card = React.forwardRef<HTMLDivElement, React.HTMLAttributes<HTMLDivElement>>(({ className, ...props }, ref) => (
  <div
    ref={ref}
    className={cn(
      "bg-card text-card-foreground border-border/70 shadow-sm rounded-xl border",
      className,
    )}
    {...props}
  />
));
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
