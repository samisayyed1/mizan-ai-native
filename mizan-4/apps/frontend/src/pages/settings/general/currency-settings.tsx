import { zodResolver } from "@hookform/resolvers/zod";
import { useForm } from "react-hook-form";
import * as z from "zod";

import { logger } from "@/adapters";
import { useRecalculatePortfolioMutation } from "@/hooks/use-calculate-portfolio";
import { useSettingsContext } from "@/lib/settings-provider";
import { Button } from "@mizan/ui/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@mizan/ui/components/ui/card";
import { Form, FormControl, FormField, FormItem, FormMessage } from "@mizan/ui/components/ui/form";
import { Icons } from "@mizan/ui/components/ui/icons";
import { CurrencyInput } from "@mizan/ui";
import { useQueryClient } from "@tanstack/react-query";

const primaryCurrencyFormSchema = z.object({
  baseCurrency: z.string({ required_error: "Please select a primary currency." }),
});

type PrimaryCurrencyFormValues = z.infer<typeof primaryCurrencyFormSchema>;

export function PrimaryCurrencyForm() {
  const { settings, updateBaseCurrency } = useSettingsContext();
  const { mutateAsync: recalculatePortfolio } = useRecalculatePortfolioMutation();
  const queryClient = useQueryClient();

  const form = useForm<PrimaryCurrencyFormValues>({
    resolver: zodResolver(primaryCurrencyFormSchema),
    defaultValues: { baseCurrency: settings?.baseCurrency || "USD" },
    // Reset form when settings change from external source
    values: { baseCurrency: settings?.baseCurrency || "USD" },
  });

  async function onSubmit(data: PrimaryCurrencyFormValues) {
    const next = data.baseCurrency;
    // Nothing to do if the currency didn't actually change — avoids a
    // needless full-portfolio recalc when the user re-saves the form.
    if (!next || next === settings?.baseCurrency) return;

    try {
      // 1. Persist the new primary currency.
      await updateBaseCurrency(next);
      // 2. Recompute every stored valuation in the new currency. Without
      //    this, the dashboard label flips to the new currency but the
      //    numbers stay computed in the old one until something else
      //    happens to refetch — a silent correctness bug.
      await recalculatePortfolio();
      // 3. Refetch everything that renders a converted (.base) value:
      //    holdings, valuations, performance, net worth, goals. A blanket
      //    invalidation is correct here because a primary-currency change
      //    touches essentially every monetary value in the app.
      await queryClient.invalidateQueries();
    } catch (error) {
      logger.error(`Failed to update primary currency: ${String(error)}`);
    }
  }

  const isApplying = form.formState.isSubmitting;

  return (
    <Form {...form}>
      <form onSubmit={form.handleSubmit(onSubmit)} className="space-y-6">
        <FormField
          control={form.control}
          name="baseCurrency"
          render={({ field }) => (
            <FormItem className="flex flex-col">
              <FormControl className="w-[300px]">
                <CurrencyInput value={field.value} onChange={field.onChange} />
              </FormControl>
              <FormMessage />
            </FormItem>
          )}
        />
        <p className="text-muted-foreground max-w-md text-xs leading-snug">
          Changing your primary currency recalculates every portfolio, net-worth and goal total in
          the new currency. This can take a moment on large portfolios.
        </p>
        {/* Disabled while the async submit + recalc is in flight so the
            user can't fire two recalculations by double-clicking. */}
        <Button type="submit" disabled={isApplying}>
          {isApplying ? (
            <>
              <Icons.Spinner className="mr-2 h-4 w-4 animate-spin" />
              Applying&hellip;
            </>
          ) : (
            "Save Currency"
          )}
        </Button>
      </form>
    </Form>
  );
}

export function PrimaryCurrencySettings() {
  return (
    <Card>
      <CardHeader>
        <div>
          <CardTitle className="text-lg">Primary Currency</CardTitle>
          <CardDescription>
            All dashboard and net-worth totals are shown in this currency. Individual holdings keep
            their own currencies.
          </CardDescription>
        </div>
      </CardHeader>
      <CardContent>
        <PrimaryCurrencyForm />
      </CardContent>
    </Card>
  );
}
