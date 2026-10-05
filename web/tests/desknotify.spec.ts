import { test, expect } from "@playwright/test";

test.describe("Desk notifications", () => {
  test.beforeEach(async ({ page }) => {
    await page.addInitScript(() => {
      (window as any).__notes = [];

      class FakeNotification {
        title: string;
        body: string;
        tag?: string;
        onclick: () => void;

        constructor(title: string, options?: { body?: string; tag?: string }) {
          this.title = title;
          this.body = options?.body ?? "";
          this.tag = options?.tag;
          this.onclick = () => {};
          (window as any).__notes.push({ title, body: this.body, tag: this.tag });
        }

        close() {
          // Implementation not needed for test
        }
      }

      (window as any).Notification = FakeNotification;
      (window as any).Notification.permission = "granted";
      (window as any).Notification.requestPermission = async () => "granted";

      (document as any).hasFocus = () => false;
    });

    page.on("dialog", (d) => d.accept()); // "Stop this task?"
    await page.goto("/?demo");
    await page.click("button.found-server");
  });

  test("off by default: no notification", async ({ page }) => {
    await page.click('[data-action="project"][data-id="p-kk"]');
    const taskLine = page.locator(".tasks .task-main", { hasText: "Add a --vk-validation flag and setting" });
    await taskLine.click();

    await page.click('[data-action="task-stop"]');
    await expect(page.locator("#right")).toContainText("stopped by you");

    const notes = await page.evaluate(() => (window as any).__notes ?? []);
    await expect(notes).toEqual([]);
  });

  test("switched on in Settings, a task that needs me raises one notification", async ({ page }) => {
    await page.click('[data-action="settings"]');
    await page.check("#desk-notify");
    await expect(page.locator("#desk-notify-msg")).toHaveText("On for this device.");
    await page.keyboard.press("Escape");

    await page.click('[data-action="project"][data-id="p-kk"]');
    const taskLine = page.locator(".tasks .task-main", { hasText: "Add a --vk-validation flag and setting" });
    await taskLine.click();

    await page.click('[data-action="task-stop"]');
    await expect(page.locator("#right")).toContainText("stopped by you");

    const count = await page.evaluate(() => ((window as any).__notes ?? []).length);
    await expect(count).toBe(1);

    const note = await page.evaluate(() => (window as any).__notes[0]);
    await expect(note.title).toBe("Kompanion: Add a --vk-validation flag and setting");
    await expect(note.body).toBe("Needs you");
  });
});
