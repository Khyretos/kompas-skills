import { test, expect } from "@playwright/test";

test.describe("Stop a running step", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/?demo");
    await page.click("button.found-server");
    await page.selectOption("#pc-machine", { label: "soucouyant" });
    await page.fill("#prompt", "run a long step");
    await page.press("#prompt", "Enter");
  });

  test("a running step shows its elapsed time and stops with Stop", async ({ page }) => {
    const step = page.locator('#messages details.step[data-state="running"]').first();
    await expect(step).toBeVisible();
    await expect(step.locator(".elapsed")).toHaveText(/^\d+:\d\d$/, { timeout: 3000 });
    await step.locator('[data-action="step-stop"]').click();
    const stopped = page.locator('#messages details.step[data-state="stopped"]');
    await expect(stopped).toHaveCount(1);
    await expect(stopped.locator("summary .chip.stopped")).toContainText("stopped by you");
  });

  test("Escape doesn't stop a running step", async ({ page }) => {
    const step = page.locator('#messages details.step[data-state="running"]').first();
    await expect(step).toBeVisible();
    await page.keyboard.press("Escape");
    await page.waitForTimeout(300);
    await expect(page.locator('#messages details.step[data-state="stopped"]')).toHaveCount(0);
  });
});
