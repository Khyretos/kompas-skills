import { test, expect, type Page } from "@playwright/test";

async function openDemo(page: Page): Promise<void> {
  await page.goto("/?demo");
  await page.click("button.found-server");
  await expect(page.locator("#left")).toBeVisible();
}

test.describe("Activity tab demo", () => {
  test.beforeEach(async ({ page }) => {
    await openDemo(page);
    page.on("dialog", (d) => d.accept());
  });

  test("a step and its grant render as one card", async ({ page }) => {
    await page.click('[data-action="tab"][data-tab="activity"]');
    
    // The card details.act-card with text "Edit hyprland.lua" contains the text "allowed once"
    const card = page.locator("details.act-card").filter({ hasText: "Edit hyprland.lua" }).first();
    await expect(card).toBeVisible();
    await expect(card.locator("summary")).toContainText("Edit hyprland.lua");
    await expect(card.locator(".act-grant")).toContainText("allowed once · 10 min · removed after");

    // There is no separate card whose text is just the one-step grant
    await expect(page.locator(".act-grant-card", { hasText: "one step" })).toHaveCount(0);

    await page.screenshot({ path: "test-results/activity-1280.png" });
  });

  test("the standalone always-allow grant has its own compact card", async ({ page }) => {
    await page.click('[data-action="tab"][data-tab="activity"]');
    
    // .act-grant-card with text "always allow" is visible
    await expect(page.locator(".act-grant-card", { hasText: "always allow" })).toBeVisible();
  });

  test("failed only hides the done step", async ({ page }) => {
    await page.click('[data-action="tab"][data-tab="activity"]');
    
    await page.check("#activity-failed");
    await expect(page.locator("details.act-card", { hasText: "Edit hyprland.lua" })).toHaveCount(0);
    
    // "Read /etc/shadow" is visible
    await expect(page.locator("details.act-card", { hasText: "Read /etc/shadow" })).toBeVisible();
  });

  test("clicking a step shows its output", async ({ page }) => {
    await page.click('[data-action="tab"][data-tab="activity"]');
    const card = page.locator("details.act-card", { hasText: "Edit hyprland.lua" });
    await card.locator("summary").click();
    await expect(card.locator("figure.output.diff")).toBeVisible();
  });
});
