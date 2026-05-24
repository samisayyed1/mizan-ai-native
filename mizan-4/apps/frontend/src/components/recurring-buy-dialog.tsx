import { createRecurringBuyPlan, type RspFrequency } from "@/adapters";
import { QueryKeys } from "@/lib/query-keys";
import { zodResolver } from "@hookform/resolvers/zod";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Button } from "@mizan/ui/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@mizan/ui/components/ui/dialog";
import {
  Form,
  FormControl,
  FormField,
  FormItem,
  FormLabel,
  FormMessage,
} from "@mizan/ui/components/ui/form";
import { Icons } from "@mizan/ui/components/ui/icons";
import { Input } from "@mizan/ui/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@mizan/ui/components/ui/select";
import { Textarea } from "@mizan/ui/components/ui/textarea";
import { toast } from "@mizan/ui/components/ui/use-toast";
import { useEffect } from "react";
import { useForm } from "react-hook-form";
import { z } from "zod";

const FREQUENCIES: readonly { value: RspFrequency; label: string; cadence: string }[] = [
  { value: "weekly", label: "Weekly", cadence: "Every 7 days" },
  { value: "biweekly", label: "Biweekly", cadence: "Every 14 days" },
  { value: "monthly", label: "Monthly", cadence: "Same day each month" },
  { value: "quarterly", label: "Quarterly", cadence: "Every 3 months" },
  { value: "semi_annual", label: "Semi-annually", cadence: "Every 6 months" },
  { value: "annual", label: "Annually", cadence: "Same day each year" },
] as const;

const rspSchema = z.object({
  symbol: z
    .string()
    .min(1, "Symbol required")
    .max(32)
    .regex(/^[A-Za-z0-9.\-:]+$/u, "Use letters, digits, dot, dash, or colon"),
  amountPerBuy: z.coerce
    .number({ invalid_type_error: "Amount must be a number" })
    .positive("Amount must be greater than zero"),
  unitPrice: z.coerce
    .number({ invalid_type_error: "Reference price must be a number" })
    .positive("Reference price must be greater than zero"),
  frequency: z.enum(["weekly", "biweekly", "monthly", "quarterly", "semi_annual", "annual"]),
  startDate: z.string().regex(/^\d{4}-\d{2}-\d{2}$/u, "Start date must be YYYY-MM-DD"),
  installments: z.coerce
    .number({ invalid_type_error: "Number of buys must be a whole number" })
    .int("Must be a whole number")
    .min(1, "At least 1 buy required")
    .max(240, "Capped at 240 buys — split into multiple plans if longer"),
  currency: z.string().min(3, "Currency code required").max(8),
  notes: z.string().max(500).optional(),
});

type RspFormValues = z.infer<typeof rspSchema>;

interface RecurringBuyDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Account that holds the buys — BUY activities post into here. */
  accountId: string;
  /** Default currency to pre-fill (the account's currency). */
  defaultCurrency: string;
}

/**
 * Schedule a recurring stock-purchase plan (RSP / SIP / DCA). Posts to
 * the `create_recurring_buy_plan` Tauri command which generates a series
 * of BUY activities at the chosen cadence into the selected account.
 * Each emitted BUY has `quantity = amountPerBuy / unitPrice` (rounded to
 * 6 d.p. for fractional shares) and lands as POSTED — the portfolio
 * reflects the plan immediately, just like manually-entered buys.
 */
