import { test, expect } from "@playwright/test";

test.describe("Right pane scroll stability", () => {
  test.beforeEach(async ({ page }) => {
    await page.setViewportSize({ width: 1400, height: 600 });
    await page.goto("/?demo");
    await page.click("button.found-server");
    await page.locator('[data-action="tab"][data-tab="machines"]').first().click();
  });

  test("the right pane keeps its scroll on live updates while a field there has focus", async ({ page }) => {
    // it guards against focus() scrolling the refocused field into view (the pane jumped to the top every tick)
    await page.locator("#machines-refresh").focus();
    const pane = page.locator("#right .task-groups");
    await pane.evaluate((el) => { el.scrollTop = 300; });
    await page.waitForTimeout(6500); // the demo sends a live machines update every few seconds
    expect(await pane.evaluate((el) => el.scrollTop)).toBeGreaterThan(250);
    await expect(page.locator("#machines-refresh")).toBeFocused();
  });
});
