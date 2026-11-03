/**
 * Dev-server-only toolbar of shortcuts for the local stack (DEVELOPMENT.md §10).
 * Mounted by App under `import.meta.env.DEV`, so production builds never contain it.
 */
import { Fragment, useCallback, useEffect, useState } from "react";
import kioskKey from "../../../local/seed/kiosk-signing-key.json";
import { cn, tw } from "../lib/tw";
import { getAdminToken, setAdminToken } from "../lib/adminToken";
import { getGraphQLEndpoint } from "../lib/api";
import { getCurrentEnvironment } from "../lib/clientInfo";
import { useEnvironmentInfo } from "../lib/environmentInfo";
import {
  forcePasskeyEnrollPrompt,
  markPasskeyEnrollPromptShown,
} from "../lib/passkey";
import { clearKioskKeys, storeKioskKey } from "../kiosk/lib/kioskKey";
import { markKeyEnrolled } from "../kiosk/lib/kioskSettings";

// Seeded logins; api/tests/seed_fixtures.rs checks each against its fixture hash.
const SEEDED_USERS = [
  ["Super", "slu_TokenSuperUsr.localdev-super"],
  ["Unit Admin", "slu_TokenUnitUser.localdev-testunit"],
  ["Read Only", "slu_TokenReadOnly.localdev-readonly"],
] as const;

// dynamodb-admin from local/docker-compose.yml
const DB_BROWSER_URL = "http://localhost:8101";

const OPEN_KEY = "devbar_open";
const THEME_KEY = "devbar_theme";
const THEMES = ["Auto", "Light", "Dark"] as const;
type Theme = (typeof THEMES)[number];

/** Restore the seeded tokens first: Log out deletes the one it used. */
async function signInAs(token: string) {
  await devApi("tokens");
  setAdminToken(token);
  markPasskeyEnrollPromptShown();
  window.location.assign("/admin");
}

/** Install the seeded kiosk key (TestAKiosk02) and open its kiosk, as local/examples/kiosk-scan.mjs does. */
async function openSeededKiosk() {
  const bytes = (b64: string) =>
    Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
  const algorithm = { name: "ECDSA", namedCurve: "P-256" };
  await storeKioskKey(kioskKey.profile, {
    privateKey: await crypto.subtle.importKey(
      "pkcs8",
      bytes(kioskKey.private_key_pkcs8_b64),
      algorithm,
      false,
      ["sign"],
    ),
    publicKey: await crypto.subtle.importKey(
      "spki",
      bytes(kioskKey.public_key_spki_b64),
      algorithm,
      true,
      ["verify"],
    ),
  });
  markKeyEnrolled(kioskKey.profile);
  window.location.assign(`/kiosk/${kioskKey.profile}`);
}

/** Call poem-local's `/dev` routes (api/src/local_dev.rs); they 404 under `make dev`. */
async function devApi(path: string, method = "POST"): Promise<string> {
  const resp = await fetch(new URL(`dev/${path}`, getGraphQLEndpoint()), {
    method,
  });
  const text = await resp.text();
  if (!resp.ok) throw new Error(`${resp.status} ${text}`);
  return text;
}

/** Pull `pattern`'s first group (or whole match) out of command output, or fail with it. */
function extract(text: string, pattern: RegExp): string {
  const m = pattern.exec(text);
  if (!m) throw new Error(`Unexpected output:\n${text}`);
  return m[1] ?? m[0];
}

/** Alice signed in at the seeded kiosk, backdated; returns the command output. */
const aliceSignin = (hoursAgo: number) =>
  devApi(
    `signin?person=TestAMember1&location=TestAUnit001&session=${kioskKey.session_id}&hours_ago=${hoursAgo}`,
  );

/** Open a member edit link, as the Remind email carries, for a fresh open period of Alice's. */
async function openEditLink() {
  const id = extract(await aliceSignin(1), /created period (\w+)/);
  const link = extract(
    await devApi(`period-link?period=${id}`),
    /\S+\/period#\S+/,
  );
  window.open(link, "_blank", "noopener");
  return `Opened Alice's edit link for period ${id} (signed in 1h ago):\n${link}`;
}

/** Copy this browser's admin token as GraphiQL headers, then open GraphiQL. */
async function openGraphiql() {
  const headers = JSON.stringify({
    Authorization: `Bearer ${getAdminToken()}`,
  });
  await navigator.clipboard.writeText(headers);
  window.open(getGraphQLEndpoint(), "_blank", "noopener");
  return `Copied; paste into GraphiQL's Headers tab:\n${headers}`;
}

const EMAIL = /^(Subject: .*\n)([\s\S]*?)(\n--- END ---)$/gm;

