import { test, expect } from "@playwright/test";

test.describe("Lessons", () => {
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

    // Wait for the lessons pane to appear
    await expect(page.locator("#lessons article.lesson")).toBeVisible({ timeout: 5000 });
  });

  test("an edited lesson is accepted and the thread says where it went", async ({ page }) => {
    const textarea = page.locator("#lessons article.lesson textarea");
    await textarea.fill("Time steps with steady_clock.");
    await page.click("#lessons [data-action=\"lesson-accept\"]");

    await expect(page.locator("#lessons article.lesson")).toHaveCount(0);
    await expect(page.locator("#messages")).toContainText("Lesson added to");
    await expect(page.locator("#messages")).toContainText("worker/cpp-games/SKILL");
  });

  test("a dismissed lesson goes away and the thread says so", async ({ page }) => {
    await expect(page.locator("#lessons article.lesson")).toHaveCount(1);
    await page.click("#lessons [data-action=\"lesson-dismiss\"]");

    await expect(page.locator("#lessons article.lesson")).toHaveCount(0);
    await expect(page.locator("#messages")).toContainText("dismissed");
  });
});
