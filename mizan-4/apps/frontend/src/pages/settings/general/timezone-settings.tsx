import { zodResolver } from "@hookform/resolvers/zod";
import { useMemo } from "react";
import { useForm } from "react-hook-form";
import * as z from "zod";

import { Button } from "@mizan/ui/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@mizan/ui/components/ui/card";
import { Form, FormControl, FormField, FormItem, FormMessage } from "@mizan/ui/components/ui/form";
import { useSettingsContext } from "@/lib/settings-provider";
import { TimezoneInput } from "./timezone-input";

const timezoneFormSchema = z.object({
  timezone: z.string().min(1, "Please select a timezone."),
});

type TimezoneFormValues = z.infer<typeof timezoneFormSchema>;

const TIMEZONE_FALLBACKS = [
  "UTC",
  "America/New_York",
  "America/Chicago",
  "America/Denver",
  "America/Los_Angeles",
  "America/Toronto",
  "Europe/London",
  "Europe/Paris",
  "Europe/Berlin",
  "Asia/Tokyo",
  "Asia/Singapore",
  "Australia/Sydney",
];

function getSupportedTimezones(): string[] {
  const supportedValuesOf = (
    Intl as unknown as {
      supportedValuesOf?: (key: "timeZone") => string[];
    }
  ).supportedValuesOf;

  const rawValues: string[] =
    typeof supportedValuesOf === "function" ? supportedValuesOf("timeZone") : TIMEZONE_FALLBACKS;

  const merged = rawValues.includes("UTC") ? rawValues : ["UTC", ...rawValues];
  return Array.from(new Set(merged)).sort((a, b) => a.localeCompare(b));
}

export function detectBrowserTimezone(): string {
  const detected = Intl.DateTimeFormat().resolvedOptions().timeZone?.trim();
  if (!detected) {
    return "UTC";
  }

  try {
    new Intl.DateTimeFormat("en-US", { timeZone: detected }).format(new Date());
    return detected;
  } catch {
    return "UTC";
  }
}

export function resolveInitialTimezone(configuredTimezone: string | null | undefined): string {
  const configured = configuredTimezone?.trim();
  if (configured && configured.length > 0) {
    return configured;
  }

  return "";
}

export function TimezoneSettings() {
  const { settings, updateSettings } = useSettingsContext();
  const browserTimezone = useMemo(() => detectBrowserTimezone(), []);
  const initialTimezone = resolveInitialTimezone(settings?.timezone);
  const timezones = useMemo(() => {
    const supported = getSupportedTimezones();
    // Put detected browser timezone first for easy access
    const filtered = supported.filter((tz) => tz !== browserTimezone);
    return [browserTimezone, ...filtered];
  }, [browserTimezone]);

  const form = useForm<TimezoneFormValues>({
    resolver: zodResolver(timezoneFormSchema),
    defaultValues: {
      timezone: initialTimezone,
    },
    values: {
      timezone: initialTimezone,
    },
  });

  async function onSubmit(data: TimezoneFormValues) {
    await updateSettings({ timezone: data.timezone });
  }

  return (
    <Card>
      <CardHeader>
        <div>
          <CardTitle className="text-lg">Timezone</CardTitle>
          <CardDescription>
            Choose the timezone used for dates, daily buckets, and yearly contribution boundaries.
          </CardDescription>
        </div>
      </CardHeader>
      <CardContent>
        <Form {...form}>
          <form onSubmit={form.handleSubmit(onSubmit)} className="space-y-6">
            <FormField
              control={form.control}
              name="timezone"
              render={({ field }) => (
                <FormItem className="flex flex-col">
                  <FormControl className="w-full max-w-[360px]">
                    <TimezoneInput
                      value={field.value}
                      onChange={field.onChange}
                      timezones={timezones}
                    />
                  </FormControl>
                  <FormMessage />
                </FormItem>
              )}
            />
            {/* Disable while the async submit is in flight so the user
                can't fire two settings updates by double-clicking. */}
            <Button type="submit" disabled={form.formState.isSubmitting}>
              Save Timezone
            </Button>
          </form>
        </Form>
      </CardContent>
    </Card>
  );
}
