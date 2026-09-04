/**
 * Shared setup for the example scripts.
 *
 * Everything here is read from `local/seed/`, the same files `make local-seed` writes into
 * the database, so a script and the database cannot disagree about a token or a key.
 */
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { chromium } from "playwright";

const HERE = dirname(fileURLToPath(import.meta.url));
const SEED = join(HERE, "..", "seed");

const readJson = (p) => JSON.parse(readFileSync(p, "utf8"));

export const BASE_URL = process.env.BASE_URL ?? "http://localhost:5173";
export const kioskKey = readJson(join(SEED, "kiosk-signing-key.json"));

/** The seeded user tokens, by email. Documented in DEVELOPMENT.md §9. */
export const USER_TOKENS = {
  "super@seslogin.test": "slu_localdev0000000000000000000super",
  "testunit@seslogin.test": "slu_localdev0000000000000000testunit",
};

// Chromium phones home on startup — component updates, the optimisation-hints
// service, translate, account sync. On a normal workstation those resolve or fail
// fast and nobody notices. Behind an egress proxy that blackholes them (CI images,
// Claude Code sandboxes, locked-down corporate networks) they hang instead, and
// `networkidle` below then never fires: the script sits there until it times out,
// with nothing on screen to say why. Turning them off costs nothing locally and is
// the difference between working and not working everywhere else.
const QUIET_STARTUP_ARGS = [
  "--disable-background-networking",
  "--disable-component-update",
  "--disable-sync",
  "--no-first-run",
  "--disable-features=Translate,OptimizationHints",
];

/** Launch a browser, honouring HEADED and CHROMIUM_PATH. */
export async function launch() {
  const browser = await chromium.launch({
    headless: process.env.HEADED !== "1",
    args: QUIET_STARTUP_ARGS,
    ...(process.env.CHROMIUM_PATH
      ? { executablePath: process.env.CHROMIUM_PATH }
      : {}),
  });
  const page = await browser.newPage({
    viewport: { width: 1280, height: 900 },
  });
  page.on("pageerror", (e) => console.error(`[pageerror] ${e.message}`));
  page.on("console", (m) => {
    if (m.type() === "error") console.error(`[console] ${m.text()}`);
  });
  return { browser, page };
}

// The two things the app puts on screen while it is still working. The admin side
// suspends behind "Loading..."; the kiosk shows "Fetching information for <id>"
// while it resolves a scanned member (ScanScreenMain). Waiting only for the first
// means every kiosk script reads the page mid-flight and screenshots the spinner.
const BUSY_TEXT = /^(Loading\.\.\.|Fetching information for )/;

/**
 * Wait for the page to finish rendering, not just to finish fetching. Relay pages suspend
 * behind a busy indicator after networkidle, so reading text without this gets you the
 * spinner instead of the page.
 */
export async function settle(page) {
  await page.waitForLoadState("networkidle");
  await page
    .getByText(BUSY_TEXT)
    .first()
    .waitFor({ state: "detached", timeout: 15_000 })
    .catch(() => {});
}

/**
 * Wait out a kiosk screen transition. The scan screens are 500ms sliding panels that
 * all stay mounted, so for half a second after a tap the screen you want is still
 * moving and the one you left is still on top of it. `settle` covers the network and
 * the busy text but knows nothing about the animation.
 */
export async function settleKiosk(page) {
  await settle(page);
  await page.waitForTimeout(700);
}

/**
 * Close, or hold the browser open when KEEP_OPEN=1 so you can keep poking at the state
 * the script set up — which is usually why you ran it.
 */
export async function finish(browser, page, screenshotPath) {
  if (screenshotPath) {
    await page.screenshot({ path: screenshotPath, fullPage: true });
    console.log(`screenshot: ${screenshotPath}`);
  }
  if (process.env.KEEP_OPEN === "1") {
    console.log("KEEP_OPEN=1 — leaving the browser open. Ctrl-C to quit.");
    await new Promise(() => {});
  }
  await browser.close();
}

/** Fail loudly and early rather than after a confusing timeout deep in a script. */
export async function requireStack() {
  for (const url of [
    BASE_URL,
    process.env.API_URL ?? "http://localhost:8000",
  ]) {
    try {
      await fetch(url, { method: "GET" });
    } catch {
      throw new Error(
        `${url} is not answering. Start the stack first:\n  make local-e2e`,
      );
    }
  }
}
