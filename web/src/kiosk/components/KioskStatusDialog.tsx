import {
  useEffect,
  useState,
  useSyncExternalStore,
  type ReactNode,
} from "react";
import { Dialog } from "../../components/ui/Dialog";
import { Button } from "../../components/ui/Button";
import {
  getCurrentEnvironment,
  getScreen,
  getTimezone,
  getUptimeSecs,
} from "../../lib/clientInfo";
import { getClientUpdateState } from "../../lib/clientUpdate";
import { getClientUpdateLeases } from "../../lib/clientUpdateLeases";
import {
  getCurrentClientVersion,
  shortenGitRev,
} from "../../lib/clientVersion";
import { useEnvironmentInfo } from "../../lib/environmentInfo";
import { cn } from "../../lib/tw";
import { formatFullDateTime, formatShortDuration } from "../../lib/time";
import { getKioskServerStatus } from "../lib/kioskServerStatus";
import {
  isAppleMobileSafari,
  isFullscreen,
  isFullscreenSupported,
  isStandalone,
  subscribeFullscreen,
  toggleFullscreen,
} from "../lib/fullscreen";
import { getWakeLockStatus, subscribeWakeLock } from "../lib/wakeLock";
import { POLL_INTERVAL_MS } from "./LivePeriodsProvider";
import KioskReEnrollPanel from "./KioskReEnrollPanel";
import useKioskEnvironment from "./useKioskEnvironment";
import { useKioskSession } from "./useKioskSession";
import { useLivePeriods } from "./useLivePeriods";
import type { JsonValue } from "./KioskSessionContext";

const TICK_INTERVAL_MS = 1_000;
/** Check-ins fresher than this are healthy. The session poll runs every 15 min
 * ±10% while realtime session pushes are live (2 min otherwise), so this
 * matches the server's ONLINE_SESSION_SECONDS: one poll plus room for splay. */
const CHECK_IN_OK_SECS = 20 * 60;
/** Past this — at least one poll missed outright — the check-in reads as stale. */
const CHECK_IN_STALE_SECS = 35 * 60;
/** A fallback poll (period sync, every POLL_INTERVAL_MS) fresher than this is
 * healthy; past it, either the poll is failing or the tab is backgrounded. */
const PERIODS_POLL_OK_SECS = (POLL_INTERVAL_MS / 1000) * 2;
const PERIODS_POLL_STALE_SECS = (POLL_INTERVAL_MS / 1000) * 4;

/**
 * One label/value pair. The list is two pairs across from `sm` up; `wide` puts a value
 * that is often long (an error message, the config flags) on a line of its own.
 */
function Row({
  label,
  wide = false,
  children,
}: {
  label: string;
  wide?: boolean;
  children: ReactNode;
}) {
  return (
    <>
      <dt
        className={cn(
          "text-neutral-500 dark:text-neutral-400",
          wide && "sm:col-start-1",
        )}
      >
        {label}
      </dt>
      <dd
        className={cn("m-0 min-w-0 wrap-break-word", wide && "sm:col-span-3")}
      >
        {children}
      </dd>
    </>
  );
}

function Mono({ children }: { children: ReactNode }) {
  return <span className="font-mono">{children}</span>;
}

function formatConfigFlags(config: { [key: string]: JsonValue }): string {
  const parts = Object.entries(config)
    .filter(([, value]) => value !== false && value !== null && value !== "")
    .map(([key, value]) =>
      value === true ? key : `${key}=${JSON.stringify(value)}`,
    );
  return parts.length > 0 ? parts.join(", ") : "none";
}

function checkInColour(
  ageSecs: number,
  okSecs: number = CHECK_IN_OK_SECS,
  staleSecs: number = CHECK_IN_STALE_SECS,
): string {
  if (ageSecs <= okSecs) {
    return "text-green-700 dark:text-green-400";
  }
  if (ageSecs <= staleSecs) {
    return "text-amber-700 dark:text-amber-400";
  }
  return "text-red-700 dark:text-red-400";
}

function readTick() {
  return {
    now: Date.now(),
    uptimeSecs: getUptimeSecs(),
    screenSize: getScreen(),
  };
}

/**
 * Read-only diagnostics for whoever is standing at the kiosk, opened by tapping the
 * logo. Everything is rendered from local state so it still works while the kiosk is
 * offline — which is when it matters most. Deliberately contains no links: a kiosk
 * that navigates away from /kiosk needs someone with browser chrome to rescue it.
 */
