/**
 * Context-aware Add menu. The primary path is now Assistant-first: describe
 * the asset, drop a statement, or connect Plaid for Gold live sync.
 */

import { AssetClass, ASSET_CLASS_LABELS } from "@/lib/asset-classes";
import { Button } from "@mizan/ui/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@mizan/ui/components/ui/dropdown-menu";
import { Icons } from "@mizan/ui/components/ui/icons";
import { Link } from "react-router-dom";

/**
 * Asset classes that have a meaningful broker-API path. Property,
 * Collectibles, Precious Metals, Other → not supported by live account
 * sync, so the connect option is suppressed for those. Adding new
 * tradeable classes? Add them here.
 */
const BROKER_SUPPORTED_CLASSES = new Set<AssetClass>([
  AssetClass.STOCKS,
  AssetClass.SUKUKS,
  AssetClass.ETFS,
  AssetClass.BONDS,
  AssetClass.BANK_ACCOUNTS,
]);

export interface AddHoldingMenuProps {
  /** The asset class the menu is being opened for. Drives copy + which options appear. */
  cls: AssetClass;
  /** Portfolio (account) id — used to prefill the CSV import destination. */
  accountId: string;
  /** Fires the Assistant add flow for this asset class. */
  onManualAdd: () => void;
  /**
   * `inline` — small button suitable for the drill-down header
   *            (next to the Back button).
   * `cta`    — large button suitable for the empty-state full card.
   */
  size?: "inline" | "cta";
}

export function AddHoldingMenu({
  cls,
  accountId,
  onManualAdd,
  size = "inline",
}: AddHoldingMenuProps) {
  const labels = ASSET_CLASS_LABELS[cls];
  const showBroker = BROKER_SUPPORTED_CLASSES.has(cls);

  const trigger =
    size === "cta" ? (
      <Button size="default">
        <Icons.Plus className="mr-1.5 h-4 w-4" />
        Add {labels.singular}
        <Icons.ChevronDown className="ml-1 h-4 w-4 opacity-70" />
      </Button>
    ) : (
      <Button size="sm" variant="outline">
        <Icons.Plus className="mr-1 h-4 w-4" />
        Add {labels.singular}
        <Icons.ChevronDown className="ml-1 h-3.5 w-3.5 opacity-70" />
      </Button>
    );

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>{trigger}</DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-72">
        <DropdownMenuLabel className="text-muted-foreground text-xs font-medium">
          Add a {labels.singular.toLowerCase()}
        </DropdownMenuLabel>
        <DropdownMenuSeparator />

        <DropdownMenuItem
          onSelect={onManualAdd}
          className="cursor-pointer items-start gap-3 py-2.5"
        >
          <Icons.Pencil className="text-muted-foreground mt-0.5 h-4 w-4 shrink-0" />
          <div className="min-w-0 flex-1">
            <p className="text-sm font-medium">Tell Mizan what you own</p>
            <p className="text-muted-foreground text-xs leading-snug">
              Create a reviewed AI draft from a sentence like 15 oz gold or a rental property.
            </p>
          </div>
        </DropdownMenuItem>

        {showBroker && (
          <DropdownMenuItem asChild className="cursor-pointer items-start gap-3 py-2.5">
            <Link to="/connect">
              <Icons.CloudSync className="text-muted-foreground mt-0.5 h-4 w-4 shrink-0" />
              <div className="min-w-0 flex-1">
                <p className="text-sm font-medium">Connect with Plaid</p>
                <p className="text-muted-foreground text-xs leading-snug">
                  Gold keeps banks, cards, liabilities, and supported investments synced.
                </p>
              </div>
            </Link>
          </DropdownMenuItem>
        )}

        <DropdownMenuItem asChild className="cursor-pointer items-start gap-3 py-2.5">
          <Link
            to={`/assistant?intent=import-csv&prompt=${encodeURIComponent(
              `I want to import a CSV or statement into portfolio ${accountId}. Help me map it, validate it, and review before saving.`,
            )}`}
          >
            <Icons.Import className="text-muted-foreground mt-0.5 h-4 w-4 shrink-0" />
            <div className="min-w-0 flex-1">
              <p className="text-sm font-medium">Drop a file</p>
              <p className="text-muted-foreground text-xs leading-snug">
                Upload a broker or bank file. Mizan maps columns and asks for review when needed.
              </p>
            </div>
          </Link>
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

export default AddHoldingMenu;
