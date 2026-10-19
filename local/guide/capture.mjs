/**
 * Capture the admin user guide's screenshots into web/guide/screenshots/.
 *
 *   make local-e2e                       # the local stack, invented data only
 *   node local/guide/capture.mjs         # every shot
 *   node local/guide/capture.mjs kiosks  # only save shots whose name starts "kiosks"
 *
 * The database is reset to the demo state first (reset.sh), so every capture
 * shows the same invented unit, members and history. Shots are defined in
 * shots.mjs; this file is the machinery: a browser that looks like production,
 * highlight rings with labels, and cropping to the part of the screen that matters.
 *
 * Point it elsewhere with GUIDE_APP_URL / GUIDE_PHONE_URL / GUIDE_API_URL (the
 * phone needs a second origin for the same web server; [::1] is one for free).
 */
import { execFileSync } from "node:child_process";
import { cpSync, mkdirSync, readdirSync, rmSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium, devices } from "playwright";
import { SHOTS } from "./shots.mjs";

const HERE = dirname(fileURLToPath(import.meta.url));
const DEST = join(HERE, "..", "..", "web", "guide", "screenshots");
// Captured into a scratch folder and copied over at the end: writing straight into
// web/guide/ would make the dev server live-reload the page mid-capture.
const OUT = join(HERE, "..", ".guide", "screenshots");

export const APP = process.env.GUIDE_APP_URL ?? "http://localhost:5173";
export const PHONE_APP = process.env.GUIDE_PHONE_URL ?? "http://[::1]:5173";
export const API = process.env.GUIDE_API_URL ?? "http://localhost:8000/";
export const ADMIN_TOKEN = "slu_TokenUnitUser.localdev-testunit";

if (
  ![APP, PHONE_APP, API].every((u) =>
    /\/\/(localhost|127\.0\.0\.1|\[::1\])[:/]/.test(u),
  )
) {
  throw new Error("capture.mjs only runs against the local stack");
}

const ORANGE = "#f28c28";

// Runs in every page and frame: the production look, without dev-only chrome.
function presentation() {
  const relabel = () => {
    const walk = document.createTreeWalker(
      document.documentElement,
      NodeFilter.SHOW_TEXT,
    );
    for (let n = walk.nextNode(); n; n = walk.nextNode()) {
      if (n.nodeValue.includes("SES Activity"))
        n.nodeValue = n.nodeValue.replaceAll("SES Activity", "SES Login");
    }
    for (const el of document.querySelectorAll(
      "footer a, footer span, .fixed, [class*='bottom-']",
    )) {
      const t = el.childElementCount === 0 ? el.textContent.trim() : "";
      if (t === "GraphiQL" || t === "dev") el.style.visibility = "hidden";
    }
  };
  new MutationObserver(relabel).observe(document, {
    childList: true,
    subtree: true,
    characterData: true,
  });
  const style = document.createElement("style");
  style.textContent =
    "*,*::before,*::after{transition:none!important;animation:none!important;caret-color:transparent!important}";
  document.addEventListener("DOMContentLoaded", () =>
    document.head.appendChild(style),
  );
}

async function newContext(
  browser,
  { signedIn = true, passkeyPrompt = false, phone = false, width = 1024 } = {},
) {
  const context = await browser.newContext({
    // 1024 wide: still the desktop layout, but tables stay narrow enough to read
    // once the guide scales the picture down.
    ...(phone ? devices["iPhone 13"] : { viewport: { width, height: 800 } }),
    deviceScaleFactor: 2,
    colorScheme: "light",
    timezoneId: "Australia/Sydney",
    locale: "en-AU",
    acceptDownloads: true,
  });
  await context.addInitScript(presentation);
  if (signedIn) {
    await context.addInitScript(
      ([token, prompt]) => {
        if (
          !location.pathname.startsWith("/admin") &&
          !location.pathname.startsWith("/enroll")
        )
          return;
        localStorage.setItem("admin_auth_token", token);
        if (!prompt)
          localStorage.setItem("passkey_enroll_prompt_ts", String(Date.now()));
      },
      [ADMIN_TOKEN, passkeyPrompt],
    );
  }
  // The local database is flagged as non-production, which turns the top bar red
  // with a warning. Report it as production so screenshots match the real site.
  await context.route(`${API}**`, async (route) => {
    const res = await route.fetch();
    await route.fulfill({
      response: res,
      body: (await res.text()).replace(/"isProdDb":false/g, '"isProdDb":true'),
    });
  });
  return context;
}

export async function settle(page) {
  await page.waitForLoadState("networkidle").catch(() => {});
  await page
    .getByText(/^(Loading\.\.\.|Fetching information for )/)
    .first()
    .waitFor({ state: "detached", timeout: 15_000 })
    .catch(() => {});
  await page.waitForTimeout(300);
}

/** Page-coordinate box of a locator (works inside iframes too). */
async function docBox(page, locator) {
  await locator.waitFor({ state: "visible", timeout: 15_000 });
  const b = await locator.boundingBox();
  const { x, y } = await page.evaluate(() => ({ x: scrollX, y: scrollY }));
  return { x: b.x + x, y: b.y + y, width: b.width, height: b.height };
}

function union(boxes) {
  const x = Math.min(...boxes.map((b) => b.x));
  const y = Math.min(...boxes.map((b) => b.y));
  return {
    x,
    y,
    width: Math.max(...boxes.map((b) => b.x + b.width)) - x,
    height: Math.max(...boxes.map((b) => b.y + b.height)) - y,
  };
}

/**
 * Draw highlight rings, each with an optional label, then save a crop around the
 * `frame` elements (default: the highlighted ones) with some breathing room.
 */
