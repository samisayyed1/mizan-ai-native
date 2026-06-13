import { cn } from "@/lib/cn";

export function Container({
  children,
  className,
  as: As = "div",
}: {
  children: React.ReactNode;
  className?: string;
  as?: "div" | "section" | "header" | "footer" | "main" | "aside";
}) {
  return (
    <As className={cn("mx-auto w-full max-w-6xl px-6 md:px-8", className)}>
      {children}
    </As>
  );
}
