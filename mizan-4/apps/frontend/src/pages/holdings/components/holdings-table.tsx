import { Button } from "@mizan/ui/components/ui/button";
import { DataTable } from "@mizan/ui/components/ui/data-table";
import { DataTableColumnHeader } from "@mizan/ui/components/ui/data-table/data-table-column-header";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@mizan/ui/components/ui/dropdown-menu";
import { Icons } from "@mizan/ui/components/ui/icons";
import { parseOccSymbol } from "@/lib/occ-symbol";
import { safeDivide } from "@/lib/utils";
import type { ColumnDef } from "@tanstack/react-table";
import { GainPercent, Badge } from "@mizan/ui";

import { TickerAvatar } from "@/components/ticker-avatar";
import { Skeleton } from "@mizan/ui/components/ui/skeleton";
import { Tooltip, TooltipContent, TooltipTrigger } from "@mizan/ui/components/ui/tooltip";
import { useBalancePrivacy } from "@/hooks/use-balance-privacy";
import { useSettingsContext } from "@/lib/settings-provider";
import { Holding } from "@/lib/types";
import { AmountDisplay, QuantityDisplay } from "@mizan/ui";
import { useState } from "react";
import { useNavigate } from "react-router-dom";

import { AnimatedToggleGroup } from "@mizan/ui";
import { useAddAsset } from "@/features/add-asset";
import { useAccounts } from "@/hooks/use-accounts";

// Helper function to get display value and currency based on toggle state.
//
// When showConvertedToBase is false we report the holding in its OWN
// currency. That requires an FX rate from base → local. If the rate is
// missing AND the currencies actually differ, we can't honestly do the
// conversion — so we fall back to the base-currency value rather than
// silently rendering a base-currency number under a local-currency
// label (which would mis-state the holding to the user). QA Pass 6.
const getDisplayValueAndCurrency = (
  holding: Holding,
  valueInBase: number | null | undefined,
  showConvertedToBase: boolean,
): { value: number; currency: string } => {
  if (showConvertedToBase) {
    return {
      value: valueInBase ?? 0,
      currency: holding.baseCurrency,
    };
  }

  // Local-currency view. If FX is unavailable and the currencies
  // differ, the safest honest answer is to show the base value under
  // the base label; rendering "₹1,000" when the underlying is $1,000
  // USD would be wrong by ~85x.
  const fxRate = holding.fxRate;
  if (
    fxRate == null &&
    holding.localCurrency &&
    holding.baseCurrency &&
    holding.localCurrency !== holding.baseCurrency
  ) {
    return {
      value: valueInBase ?? 0,
      currency: holding.baseCurrency,
    };
  }
  const valueInOriginal = safeDivide(valueInBase ?? 0, fxRate ?? 1);
  return {
    value: valueInOriginal,
    currency: holding.localCurrency,
  };
};

