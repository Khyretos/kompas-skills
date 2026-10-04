// Assets section in demo mode: filtering, the detail panel and a live scan all
// update the page without a reload (Kees's standing rule).
import { test, expect, type Page } from "@playwright/test";

async function openAssets(page: Page): Promise<void> {
  await page.goto("/?demo");
  await page.click("button.found-server");
  await page.click('[data-action="assets"]');
  await expect(page.locator(".asset-card:not(.skeleton)").first()).toBeVisible();
}

test.describe("Assets", () => {
  let loads = 0;
  test.beforeEach(async ({ page }) => {
    loads = 0;
    page.on("load", () => loads++);
    await openAssets(page);
  });
  test.afterEach(() => expect(loads).toBe(1));

  test("only the visible cards are in the page", async ({ page }) => {
    await expect(page.locator("#asset-summary")).toContainText("files in");
    const total = Number((await page.locator('[data-action="asset-cat"][data-cat=""] span').textContent())?.replace(/,/g, ""));
    expect(total).toBeGreaterThan(500);
    expect(await page.locator(".asset-card").count()).toBeLessThan(total / 2);
    // Scrolling far down draws the cards that are now in view.
    await page.locator("#asset-scroll").evaluate((el) => { el.scrollTop = el.scrollHeight; });
    await expect(page.locator(".asset-card", { hasText: "PolygonNature_Texture_40" }).first()).toBeVisible();
  });

  test("a category chip and the search box filter at once", async ({ page }) => {
    await page.click('[data-action="asset-cat"][data-cat="music"]');
    await expect(page.locator('[data-action="asset-cat"][data-cat="music"]')).toHaveAttribute("aria-pressed", "true");
    await expect(page.locator(".asset-card:not(.skeleton)").first()).toContainText("Music");
    await expect(page.locator(".asset-card.cat-3d-model")).toHaveCount(0);
    await page.fill("#asset-q", "town");
    await expect(page.locator('[data-action="asset-cat"][data-cat="music"] span')).toHaveText("40");
    await expect(page.locator(".asset-card:not(.skeleton)").first()).toContainText("Town_01");
    await page.fill("#asset-q", "nothing-like-this");
    await expect(page.locator(".asset-empty")).toContainText("Nothing matches");
    await page.click('[data-action="asset-clear"]');
    await expect(page.locator('[data-action="asset-cat"][data-cat=""]')).toHaveAttribute("aria-pressed", "true");
  });

  test("a card opens its details with the licence state", async ({ page }) => {
    await page.fill("#asset-q", "fantasy action 01");
    await page.locator(".asset-card", { hasText: "Action_01" }).click();
    const panel = page.locator("#asset-detail");
    await expect(panel).toBeVisible();
    await expect(panel).toContainText("Not cleared to ship");
    await expect(panel).toContainText("LICENSE.pdf");
    // The pack link filters the grid to that pack.
    await panel.locator('[data-action="asset-pack"]').click();
    await expect(panel).toBeHidden();
    await expect(page.locator("#asset-pack")).not.toHaveValue("");
    await page.keyboard.press("Escape");
  });

  test("a scan shows progress and its result arrives live", async ({ page }) => {
    await expect(page.locator("#asset-pack option", { hasText: "Free Ambience Loops" })).toHaveCount(0);
    await page.click('[data-action="asset-scan"]');
    await expect(page.locator(".scan-live")).toBeVisible({ timeout: 300 });
    await expect(page.locator(".scan-live")).toBeHidden({ timeout: 5000 });
    await expect(page.locator("#asset-pack option", { hasText: "Free Ambience Loops" })).toHaveCount(1);
    await expect(page.locator("#asset-summary")).toContainText("just now");
  });

  test("going back to a chat leaves the Assets section", async ({ page }) => {
    await page.locator('[data-action="open-chat"]').first().click();
    await expect(page.locator("#assets")).toBeHidden();
    await expect(page.locator("#messages")).toBeVisible();
    await page.click('[data-action="assets"]');
    await expect(page.locator(".asset-card:not(.skeleton)").first()).toBeVisible();
  });
});
