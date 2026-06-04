/**
 * Playwright config — Track E2E PR-1 / Goal v3 §V Phase 9.
 *
 * Drives the §23 Ramadan Zakat scenario end-to-end against the
 * Mizan desktop frontend running in dev mode. The test asserts the
 * Singapore Sharia-aware millionaire's reference user flow:
 *   - Dashboard cold start < 1.2s
 *   - Zakat compute < 2s with school-aware breakdown
 *   - Every figure carries a Mizan Badge with origin + modifier
 *   - Today's Signal + News Relevant + Sukuks panel + Net Worth Sankey
 *   - Pay Zakat Stripe test-mode flow with receipt generation
 *
 * # Running
 *
 *   pnpm exec playwright install chromium webkit   # one-time
 *   MIZAN_E2E_BASE_URL=http://localhost:1420 \
 *     pnpm exec playwright test                    # against running dev
 *
 * # CI gating
 *
 * The CI runner enables this suite as a required check on every PR
 * post-§23 closeout. The desktop's dev server is launched in headless
 * mode by the CI workflow (see `.github/workflows/e2e.yml`, landed
 * separately as PR-E2E.b).
 *
 * # Skipped assertions
 *
 * Tests marked `test.skip` need infrastructure not yet present:
 *   - Fixture user database seeding (`MIZAN_E2E_FIXTURE_USER=s23`)
 *   - Stripe test-mode webhook on the local dev server
 *   - Mizan Connect staging endpoint reachable from CI
 *
 * Each skipped block carries a TODO with the exact env var or
 * infrastructure that needs to land before the assertion turns on.
 */

import { defineConfig, devices } from "@playwright/test";

const BASE_URL = process.env.MIZAN_E2E_BASE_URL ?? "http://localhost:1420";
const IS_CI = !!process.env.CI;

export default defineConfig({
  testDir: "./e2e",
  testMatch: /.*\.spec\.ts$/,

  // Per the directive: "retries=1, video on failure".
  retries: 1,
  workers: IS_CI ? 1 : undefined,
  fullyParallel: false, // §23 scenario builds incrementally — keep ordered

  // Reasonable timeouts for a real-app E2E (not unit test).
  timeout: 60_000,
  expect: { timeout: 10_000 },

  reporter: [
    ["list"],
    ["html", { outputFolder: "playwright-report", open: "never" }],
    ["json", { outputFile: "playwright-report/results.json" }],
  ],

  use: {
    baseURL: BASE_URL,
    actionTimeout: 10_000,
    navigationTimeout: 15_000,
    trace: "on-first-retry",
    video: "retain-on-failure",
    screenshot: "only-on-failure",
  },

  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
    {
      name: "webkit",
      use: { ...devices["Desktop Safari"] },
    },
  ],

  // The dev server is started externally (Tauri dev mode or
  // `pnpm dev`). The webServer block here is the fallback for CI.
  webServer: process.env.MIZAN_E2E_AUTOSTART
    ? {
        command: "pnpm dev",
        url: BASE_URL,
        reuseExistingServer: !IS_CI,
        timeout: 120_000,
      }
    : undefined,
});