/** Dry-run an email job via `/dev/<path>`, opening the first email as a page; full output if the popup is blocked. */
async function previewEmails(path: string) {
  const out = await devApi(path);
  // Text-and-HTML emails (change digest): keep the HTML part.
  const email = out
    .matchAll(EMAIL)
    .next()
    .value?.[2].split("\n--- HTML ---\n")
    .pop();
  // No noopener: a same-origin blob, and the return value says whether the popup opened.
  const tab =
    email &&
    window.open(
      URL.createObjectURL(new Blob([email], { type: "text/html" })),
      "_blank",
    );
  return tab
    ? out.replace(EMAIL, "$1[HTML shown in a new tab for the first email]$3")
    : out;
}

const LAGS = [0, 500, 2000, 5000];

/** Drop every token, setting and kiosk key this origin holds. */
async function resetBrowser() {
  localStorage.clear();
  sessionStorage.clear();
  await clearKioskKeys();
  window.location.assign("/");
}

/** localStorage, or nothing when the browser blocks it: settings then last until reload. */
function readStored(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function writeStored(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    // Blocked: see readStored.
  }
}

const storedOpen = () => readStored(OPEN_KEY) === "1";

const storedTheme = (): Theme =>
  THEMES.find((t) => t === readStored(THEME_KEY)) ?? "Auto";

const onKiosk = () => window.location.pathname.startsWith("/kiosk");

/** Pin the page theme through app.css's `data-theme` hook; Auto follows the OS. The kiosk pins its own. */
function applyTheme(theme: Theme) {
  if (onKiosk()) return;
  const root = document.documentElement;
  if (theme === "Auto") root.removeAttribute("data-theme");
  else root.setAttribute("data-theme", theme.toLowerCase());
}

async function cycleTheme() {
  const next = THEMES[(THEMES.indexOf(storedTheme()) + 1) % THEMES.length];
  writeStored(THEME_KEY, next);
  applyTheme(next);
}

const panel = tw`rounded-xl bg-neutral-800/85 p-2 shadow-lg backdrop-blur-sm transition-colors hover:bg-neutral-800/95`;
// Bare text controls: ×, Copy.
const plain = tw`px-1.5 text-white/60 hover:text-white disabled:opacity-50`;
const chip = tw`rounded-md bg-white/10 px-2 py-1 transition-colors hover:bg-white/20 disabled:cursor-not-allowed disabled:opacity-40`;

/** The local API's `/dev/status`. */
type ServerStatus = {
  lagMs: number;
  errorTargets: string;
  lastMail: string | null;
};

/** This browser's and the local API's current settings, for highlighting and labelling controls. */
type DevState = ServerStatus & { token: string | null; theme: Theme };

const fetchStatus = async () =>
  JSON.parse(await devApi("status", "GET")) as ServerStatus;

type Action = {
  label: string;
  run: (state: DevState) => Promise<string | void>;
  /** Destructive: shown red, and runs only once this warning is confirmed in the side panel. */
  danger?: string;
  /** Needs poem-local's `/dev` routes, or the seed they imply; disabled without them. */
  api?: boolean;
  /** Highlighted when it matches this browser's current state. */
  active?: (state: DevState) => boolean;
  /** Shown only when this holds. */
  show?: (state: DevState) => boolean;
  /** Current setting, shown after the label. */
  value?: (state: DevState) => string;
};

