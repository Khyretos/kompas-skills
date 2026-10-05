import { test, expect } from "@playwright/test";

test.describe("Step groups keep the user's choice", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/?demo");
    await page.click("button.found-server");
    await page.selectOption("#pc-machine", { label: "soucouyant" });
    await page.fill("#prompt", "run three steps");
    await page.press("#prompt", "Enter");
  });

  test("a group the user opened stays open while new steps land", async ({ page }) => {
    const group = page.locator("#messages details.steps.group").last();
    await expect(group).toBeVisible();
    await expect(group).toHaveAttribute("open", ""); // open while a step runs
    // Close and reopen it: now it is the user's choice.
    await group.locator("> summary").click();
    await group.locator("> summary").click();
    await expect(group).toHaveAttribute("open", "");
    // All three finish (each one re-renders the chat); it stays open.
    await expect(group.locator('details.step[data-state="done"]')).toHaveCount(3, { timeout: 8000 });
    await expect(group).toHaveAttribute("open", "");
  });

  test("a group the user closed stays closed, even while steps run", async ({ page }) => {
    const group = page.locator("#messages details.steps.group").last();
    await expect(group).toHaveAttribute("open", "");
    await group.locator("> summary").click();
    await expect(group).not.toHaveAttribute("open", "");
    await expect(group.locator('details.step[data-state="done"]')).toHaveCount(3, { timeout: 8000 });
    await expect(group).not.toHaveAttribute("open", "");
  });
});
