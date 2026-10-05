import { test, expect } from "@playwright/test";

test.describe("Global search", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/?demo");
    await page.click("button.found-server");
  });

  test("Ctrl+K opens it, Escape closes it", async ({ page }) => {
    await page.keyboard.press("Control+k");
    await expect(page.locator(".search-box")).toBeVisible();
    await expect(page.locator("#search-input")).toBeFocused();

    await page.keyboard.press("Escape");
    await expect(page.locator(".search-box")).toHaveCount(0);
  });

  test("the header field opens it and a task opens exactly", async ({ page }) => {
    await page.click(".search-field");
    await page.locator("#search-input").fill("vk valid");

    await expect(page.locator("#search-results li.search-item .search-title").first())
      .toHaveText("Add a --vk-validation flag and setting");

    await page.keyboard.press("Enter");
    await expect(page.locator(".search-box")).toHaveCount(0);
    await expect(page.locator("#right")).toContainText("Add a --vk-validation flag and setting");
  });

  test("a message hit scrolls to that message", async ({ page }) => {
    await page.keyboard.press("Control+k");
    await page.locator("#search-input").fill("validation layers");

    const messageItem = page.locator("li.search-item", { has: page.locator(".search-kind", { hasText: "Message" }) }).first();
    await messageItem.click();

    await expect(page.locator("#msg-m1")).toHaveClass(/search-hit/);
    await expect(page.locator("#msg-m1")).toBeInViewport();
  });

  test("a settings section is found by name", async ({ page }) => {
    await page.keyboard.press("Control+k");
    await page.locator("#search-input").fill("notif");

    await expect(page.locator(".search-title").first()).toHaveText("Notifications");

    await page.keyboard.press("Enter");
    await expect(page.locator("#settings h3:has-text(\"Notifications\")")).toBeInViewport();
  });

  test("arrow keys move the selection, nothing found says so", async ({ page }) => {
    await page.keyboard.press("Control+k");
    await page.locator("#search-input").fill("validation");

    await expect.poll(() => page.locator("li.search-item").count()).toBeGreaterThan(1);
    await expect(page.locator("li.search-item").first()).toHaveAttribute("aria-selected", "true");

    await page.keyboard.press("ArrowDown");
    await expect(page.locator("li.search-item").nth(1)).toHaveAttribute("aria-selected", "true");
    await expect(page.locator("li.search-item").first()).toHaveAttribute("aria-selected", "false");

    await page.locator("#search-input").fill("zzqqxx");
    await expect(page.locator(".search-empty")).toHaveText('Nothing found for "zzqqxx"');
  });
});
