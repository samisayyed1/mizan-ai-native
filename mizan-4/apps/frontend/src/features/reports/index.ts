// Assistant-native reports feature
// =========================
//
// Client-side PDF report rendering. Reports are React trees composed from
// `<ReportShell>` + `<ReportSection>` primitives, captured off-screen via
// html2canvas + jsPDF by `useReportRenderer`.
//
// All reports are gated on `entitlements.advancedReports` at the call site.

export { ReportShell, A4_WIDTH_PX, A4_HEIGHT_PX } from "./report-shell";
export type { ReportShellProps } from "./report-shell";
export { ReportSection } from "./report-section";
export type { ReportSectionProps } from "./report-section";
export { useReportRenderer } from "./use-report-renderer";
export type { RenderOptions, RenderResult } from "./use-report-renderer";

// Report templates
export { IncomeReport } from "./templates/income-report";
export type { IncomeReportData } from "./templates/income-report";
export { RentalReport } from "./templates/rental-report";
export type { RentalReportData, RentalProperty } from "./templates/rental-report";
export { PayoffReport } from "./templates/payoff-report";
export type { PayoffReportData } from "./templates/payoff-report";
export { HealthReport } from "./templates/health-report";
export type { HealthReportData } from "./templates/health-report";
