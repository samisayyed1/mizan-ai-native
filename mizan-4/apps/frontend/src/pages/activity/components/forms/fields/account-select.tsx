import {
  FormControl,
  FormField,
  FormItem,
  FormLabel,
  FormMessage,
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@mizan/ui";
import { useEffect } from "react";
import { useFormContext, type FieldPath, type FieldValues, type PathValue } from "react-hook-form";

export interface AccountSelectOption {
  value: string;
  label: string;
  currency: string;
  /** Activity restriction level based on account tracking mode. */
  restrictionLevel?: "none" | "limited" | "blocked";
}

interface AccountSelectProps<TFieldValues extends FieldValues = FieldValues> {
  name: FieldPath<TFieldValues>;
  accounts: AccountSelectOption[];
  label?: string;
  placeholder?: string;
  /** Optional currency field to auto-populate from selected account when untouched/empty */
  currencyName?: FieldPath<TFieldValues>;
}

export function AccountSelect<TFieldValues extends FieldValues = FieldValues>({
  name,
  accounts,
  label = "Account",
  placeholder = "Select an account",
  currencyName,
}: AccountSelectProps<TFieldValues>) {
  const { control, getFieldState, getValues, setValue, watch } = useFormContext<TFieldValues>();
  const selectedAccountId = watch(name) as string | undefined;
  const watchedCurrency = watch((currencyName ?? name) as FieldPath<TFieldValues>) as
    | string
    | undefined;

  // Backfill currency when account options arrive after mount (e.g., preselected account via URL).
  useEffect(() => {
    if (!currencyName || !selectedAccountId) return;
    const selected = accounts.find((account) => account.value === selectedAccountId);
    if (!selected) return;

    const currentCurrency = watchedCurrency?.trim();
    if (currentCurrency === selected.currency) return;

    const shouldAutoSetCurrency = !currentCurrency && !getFieldState(currencyName).isDirty;
    if (!shouldAutoSetCurrency) return;

    setValue(currencyName, selected.currency as PathValue<TFieldValues, typeof currencyName>, {
      shouldDirty: false,
      shouldValidate: true,
    });
  }, [accounts, currencyName, getFieldState, selectedAccountId, setValue, watchedCurrency]);

  return (
    <FormField
      control={control}
      name={name}
      render={({ field }) => (
        <FormItem>
          <FormLabel>{label}</FormLabel>
          <FormControl>
            <Select
              onValueChange={(value) => {
                field.onChange(value);
                if (!currencyName) return;
                const selected = accounts.find((account) => account.value === value);
                if (!selected) return;
                const currentCurrency = (getValues(currencyName) as string | undefined)?.trim();
                const shouldAutoSetCurrency =
                  !getFieldState(currencyName).isDirty || !currentCurrency;
                if (shouldAutoSetCurrency) {
                  setValue(
                    currencyName,
                    selected.currency as PathValue<TFieldValues, typeof currencyName>,
                    {
                      shouldDirty: false,
                      shouldValidate: true,
                    },
                  );
                }
              }}
              defaultValue={field.value}
            >
              <SelectTrigger aria-label={label} data-testid="account-select">
                <SelectValue placeholder={placeholder} />
              </SelectTrigger>
              <SelectContent className="max-h-[500px] overflow-y-auto">
                {accounts.length === 0 ? (
                  <div className="text-muted-foreground px-3 py-4 text-center text-sm leading-snug">
                    <p className="text-foreground mb-1 font-medium">No accounts available</p>
                    <p className="text-xs">
                      Broker-synced HOLDINGS accounts can&apos;t accept manual activities. Create a
                      TRANSACTIONS-mode account or change an existing account&apos;s tracking mode
                      in settings.
                    </p>
                  </div>
                ) : (
                  accounts.map((account) => (
                    <SelectItem value={account.value} key={account.value}>
                      {account.label}
                      <span className="text-muted-foreground font-light">({account.currency})</span>
                    </SelectItem>
                  ))
                )}
              </SelectContent>
            </Select>
          </FormControl>
          <FormMessage />
        </FormItem>
      )}
    />
  );
}
