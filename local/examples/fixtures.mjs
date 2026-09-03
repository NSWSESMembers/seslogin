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

/** Launch a browser, honouring HEADED and CHROMIUM_PATH. */
export async function launch() {
  const browser = await chromium.launch({
    headless: process.env.HEADED !== "1",
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

/**
 * Wait for the page to finish rendering, not just to finish fetching. Relay pages suspend
 * behind a "Loading..." indicator after networkidle, so reading text without this gets you
 * the spinner instead of the page.
 */
export async function settle(page) {
  await page.waitForLoadState("networkidle");
  await page
    .getByText("Loading...", { exact: true })
    .first()
    .waitFor({ state: "detached", timeout: 15_000 })
    .catch(() => {});
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
  for (const url of [BASE_URL, process.env.API_URL ?? "http://localhost:8000"]) {
    try {
      await fetch(url, { method: "GET" });
    } catch {
      throw new Error(
        `${url} is not answering. Start the stack first:\n  make local-e2e`,
      );
    }
  }
}
