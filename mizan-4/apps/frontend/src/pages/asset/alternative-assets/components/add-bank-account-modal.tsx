/**
 * Add Bank Account — dedicated modal (Feroz step C, §11 + §12).
 *
 * Feroz on the May-17 call:
 *   "bank account is an asset class... the holdings are the bank
 *    accounts themselves: Axis Bank, ICICI Bank, DBS Bank... every
 *    bank account you should be able to change the currency... bank
 *    account should mention country but do NOT map the country with
 *    the currency because I can have an account in Singapore with US
 *    dollars... DBS USD and DBS SGD as separate holdings."
 *
 * Design / data-model decision (deliberate, low-risk, no migration):
 * a bank account is persisted as an ALTERNATIVE ASSET — exactly the
 * same proven path Property / Collectible / Precious Metal / Liability
 * already use. We set:
 *   - kind:         "other"            (a real, validated alt-asset kind)
 *   - name:         the bank name      ("Axis Bank", "DBS — USD")
 *   - currency:     chosen freely      (NOT derived from country)
 *   - currentValue: the balance amount
 *   - metadata:     { sub_type: "bank_account", country }
 *
 * `sub_type` is the same metadata discriminator the quick-add modal
 * already uses for precious metals and liabilities, so the existing
 * display layer (getSubtypeLabel) picks it up with one small addition.
 *
 * Multi-currency per bank is achieved exactly as Feroz described:
 * create two bank accounts ("DBS — USD" and "DBS — SGD"), each its own
 * holding with its own currency. No special casing required.
 *
 * This is a SEPARATE modal from the shared AlternativeAssetQuickAddModal
 * on purpose: that modal carries property/liability/mortgage-chaining
 * logic, and bank accounts have a distinct, simpler field set. Keeping
 * them apart means zero regression risk to the existing flows.
 */

import { COUNTRY_SELECT_OPTIONS } from "@/lib/countries";
import type { CreateAlternativeAssetRequest } from "@/lib/types";
import { useSettingsContext } from "@/lib/settings-provider";
import { CurrencyInput, MoneyInput, ResponsiveSelect } from "@mizan/ui";
import { Button } from "@mizan/ui/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@mizan/ui/components/ui/dialog";
import { Icons } from "@mizan/ui/components/ui/icons";
import { Input } from "@mizan/ui/components/ui/input";
import { Label } from "@mizan/ui/components/ui/label";
import { useEffect, useState } from "react";
import { useAlternativeAssetMutations } from "../hooks/use-alternative-asset-mutations";

interface AddBankAccountModalProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onAssetCreated?: (response: { assetId: string }) => void;
}

interface BankAccountFormState {
  bankName: string;
  country: string;
  amount: string;
  currency: string;
}

function todayIso(): string {
  // YYYY-MM-DD in local time — matches the value-date format the
  // alternative-asset backend expects.
  const now = new Date();
  const y = now.getFullYear();
  const m = String(now.getMonth() + 1).padStart(2, "0");
  const d = String(now.getDate()).padStart(2, "0");
  return `${y}-${m}-${d}`;
}

export function AddBankAccountModal({
  open,
  onOpenChange,
  onAssetCreated,
}: AddBankAccountModalProps) {
  const { settings } = useSettingsContext();
  const baseCurrency = settings?.baseCurrency ?? "USD";

  const [form, setForm] = useState<BankAccountFormState>({
    bankName: "",
    country: "",
    amount: "",
    currency: baseCurrency,
  });

  const { createMutation } = useAlternativeAssetMutations({
    onCreateSuccess: (response) => {
      onAssetCreated?.(response);
      onOpenChange(false);
    },
  });

  // Reset on open so a reopened modal never shows stale input.
  useEffect(() => {
    if (open) {
      setForm({ bankName: "", country: "", amount: "", currency: baseCurrency });
    }
  }, [open, baseCurrency]);

  const update = <K extends keyof BankAccountFormState>(key: K, value: BankAccountFormState[K]) => {
    setForm((prev) => ({ ...prev, [key]: value }));
  };

  const canSubmit =
    form.bankName.trim().length > 0 &&
    form.currency.trim().length > 0 &&
    form.amount.trim().length > 0 &&
    Number.isFinite(Number(form.amount));

  const handleSubmit = async () => {
    if (!canSubmit || createMutation.isPending) return;

    const metadata: Record<string, string> = { sub_type: "bank_account" };
    // Country is optional — Feroz wants it captured "but" it shouldn't
    // block adding an account, and it must never imply a currency.
    if (form.country.trim()) metadata.country = form.country.trim();

    const request: CreateAlternativeAssetRequest = {
      kind: "other",
      name: form.bankName.trim(),
      currency: form.currency,
      currentValue: form.amount,
      valueDate: todayIso(),
      metadata,
    };

    await createMutation.mutateAsync(request);
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Icons.Wallet className="h-5 w-5" />
            Add Bank Account
          </DialogTitle>
          <DialogDescription>
            Each bank account is its own holding with its own currency. For a multi-currency bank,
            add one account per currency (e.g. &ldquo;DBS &mdash; USD&rdquo; and &ldquo;DBS &mdash;
            SGD&rdquo;).
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4 py-2">
          <div className="space-y-2">
            <Label className="text-sm font-medium">Bank name</Label>
            <Input
              value={form.bankName}
              onChange={(e) => update("bankName", e.target.value)}
              placeholder="Axis Bank, ICICI, DBS — USD…"
              className="h-11"
              autoFocus
            />
          </div>

          <div className="space-y-2">
            <Label className="text-sm font-medium">
              Country
              <span className="text-muted-foreground ml-1 text-xs font-normal">(optional)</span>
            </Label>
            <ResponsiveSelect
              value={form.country}
              onValueChange={(v) => update("country", v)}
              options={[...COUNTRY_SELECT_OPTIONS]}
              placeholder="Where is the account held?"
              sheetTitle="Select Country"
              sheetDescription="Where the bank account is held. This does not change the currency."
              triggerClassName="h-11"
            />
          </div>

          <div className="grid grid-cols-2 gap-4">
            <div className="space-y-2">
              <Label className="text-sm font-medium">Amount</Label>
              <MoneyInput
                value={form.amount}
                onValueChange={(v) => update("amount", v == null ? "" : String(v))}
                placeholder="0.00"
                className="h-11"
              />
            </div>
            <div className="space-y-2">
              <Label className="text-sm font-medium">Currency</Label>
              <CurrencyInput
                value={form.currency}
                onChange={(v) => update("currency", v)}
                placeholder="Select currency"
              />
            </div>
          </div>

          <p className="text-muted-foreground text-xs leading-snug">
            The currency is independent of the country &mdash; a Singapore account can hold USD,
            SGD, or anything else.
          </p>
        </div>

        <div className="flex justify-end gap-2">
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button onClick={handleSubmit} disabled={!canSubmit || createMutation.isPending}>
            {createMutation.isPending ? (
              <>
                <Icons.Spinner className="mr-2 h-4 w-4 animate-spin" />
                Adding…
              </>
            ) : (
              "Add Bank Account"
            )}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}

export default AddBankAccountModal;
