import {
  useEffect,
  useState,
  useSyncExternalStore,
  type ReactNode,
} from "react";
import { Dialog, DialogActions, DialogTitle } from "../../components/ui/Dialog";
import { Button } from "../../components/ui/Button";
import { getClientUpdateState } from "../../lib/clientUpdate";
import { getClientUpdateLeases } from "../../lib/clientUpdateLeases";
import {
  getCurrentClientVersion,
  shortenGitRev,
} from "../../lib/clientVersion";
import { useEnvironmentInfo } from "../../lib/environmentInfo";
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
/** Check-ins fresher than this are healthy; the poll runs every 2 minutes. */
const CHECK_IN_OK_SECS = 5 * 60;
/** Matches the server's ONLINE_SESSION_SECONDS — past this a kiosk reads as offline. */
const CHECK_IN_STALE_SECS = 15 * 60;
/** A fallback poll (period sync, every POLL_INTERVAL_MS) fresher than this is
 * healthy; past it, either the poll is failing or the tab is backgrounded. */
const PERIODS_POLL_OK_SECS = (POLL_INTERVAL_MS / 1000) * 2;
const PERIODS_POLL_STALE_SECS = (POLL_INTERVAL_MS / 1000) * 4;

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <>
      <dt className="text-neutral-500 dark:text-neutral-400">{label}</dt>
      <dd className="m-0 min-w-0 wrap-break-word">{children}</dd>
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
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    const intervalId = window.setInterval(
      () => setNow(Date.now()),
      TICK_INTERVAL_MS,
    );
    return () => window.clearInterval(intervalId);
  }, []);

  const serverStatus = getKioskServerStatus();
  const { pendingVersion } = getClientUpdateState();
  const updateLeases = getClientUpdateLeases();

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
    <Dialog onDismiss={onClose} className="text-base">
      <DialogTitle>Kiosk status</DialogTitle>

      <dl className="m-0 grid grid-cols-[auto_1fr] gap-x-6 gap-y-2">
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
        {serverStatus.lastFailureAt != null && (
          <Row label="Last failure">
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
        {categoryScale !== undefined && (
          <Row label="Category size">
            auto ({Math.round(categoryScale * 100)}%)
          </Row>
        )}

        <Row label="Config">{formatConfigFlags(session?.config ?? {})}</Row>
      </dl>

      {isAppleMobileSafari() && !standalone && (
        <p className="text-sm text-neutral-500 dark:text-neutral-400">
          On an iPad, the most reliable full screen kiosk is a home screen app:
          tap Share, then Add to Home Screen, and open the kiosk from the new
          icon. The home screen app keeps its own storage, so it needs enrolling
          again. Guided Access (Settings &gt; Accessibility) then stops anyone
          leaving it.
        </p>
      )}
      {fullscreenError != null && (
        <p className="text-sm text-red-700 dark:text-red-400">
          Could not change full screen: {fullscreenError}
        </p>
      )}

      <KioskReEnrollPanel currentKioskName={session?.name ?? null} />

      <DialogActions>
        <Button variant="secondary" onClick={onClose}>
          Close
        </Button>
        {canFullscreen && (
          <Button variant="secondary" onClick={onToggleFullscreen}>
            {fullscreen ? "Exit full screen" : "Full screen"}
          </Button>
        )}
        <Button variant="kiosk" onClick={() => window.location.reload()}>
          Reload
        </Button>
      </DialogActions>
    </Dialog>
  );
}
