import { test, expect, type Page } from "@playwright/test";

async function openDemo(page: Page): Promise<void> {
  await page.goto("/?demo");
  await page.click("button.found-server");
  await expect(page.locator("#left")).toBeVisible();
}

async function setting(page: Page, id: string, value?: string): Promise<void> {
  await page.click('[data-action="settings"]');
  const field = page.locator(`#${id}`);
  if (value === undefined) await field.check(); else await field.selectOption(value);
  await page.keyboard.press("Escape");
  await expect(page.locator("#settings")).toBeHidden();
}

test.describe("Voice (W4)", () => {
  let loads = 0;

  test.beforeEach(async ({ page }) => {
    loads = 0;
    page.on("load", () => { loads++; });
    await openDemo(page);
  });

  test.afterEach(() => {
    expect(loads).toBe(1); // nothing needed a refresh
  });

  test("the microphone button is off until switched on, then shows at once", async ({ page }) => {
    await expect(page.locator("#voice-mic")).toBeHidden();
    await setting(page, "voice-input");
    await expect(page.locator("#voice-mic")).toBeVisible();
  });

  test("a click records, a second click writes it into the message box without sending", async ({ page }) => {
    await setting(page, "voice-input");
    const mic = page.locator("#voice-mic");
    const before = await page.locator(".msg").count();
    await mic.click();
    await expect(mic).toHaveAttribute("aria-pressed", "true");
    await expect(page.locator("#voice-status")).toHaveText("Listening…");
    await page.waitForTimeout(600); // some fake microphone audio
    await mic.click();
    await expect(page.locator("#prompt")).toHaveValue("install htop");
    await expect(mic).toHaveAttribute("aria-pressed", "false");
    await expect(page.locator("#voice-status")).toHaveText("");
    expect(await page.locator(".msg").count()).toBe(before);
  });

  test("holding the button records until it is let go", async ({ page }) => {
    await setting(page, "voice-input");
    const mic = page.locator("#voice-mic");
    await mic.hover();
    await page.mouse.down();
    await expect(mic).toHaveAttribute("aria-pressed", "true");
    await page.waitForTimeout(700);
    await page.mouse.up();
    await expect(page.locator("#prompt")).toHaveValue("install htop");
  });

  test("replies are read aloud when switched on, and Stop reading stops", async ({ page }) => {
    await setting(page, "voice-read");
    await page.fill("#prompt", "hello");
    await page.press("#prompt", "Enter");
    const stop = page.locator("#voice-stop");
    await expect(stop).toBeVisible();
    await stop.click();
    await expect(stop).toBeHidden();
  });

  test("Dutch stays text only and Settings says so", async ({ page }) => {
    await page.click('[data-action="settings"]');
    await page.locator("#voice-read").check();
    await page.locator("#voice-lang").selectOption("nl");
    await expect(page.locator(".voice-settings .warn")).toContainText("no Dutch reading voice");
    await page.keyboard.press("Escape");
    await page.fill("#prompt", "hallo");
    await page.press("#prompt", "Enter");
    await page.waitForTimeout(1500);
    await expect(page.locator("#voice-stop")).toBeHidden();
  });
});
