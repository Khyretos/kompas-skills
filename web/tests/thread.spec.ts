import { test, expect } from "@playwright/test";

test.describe("Project thread", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/?demo");
    await page.click("button.found-server");
    await page.click('[data-action="project"][data-id="p-kk"]');
    await page.locator(".tasks .task-main", { hasText: "Profile shader compile times" }).click();

    const form = page.locator("form.task-run");
    await form.locator('input[name="folder"]').fill("/home/kees/projects");
    await expect(form.locator(".folder-check .good")).toBeVisible();
    await form.locator('button[type="submit"]').click();
    await page.click('[data-action="open-thread"][data-project="p-kk"]');
    await expect(page.locator("#messages")).toContainText("Started");
    await expect(page.locator("#messages")).toContainText("on kireserver");
  });

  test("a task run posts its updates in the project thread, live", async ({ page }) => {
    await expect(page.locator("#messages")).toContainText("step 2/2 done", { timeout: 5000 });
    await expect(page.locator("#messages")).toContainText("Done:", { timeout: 5000 });
    // Each update arrives once (the live event, not a reload, adds it).
    await expect(page.locator("#messages").getByText("Started Profile shader compile times")).toHaveCount(1);
  });

  test("the Open the task link in the thread opens the task", async ({ page }) => {
    await expect(page.locator("#messages")).toContainText("step 2/2 done", { timeout: 5000 });
    await expect(page.locator("#messages")).toContainText("Done:", { timeout: 5000 });

    await page.locator("#messages a", { hasText: "Open the task" }).click();
    await expect(page.locator("#right .task-detail")).toBeVisible();
    await expect(page.locator("#right .task-detail")).toContainText("Profile shader compile times");
  });
});
