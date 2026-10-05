import { test, expect } from "@playwright/test";

test.describe("Step groups keep the user's choice", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/?demo");
    await page.click("button.found-server");
    await page.selectOption("#pc-machine", { label: "soucouyant" });
    await page.fill("#prompt", "run three steps");
    await page.press("#prompt", "Enter");
  });

  // The message re-renders many times a second while steps stream in, so Playwright's
  // click() waits for a still element and can land after the steps are done (the group
  // then closed by itself). These tests read the state and click in one step instead.
  const toggle = (page: import("@playwright/test").Page) => page.evaluate(() => {
    const d = [...document.querySelectorAll<HTMLDetailsElement>("#messages details.steps.group")].pop()!;
    const was = d.open;
    d.querySelector<HTMLElement>(":scope > summary")!.click();
    return was;
  });

  test("a group the user opened stays open while new steps land", async ({ page }) => {
    const group = page.locator("#messages details.steps.group").last();
    await expect(group).toBeVisible();
    // Open it by hand (two toggles if it is open now): from then on it is the user's choice.
    if (await toggle(page) === false) {
      await expect(group).toHaveAttribute("open", "");
    } else {
      await expect(group).not.toHaveAttribute("open", "");
      expect(await toggle(page)).toBe(false);
      await expect(group).toHaveAttribute("open", "");
    }
    // All three finish (each one re-renders the chat); it stays open.
    await expect(group.locator('details.step[data-state="done"]')).toHaveCount(3, { timeout: 8000 });
    await expect(group).toHaveAttribute("open", "");
  });

  test("a group the user closed stays closed, even while steps run", async ({ page }) => {
    const group = page.locator("#messages details.steps.group").last();
    await expect(group).toHaveAttribute("open", "");
    // Close it by hand (two toggles if it already closed itself).
    if (await toggle(page) === false) expect(await toggle(page)).toBe(true);
    await expect(group).not.toHaveAttribute("open", "");
    await expect(group.locator('details.step[data-state="done"]')).toHaveCount(3, { timeout: 8000 });
    await expect(group).not.toHaveAttribute("open", "");
  });

  test("a click survives a re-render between press and release", async ({ page }) => {
    const group = page.locator("#messages details.steps.group").last();
    await expect(group).toHaveAttribute("open", "");
    // Press on the summary, replace it with a copy (what a live re-render does), release on the copy:
    // the browser sends no click then, so the app has to act on the release.
    await page.evaluate(() => {
      const old = [...document.querySelectorAll<HTMLElement>("#messages details.steps.group > summary")].pop()!;
      const opts = { bubbles: true, button: 0, pointerId: 1, isPrimary: true };
      old.dispatchEvent(new PointerEvent("pointerdown", opts));
      const copy = old.cloneNode(true) as HTMLElement;
      old.replaceWith(copy);
      copy.dispatchEvent(new PointerEvent("pointerup", opts));
    });
    await expect(group).not.toHaveAttribute("open", "");
  });
});
