import { forwardRef } from "react";
import Link from "next/link";

import { cn } from "@/lib/cn";

type ButtonVariant = "primary" | "ghost";
type ButtonSize = "sm" | "md" | "lg";

const VARIANT: Record<ButtonVariant, string> = {
  primary:
    "bg-gold-primary text-depth-page hover:bg-gold-cream hover:-translate-y-0.5 hover:shadow-[0_12px_32px_-12px_rgba(212,165,116,0.6)] active:translate-y-0 active:scale-[0.99]",
  ghost:
    "border border-depth-border bg-transparent text-foreground hover:bg-depth-card hover:border-gold-primary/40 active:scale-[0.99]",
};

const SIZE: Record<ButtonSize, string> = {
  sm: "h-9 px-4 t-caption",
  md: "h-12 px-5 t-body-bold",
  lg: "h-14 px-6 t-body-bold text-base",
};

interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
  href?: string;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  { variant = "primary", size = "md", className, href, children, ...props },
  ref,
) {
  const classes = cn(
    "inline-flex items-center justify-center gap-2 rounded-xl font-semibold whitespace-nowrap transition-[background-color,border-color,transform,box-shadow] duration-150 ease-out motion-reduce:hover:translate-y-0 motion-reduce:hover:shadow-none motion-reduce:active:scale-100 focus-visible:outline-none disabled:opacity-50 disabled:pointer-events-none",
    VARIANT[variant],
    SIZE[size],
    className,
  );

  if (href) {
    return (
      <Link href={href} className={classes}>
        {children}
      </Link>
    );
  }

  return (
    <button ref={ref} className={classes} {...props}>
      {children}
    </button>
  );
});
