/**
 * Visual regression — full-page snapshot per viewport.
 *
 * The form-success snapshot mocks the /api/waitlist response so the
 * confirmation card renders deterministically without hitting Supabase.
 *
 * All Framer Motion + CSS animations are paused via the Playwright
 * config (`animations: "disabled"`).
 */
import { expect, test } from "@playwright/test";

test("landing page renders", async ({ page }) => {
  await page.goto("/");
  // Wait for fonts so the snapshot uses the served face, not fallback.
  await page.evaluate(() => document.fonts.ready);
  await expect(page).toHaveScreenshot("landing.png", { fullPage: true });
});

test("waitlist form success state renders", async ({ page }) => {
  await page.route("**/api/waitlist", async (route) => {
    await route.fulfill({
      status: 201,
      contentType: "application/json",
      body: JSON.stringify({ position: 848, refCode: "TESTC0DE" }),
    });
  });

  await page.goto("/#waitlist");
  await page.evaluate(() => document.fonts.ready);

  await page.getByLabel(/email/i).fill("test@example.com");
  await page.getByRole("button", { name: /reserve my seat/i }).click();

  await expect(page.getByText(/founding member #848/i)).toBeVisible();
  await expect(page.locator("#waitlist")).toHaveScreenshot("waitlist-success.png");
});
