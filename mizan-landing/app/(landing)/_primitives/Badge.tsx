import { cn } from "@/lib/cn";

/**
 * Badge — soft pill on a backdrop-blurred dark-card surface, used
 * for: trust signals after the hero, feature chips on the Product
 * section, the "Coming August 2026" pill in the header.
 */
export function Badge({
  children,
  className,
  icon,
}: {
  children: React.ReactNode;
  className?: string;
  icon?: React.ReactNode;
}) {
  return (
    <span
      className={cn(
        "inline-flex items-center gap-2 rounded-full border border-depth-border bg-depth-card/60 px-3 py-1 backdrop-blur",
        "t-caption text-foreground/85",
        className,
      )}
    >
      {icon ? <span aria-hidden="true">{icon}</span> : null}
      <span>{children}</span>
    </span>
  );
}
