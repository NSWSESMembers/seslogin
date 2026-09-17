/**
 * Drive the kiosk through a sign-in and a sign-out (with quick pick and the
 * adjust screen) and capture screenshots at three points:
 *   1. the scan home screen right after a member signs in
 *   2. the quick pick list, reached partway through a sign-out
 *   3. the adjust screen (start/end time + category confirmation)
 *
 * Quick pick only offers suggestions once the location has a recent
 * completed period with a category, so this signs Bob in and out first
 * (building that history via the ordinary category tree, since quick pick
 * has nothing to suggest yet) before signing Alice in and out, whose
 * sign-out is what reaches the quick pick screen.
 *
 *   node local/examples/kiosk-screenshots.mjs
 *
 * Environment: same as kiosk-scan.mjs (PROFILE, BASE_URL, HEADED, KEEP_OPEN,
 * CHROMIUM_PATH).
 */
import {
  BASE_URL,
  kioskKey,
  launch,
  finish,
  settleKiosk,
  requireStack,
} from "./fixtures.mjs";

const PROFILE = process.env.PROFILE ?? kioskKey.profile;
const BOB = "10000002";
const ALICE = "10000001";

await requireStack();
const { browser, page } = await launch();

await page.goto(`${BASE_URL}/kiosk/${PROFILE}`);

await page.evaluate(
  async ({ profile, privateKeyB64, publicKeyB64 }) => {
    const bytes = (b64) =>
      Uint8Array.from(atob(b64), (c) => c.charCodeAt(0)).buffer;
    const algorithm = { name: "ECDSA", namedCurve: "P-256" };

    const privateKey = await crypto.subtle.importKey(
      "pkcs8",
      bytes(privateKeyB64),
      algorithm,
      false,
      ["sign"],
    );
    const publicKey = await crypto.subtle.importKey(
      "spki",
      bytes(publicKeyB64),
      algorithm,
      true,
      ["verify"],
    );

    await new Promise((resolve, reject) => {
      const req = indexedDB.open("seslogin-kiosk", 1);
      req.onupgradeneeded = () => req.result.createObjectStore("keys");
      req.onerror = () => reject(req.error);
      req.onsuccess = () => {
        const db = req.result;
        const tx = db.transaction("keys", "readwrite");
        tx.objectStore("keys").put({ privateKey, publicKey }, profile);
        tx.oncomplete = () => resolve();
        tx.onerror = () => reject(tx.error);
      };
    });

    const key = `kiosk_${profile}`;
    const settings = JSON.parse(localStorage.getItem(key) ?? "{}");
    localStorage.setItem(
      key,
      JSON.stringify({
        ...settings,
        authMode: "key",
        scanAuthToken: null,
        scanAuthTokenIssuedAt: null,
      }),
    );
  },
  {
    profile: PROFILE,
    privateKeyB64: kioskKey.private_key_pkcs8_b64,
    publicKeyB64: kioskKey.public_key_spki_b64,
  },
);

await page.goto(`${BASE_URL}/kiosk/${PROFILE}`, { waitUntil: "networkidle" });
await settleKiosk(page);

const centered = () => page.locator("div.translate-x-0");

async function scan(memberId) {
  const screen = centered();
  await screen.locator("input[name=id]").fill(memberId);
  await screen.locator("input[name=id]").press("Enter");
  await settleKiosk(page);
}

async function pickAnyCategory() {
  // Categories screen: first click drills into a top-level category, second
  // click selects a leaf from what it reveals.
  let screen = centered();
  await screen.locator("li button").first().click();
  await settleKiosk(page);
  screen = centered();
  await screen.locator("li button").first().click();
  await settleKiosk(page);
}

// 1. Sign Bob in fresh, and capture the home screen's sign-in confirmation.
console.log(`Signing in ${BOB} (Bob)...`);
await scan(BOB);
await page.screenshot({ path: "/tmp/kiosk-home-after-signin.png" });
console.log("screenshot: /tmp/kiosk-home-after-signin.png");

// 2. Sign Bob back out via the ordinary category tree (no quick-pick history
//    exists yet) so the location has a completed, categorized period —
//    that's what quick pick needs to have something to suggest next.
console.log(`Signing out ${BOB} (Bob) via the category tree...`);
await scan(BOB);
await pickAnyCategory();
await centered()
  .getByRole("button", { name: /submit/i })
  .click();
await settleKiosk(page);

// 3. Sign Alice in fresh.
console.log(`Signing in ${ALICE} (Alice)...`);
await scan(ALICE);

// 4. Sign Alice back out — the location now has Bob's recent category, so
//    quick pick has a suggestion to show.
console.log(`Signing out ${ALICE} (Alice) — expecting quick pick...`);
await scan(ALICE);

const quickPickHeading = centered().getByText("Quick pick", { exact: true });
await quickPickHeading.waitFor({ state: "visible", timeout: 10_000 });
await page.screenshot({ path: "/tmp/kiosk-quick-pick.png" });
console.log("screenshot: /tmp/kiosk-quick-pick.png");

// 5. Pick the first quick-pick suggestion, landing on the adjust screen.
await centered().locator("li button").first().click();
await settleKiosk(page);
await centered().getByText("Confirm", { exact: true }).waitFor({
  state: "visible",
  timeout: 10_000,
});
await page.screenshot({ path: "/tmp/kiosk-adjust-screen.png" });
console.log("screenshot: /tmp/kiosk-adjust-screen.png");

await finish(browser, page);
