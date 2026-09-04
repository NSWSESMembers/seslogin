# Playwright examples for the local stack

Two scripts that put a browser into a state worth testing from, so you don't spend the
first ten minutes of every session logging in by hand.

| Script                               | Leaves you at                                                                                        |
| ------------------------------------ | ---------------------------------------------------------------------------------------------------- |
| [`kiosk-scan.mjs`](kiosk-scan.mjs)   | The kiosk scan screen, signed in as the seeded key-enrolled kiosk — ready to sign a member in or out |
| [`admin-login.mjs`](admin-login.mjs) | The admin dashboard, as a chosen user with a chosen location selected                                |

Both assume the local stack is up and seeded:

```bash
make local-e2e
```

Playwright isn't a dependency of this repo. Install it **at the repo root** — these are
ES modules, and Node ignores `NODE_PATH` for ESM, so the only place it can resolve
`playwright` from is a `node_modules` in this directory or one above it. `/node_modules/`
is already gitignored for exactly this:

```bash
npm i playwright --no-save && npx playwright install chromium
node local/examples/admin-login.mjs
```

If the machine already has a Chromium (CI images and sandboxes usually do), set
`CHROMIUM_PATH` instead of downloading one:

```bash
CHROMIUM_PATH=/opt/pw-browsers/chromium-1194/chrome-linux/chrome node local/examples/kiosk-scan.mjs
```

Pair that with `PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1` on the install so npm's postinstall
doesn't fetch a second copy.

Both scripts run headless by default, print what they did, and take a screenshot. Set
`HEADED=1` to watch, and `KEEP_OPEN=1` to leave the browser open so you can carry on by
hand — which is usually the point.

```bash
HEADED=1 KEEP_OPEN=1 node local/examples/kiosk-scan.mjs
```

Options are environment variables so the scripts stay copy-paste-able; see the header of
each. Shared setup lives in [`fixtures.mjs`](fixtures.mjs), which reads the same seed
files the database is loaded from, so the two can't drift.

## Getting to a screen the scripts don't reach

`kiosk-scan.mjs` leaves you ready to sign a member **in**. The sign-out screens — the
category picker, the adjust screen with its time fields, and the forgot-to-sign-out
interstitial — need a member who is already signed in, and clicking through a sign-in
first just to reach them is slow and leaves state behind.

`cli period create-signin` writes that open period directly:

```bash
# Alice, signed in 3 hours ago: scan 10000001 to reach the ordinary sign-out flow.
make local-cli ARGS="period create-signin --person TestAMember1 \
  --location TestAUnit001 --session TestAKiosk02 --hours-ago 3 --dry-run false"

# 13 hours ago instead: crosses the 12h threshold, so the scan hits the
# forgot-to-sign-out interstitial first.
make local-cli ARGS="period create-signin --person TestAMember1 \
  --location TestAUnit001 --session TestAKiosk02 --hours-ago 13 --dry-run false"
```

It writes, so like the rest of `cli` it defaults to a dry run — `--dry-run false` is what
makes it real. The scanning kiosk must belong to the same location or the period is
invisible to it.

`make local-cli ARGS="..."` is `cli` pointed at the local database; without it `cli` reads
`.env` and talks to the dev snapshot in AWS. Anything `cli` can do works this way,
including `session set-config-key`, which is how you flip a kiosk setting without the
admin UI:

```bash
make local-cli ARGS="period list --location TestAUnit001"
make local-cli ARGS="session set-config-key easyTimeEntry true --location TestAUnit001"
```

## Between runs

A script that stops halfway leaves its open period behind, and that changes what the next
run does — scanning a member who is already signed in signs them _out_, not in. `make
local-seed` will not clear it: it only writes the fixture rows back, and no fixture
describes a period.

```bash
make local-clear     # drop periods, ephemeral state and passkeys; keep the fixtures
```

Reach for `make local-reset` only when you want the tables themselves rebuilt.

## Writing your own

Two things about the kiosk cost more time than they should:

- **Every scan screen stays mounted**, parked off-side with a CSS transform, so
  `body.innerText()` returns the text of screens that aren't on display and a loose
  selector (`hasText` plus `.first()`, say) can resolve to an off-screen copy of a
  control. Scope to the screen that is actually centred:
  `page.locator("div.translate-x-0")`.
- **Transitions take 500ms.** `settleKiosk(page)` from `fixtures.mjs` waits for the
  network, the busy text and the animation; plain `settle(page)` covers only the first
  two and is what the admin scripts want.
