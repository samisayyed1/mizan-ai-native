// Monthly AI Wealth Report adapter (M3.6).
//
// Both routes are server-only (the cloud cron writes the rows; the API
// surfaces them). We dispatch via the cloud client invoke layer that the rest
// of the connect adapter already uses.

import { invoke } from "./platform";

/** Mirrors `mizan-connect : src/billing/reports.rs::MonthlyReport`. */
export interface MonthlyReport {
  id: string;
  userId: string;
  periodStart: string; // ISO date (YYYY-MM-DD)
  periodEnd: string;
  summaryMd: string | null;
  model: string | null;
  creditsCharged: number;
  status: "pending" | "succeeded" | "failed";
  error: string | null;
  requestedAt: string; // RFC 3339
  generatedAt: string | null;
}

export interface MonthlyReportsResponse {
  reports: MonthlyReport[];
}

export async function listMonthlyReports(limit?: number): Promise<MonthlyReportsResponse> {
  return invoke<MonthlyReportsResponse>("list_monthly_reports", { limit: limit ?? 12 });
}

export async function requestMonthlyReport(): Promise<MonthlyReport> {
  return invoke<MonthlyReport>("request_monthly_report");
}
