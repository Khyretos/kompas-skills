import { test, expect, type Page } from "@playwright/test";

async function openDemo(page: Page): Promise<void> {
  await page.goto("/?demo");
  await page.click("button.found-server");
  await expect(page.locator("#left")).toBeVisible();
}

/** Opens kk-engine in the sidebar (7 tasks: 6 shown, then "1 more"). */
async function openProject(page: Page): Promise<void> {
  await page.click('[data-action="project"][data-id="p-kk"]');
  await expect(page.locator(".project.open .task-line")).toHaveCount(6);
}

test.describe("Tasks in an open project", () => {
  test.beforeEach(async ({ page }) => {
    await openDemo(page);
    await openProject(page);
  });

  test("a task opens the same detail as the Tasks tab", async ({ page }) => {
    const line = page.locator(".project.open .task-line button", { hasText: "Write unit tests for ModuleLoader" });
    await line.click();
    const right = page.locator("#right");
    await expect(right.locator(".task-detail h3")).toHaveText("Write unit tests for ModuleLoader");
    await expect(right.locator('.task-actions [data-action="edit-task"]')).toBeVisible();
    await expect(line).toHaveClass(/active/);
  });

  test("a task opens even with the right panel collapsed or the Assets section open", async ({ page }) => {
    await page.click('.collapse-btn[data-side="right"]');
    await expect(page.locator(".shell")).toHaveAttribute("data-right-collapsed", "true");
    await page.click('[data-action="assets"]');
    // (Done tasks come last: "Update the README build steps" is the one behind "1 more".)
    await page.locator(".project.open .task-line button", { hasText: "Benchmark the renderer at 1440p" }).click();
    await expect(page.locator(".shell")).not.toHaveAttribute("data-right-collapsed", "true");
    await expect(page.locator(".shell")).toHaveAttribute("data-section", "chat");
    await expect(page.locator("#right .task-detail h3")).toHaveText("Benchmark the renderer at 1440p");
  });

  test('"more" shows every task in place and "Show fewer" hides them again', async ({ page }) => {
    let loads = 0;
    page.on("load", () => { loads++; });
    const more = page.locator('.project.open [data-action="project-more"]');
    await expect(more).toHaveText("1 more");
    await more.click();
    await expect(page.locator(".project.open .task-line")).toHaveCount(7);
    await expect(more).toHaveText("Show fewer");
    await expect(more).toHaveAttribute("aria-expanded", "true");
    // The extra (done) task opens too.
    await page.locator(".project.open .task-line button").nth(6).click();
    await expect(page.locator("#right .task-detail h3")).toHaveText("Update the README build steps");
    await more.click();
    await expect(page.locator(".project.open .task-line")).toHaveCount(6);
    expect(loads).toBe(0);
  });
});
