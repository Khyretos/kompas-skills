import { test, expect } from "@playwright/test";

// TEN-05: the task detail shows what a task cost; Activity shows the last 7 days.
test("a task with a cost line shows Coder's share", async ({ page }) => {
  await page.goto("/?demo");
  await page.click("button.found-server");
  await page.click('[data-action="project"][data-id="p-kk"]');
  await page.locator(".tasks .task-main", { hasText: "Profile shader compile times" }).click();
  await expect(page.locator(".task-costs")).toContainText("Coder's share");
  await expect(page.locator(".task-costs")).toContainText("14 %");
});

test("Activity shows the weekly totals", async ({ page }) => {
  await page.goto("/?demo");
  await page.click("button.found-server");
  await page.click('[data-action="tab"][data-tab="activity"]');
  await expect(page.locator(".activity-costs")).toContainText("Last 7 days");
});
