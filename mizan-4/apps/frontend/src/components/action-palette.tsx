import { useHapticFeedback } from "@/hooks";
import { Button } from "@mizan/ui/components/ui/button";
import { Icons } from "@mizan/ui/components/ui/icons";
import { Popover, PopoverContent, PopoverTrigger } from "@mizan/ui/components/ui/popover";
import { cn } from "@/lib/utils";
import * as React from "react";

export interface ActionPaletteItem {
  icon: React.ComponentType<{ className?: string }>;
  label: string;
  /**
   * Click handler. Optional only when `disabled` is true (a disabled
   * item never fires its handler — the click is suppressed in
   * `handleItemClick` — so passing one would be misleading).
   */
  onClick?: () => void;
  variant?: "default" | "destructive";
  /**
   * When set, the item is rendered greyed-out and non-clickable. Use
   * this for features that aren't applicable to the current account
   * tracking mode (e.g. RSP/FD on HOLDINGS-mode broker-synced accounts)
   * so users can SEE the feature exists and understand why it's off,
   * rather than the menu item disappearing without explanation.
   */
  disabled?: boolean;
  /**
   * Short hint shown below the label when `disabled` is true.
   */
  disabledReason?: string;
}

export interface ActionPaletteGroup {
  title?: string;
  items: ActionPaletteItem[];
}

interface ActionPaletteProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title?: string;
  groups: ActionPaletteGroup[];
  trigger?: React.ReactNode;
  align?: "start" | "center" | "end";
  side?: "top" | "bottom" | "left" | "right";
}

export function ActionPalette({
  open,
  onOpenChange,
  title,
  groups,
  trigger,
  align = "end",
  side = "bottom",
}: ActionPaletteProps) {
  const { triggerHaptic } = useHapticFeedback();

  const handleItemClick = React.useCallback(
    (item: ActionPaletteItem) => {
      if (item.disabled || !item.onClick) return;
      triggerHaptic();
      item.onClick();
      onOpenChange(false);
    },
    [triggerHaptic, onOpenChange],
  );

  const handleClose = React.useCallback(() => {
    onOpenChange(false);
  }, [onOpenChange]);

  return (
    <Popover open={open} onOpenChange={onOpenChange}>
      <PopoverTrigger asChild>
        {trigger ?? (
          <Button variant="outline" size="icon" className="h-9 w-9" aria-label="More actions">
            <Icons.DotsThreeVertical className="h-5 w-5" weight="fill" />
          </Button>
        )}
      </PopoverTrigger>
      <PopoverContent
        align={align}
        side={side}
        sideOffset={8}
        className={cn(
          "w-auto min-w-[260px] max-w-[320px] p-0",
          "rounded-2xl",
          "border-border/50 border dark:border-white/10",
          "bg-card backdrop-blur-xl",
          "shadow-lg",
        )}
      >
        {/* Header - only show if title provided */}
        {title && (
          <div className="flex items-center justify-between px-5 pb-3 pt-5">
            <h3 className="text-foreground text-lg font-bold">{title}</h3>
            <button
              type="button"
              onClick={handleClose}
              className={cn(
                "flex h-8 w-8 items-center justify-center rounded-full",
                "bg-muted/80 hover:bg-muted",
                "text-muted-foreground hover:text-foreground",
                "transition-colors duration-150",
                "focus-visible:ring-ring focus:outline-none focus-visible:ring-2",
              )}
              aria-label="Close"
            >
              <Icons.X className="h-4 w-4" />
            </button>
          </div>
        )}

        {/* Action Groups */}
        <div className={cn("px-3 pb-4", !title && "pt-3")}>
          {groups.map((group, groupIndex) => (
            <div key={groupIndex}>
              {group.title && (
                <div className="text-muted-foreground px-2 py-1.5 text-xs font-medium uppercase tracking-wider">
                  {group.title}
                </div>
              )}
              <div>
                {group.items.map((item, itemIndex) => {
                  const IconComponent = item.icon;
                  const isDestructive = item.variant === "destructive";
                  const isDisabled = item.disabled === true;
                  return (
                    <React.Fragment key={itemIndex}>
                      {itemIndex > 0 && <div className="bg-border/70 mx-3 h-px" />}
                      <button
                        type="button"
                        onClick={() => handleItemClick(item)}
                        disabled={isDisabled}
                        aria-disabled={isDisabled}
                        title={isDisabled ? item.disabledReason : undefined}
                        className={cn(
                          "flex w-full items-center gap-4 rounded-xl px-3 py-3 text-left",
                          "transition-colors duration-150",
                          isDisabled
                            ? "text-muted-foreground/60 cursor-not-allowed"
                            : isDestructive
                              ? "text-destructive hover:bg-destructive/10 active:bg-destructive/15"
                              : "text-foreground hover:bg-accent active:bg-accent/80",
                          "focus-visible:ring-ring focus:outline-none focus-visible:ring-2 focus-visible:ring-inset",
                        )}
                      >
                        <IconComponent
                          className={cn(
                            "h-5 w-5 shrink-0",
                            isDisabled
                              ? "text-muted-foreground/50"
                              : isDestructive
                                ? "text-destructive"
                                : "text-muted-foreground",
                          )}
                        />
                        <div className="flex min-w-0 flex-col">
                          <span className="text-[15px] font-medium">{item.label}</span>
                          {isDisabled && item.disabledReason && (
                            <span className="text-muted-foreground/70 mt-0.5 text-xs leading-snug">
                              {item.disabledReason}
                            </span>
                          )}
                        </div>
                      </button>
                    </React.Fragment>
                  );
                })}
              </div>
              {groupIndex < groups.length - 1 && <div className="bg-border/70 mx-3 my-1.5 h-px" />}
            </div>
          ))}
        </div>
      </PopoverContent>
    </Popover>
  );
}
