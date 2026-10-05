// Takes the README screenshots from the demo (example data, no server needed).
// Run from web/ with the dev server up (PORT=5174 node build.mjs --serve):
//   KK_URL=http://localhost:5174 node ../docs/screenshots/shoot.mjs
// The sign-in shot comes from the public sign-in page (KK_SIGNIN, optional).
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

// Resolve Playwright from the current folder (web/), not from docs/.
const { chromium } = createRequire(join(process.cwd(), "x.js"))("playwright");
const here = dirname(fileURLToPath(import.meta.url));
const base = process.env.KK_URL ?? "http://localhost:5173";
const signin = process.env.KK_SIGNIN;
const browser = await chromium.launch(process.env.CHROMIUM ? { executablePath: process.env.CHROMIUM } : {});

async function open(opts = {}) {
  const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 1, colorScheme: "dark", ...opts });
  const page = await ctx.newPage();
  await page.goto(base + "/?demo");
  await page.click("button.found-server");
  await page.locator("#left").waitFor();
  await page.evaluate(() => document.fonts.ready);
  return page;
}

async function shot(page, name) {
  await page.waitForTimeout(600);
  await page.screenshot({ path: join(here, name) });
  console.log("wrote", name);
}

if (signin) {
  const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 }, colorScheme: "dark" });
  const page = await ctx.newPage();
  await page.goto(signin);
  await shot(page, "01-sign-in.png");
  await ctx.close();
}

{
  const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 }, colorScheme: "dark" });
  const page = await ctx.newPage();
  await page.goto(base + "/?demo");
  await page.locator("button.found-server").waitFor();
  await shot(page, "02-connect.png");
  await ctx.close();
}

let page = await open();
await shot(page, "03-dashboard.png");

await page.click('[data-action="project"][data-id="p-kk"]');
await page.locator(".tasks .task-main", { hasText: "Profile shader compile times" }).click();
await shot(page, "04-task.png");
await page.click('[data-action="close-task"]');

await page.click('[data-action="tab"][data-tab="machines"]');
await shot(page, "06-machines.png");

await page.click('[data-action="tab"][data-tab="access"]');
await shot(page, "07-access.png");

await page.click('[data-action="tab"][data-tab="activity"]');
await shot(page, "08-activity.png");

await page.click('[data-action="capabilities"]');
await page.locator(".tl-gpu").first().waitFor();
await shot(page, "09-capabilities.png");

await page.click('[data-action="assets"]');
await page.locator(".asset-card:not(.skeleton)").first().waitFor();
await shot(page, "10-assets.png");

await page.click('[data-action="asset-tab"][data-tab="games"]');
await page.locator(".game-list .game-item", { hasText: "KKE Showcase" }).click();
await shot(page, "11-games.png");
await page.context().close();

// A task that runs step by step on a computer (W2), in its own chat.
page = await open();
await page.selectOption("#pc-machine", { label: "soucouyant" });
await page.fill("#prompt", "run three steps");
await page.press("#prompt", "Enter");
await page.locator('#messages details.step[data-state="done"]').nth(2).waitFor({ state: "attached", timeout: 10000 });
// The finished group folds itself shut; open it and its last step for the picture.
await page.evaluate(() => {
  const g = [...document.querySelectorAll("#messages details.steps.group")].pop();
  if (g) g.open = true;
  const last = [...document.querySelectorAll("#messages details.step")].pop();
  if (last) last.open = true;
});
await shot(page, "05-run-steps.png");
await page.context().close();

page = await open();
await page.keyboard.press("Control+K");
await page.locator("#search-input").fill("validation");
await page.locator("#search-results li.search-item").first().waitFor();
await shot(page, "12-search.png");
await page.context().close();

page = await open();
await page.click('[data-action="settings"]');
await page.locator("#settings").waitFor();
// Roles and connected models: any model can take any role.
await page.locator('#settings h3:has-text("Roles")').evaluate((h) => {
  // Scroll only the dialog: scrollIntoView would also move the page behind it.
  let box = h.parentElement;
  while (box && box.scrollHeight <= box.clientHeight) box = box.parentElement;
  if (box) box.scrollTop += h.getBoundingClientRect().top - box.getBoundingClientRect().top - 16;
});
await shot(page, "13-models-and-roles.png");
await page.context().close();

page = await open({ colorScheme: "light" });
await shot(page, "14-light-theme.png");
await page.context().close();

page = await open({ viewport: { width: 390, height: 844 }, deviceScaleFactor: 2, isMobile: true, hasTouch: true });
await shot(page, "15-phone.png");
await page.context().close();

await browser.close();