export function RecurringBuyDialog({
  open,
  onOpenChange,
  accountId,
  defaultCurrency,
}: RecurringBuyDialogProps) {
  const queryClient = useQueryClient();
  const today = new Date().toISOString().slice(0, 10);

  const form = useForm<RspFormValues>({
    resolver: zodResolver(rspSchema),
    defaultValues: {
      symbol: "",
      amountPerBuy: 500,
      unitPrice: 100,
      frequency: "monthly",
      startDate: today,
      installments: 12,
      currency: defaultCurrency,
      notes: "",
    },
  });

  // Reset whenever the dialog opens — avoids carrying over stale values
  // if the user closes mid-edit and re-opens.
  useEffect(() => {
    if (open) {
      form.reset({
        symbol: "",
        amountPerBuy: 500,
        unitPrice: 100,
        frequency: "monthly",
        startDate: today,
        installments: 12,
        currency: defaultCurrency,
        notes: "",
      });
    }
    // form.reset is stable; today changes per-render but only matters on open
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, defaultCurrency]);

  const mutation = useMutation({
    mutationFn: async (values: RspFormValues) => {
      const result = await createRecurringBuyPlan({
        accountId,
        symbol: values.symbol.trim().toUpperCase(),
        amountPerBuy: values.amountPerBuy,
        unitPrice: values.unitPrice,
        frequency: values.frequency,
        startDate: values.startDate,
        installments: values.installments,
        currency: values.currency.toUpperCase(),
        notes: values.notes && values.notes.length > 0 ? values.notes : null,
      });
      return {
        activities: result,
        symbol: values.symbol.trim().toUpperCase(),
        currency: values.currency.toUpperCase(),
      };
    },
    onSuccess: ({ activities, symbol, currency }) => {
      const totalCash = activities.reduce(
        (sum, a) =>
          sum +
          Number.parseFloat(String(a.quantity ?? 0)) * Number.parseFloat(String(a.unitPrice ?? 0)),
        0,
      );
      toast.success(`Recurring buy plan scheduled`, {
        description: `${activities.length} buy${
          activities.length === 1 ? "" : "s"
        } of ${symbol} generated · total ${totalCash.toLocaleString(undefined, {
          maximumFractionDigits: 2,
        })} ${currency} committed`,
      });
      void queryClient.invalidateQueries({ queryKey: [QueryKeys.ACTIVITIES] });
      void queryClient.invalidateQueries({ queryKey: [QueryKeys.ACCOUNTS] });
      void queryClient.invalidateQueries({ queryKey: [QueryKeys.ACCOUNTS_SUMMARY] });
      onOpenChange(false);
    },
    onError: (err) => {
      const message =
        err instanceof Error
          ? err.message
          : typeof err === "string"
            ? err
            : "Couldn't schedule the plan. Try again.";
      toast.error(`Couldn't schedule recurring buy: ${message}`);
    },
  });

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>Schedule a recurring buy</DialogTitle>
          <DialogDescription>
            Generates a series of <span className="font-mono">BUY</span> activities at the chosen
            cadence — your SIP / DCA / RSP, modelled deterministically. Each buy spends{" "}
            <span className="font-mono">amount</span> at <span className="font-mono">price</span>{" "}
            for <span className="font-mono">amount / price</span> shares.
          </DialogDescription>
        </DialogHeader>

        <Form {...form}>
          <form
            onSubmit={form.handleSubmit((values) => mutation.mutate(values))}
            className="space-y-4"
          >
            <div className="grid grid-cols-2 gap-3">
              <FormField
                control={form.control}
                name="symbol"
                render={({ field }) => (
                  <FormItem>
                    <FormLabel>Symbol</FormLabel>
                    <FormControl>
                      <Input
                        placeholder="VTI"
                        autoCapitalize="characters"
                        autoComplete="off"
                        spellCheck={false}
                        {...field}
                      />
                    </FormControl>
                    <FormMessage />
                  </FormItem>
                )}
              />
              <FormField
                control={form.control}
                name="currency"
                render={({ field }) => (
                  <FormItem>
                    <FormLabel>Currency</FormLabel>
                    <FormControl>
                      <Input maxLength={8} placeholder="USD" {...field} />
                    </FormControl>
                    <FormMessage />
                  </FormItem>
                )}
              />
            </div>

            <div className="grid grid-cols-2 gap-3">
              <FormField
                control={form.control}
                name="amountPerBuy"
                render={({ field }) => (
                  <FormItem>
                    <FormLabel>Amount per buy</FormLabel>
                    <FormControl>
                      <Input
                        type="number"
                        inputMode="decimal"
                        step="0.01"
                        min="0"
                        placeholder="500"
                        {...field}
                      />
                    </FormControl>
                    <FormMessage />
                  </FormItem>
                )}
              />
              <FormField
                control={form.control}
                name="unitPrice"
                render={({ field }) => (
                  <FormItem>
                    <FormLabel>Reference price / share</FormLabel>
                    <FormControl>
                      <Input
                        type="number"
                        inputMode="decimal"
                        step="0.0001"
                        min="0"
                        placeholder="245.30"
                        {...field}
                      />
                    </FormControl>
                    <FormMessage />
                  </FormItem>
                )}
              />
            </div>

            <div className="grid grid-cols-2 gap-3">
              <FormField
                control={form.control}
                name="frequency"
                render={({ field }) => (
                  <FormItem>
                    <FormLabel>Frequency</FormLabel>
                    <Select value={field.value} onValueChange={field.onChange}>
                      <FormControl>
                        <SelectTrigger>
                          <SelectValue />
                        </SelectTrigger>
                      </FormControl>
                      <SelectContent>
                        {FREQUENCIES.map((f) => (
                          <SelectItem key={f.value} value={f.value}>
                            <div className="flex flex-col">
                              <span>{f.label}</span>
                              <span className="text-muted-foreground text-xs">{f.cadence}</span>
                            </div>
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                    <FormMessage />
                  </FormItem>
                )}
              />
              <FormField
                control={form.control}
                name="installments"
                render={({ field }) => (
                  <FormItem>
                    <FormLabel>Number of buys</FormLabel>
                    <FormControl>
                      <Input
                        type="number"
                        inputMode="numeric"
                        step="1"
                        min="1"
                        max="240"
                        placeholder="12"
                        {...field}
                      />
                    </FormControl>
                    <FormMessage />
                  </FormItem>
                )}
              />
            </div>

            <FormField
              control={form.control}
              name="startDate"
              render={({ field }) => (
                <FormItem>
                  <FormLabel>Start date</FormLabel>
                  <FormControl>
                    <Input type="date" {...field} />
                  </FormControl>
                  <FormMessage />
                </FormItem>
              )}
            />

            <FormField
              control={form.control}
              name="notes"
              render={({ field }) => (
                <FormItem>
                  <FormLabel>Notes (optional)</FormLabel>
                  <FormControl>
                    <Textarea
                      rows={2}
                      placeholder="Monthly VTI SIP — $500/mo into IRA"
                      {...field}
                    />
                  </FormControl>
                  <FormMessage />
                </FormItem>
              )}
            />

            <DialogFooter>
              <Button
                type="button"
                variant="ghost"
                onClick={() => onOpenChange(false)}
                disabled={mutation.isPending}
              >
                Cancel
              </Button>
              <Button type="submit" disabled={mutation.isPending}>
                {mutation.isPending ? (
                  <>
                    <Icons.Spinner className="mr-2 h-4 w-4 animate-spin" />
                    Scheduling…
                  </>
                ) : (
                  "Schedule plan"
                )}
              </Button>
            </DialogFooter>
          </form>
        </Form>
      </DialogContent>
    </Dialog>
  );
}
