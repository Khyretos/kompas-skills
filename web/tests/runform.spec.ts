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

test.describe("Run form folder check", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/?demo");
    await page.click("button.found-server");
    await page.click('[data-action="project"][data-id="p-kk"]');
    await page.locator(".tasks .task-main", { hasText: "Profile shader compile times" }).click();
  });

  test("a folder the computer doesn't have stops Start with a clear message", async ({ page }) => {
    const form = page.locator("form.task-run");
    await form.locator('input[name="folder"]').fill("/home/khyretos/Docker/Personal-projects/kompanion-w2-demo");
    await form.locator('button[type="submit"]').click();
    await expect(form.locator(".folder-check .bad")).toContainText("Folder not found on soucouyant");
    await expect(page.locator("#right .task-detail")).toBeVisible(); // not started: still on the task
  });

  test("Browse lists folders and a click goes into one", async ({ page }) => {
    const form = page.locator("form.task-run");
    await form.locator('[data-action="folder-browse"]').click();
    const list = form.locator(".folder-list");
    await expect(list).toContainText("kees/");
    await list.locator("button", { hasText: "kees/" }).click();
    await list.locator("button", { hasText: "projects/" }).click();
    await expect(form.locator('input[name="folder"]')).toHaveValue("/home/kees/projects");
    await expect(form.locator(".folder-check .good")).toContainText("Found: 2 folders");
  });
});
