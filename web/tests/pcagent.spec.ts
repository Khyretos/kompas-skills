import { test, expect, type Page } from "@playwright/test";

async function openDemo(page: Page): Promise<void> {
  await page.goto("/?demo");
  await page.click("button.found-server");
  await expect(page.locator("#left")).toBeVisible();
}

async function ask(page: Page): Promise<void> {
  await page.selectOption("#pc-machine", { label: "soucouyant" });
  await page.fill("#prompt", "install htop");
  await page.press("#prompt", "Enter");
}

test.describe("PC Agent approval cards", () => {
  test.beforeEach(async ({ page }) => {
    await openDemo(page);
    page.on("dialog", (d) => d.accept());
  });

  test("a step on a computer without grants shows a card that says what it needs", async ({ page }) => {
    await ask(page);
    
    const card = page.locator(".pc-action").last();
    await expect(card).toBeVisible();
    await expect(card.locator("text=install htop with paru")).toContainText("install htop with paru");
    await expect(card.locator("text=Needs: packages + root")).toContainText("Needs: packages + root");
  });

  test("Approve runs the step once and leaves no grant", async ({ page }) => {
    await ask(page);
    
    const card = page.locator(".pc-action").last();
    await card.locator("[data-decision=\"approve\"]').click();
    
    await expect(card.locator(".chip")).toContainText("done");
    
    await page.click('[data-action="tab"][data-tab="access"]');
    const section = page.locator("section.group", { hasText: "soucouyant" });
    const count = await section.locator("li.grant-row", { hasText: "system" }).count();
    await expect(count).toBe(0);
  });

  test("Always allow leaves a 24 h grant", async ({ page }) => {
    await ask(page);
    
    const card = page.locator(".pc-action").last();
    await card.locator("[data-decision=\"always\"]').click();
    
    await expect(card.locator(".chip")).toContainText("done");
    
    await page.click('[data-action="tab"][data-tab="access"]');
    const section = page.locator("section.group", { hasText: "soucouyant" });
    const row = section.locator("li.grant-row").filter({ hasText: "system" }).filter({ hasText: "packages" });
    await expect(row).toHaveCount(1);
  });

  test("Deny declines", async ({ page }) => {
    await ask(page);
    
    const card = page.locator(".pc-action").last();
    await card.locator("[data-decision=\"deny\"]').click();
    
    await expect(card.locator(".chip")).toContainText("declined");
  });
});
