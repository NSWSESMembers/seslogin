import { useEffect, useState, type ReactNode } from "react";
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
import KioskReEnrollPanel from "./KioskReEnrollPanel";
import useKioskEnvironment from "./useKioskEnvironment";
import { useKioskSession } from "./useKioskSession";
import type { JsonValue } from "./KioskSessionContext";

const TICK_INTERVAL_MS = 1_000;
/** Check-ins fresher than this are healthy; the poll runs every 2 minutes. */
const CHECK_IN_OK_SECS = 5 * 60;
/** Matches the server's ONLINE_SESSION_SECONDS — past this a kiosk reads as offline. */
const CHECK_IN_STALE_SECS = 15 * 60;

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

function checkInColour(ageSecs: number): string {
  if (ageSecs <= CHECK_IN_OK_SECS) {
    return "text-green-700 dark:text-green-400";
  }
  if (ageSecs <= CHECK_IN_STALE_SECS) {
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
}: {
  onClose: () => void;
}) {
  const session = useKioskSession();
  const { profile, authMode } = useKioskEnvironment();
  const environmentInfo = useEnvironmentInfo();
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

        <Row label="Config">{formatConfigFlags(session?.config ?? {})}</Row>
      </dl>

      <KioskReEnrollPanel currentKioskName={session?.name ?? null} />

      <DialogActions>
        <Button variant="secondary" onClick={onClose}>
          Close
        </Button>
        <Button variant="kiosk" onClick={() => window.location.reload()}>
          Reload
        </Button>
      </DialogActions>
    </Dialog>
  );
}
