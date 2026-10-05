import { test, expect } from "@playwright/test";

test.describe("PC Action Cards", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/?demo");
    await page.click("button.found-server");
    await page.selectOption("#pc-machine", { label: "soucouyant" });
    await page.fill("#prompt", "install htop");
    await page.press("#prompt", "Enter");
  });

  test("an approval card has a coloured header by action type and says it needs approval", async ({ page }) => {
    const card = page.locator(".pc-action").last();
    
    // Check text content
    await expect(card.locator(".card-kind")).toContainText("Package / system");
    await expect(card.locator(".card-kind .need")).toHaveText("needs your approval");

    // Check computed styles
    const backgroundColor = await card.locator(".card-kind").evaluate((el) => 
      getComputedStyle(el).backgroundColor
    );
    const color = await card.locator(".card-kind").evaluate((el) => 
      getComputedStyle(el).color
    );

    expect(backgroundColor).toBe("rgb(191, 78, 255)");
    expect(color).toBe("rgb(12, 9, 23)");
  });

  test("a colour changed in Settings shows at once", async ({ page }) => {
    // Open settings
    await page.click('[data-action="settings"]');
    await expect(page.locator("#settings")).toBeVisible();

    // Fill the label field (triggers change event)
    await page.fill("#card-label-system", "System");
    await page.keyboard.press("Tab");

    // Close settings
    await page.keyboard.press("Escape");
    await expect(page.locator("#settings")).toBeHidden();

    // Verify the card header updated immediately
    await expect(page.locator(".pc-action").last().locator(".card-kind")).toContainText("System");

    // Re-open settings and reset
    await page.click('[data-action="settings"]');
    await expect(page.locator("#settings")).toBeVisible();
    await page.click('[data-action="card-reset"]');
    await page.keyboard.press("Escape");
    await expect(page.locator("#settings")).toBeHidden();

    // Verify the original text is back
    await expect(page.locator(".pc-action").last().locator(".card-kind")).toContainText("Package / system");
  });
});
