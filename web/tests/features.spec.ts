import { test, expect, type Page } from "@playwright/test";

// TEN-02: an area switched off in the server's [features] has no menu item and no section.
async function openDemo(page: Page, off: Record<string, boolean> = {}): Promise<void> {
  await page.addInitScript((f) => { (globalThis as { __kkDemoFeatures?: unknown }).__kkDemoFeatures = f; }, off);
  await page.goto("/?demo");
  await page.click("button.found-server");
  await expect(page.locator("#left")).toBeVisible();
}

test("all areas on: Assets in the menu, GPUs on the Capabilities page", async ({ page }) => {
  await openDemo(page);
  await expect(page.locator('[data-action="assets"]')).toBeVisible();
  await page.click('[data-action="capabilities"]');
  await expect(page.locator("#caps h1")).toHaveText("Capabilities");
  await expect(page.locator("#caps")).toContainText("GPUs");
});

test("assets and gpus off: no Assets item, no GPU section", async ({ page }) => {
  await openDemo(page, { assets: false, gpus: false });
  await expect(page.locator('[data-action="assets"]')).toHaveCount(0);
  await page.click('[data-action="capabilities"]');
  await expect(page.locator("#caps h1")).toHaveText("Capabilities");
  await expect(page.locator(".tl-gpu")).toHaveCount(0);
  await expect(page.locator("#caps")).not.toContainText("No GPUs configured");
});
