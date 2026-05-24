import { createFixedDeposit, type FdPaymentFrequency } from "@/adapters";
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

const FREQUENCIES: readonly { value: FdPaymentFrequency; label: string; perYear: string }[] = [
  { value: "monthly", label: "Monthly", perYear: "12 payments/year" },
  { value: "quarterly", label: "Quarterly", perYear: "4 payments/year" },
  { value: "semi_annual", label: "Semi-annually", perYear: "2 payments/year" },
  { value: "annual", label: "Annually", perYear: "1 payment/year" },
  { value: "at_maturity", label: "At maturity", perYear: "Single lump sum at end of term" },
] as const;

const fdSchema = z.object({
  principal: z.coerce
    .number({ invalid_type_error: "Principal must be a number" })
    .positive("Principal must be greater than zero"),
  annualRatePercent: z.coerce
    .number({ invalid_type_error: "Rate must be a number" })
    .min(0, "Rate cannot be negative")
    .max(100, "Rate above 100% looks like a typo — verify and re-enter"),
  termMonths: z.coerce
    .number({ invalid_type_error: "Term must be a whole number of months" })
    .int("Term must be whole months")
    .min(1, "Term must be at least 1 month")
    .max(600, "Term capped at 600 months (50 years) — split into two FDs if longer"),
  paymentFrequency: z.enum(["monthly", "quarterly", "semi_annual", "annual", "at_maturity"]),
  startDate: z.string().regex(/^\d{4}-\d{2}-\d{2}$/u, "Start date must be YYYY-MM-DD"),
  currency: z.string().min(3, "Currency code required").max(8),
  notes: z.string().max(500).optional(),
});

type FdFormValues = z.infer<typeof fdSchema>;

interface FixedDepositDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Account that holds the principal — interest activities post into here. */
  accountId: string;
  /** Default currency to pre-fill (the account's currency). */
  defaultCurrency: string;
}

/**
 * Schedule a fixed deposit. Posts to the `create_fixed_deposit` Tauri
 * command which generates a series of INTEREST activities at the chosen
 * cadence into the selected account. The principal stays in the user's
 * cash balance (not double-counted), and the engine treats the interest
 * payments as gain (no `net_contribution` change), so they feed CAGR/TWR
 * through the same path as any broker-synced interest payment.
 */
export function FixedDepositDialog({
  open,
  onOpenChange,
  accountId,
  defaultCurrency,
}: FixedDepositDialogProps) {
  const queryClient = useQueryClient();
  const today = new Date().toISOString().slice(0, 10);

  const form = useForm<FdFormValues>({
    resolver: zodResolver(fdSchema),
    defaultValues: {
      principal: 10_000,
      annualRatePercent: 5,
      termMonths: 12,
      paymentFrequency: "quarterly",
      startDate: today,
      currency: defaultCurrency,
      notes: "",
    },
  });

  // Reset form whenever the dialog opens — avoids carrying over stale
  // values if the user closes mid-edit and re-opens.
  useEffect(() => {
    if (open) {
      form.reset({
        principal: 10_000,
        annualRatePercent: 5,
        termMonths: 12,
        paymentFrequency: "quarterly",
        startDate: today,
        currency: defaultCurrency,
        notes: "",
      });
    }
    // form.reset is stable; today changes per-render but only matters on open
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, defaultCurrency]);

  const mutation = useMutation({
    mutationFn: (values: FdFormValues) =>
      createFixedDeposit({
        accountId,
        principal: values.principal,
        annualRatePercent: values.annualRatePercent,
        termMonths: values.termMonths,
        paymentFrequency: values.paymentFrequency,
        startDate: values.startDate,
        currency: values.currency.toUpperCase(),
        notes: values.notes && values.notes.length > 0 ? values.notes : null,
      }),
    onSuccess: (activities) => {
      const total = activities.reduce(
        (sum, a) => sum + Number.parseFloat(String(a.amount ?? 0)),
        0,
      );
      toast.success(`Fixed deposit scheduled`, {
        description: `${activities.length} interest payment${
          activities.length === 1 ? "" : "s"
        } generated · total ${total.toLocaleString(undefined, {
          maximumFractionDigits: 2,
        })} expected`,
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
            : "Couldn't schedule the deposit. Try again.";
      toast.error(`Couldn't schedule fixed deposit: ${message}`);
    },
  });

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>Schedule a fixed deposit</DialogTitle>
          <DialogDescription>
            Generates a series of <span className="font-mono">INTEREST</span> activities at the
            chosen cadence. Principal stays in your cash balance — only the interest counts toward
            CAGR.
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
                name="principal"
                render={({ field }) => (
                  <FormItem>
                    <FormLabel>Principal</FormLabel>
                    <FormControl>
                      <Input
                        type="number"
                        inputMode="decimal"
                        step="0.01"
                        min="0"
                        placeholder="10000"
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
                name="annualRatePercent"
                render={({ field }) => (
                  <FormItem>
                    <FormLabel>Annual rate (%)</FormLabel>
                    <FormControl>
                      <Input
                        type="number"
                        inputMode="decimal"
                        step="0.01"
                        min="0"
                        max="100"
                        placeholder="5.0"
                        {...field}
                      />
                    </FormControl>
                    <FormMessage />
                  </FormItem>
                )}
              />
              <FormField
                control={form.control}
                name="termMonths"
                render={({ field }) => (
                  <FormItem>
                    <FormLabel>Term (months)</FormLabel>
                    <FormControl>
                      <Input
                        type="number"
                        inputMode="numeric"
                        step="1"
                        min="1"
                        max="600"
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
              name="paymentFrequency"
              render={({ field }) => (
                <FormItem>
                  <FormLabel>Payment frequency</FormLabel>
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
                            <span className="text-muted-foreground text-xs">{f.perYear}</span>
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
                    <Textarea rows={2} placeholder="HSBC AED FD @ 5%, 12-month tenor" {...field} />
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
                  "Schedule deposit"
                )}
              </Button>
            </DialogFooter>
          </form>
        </Form>
      </DialogContent>
    </Dialog>
  );
}