export default function KioskStatusDialog({
  onClose,
  categoryScale,
}: {
  onClose: () => void;
  /** The auto-chosen category button scale (1 = full size); see categoryScale.ts. */
  categoryScale?: number;
}) {
  const session = useKioskSession();
  const { profile, authMode } = useKioskEnvironment();
  const environmentInfo = useEnvironmentInfo();
  const { live, connectionState, lastPollAt } = useLivePeriods();
  // Uptime and screen size are read on the tick alongside the clock: called bare in
  // render, the React Compiler would cache them like any other no-input call.
  const [{ now, uptimeSecs, screenSize }, setTick] = useState(readTick);
  const [reEnrollShown, setReEnrollShown] = useState(false);

  useEffect(() => {
    const intervalId = window.setInterval(
      () => setTick(readTick()),
      TICK_INTERVAL_MS,
    );
    return () => window.clearInterval(intervalId);
  }, []);

  const serverStatus = getKioskServerStatus();
  const { pendingVersion } = getClientUpdateState();
  const updateLeases = getClientUpdateLeases();
  const buildEnv = getCurrentEnvironment();
  const timezone = getTimezone();

  // Subscribed rather than read on the tick: the React Compiler would cache a bare
  // getter call with no inputs, freezing whatever it returned on first render.
  const wakeLock = useSyncExternalStore(subscribeWakeLock, getWakeLockStatus);
  const standalone = isStandalone();
  const canFullscreen = !standalone && isFullscreenSupported();
  const fullscreen = useSyncExternalStore(subscribeFullscreen, isFullscreen);
  const [fullscreenError, setFullscreenError] = useState<string | null>(null);

  const onToggleFullscreen = () => {
    setFullscreenError(null);
    toggleFullscreen().catch((error: unknown) => {
      setFullscreenError(
        error instanceof Error ? error.message : String(error),
      );
    });
  };

  const checkInAgeSecs =
    serverStatus.lastSuccessAt == null
      ? null
      : (now - serverStatus.lastSuccessAt) / 1000;

  return (
    <Dialog
      onDismiss={onClose}
      width="w-200"
      className="gap-2 p-3 text-sm sm:p-4"
    >
      {/* Controls first, so they are reachable without scrolling on a short screen. */}
      <div className="flex flex-wrap items-center justify-between gap-2">
        <h2 className="m-0 text-lg font-bold">Kiosk status</h2>
        <div className="flex flex-wrap justify-end gap-2">
          {!reEnrollShown && (
            <Button variant="secondary" onClick={() => setReEnrollShown(true)}>
              Re-enroll this kiosk
            </Button>
          )}
          {canFullscreen && (
            <Button variant="secondary" onClick={onToggleFullscreen}>
              {fullscreen ? "Exit full screen" : "Full screen"}
            </Button>
          )}
          <Button variant="secondary" onClick={onClose}>
            Close
          </Button>
          <Button variant="kiosk" onClick={() => window.location.reload()}>
            Reload
          </Button>
        </div>
      </div>

      {fullscreenError != null && (
        <p className="m-0 text-red-700 dark:text-red-400">
          Could not change full screen: {fullscreenError}
        </p>
      )}
      {isAppleMobileSafari() && !standalone && (
        <p className="m-0 text-xs text-neutral-500 dark:text-neutral-400">
          On an iPad, the most reliable full screen kiosk is a home screen app:
          tap Share, then Add to Home Screen, and open the kiosk from the new
          icon. The home screen app keeps its own storage, so it needs enrolling
          again. Guided Access (Settings &gt; Accessibility) then stops anyone
          leaving it.
        </p>
      )}

      <KioskReEnrollPanel
        currentKioskName={session?.name ?? null}
        shown={reEnrollShown}
        onHide={() => setReEnrollShown(false)}
      />

      <dl className="m-0 grid grid-cols-[auto_1fr] gap-x-3 gap-y-0.5 border-t border-line pt-2 text-left sm:grid-cols-[auto_1fr_auto_1fr]">
        <Row label="Kiosk">{session?.name ?? "unknown"}</Row>
        <Row label="Kiosk ID">
          <Mono>{session?.id ?? "unknown"}</Mono>
        </Row>
        <Row label="Location">{session?.location.name ?? "unknown"}</Row>
        <Row label="Location ID">
          <Mono>{session?.location.id ?? "unknown"}</Mono>
        </Row>

        <Row label="Last server check-in">
          {checkInAgeSecs == null || serverStatus.lastSuccessAt == null ? (
            <span className="text-red-700 dark:text-red-400">never</span>
          ) : (
            <span className={checkInColour(checkInAgeSecs)}>
              {formatShortDuration(checkInAgeSecs)} ago (
              {formatFullDateTime(new Date(serverStatus.lastSuccessAt))})
            </span>
          )}
        </Row>
        <Row label="Failed check-ins">
          {serverStatus.failureCount === 0 ? (
            "none since load"
          ) : (
            <span className="text-amber-700 dark:text-amber-400">
              {serverStatus.failureCount} since load
            </span>
          )}
        </Row>
        {serverStatus.lastFailureAt != null && (
          <Row label="Last failure" wide>
            <span className="text-red-700 dark:text-red-400">
              {formatShortDuration((now - serverStatus.lastFailureAt) / 1000)}{" "}
              ago: {serverStatus.lastErrorMessage ?? "unknown error"}
            </span>
          </Row>
        )}

        <Row label="Realtime">
          {live ? (
            <span className="text-green-700 dark:text-green-400">
              connected
              {connectionState != null && connectionState !== "connected"
                ? ` (${connectionState})`
                : ""}
            </span>
          ) : connectionState != null ? (
            <span className="text-amber-700 dark:text-amber-400">
              {connectionState}, falling back to polling
            </span>
          ) : (
            <span className="text-neutral-500 dark:text-neutral-400">
              off, polling
            </span>
          )}
        </Row>
        {!live && (
          <Row label="Last poll">
            {lastPollAt == null ? (
              <span className="text-red-700 dark:text-red-400">never</span>
            ) : (
              <span
                className={checkInColour(
                  (now - lastPollAt) / 1000,
                  PERIODS_POLL_OK_SECS,
                  PERIODS_POLL_STALE_SECS,
                )}
              >
                {formatShortDuration((now - lastPollAt) / 1000)} ago
              </span>
            )}
          </Row>
        )}

        <Row label="Auth mode">
          {authMode === "key" ? "enrolled key" : "setup code"}
        </Row>
        {serverStatus.keyExpiresAt != null && (
          <Row label="Key expires">
            in {formatShortDuration(serverStatus.keyExpiresAt - now / 1000)} (
            {formatFullDateTime(new Date(serverStatus.keyExpiresAt * 1000))})
          </Row>
        )}
        <Row label="Profile">
          <Mono>{profile}</Mono>
        </Row>

        <Row label="Version">
          <Mono>{shortenGitRev(getCurrentClientVersion())}</Mono>
        </Row>
        <Row label="Server">
          <Mono>
            {environmentInfo == null
              ? "unknown"
              : shortenGitRev(environmentInfo.gitRev)}
          </Mono>
        </Row>
        <Row label="Build">
          {buildEnv === "prod" ? (
            <Mono>{buildEnv}</Mono>
          ) : (
            <span className="font-mono text-red-700 dark:text-red-400">
              {buildEnv}
            </span>
          )}
        </Row>
        <Row label="Database">
          {environmentInfo == null ? (
            "unknown"
          ) : environmentInfo.isProdDb ? (
            "production"
          ) : (
            <span className="text-red-700 dark:text-red-400">
              NOT production
            </span>
          )}
        </Row>
        <Row label="Origin">
          <Mono>{window.location.origin}</Mono>
        </Row>
        <Row label="Update">
          {pendingVersion == null
            ? "up to date"
            : updateLeases.length > 0
              ? `pending, held by ${updateLeases.length} task(s)`
              : "pending"}
        </Row>

        <Row label="Keep awake">
          {wakeLock.state === "held" ? (
            <span className="text-green-700 dark:text-green-400">
              on, screen will not sleep
            </span>
          ) : wakeLock.state === "unsupported" ? (
            <span className="text-amber-700 dark:text-amber-400">
              not supported, set the device's auto-lock to Never
            </span>
          ) : wakeLock.state === "pending" ? (
            "requesting"
          ) : (
            <span className="text-amber-700 dark:text-amber-400">
              not held{wakeLock.lastError ? `: ${wakeLock.lastError}` : ""},
              retrying on next tap
            </span>
          )}
        </Row>
        <Row label="Display">
          {standalone
            ? "home screen app"
            : fullscreen
              ? "full screen"
              : "browser"}
        </Row>
        <Row label="Screen">
          <Mono>{screenSize ?? "unknown"}</Mono>
        </Row>
        {categoryScale !== undefined && (
          <Row label="Category size">
            auto ({Math.round(categoryScale * 100)}%)
          </Row>
        )}
        <Row label="Time zone">{timezone ?? "unknown"}</Row>
        <Row label="Page uptime">
          {uptimeSecs == null ? "unknown" : formatShortDuration(uptimeSecs)}
        </Row>

        <Row label="Config" wide>
          {formatConfigFlags(session?.config ?? {})}
        </Row>
      </dl>
    </Dialog>
  );
}
