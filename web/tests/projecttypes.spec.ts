import { test, expect, type Page } from "@playwright/test";

async function openDemo(page: Page): Promise<void> {
  await page.goto("/?demo");
  await page.click("button.found-server");
  await expect(page.locator("#left")).toBeVisible();
}

/** kk-engine selected: its Tasks tab shows the project panel. */
async function openProject(page: Page): Promise<void> {
  await page.click('[data-action="project"][data-id="p-kk"]');
  await expect(page.locator("#project-type")).toBeVisible();
}

test.describe("Project types", () => {
  let loads = 0;

  test.beforeEach(async ({ page }) => {
    loads = 0;
    page.on("load", () => { loads++; });
    await openDemo(page);
    await openProject(page);
  });

  test.afterEach(() => {
    expect(loads).toBe(1); // only the first page load: nothing needed a refresh
  });

  test("a new project is a chat project without extra settings", async ({ page }) => {
    await expect(page.locator("#project-type")).toHaveValue("chat");
    await expect(page.locator("form.project-repo")).toHaveCount(0);
    await expect(page.locator(".asset-picker")).toHaveCount(0);
  });

  test("a programming project's repo becomes the default for Run it on a computer", async ({ page }) => {
    await page.selectOption("#project-type", "programming");
    const form = page.locator("form.project-repo");
    await expect(form).toBeVisible();
    await form.locator('select[name="machine"]').selectOption({ label: "soucouyant" });
    await form.locator('input[name="folder"]').fill("/home/kees/projects/kk-engine/");
    await form.locator('button[type="submit"]').click();
    await expect(form.locator('input[name="folder"]')).toHaveValue("/home/kees/projects/kk-engine");
    // A task of this project starts with that computer and folder.
    await page.locator(".tasks .task-main", { hasText: "Profile shader compile times" }).click();
    const run = page.locator("form.task-run");
    await expect(run.locator('input[name="folder"]')).toHaveValue("/home/kees/projects/kk-engine");
    await expect(run.locator('select[name="machine"]')).toHaveValue("soucouyant");
  });

  test("a game project attaches assets from the library and detaches them", async ({ page }) => {
    await page.selectOption("#project-type", "game");
    await expect(page.locator(".project-panel")).toContainText("No assets yet");
    const q = page.locator("#asset-pick-q");
    await q.pressSequentially("Tree_01");
    await expect(q).toHaveValue("Tree_01"); // the caret stayed put while the panel re-rendered
    const hit = page.locator(".pick-results li", { hasText: "SM_Env_Tree_01.fbx" });
    await hit.locator('[data-action="attach-asset"]').click();
    const card = page.locator(".project-assets li.project-asset", { hasText: "SM_Env_Tree_01.fbx" });
    await expect(card).toBeVisible();
    await expect(hit.locator("button")).toHaveText("Attached");
    await expect(page.locator(".project-panel h3 .count")).toHaveText("1");
    await card.locator('[data-action="detach-asset"]').click();
    await expect(card).toHaveCount(0);
    await expect(hit.locator('[data-action="attach-asset"]')).toBeVisible();
  });
});
