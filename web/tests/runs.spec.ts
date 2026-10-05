import { test, expect } from "@playwright/test";

test.describe("Runs and Chat exports", () => {
  test("a task's runs offer Copy run ID and Export report", async ({ page }) => {
    await page.goto("/?demo");
    await page.click("button.found-server");
    await page.click('[data-action="project"][data-id="p-kk"]');
    await page.locator(".tasks .task-main", { hasText: "Profile shader compile times" }).click();
    
    const runs = page.locator("#right .task-runs");
    await expect(runs).toBeVisible();
    
    await expect(runs.locator("li")).toHaveCount(1);
    await expect(runs).toContainText("folder not found");
    
    await expect(runs.locator('[data-action="copy-text"]')).toHaveAttribute("data-text", "run-demo-1");
    await expect(runs.locator("a[download]")).toHaveAttribute("href", "/api/runs/run-demo-1/report?download=1");
  });

  test("a chat can export its report", async ({ page }) => {
    await page.goto("/?demo");
    await page.click("button.found-server");
    await expect(page.locator('#conv-head a[download]')).toHaveAttribute("href", /\/api\/chats\/.+\/report\?download=1/);
  });
});