export const HoldingsTable = ({
  holdings,
  isLoading,
  showTotalReturn = true,
  setShowTotalReturn,
  onClassify,
}: {
  holdings: Holding[];
  isLoading: boolean;
  showTotalReturn?: boolean;
  setShowTotalReturn?: (value: boolean) => void;
  onClassify?: (holding: Holding) => void;
}) => {
  const { isBalanceHidden } = useBalancePrivacy();
  const { settings } = useSettingsContext();
  const [showConvertedValues, setShowConvertedValues] = useState(false);

  const baseCurrency = settings?.baseCurrency ?? holdings[0]?.baseCurrency;
  const hasMultipleCurrencies = holdings.some((holding) => {
    if (!baseCurrency || !holding.localCurrency) {
      return false;
    }

    return holding.localCurrency.toUpperCase() !== baseCurrency.toUpperCase();
  });

  if (isLoading) {
    return (
      <div className="space-y-4 pt-6">
        <Skeleton className="h-12 w-full" />
        <Skeleton className="h-12 w-full" />
        <Skeleton className="h-12 w-full" />
        <Skeleton className="h-12 w-full" />
      </div>
    );
  }

  const uniqueTypesSet = new Set();
  const assetsTypes: { label: string; value: string }[] = holdings.reduce(
    (result: { label: string; value: string }[], asset) => {
      // Use taxonomy-based assetType classification
      const type = asset.instrument?.classifications?.assetType?.name;
      if (type && !uniqueTypesSet.has(type)) {
        uniqueTypesSet.add(type);
        result.push({ label: type.toUpperCase(), value: type });
      }
      return result;
    },
    [],
  );

  const filters = [
    {
      id: "holdingType",
      title: "Type",
      options: assetsTypes,
    },
  ];

  return (
    <div className="flex h-full flex-col">
      <DataTable
        data={holdings}
        columns={getColumns(isBalanceHidden, showConvertedValues, showTotalReturn, onClassify)}
        searchBy="symbol"
        filters={filters}
        showColumnToggle={true}
        storageKey="holdings-table"
        defaultColumnVisibility={{
          currency: false,
          symbolName: false,
          holdingType: false,
          bookValue: false,
          realizedGain: false,
        }}
        defaultSorting={[{ id: "symbol", desc: false }]}
        scrollable={true}
        toolbarActions={
          <div className="mr-2 flex items-center gap-2">
            {setShowTotalReturn && (
              <AnimatedToggleGroup
                value={showTotalReturn ? "total" : "daily"}
                onValueChange={(value) => setShowTotalReturn(value === "total")}
                items={[
                  { value: "total", label: "Total" },
                  { value: "daily", label: "Daily" },
                ]}
                size="xs"
                rounded="md"
              />
            )}
            {hasMultipleCurrencies && (
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button
                    variant="outline"
                    size="icon"
                    onClick={() => setShowConvertedValues(!showConvertedValues)}
                    className="h-8 w-8 rounded-lg"
                  >
                    {showConvertedValues ? (
                      <Icons.Globe className="h-4 w-4" />
                    ) : (
                      <Icons.DollarSign className="h-4 w-4" />
                    )}
                  </Button>
                </TooltipTrigger>
                <TooltipContent>
                  <p>Show values in {showConvertedValues ? "Asset Currency" : "Base Currency"}</p>
                </TooltipContent>
              </Tooltip>
            )}
          </div>
        }
      />
    </div>
  );
};

export default HoldingsTable;