// Add a control here: one entry in its row, sharing the busy flag and output panel.
const GROUPS: [string, Action[]][] = [
  [
    "Admin",
    [
      ...SEEDED_USERS.map(([label, token]) => ({
        label,
        run: async () => signInAs(token),
        api: true,
        active: (state: DevState) => state.token === token,
      })),
      {
        label: "Passkey Prompt",
        run: async () => forcePasskeyEnrollPrompt(),
        show: () => window.location.pathname.startsWith("/admin"),
      },
      {
        label: "GraphiQL",
        run: openGraphiql,
        show: () => getAdminToken() !== null,
      },
    ],
  ],
  [
    "Kiosk",
    [
      { label: "Default", run: openSeededKiosk, api: true },
      { label: "Alice 13h", run: () => aliceSignin(13), api: true },
      {
        label: "Needs Reactivation",
        run: () => devApi(`reactivate?session=${kioskKey.session_id}`),
        api: true,
        danger: `Expire ${kioskKey.session_id}'s key so admin Kiosks offers Reactivate?`,
      },
    ],
  ],
  [
    "Data",
    [
      {
        label: "Seed: Synthetic",
        run: () => devApi("seed"),
        api: true,
        danger:
          "Overwrite every seeded row with the fixtures, undoing edits to them?",
      },
      {
        label: "Clear",
        run: () => devApi("clear"),
        api: true,
        danger:
          "Delete every period, passkey, audit entry, ephemeral row and Tally record (tasks, notes, map, equipment, calendar)?",
      },
      {
        label: "DB Browser",
        run: async () => void window.open(DB_BROWSER_URL, "_blank", "noopener"),
      },
    ],
  ],
  [
    "Mail",
    [
      {
        label: "Last Email",
        run: async () =>
          (await fetchStatus()).lastMail ??
          "No mail sent since the API started",
        api: true,
      },
      { label: "Edit Link", run: openEditLink, api: true },
      {
        label: "Summary Preview",
        run: () => previewEmails("summary"),
        api: true,
      },
      {
        label: "Changes Preview",
        run: () => previewEmails("changes"),
        api: true,
      },
    ],
  ],
  [
    "Badges",
    [
      { label: "Award Nightly", run: () => devApi("badges"), api: true },
      {
        label: "Digest Preview",
        run: () => previewEmails("badges/digest"),
        api: true,
      },
      {
        label: "Clear",
        run: () => devApi("badges/clear"),
        api: true,
        danger: "Clear every member's badges at units with badges on?",
      },
    ],
  ],
  [
    "Server",
    [
      {
        label: "Latency",
        value: (state) => `${state.lagMs}ms`,
        run: (state) =>
          devApi(
            `lag?ms=${LAGS[(LAGS.indexOf(state.lagMs) + 1) % LAGS.length]}`,
          ),
        api: true,
      },
      {
        label: "Errors Off",
        run: () => devApi("errors?targets="),
        show: (state) => state.errorTargets !== "",
        api: true,
      },
    ],
  ],
  [
    "Browser",
    [
      {
        label: "Theme",
        run: cycleTheme,
        value: (state) => state.theme,
        show: () => !onKiosk(),
      },
      {
        label: "Reset",
        run: resetBrowser,
        danger: "Clear this origin's storage and kiosk keys, signing you out?",
      },
    ],
  ],
];

const LOGIN_CODE = /(?:login|sign-in) code is: (\d+)/;
const NO_STATUS: ServerStatus = { lagMs: 0, errorTargets: "", lastMail: null };

/** Whether `/dev` answers, and its status; polled every 3s while `active`. */
function useDevServer(active: boolean) {
  const [up, setUp] = useState<boolean | null>(null);
  const [status, setStatus] = useState(NO_STATUS);
  const poll = useCallback(
    () =>
      fetchStatus().then(
        (s) => {
          setUp(true);
          setStatus(s);
        },
        () => {
          setUp(false);
          setStatus(NO_STATUS);
        },
      ),
    [],
  );
  useEffect(() => {
    if (!active) return;
    void poll();
    const id = setInterval(poll, 3000);
    return () => clearInterval(id);
  }, [active, poll]);
  const code = LOGIN_CODE.exec(status.lastMail ?? "")?.[1] ?? null;
  return { up, code, status, refresh: poll };
}

