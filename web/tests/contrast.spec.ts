import { test, expect, type Page } from "@playwright/test";

// Kees, 2026-10-05: "I like my text bright, both in buttons and headers. Do not make me squint."
// Card headers, titles, badges, state chips and buttons need 7:1; secondary text 4.5:1.

async function showCards(page: Page, scheme: "dark" | "light") {
  await page.emulateMedia({ colorScheme: scheme });
  await page.goto("/?demo");
  await page.click("button.found-server");
  await page.selectOption("#pc-machine", { label: "soucouyant" });
  await page.fill("#prompt", "show every card type");
  await page.press("#prompt", "Enter");
  await expect(page.locator(".pc-action")).toHaveCount(6);
}

/** Every text on the approval cards below its minimum contrast, as readable lines. */
function failures(page: Page): Promise<string[]> {
  return page.evaluate(() => {
    type RGBA = [number, number, number, number];
    const parse = (c: string): RGBA => {
      const srgb = /^color\(srgb ([\d.]+) ([\d.]+) ([\d.]+)(?: \/ ([\d.]+))?\)$/.exec(c);
      if (srgb) return [+srgb[1] * 255, +srgb[2] * 255, +srgb[3] * 255, srgb[4] === undefined ? 1 : +srgb[4]];
      const rgb = /^rgba?\(([\d.]+),? ([\d.]+),? ([\d.]+)(?:[,/] ?([\d.]+))?\)$/.exec(c);
      if (rgb) return [+rgb[1], +rgb[2], +rgb[3], rgb[4] === undefined ? 1 : +rgb[4]];
      throw new Error(`unknown colour ${c}`);
    };
    // The colour behind an element: its own background, else its parents', blending transparency.
    const bg = (el: Element | null): [number, number, number] => {
      if (!el) return [255, 255, 255];
      const [r, g, b, a] = parse(getComputedStyle(el).backgroundColor);
      if (a >= 1) return [r, g, b];
      const under = bg(el.parentElement);
      return [r * a + under[0] * (1 - a), g * a + under[1] * (1 - a), b * a + under[2] * (1 - a)];
    };
    const lum = (c: number[]) => {
      const [r, g, b] = c.map((v) => { const x = v / 255; return x <= 0.03928 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4; });
      return 0.2126 * r + 0.7152 * g + 0.0722 * b;
    };
    const ratio = (a: number[], b: number[]) => { const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p); return (x + 0.05) / (y + 0.05); };
    const out: string[] = [];
    const check = (el: Element | null, min: number, what: string) => {
      if (!el) { out.push(`${what}: missing`); return; }
      // Contrast is computed from colours, so a faded ancestor would hide the real problem.
      let opacity = 1;
      for (let e: Element | null = el; e; e = e.parentElement) opacity *= +getComputedStyle(e).opacity;
      if (opacity < 1) { out.push(`${what}: faded to ${Math.round(opacity * 100)} %`); return; }
      const fg = parse(getComputedStyle(el).color);
      const r = ratio(fg, bg(el));
      if (r < min) out.push(`${what}: ${r.toFixed(2)} < ${min} (${el.textContent?.trim().slice(0, 30)})`);
    };
    for (const card of document.querySelectorAll(".pc-action")) {
      const kind = card.querySelector(".card-kind > span:first-child")?.textContent?.trim() ?? "?";
      check(card.querySelector(".card-kind > span:first-child"), 7, `${kind} header`);
      check(card.querySelector(".card-kind .need"), 7, `${kind} badge`);
      check(card.querySelector(".chip"), 7, `${kind} state chip`);
      card.querySelectorAll(".actions .btn").forEach((b) => check(b, 7, `${kind} button`));
      card.querySelectorAll(".muted").forEach((m) => check(m, 4.5, `${kind} secondary`));
    }
    return out;
  });
}

async function setColour(page: Page, kind: string, value: string) {
  await page.locator(`#card-color-${kind}`).evaluate((el, v) => {
    (el as HTMLInputElement).value = v;
    el.dispatchEvent(new Event("input", { bubbles: true }));
    el.dispatchEvent(new Event("change", { bubbles: true }));
  }, value);
}

for (const scheme of ["dark", "light"] as const) {
  test(`every card type is readable (${scheme})`, async ({ page }) => {
    await showCards(page, scheme);
    expect(await failures(page)).toEqual([]);
  });

  test(`custom card colours stay readable (${scheme})`, async ({ page }) => {
    await showCards(page, scheme);
    await page.click('[data-action="settings"]');
    await setColour(page, "read", "#ffffff");
    await setColour(page, "edit", "#ffff00");
    await setColour(page, "run", "#000000");
    await page.keyboard.press("Escape");
    await expect(page.locator("#settings")).toBeHidden();
    await expect(page.locator(".pc-action .card-kind").first()).toHaveCSS("border-left-color", "rgb(255, 255, 255)");
    expect(await failures(page)).toEqual([]);
  });
}
