import { Card, CardContent, CardHeader, CardTitle } from "@mizan/ui/components/ui/card";
import { cn } from "@/lib/utils";

interface ComingSoonCardProps {
  title: string;
  message: string;
  /**
   * Optional supporting line — e.g. "Available in Chunk 3."
   * Kept short and de-emphasised; appears under the message.
   */
  detail?: string;
  className?: string;
}

/**
 * Generic placeholder for Mizan Connect features whose backend endpoints
 * are not yet implemented. Renders a quiet, on-brand card so the surface
 * stays readable instead of erroring.
 *
 * Used when the desktop's Connect-API call sites would otherwise hit
 * non-existent endpoints (broker sync, device sync). Replace the placeholder
 * with the real component once the backend lands.
 */
export function ComingSoonCard({ title, message, detail, className }: ComingSoonCardProps) {
  return (
    <Card className={cn("border-border/50 bg-muted/30 shadow-none", className)}>
      <CardHeader className="pb-2">
        <div className="flex items-center gap-2">
          <span
            aria-hidden="true"
            className={cn(
              "inline-block h-1.5 w-1.5 rounded-full",
              "bg-gradient-to-br from-[#F5E6C8] via-[#D4A574] to-[#8B6F47]",
            )}
          />
          <span className="text-muted-foreground text-xs font-medium uppercase tracking-wider">
            Coming soon
          </span>
        </div>
        <CardTitle className="text-base font-semibold">{title}</CardTitle>
      </CardHeader>
      <CardContent className="space-y-1">
        <p className="text-muted-foreground text-sm">{message}</p>
        {detail ? <p className="text-muted-foreground/70 text-xs">{detail}</p> : null}
      </CardContent>
    </Card>
  );
}