export default function DevBar() {
  const [open, setOpen] = useState(storedOpen);
  const [running, setRunning] = useState<Action | null>(null);
  const [output, setOutput] = useState("");
  const [failed, setFailed] = useState(false);
  const [pending, setPending] = useState<Action | null>(null);
  // The text last copied, so whichever control copied it can say so.
  const [copied, setCopied] = useState<string | null>(null);
  const env = useEnvironmentInfo();
  const { up, code, status, refresh } = useDevServer(open);
  const [errorsDraft, setErrorsDraft] = useState("");
  // Read each render: cheap, and any action that changes them ends in a re-render.
  const state: DevState = {
    token: getAdminToken(),
    theme: storedTheme(),
    ...status,
  };

  useEffect(() => {
    if (!navigator.webdriver) applyTheme(storedTheme());
  }, []);

  useEffect(() => writeStored(OPEN_KEY, open ? "1" : "0"), [open]);

  // Esc cancels a confirmation, else closes the bar; never in a text field or once the page handled it.
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape" || e.defaultPrevented) return;
      if (pending) setPending(null);
      else if (
        !(e.target instanceof Element) ||
        !e.target.closest("input, textarea, select, [contenteditable]")
      )
        setOpen(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, pending]);

  // Automated browsers (guide screenshots, local/examples) must not capture it.
  if (navigator.webdriver) return null;

  async function act(action: Action, confirmed = false) {
    if (action.danger && !confirmed) {
      setOutput("");
      setPending(action);
      return;
    }
    setPending(null);
    setRunning(action);
    setOutput("");
    setFailed(false);
    try {
      setOutput((await action.run(state)) ?? "");
    } catch (e) {
      setFailed(true);
      setOutput(String(e));
    } finally {
      setRunning(null);
      void refresh();
    }
  }

  function copy(text: string) {
    void navigator.clipboard.writeText(text).then(() => {
      setCopied(text);
      setTimeout(() => setCopied(null), 1500);
    });
  }

  return (
    <div
      data-devbar
      className="fixed bottom-2 left-2 z-50 flex max-w-[calc(100vw-1rem)] flex-wrap items-stretch gap-2 font-mono text-xs text-white print:hidden"
    >
      {open ? (
        <div className={cn(panel, "flex flex-col gap-1.5")}>
          <div className="flex items-center justify-between gap-2">
            <span className="font-bold">
              {"</>"} {getCurrentEnvironment()}
              {env && ` ${env.gitRev.slice(0, 7)}`}
              {env?.isProdDb && <b className="text-red-400"> PROD DB</b>}
            </span>
            <button
              type="button"
              className={plain}
              aria-label="Close dev bar"
              onClick={() => setOpen(false)}
            >
              ×
            </button>
          </div>
          <div className="grid grid-cols-[auto_1fr] items-center gap-x-2 gap-y-1">
            {GROUPS.map(([group, actions]) => (
              <Fragment key={group}>
                <span className="text-white/50">{group}</span>
                <div className="flex flex-wrap gap-1">
                  {actions
                    .filter((action) => action.show?.(state) ?? true)
                    .map((action) => (
                      <button
                        key={action.label}
                        type="button"
                        className={cn(
                          chip,
                          action.danger && "bg-red-600/60",
                          action.active?.(state) && "ring-1 ring-green-400",
                          running === action && "animate-pulse",
                        )}
                        title={
                          action.api && up === false
                            ? "Needs make dev-local (poem-local's /dev routes)"
                            : undefined
                        }
                        disabled={
                          running !== null ||
                          pending !== null ||
                          (action.api && up === false)
                        }
                        onClick={() => void act(action)}
                      >
                        {action.label}
                        {action.value && `: ${action.value(state)}`}
                      </button>
                    ))}
                  {group === "Server" && (
                    <input
                      className="w-56 rounded-md bg-white/10 px-2 py-1 placeholder:text-white/40"
                      aria-label="Fields to fail"
                      placeholder={
                        state.errorTargets ||
                        "Fail: location.periods.edges.*.node.person@0.5"
                      }
                      disabled={up === false || running !== null}
                      value={errorsDraft}
                      onChange={(e) => setErrorsDraft(e.target.value)}
                      onKeyDown={(e) => {
                        if (e.key !== "Enter" || !errorsDraft.trim()) return;
                        void act({
                          label: "Errors",
                          run: () =>
                            devApi(
                              `errors?targets=${encodeURIComponent(errorsDraft)}`,
                            ),
                        });
                        setErrorsDraft("");
                      }}
                    />
                  )}
                  {group === "Mail" && code && (
                    <button
                      type="button"
                      className={chip}
                      title="Copy"
                      onClick={() => copy(code)}
                    >
                      <span className="text-green-400">●</span>{" "}
                      {copied === code ? "Copied" : `Code ${code}`}
                    </button>
                  )}
                </div>
              </Fragment>
            ))}
          </div>
        </div>
      ) : (
        <button
          type="button"
          className="rounded-md bg-neutral-500/30 px-2 py-0.5 text-white/60 transition-colors hover:bg-neutral-700/90 hover:text-white"
          onClick={() => setOpen(true)}
        >
          {"</>"} Dev
        </button>
      )}
      {/* Side panel: stretched to the bar's height, so it only shows while the bar is open. */}
      {open && pending ? (
        <div
          className={cn(
            panel,
            "flex w-xl max-w-full flex-col justify-between gap-2",
          )}
        >
          <p>{pending.danger}</p>
          <div className="flex justify-end gap-1">
            <button
              type="button"
              className={chip}
              autoFocus
              onClick={() => setPending(null)}
            >
              Cancel
            </button>
            <button
              type="button"
              className={cn(chip, "bg-red-600/80 hover:bg-red-600")}
              onClick={() => void act(pending, true)}
            >
              {pending.label}
            </button>
          </div>
        </div>
      ) : (
        open &&
        output && (
          <div
            className={cn(
              panel,
              "relative min-h-32 w-xl max-w-full",
              failed && "ring-1 ring-red-500",
            )}
          >
            {/* Out of flow, so it scrolls within the bar's height; keyed so each result starts at the top. */}
            <pre
              key={output}
              className={cn(
                "absolute inset-2 overflow-auto wrap-break-word whitespace-pre-wrap",
                failed && "text-red-300",
              )}
            >
              {output}
            </pre>
            {/* Over the output's top-right corner, so the text keeps the full width. */}
            <div className="absolute top-1 right-1 flex gap-1 rounded-md bg-neutral-800">
              <button
                type="button"
                className={plain}
                onClick={() => copy(output)}
              >
                {copied === output ? "Copied" : "Copy"}
              </button>
              <button
                type="button"
                className={plain}
                aria-label="Dismiss output"
                onClick={() => setOutput("")}
              >
                ×
              </button>
            </div>
          </div>
        )
      )}
    </div>
  );
}