const getColumns = (
  isHidden: boolean,
  showConvertedValues: boolean,
  showTotalReturn: boolean,
  onClassify?: (holding: Holding) => void,
): ColumnDef<Holding>[] => [
  {
    id: "symbol",
    accessorKey: "instrument.symbol",
    header: ({ column }) => <DataTableColumnHeader column={column} title="Position" />,
    meta: {
      label: "Position",
    },
    cell: ({ row }) => {
      const navigate = useNavigate();
      const holding = row.original;
      const symbol = holding.instrument?.symbol ?? holding.id;

      // Parse OCC symbol for options
      const parsedOption = parseOccSymbol(symbol);
      const displaySymbol = parsedOption ? parsedOption.underlying : symbol;

      // Option subtitle: "Mar 29 $150 CALL"
      const optionSubtitle = parsedOption
        ? `${new Date(parsedOption.expiration + "T12:00:00").toLocaleDateString("en-US", { month: "short", day: "numeric" })} $${parsedOption.strikePrice} ${parsedOption.optionType}`
        : null;

      const handleNavigate = () => {
        // Use instrument.id (asset ID) for navigation, not symbol (which may be stripped)
        const navSymbol = holding.instrument?.id ?? holding.id;
        navigate(`/holdings/${encodeURIComponent(navSymbol)}`, { state: { holding } });
      };

      const isManual = holding.instrument?.quoteMode === "MANUAL";
      const content = (
        <div className="flex items-center">
          <TickerAvatar
            symbol={parsedOption ? parsedOption.underlying : symbol}
            className="mr-2 h-8 w-8"
          />
          <div className="flex flex-col">
            <div className="flex items-center gap-1.5">
              <span className="font-semibold tracking-tight">{displaySymbol}</span>
              {isManual && (
                <Badge variant="secondary" className="h-4 px-1 py-0 text-[10px]">
                  Manual
                </Badge>
              )}
            </div>
            <span className="text-muted-foreground line-clamp-1 text-xs">
              {optionSubtitle ?? holding.instrument?.name ?? null}
            </span>
          </div>
        </div>
      );

      return (
        <div className="-m-1 cursor-pointer p-1" onClick={handleNavigate}>
          {content}
        </div>
      );
    },
    sortingFn: (rowA, rowB) => {
      const symbolA = rowA.original.instrument?.symbol ?? rowA.original.id;
      const symbolB = rowB.original.instrument?.symbol ?? rowB.original.id;
      return symbolA.localeCompare(symbolB);
    },
    filterFn: (row, _columnId, filterValue) => {
      const holding = row.original;
      const searchTerm = filterValue as string;
      const lowerSearch = searchTerm.toLowerCase();
      const nameMatch = holding.instrument?.name?.toLowerCase().includes(lowerSearch);
      const symbolMatch = holding.instrument?.symbol?.toLowerCase().includes(lowerSearch);
      const idMatch = holding.id.toLowerCase().includes(lowerSearch);
      // Also match on the underlying symbol for options
      const parsed = parseOccSymbol(holding.instrument?.symbol ?? "");
      const underlyingMatch = parsed?.underlying.toLowerCase().includes(lowerSearch);
      return !!(symbolMatch || nameMatch || idMatch || underlyingMatch);
    },
    enableHiding: false,
  },
  {
    id: "symbolName",
    accessorFn: (row) => row.instrument?.name || row.id,
    meta: {
      label: "Symbol Name",
    },
    enableHiding: false,
  },
  {
    id: "quantity",
    accessorKey: "quantity",
    enableHiding: true,
    header: ({ column }) => (
      <DataTableColumnHeader className="justify-end text-right" column={column} title="Qty" />
    ),
    meta: {
      label: "Quantity",
    },
    cell: ({ row }) => {
      const symbol = row.original.instrument?.symbol ?? row.original.id;
      const isOption = !!parseOccSymbol(symbol);
      const assetTypeKey = row.original.instrument?.classifications?.assetType?.key ?? "";
      const isBond =
        assetTypeKey.startsWith("BOND_") ||
        assetTypeKey === "DEBT_SECURITY" ||
        assetTypeKey === "MONEY_MARKET_DEBT";
      return (
        <div className="flex min-h-[40px] flex-col items-end justify-center px-4">
          <QuantityDisplay value={row.original.quantity} isHidden={isHidden} />
          <span className="text-muted-foreground text-xs">
            {isOption ? "contracts" : isBond ? "bonds" : "shares"}
          </span>
        </div>
      );
    },
    sortingFn: (rowA, rowB) => rowA.original.quantity - rowB.original.quantity,
  },
  {
    id: "marketPrice",
    accessorFn: (row) => row.price ?? 0,
    enableHiding: true,
    enableSorting: true,
    header: ({ column }) => (
      <DataTableColumnHeader
        className="justify-end text-right"
        column={column}
        title="Today's Price"
      />
    ),
    meta: {
      label: "Today's Price",
    },
    cell: ({ row }) => {
      const holding = row.original;
      const price = holding.price ?? 0;
      const currency = holding.localCurrency;
      return (
        <div className="flex min-h-[40px] flex-col items-end justify-center px-4">
          <AmountDisplay value={price} currency={currency} isHidden={isHidden} precision="price" />
          <GainPercent className="text-xs" value={holding.dayChangePct || 0} />
        </div>
      );
    },
  },
  {
    id: "bookValue",
    accessorFn: (row) => row.costBasis?.local ?? 0,
    enableHiding: true,
    header: ({ column }) => (
      <DataTableColumnHeader className="justify-end" column={column} title="Book Cost" />
    ),
    meta: {
      label: "Book Cost",
    },
    cell: ({ row }) => {
      const holding = row.original;
      const value = holding.costBasis?.local ?? 0;
      const currency = holding.localCurrency;

      return (
        <div className="flex min-h-[40px] flex-col items-end justify-center px-4">
          <AmountDisplay value={value} currency={currency} isHidden={isHidden} />
          <div className="text-xs text-transparent">-</div>
        </div>
      );
    },
    sortingFn: (rowA, rowB) => {
      const valueA = rowA.original.costBasis?.local ?? 0;
      const valueB = rowB.original.costBasis?.local ?? 0;
      return valueA - valueB;
    },
  },
  {
    id: "marketValue",
    accessorFn: (row) => row.marketValue.base ?? 0,
    enableHiding: false,
    header: ({ column }) => (
      <DataTableColumnHeader className="justify-end" column={column} title="Total Value" />
    ),
    meta: {
      label: "Total Value",
    },
    cell: ({ row }) => {
      const holding = row.original;
      const { value, currency } = getDisplayValueAndCurrency(
        holding,
        holding.marketValue.base,
        showConvertedValues,
      );

      return (
        <div className="flex min-h-[40px] flex-col items-end justify-center px-4">
          <AmountDisplay value={value} currency={currency} isHidden={isHidden} />
          <div className="text-muted-foreground text-xs">{currency}</div>
        </div>
      );
    },
    sortingFn: (rowA, rowB) => {
      const holdingA = rowA.original;
      const holdingB = rowB.original;

      // Always sort by base currency value for consistency
      const valueA = holdingA.marketValue.base ?? 0;
      const valueB = holdingB.marketValue.base ?? 0;

      return valueA - valueB;
    },
  },
  {
    id: "performance",
    accessorFn: (row) => row.totalGain?.base ?? 0,
    enableHiding: false,
    header: ({ column }) => (
      <DataTableColumnHeader
        className="justify-end"
        column={column}
        title={showTotalReturn ? "Total Return" : "Day Change"}
      />
    ),
    meta: {
      label: "Total Return",
    },
    cell: ({ row }) => {
      const holding = row.original;
      const valueBase = showTotalReturn ? holding.totalGain?.base : holding.dayChange?.base;
      const pct = showTotalReturn ? holding.totalGainPct : holding.dayChangePct;

      const { value, currency } = getDisplayValueAndCurrency(
        holding,
        valueBase,
        showConvertedValues,
      );

      return (
        <div className="flex min-h-[40px] flex-col items-end justify-center px-4">
          <AmountDisplay value={value} currency={currency} colorFormat={true} isHidden={isHidden} />
          <GainPercent className="text-xs" value={pct || 0} />
        </div>
      );
    },
    sortingFn: (rowA, rowB) => {
      const holdingA = rowA.original;
      const holdingB = rowB.original;

      // Always sort by base currency value for consistency
      const valueA = (showTotalReturn ? holdingA.totalGain?.base : holdingA.dayChange?.base) ?? 0;
      const valueB = (showTotalReturn ? holdingB.totalGain?.base : holdingB.dayChange?.base) ?? 0;

      return valueA - valueB;
    },
  },
  {
    id: "realizedGain",
    accessorFn: (row) => row.realizedGain?.base ?? 0,
    enableHiding: true,
    header: ({ column }) => (
      <DataTableColumnHeader className="justify-end" column={column} title="Realized Gain" />
    ),
    meta: {
      label: "Realized Gain",
    },
    cell: ({ row }) => {
      const holding = row.original;
      const valueBase = holding.realizedGain?.base;
      const pct = holding.realizedGainPct;

      if (valueBase == null || Number(valueBase) === 0) {
        return (
          <div className="flex min-h-[40px] flex-col items-end justify-center px-4">
            <span className="text-muted-foreground text-xs">—</span>
          </div>
        );
      }

      const { value, currency } = getDisplayValueAndCurrency(
        holding,
        valueBase,
        showConvertedValues,
      );

      return (
        <div className="flex min-h-[40px] flex-col items-end justify-center px-4">
          <AmountDisplay value={value} currency={currency} colorFormat={true} isHidden={isHidden} />
          {pct != null ? <GainPercent className="text-xs" value={pct} /> : null}
        </div>
      );
    },
    sortingFn: (rowA, rowB) => {
      const valueA = rowA.original.realizedGain?.base ?? 0;
      const valueB = rowB.original.realizedGain?.base ?? 0;
      return Number(valueA) - Number(valueB);
    },
  },
  {
    id: "holdingType",
    accessorFn: (row) => row.instrument?.classifications?.assetType?.name,
    meta: {
      label: "Asset Type",
    },
    header: ({ column }) => <DataTableColumnHeader column={column} title="Asset Type" />,
    filterFn: "arrIncludesSome",
  },
  {
    id: "currency",
    accessorKey: "localCurrency",
    header: ({ column }) => <DataTableColumnHeader column={column} title="Currency" />,
    meta: {
      label: "Currency",
    },
    cell: ({ row }) => <div className="text-muted-foreground">{row.original.localCurrency}</div>,
    filterFn: (row, id, value) => {
      return value.includes(row.getValue(id));
    },
  },
  {
    id: "actions",
    enableHiding: false,
    header: () => null,
    cell: ({ row }) => {
      const navigate = useNavigate();
      const addAsset = useAddAsset();
      // Look up the account name for the holding so the AddAsset dialog
      // can show "for the Schwab portfolio" rather than the raw ID hash.
      const { accounts } = useAccounts({ filterActive: true });
      const holding = row.original;
      const hasInstrument = !!holding.instrument;
      const accountName = accounts?.find((a) => a.id === holding.accountId)?.name;
      const symbol = holding.instrument?.symbol ?? holding.id;
      const assetId = holding.instrument?.id ?? holding.id;

      const handleNavigate = () => {
        navigate(`/holdings/${encodeURIComponent(assetId)}`, {
          state: { holding },
        });
      };

      // Enterprise UX-5: one-click "Record activity" routes to the
      // activity manager with account AND asset pre-filled. No
      // re-asking the user which portfolio or which stock — they just
      // told us by clicking this row's menu.
      const handleRecordActivity = () => {
        navigate(
          `/activities/manage?account=${encodeURIComponent(holding.accountId)}` +
            `&assetId=${encodeURIComponent(assetId)}` +
            `&redirect-to=${encodeURIComponent(`/accounts/${holding.accountId}`)}`,
        );
      };

      // Ask Mizan AI to update — opens the AI dialog with full context
      // (account + asset symbol). The agent's record_activity tool
      // resolves both automatically.
      const handleAskAi = () => {
        addAsset.open({
          source: "portfolio",
          accountId: holding.accountId,
          accountName,
          prompt: `Update my ${symbol} holding. What would you like to change?`,
        });
      };

      return (
        <div className="flex items-center justify-end">
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="ghost" size="sm">
                <Icons.MoreVertical className="h-4 w-4" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end">
              {hasInstrument && (
                <DropdownMenuItem onClick={handleRecordActivity}>
                  <Icons.Plus className="mr-2 h-4 w-4" />
                  Record activity
                </DropdownMenuItem>
              )}
              {hasInstrument && (
                <DropdownMenuItem onClick={handleAskAi}>
                  <Icons.Sparkles className="mr-2 h-4 w-4" />
                  Ask Mizan AI to update
                </DropdownMenuItem>
              )}
              {hasInstrument && onClassify && (
                <DropdownMenuItem onClick={() => onClassify(holding)}>
                  <Icons.Tag className="mr-2 h-4 w-4" />
                  Classify
                </DropdownMenuItem>
              )}
              <DropdownMenuItem onClick={handleNavigate}>
                <Icons.ChevronRight className="mr-2 h-4 w-4" />
                View details
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      );
    },
  },
];
