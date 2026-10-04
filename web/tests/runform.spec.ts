import { test, expect } from "@playwright/test";

test("the Run form lists every computer, never hiding one", async ({ page }) => {
  await page.goto("/?demo");
  await page.click("button.found-server");
  await page.click('[data-action="project"][data-id="p-kk"]');
  await page.locator(".tasks .task-main", { hasText: "Profile shader compile times" }).click();
  const options = page.locator('form.task-run select[name="machine"] option');
  await expect(options.filter({ hasText: "kireserver" })).toHaveCount(1);
  await expect(options.filter({ hasText: "soucouyant" })).toHaveCount(1);
});
