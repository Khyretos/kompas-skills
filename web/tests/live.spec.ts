import { test, expect, type Page } from "@playwright/test";

async function openDemo(page: Page): Promise<void> {
  await page.goto("/?demo");
  await page.click("button.found-server");
  await expect(page.locator("#left")).toBeVisible();
}

test.describe("Live updates without reload", () => {
  let loads = 0;

  test.beforeEach(async ({ page }) => {
    loads = 0;
    page.on("load", () => loads++);
    await openDemo(page);
    page.on("dialog", (d) => d.accept());
  });

  test.afterEach(async ({ page }) => {
    expect(loads).toBe(1);
  });

  test("a revoked grant goes pending at once and then disappears", async ({ page }) => {
    const targetText = "/home/kees/projects/kompanion";
    
    await page.click('[data-action="tab"][data-tab="access"]');
    await expect(page.locator('li.grant-row')).toContainText(targetText);

    await page.click(`[data-action="grant-revoke"]`);
    
    await expect(page.locator('li.grant-row.pending', { hasText: targetText })).toBeVisible({ timeout: 300 });
    await expect(page.locator('li.grant-row', { hasText: targetText })).toHaveCount(0);
  });

  test("a new grant shows at once and is confirmed live", async ({ page }) => {
    const targetText = "/home/kees/notes";
    
    await page.click('[data-action="tab"][data-tab="access"]');
    
    const form = page.locator("section.group").filter({ hasText: "soucouyant" }).locator("form.grant-add").first();
    await form.locator('input[name="target"]').fill(targetText);
    await form.locator('button[type="submit"]').click();

    await expect(page.locator('li.grant-row.pending', { hasText: targetText })).toBeVisible({ timeout: 300 });
    await expect(page.locator('li.grant-row:not(.pending)', { hasText: targetText })).toBeVisible();
  });

  test("renaming a pinned chat updates the sidebar at once", async ({ page }) => {
    const buttons = page.locator('[data-action="chat-menu"]');
    expect(await buttons.count()).toBeGreaterThan(0);
    
    const firstButton = buttons.first();
    const id = await firstButton.getAttribute('data-id');
    if (!id) throw new Error("No chat menu button found");

    await firstButton.click();
    await page.click(`[data-action="chat-pin"][data-id="${id}"]`);

    await expect(page.locator(`[data-action="chat-menu"][data-id="${id}"]`).first()).toBeVisible({ timeout: 300 });
    await expect(page.locator('[data-action="chat-pin"]')).toHaveCount(0, { timeout: 300 });
  });
});
