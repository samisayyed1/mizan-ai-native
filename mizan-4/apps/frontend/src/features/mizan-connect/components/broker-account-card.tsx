import { Card, CardContent } from "@mizan/ui/components/ui/card";
import { Badge } from "@mizan/ui/components/ui/badge";
import { Icons } from "@mizan/ui/components/ui/icons";
import { formatDate } from "@/lib/utils";
import { isValid } from "date-fns";
import { useState } from "react";
import type { BrokerAccount } from "../types";

interface BrokerAccountCardProps {
  account: BrokerAccount;
}

/**
 * Mask account number to show only last 4 characters
 */
function maskAccountNumber(number?: string): string {
  if (!number) return "";
  const last4 = number.slice(-4);
  return `\u2022\u2022${last4}`;
}

/**
 * Get the latest sync date from transactions or holdings
 */
function getLastSyncDate(account: BrokerAccount): string | null {
  const txDate = account.sync_status?.transactions?.last_successful_sync;
  const holdingsDate = account.sync_status?.holdings?.last_successful_sync;

  // Pick the most recent of the two — but only after confirming each
  // is a parseable date. A malformed string would otherwise compare
  // `Invalid Date > Invalid Date` (always false) and silently drop
  // the value we should have shown.
  const txValid = !!txDate && isValid(new Date(txDate));
  const holdingsValid = !!holdingsDate && isValid(new Date(holdingsDate));

  if (txValid && holdingsValid) {
    return new Date(txDate) > new Date(holdingsDate) ? txDate : holdingsDate;
  }
  if (txValid) return txDate ?? null;
  if (holdingsValid) return holdingsDate ?? null;
  return null;
}

/**
 * Format the last sync date for display
 */
function formatLastSyncDate(dateStr: string | null): string {
  if (!dateStr) return "No data yet";
  return `Data as of ${formatDate(dateStr)}`;
}

export function BrokerAccountCard({ account }: BrokerAccountCardProps) {
  const lastSyncDate = getLastSyncDate(account);
  const isShared = account.owner && !account.owner.is_own_account;
  const ownerName = account.owner?.full_name;
  // Pre-fix the <img> + <Icons.Wallet> were both rendered into the
  // same 40×40 container — the Wallet icon was supposed to show "on
  // error" via a CSS class toggle, but when the logo loaded both
  // icons rendered overlapping. React-state fallback is cleaner and
  // never overlaps.
  const [logoFailed, setLogoFailed] = useState(false);
  const showLogo = !!account.institution_name && !logoFailed;

  return (
    <Card>
      <CardContent className="flex items-center justify-between p-4">
        <div className="flex items-center gap-3">
          {/* Platform logo or fallback */}
          <div className="bg-muted flex h-10 w-10 items-center justify-center rounded-lg">
            {showLogo ? (
              <img
                src={`https://logo.clearbit.com/${account.institution_name!.toLowerCase().replace(/\s+/g, "")}.com`}
                alt={account.institution_name}
                className="h-6 w-6"
                onError={() => setLogoFailed(true)}
              />
            ) : (
              <Icons.Wallet className="text-muted-foreground h-5 w-5" />
            )}
          </div>

          <div>
            <div className="flex items-center gap-2">
              <p className="font-medium">{account.name || "Account"}</p>
              {account.is_paper && (
                <Badge variant="outline" className="text-xs">
                  Paper
                </Badge>
              )}
              {isShared && ownerName && (
                <span className="text-muted-foreground flex items-center gap-1 text-sm">
                  <Icons.Users className="h-3.5 w-3.5" />
                  Shared by {ownerName}
                </span>
              )}
            </div>
            <div className="text-muted-foreground flex items-center gap-2 text-sm">
              <span>{account.institution_name}</span>
              {account.number && (
                <>
                  <span className="text-muted-foreground/50">
                    {maskAccountNumber(account.number)}
                  </span>
                </>
              )}
            </div>
          </div>
        </div>

        <div className="flex items-center gap-3">
          <span className="text-muted-foreground text-sm">{formatLastSyncDate(lastSyncDate)}</span>

          {/* Sync enabled indicator */}
          {account.sync_enabled ? (
            <Icons.Eye className="h-4 w-4 text-blue-500" />
          ) : (
            <Icons.EyeOff className="text-muted-foreground h-4 w-4" />
          )}
        </div>
      </CardContent>
    </Card>
  );
}