export async function snap(
  page,
  name,
  { highlight = [], frame, pad = 24, minWidth = 640 } = {},
) {
  const marks = [];
  for (const h of highlight) {
    const { locator, label, where = "above" } = h.locator ? h : { locator: h };
    // A list of locators is ringed as one group (e.g. a row of buttons).
    const boxes = await Promise.all(
      (Array.isArray(locator) ? locator : [locator]).map((l) =>
        docBox(page, l),
      ),
    );
    marks.push({ box: union(boxes), label, where });
  }
  await page.evaluate(
    ({ marks, orange }) => {
      document.querySelectorAll(".__guide-mark").forEach((n) => n.remove());
      for (const { box, label, where } of marks) {
        const ring = document.createElement("div");
        ring.className = "__guide-mark";
        Object.assign(ring.style, {
          position: "absolute",
          left: `${box.x - 6}px`,
          top: `${box.y - 6}px`,
          width: `${box.width + 12}px`,
          height: `${box.height + 12}px`,
          border: `3px solid ${orange}`,
          borderRadius: "10px",
          boxShadow: "0 0 0 5px rgb(242 140 40 / 0.22)",
          pointerEvents: "none",
          zIndex: 2147483646,
          boxSizing: "border-box",
        });
        document.body.appendChild(ring);
        if (!label) continue;
        const tag = document.createElement("div");
        tag.className = "__guide-mark";
        tag.textContent = label;
        Object.assign(tag.style, {
          position: "absolute",
          left: "0",
          top: "0",
          padding: "3px 10px",
          borderRadius: "999px",
          background: orange,
          color: "#fff",
          font: "700 13px/1.4 system-ui, sans-serif",
          whiteSpace: "nowrap",
          boxShadow: "0 2px 6px rgb(0 0 0 / 0.2)",
          pointerEvents: "none",
          zIndex: 2147483647,
        });
        document.body.appendChild(tag);
        const w = tag.offsetWidth;
        const h = tag.offsetHeight;
        const left =
          where === "right"
            ? box.x + box.width + 14
            : where === "left"
              ? box.x - w - 14
              : box.x - 6;
        const top =
          where === "below"
            ? box.y + box.height + 12
            : where === "right" || where === "left"
              ? box.y + box.height / 2 - h / 2
              : box.y - h - 12;
        // Keep the label on the page; spilling past its edge would widen it.
        tag.style.left = `${Math.max(4, Math.min(left, document.documentElement.clientWidth - w - 4))}px`;
        tag.style.top = `${Math.max(4, top)}px`;
      }
    },
    { marks, orange: ORANGE },
  );

  const frames = frame
    ? Array.isArray(frame)
      ? frame
      : [frame]
    : highlight.flatMap((h) => h.locator ?? h);
  const area = union([
    ...(await Promise.all(frames.map((l) => docBox(page, l)))),
    // Keep labels inside the picture.
    ...(await page.evaluate(() =>
      [...document.querySelectorAll(".__guide-mark")].map((n) => {
        const r = n.getBoundingClientRect();
        return {
          x: r.x + scrollX,
          y: r.y + scrollY,
          width: r.width,
          height: r.height,
        };
      }),
    )),
  ]);
  const doc = await page.evaluate(() => ({
    w: document.documentElement.scrollWidth,
    h: document.documentElement.scrollHeight,
  }));
  let x = area.x - pad;
  let width = area.width + pad * 2;
  if (width < minWidth) {
    x -= (minWidth - width) / 2;
    width = minWidth;
  }
  x = Math.max(0, Math.min(x, doc.w - width));
  width = Math.min(width, doc.w);
  const y = Math.max(0, area.y - pad);
  const height = Math.min(area.height + pad * 2, doc.h - y);

  const file = join(OUT, `${name}.png`);
  await page.screenshot({
    path: file,
    clip: { x, y, width, height },
    fullPage: true,
    animations: "disabled",
  });
  await page.evaluate(() =>
    document.querySelectorAll(".__guide-mark").forEach((n) => n.remove()),
  );
  console.log(`  ${name}.png  ${Math.round(width)}×${Math.round(height)}`);
}

// ── run ─────────────────────────────────────────────────────────────────────

const only = process.argv.slice(2);
rmSync(OUT, { recursive: true, force: true });
mkdirSync(OUT, { recursive: true });
execFileSync(join(HERE, "reset.sh"), {
  stdio: ["ignore", "inherit", "inherit"],
  env: { ...process.env, API_URL: API },
});

const browser = await chromium.launch();
try {
  const ctx = {
    APP,
    PHONE_APP,
    settle,
    snap: (page, name, opts) =>
      !only.length || only.some((p) => name.startsWith(p))
        ? snap(page, name, opts)
        : undefined,
    open: async (path, opts) => {
      const context = await newContext(browser, opts);
      const page = await context.newPage();
      page.on("pageerror", (e) => console.error(`  [pageerror] ${e.message}`));
      if (path) {
        await page.goto(path.startsWith("http") ? path : `${APP}${path}`);
        await settle(page);
      }
      return page;
    },
  };
  // Every scene runs, in order, even when only some shots are being saved: later
  // scenes rely on what earlier ones did (the kiosk enrolled in "kiosks" is the
  // one the Kiosks page then lists).
  for (const scene of SHOTS) {
    console.log(`${scene.name}`);
    await scene.run(ctx);
  }
} finally {
  await browser.close();
}
mkdirSync(DEST, { recursive: true });
for (const file of readdirSync(OUT)) cpSync(join(OUT, file), join(DEST, file));
console.log(
  `Saved ${readdirSync(OUT).length} screenshot(s) to web/guide/screenshots/`,
);
