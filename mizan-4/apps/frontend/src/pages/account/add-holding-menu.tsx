/**
 * Per-asset-class "Add" affordance.
 *
 * Previously this surfaced a three-row dropdown ("Tell Mizan what you
 * own" / "Connect with Plaid" / "Drop a file") whose two AI options
 * both ended up in the same AddAssetDialog. The copy was also class-
 * agnostic — clicking "Add Bond" surfaced an example about "15 oz gold
 * or a rental property", which read as nonsense in context.
 *
 * The fix: replace the dropdown with a single class-aware button that
 * routes straight to AddAssetDialog (which already handles describe-
 * vs-drop-file in one composer). For broker-supported classes we add
 * a small "or connect with Plaid" sibling so the live-sync path stays
 * one click away without padding every menu with three rows.
 */

import { AssetClass, ASSET_CLASS_LABELS } from "@/lib/asset-classes";
import { Button } from "@mizan/ui/components/ui/button";
import { Icons } from "@mizan/ui/components/ui/icons";
import { Link } from "react-router-dom";

/**
 * Asset classes that have a meaningful broker-API path. Property,
 * Collectibles, Precious Metals, Other → no live sync available, so
 * the Plaid sibling is suppressed for those.
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
  /** Fires the inline AddAssetDialog with a class-flavoured seed prompt. */
  onManualAdd: () => void;
  /**
   * `inline` — small button next to a section header.
   * `cta`    — large button anchoring the empty-state card.
   */
  size?: "inline" | "cta";
}

export function AddHoldingMenu({ cls, onManualAdd, size = "inline" }: AddHoldingMenuProps) {
  const labels = ASSET_CLASS_LABELS[cls];
  const showBroker = BROKER_SUPPORTED_CLASSES.has(cls);

  if (size === "cta") {
    return (
      <div className="flex flex-col items-center gap-2 sm:flex-row">
        <Button size="default" onClick={onManualAdd}>
          <Icons.Plus className="mr-1.5 h-4 w-4" />
          Add {labels.singular.toLowerCase()}
        </Button>
        {showBroker && (
          <Button size="default" variant="outline" asChild>
            <Link to="/connect">
              <Icons.CloudSync className="mr-1.5 h-4 w-4" />
              Or connect with Plaid
            </Link>
          </Button>
        )}
      </div>
    );
  }

  // Inline (small button beside a header).
  if (showBroker) {
    return (
      <div className="flex items-center gap-1.5">
        <Button size="sm" variant="outline" onClick={onManualAdd}>
          <Icons.Plus className="mr-1 h-4 w-4" />
          Add {labels.singular.toLowerCase()}
        </Button>
        <Button
          size="sm"
          variant="ghost"
          asChild
          className="text-muted-foreground hover:text-foreground text-xs"
        >
          <Link to="/connect">
            <Icons.CloudSync className="mr-1 h-3.5 w-3.5" />
            Or connect
          </Link>
        </Button>
      </div>
    );
  }

  return (
    <Button size="sm" variant="outline" onClick={onManualAdd}>
      <Icons.Plus className="mr-1 h-4 w-4" />
      Add {labels.singular.toLowerCase()}
    </Button>
  );
}

export default AddHoldingMenu;
