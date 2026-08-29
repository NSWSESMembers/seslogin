import type { ReactNode } from "react";
import { formatSeconds } from "../../lib/time";
import { getCurrentEnvironment } from "../../lib/clientInfo";

/**
 * Structurally compatible with what Relay generates for the `clientInfo` selection, so
 * this component stays independent of which query fetched it. Every field is optional
 * twice over — `null` from the server (not reported) and `undefined` from Relay.
 */
export type ClientInfoFields = {
  readonly env?: string | null;
  readonly origin?: string | null;
  readonly apiUrl?: string | null;
  readonly profile?: string | null;
  readonly userAgent?: string | null;
  readonly screen?: string | null;
  readonly displayMode?: string | null;
  readonly timezone?: string | null;
  readonly clockSkewSecs?: number | null;
  readonly uptimeSecs?: number | null;
  readonly pendingVersion?: string | null;
  readonly contactFailures?: number | null;
  readonly updatedAt?: number | null;
};

/** Past this, a signed-key kiosk's requests start falling outside the timestamp window. */
const CLOCK_SKEW_WARN_SECS = 60;

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <>
      <dt className="text-ink-muted">{label}</dt>
      <dd className="m-0 min-w-0 wrap-break-word">{children}</dd>
    </>
  );
}

function Warn({ children, title }: { children: ReactNode; title: string }) {
  return (
    <span
      className="font-semibold text-amber-700 dark:text-amber-400"
      title={title}
    >
      {children}
    </span>
  );
}

function formatSkew(secs: number): string {
  const direction = secs < 0 ? "behind" : "ahead of";
  return `${formatSeconds(Math.abs(secs))} ${direction} the server`;
}

/**
 * Everything a kiosk last reported about itself, for the kiosk edit page.
 *
 * These exist to answer "why is *this* kiosk behaving differently?" without anyone
 * driving out to stand in front of it. Rows the kiosk didn't report are dropped rather
 * than rendered as blanks, so the panel stays short for a healthy kiosk and gets longer
 * exactly when there's something to look at.
 */
export default function SessionClientInfo({
  clientInfo,
}: {
  clientInfo: ClientInfoFields | null | undefined;
}) {
  if (clientInfo == null) {
    return (
      <p className="text-ink-muted">
        This kiosk hasn't reported anything about itself yet. It will on its
        next check-in, unless it is running a client build older than this
        feature.
      </p>
    );
  }

  const expectedEnv = getCurrentEnvironment();
  const skew = clientInfo.clockSkewSecs;

  return (
    // `body` sets `text-align: center` app-wide (a legacy global), which centres both
    // columns and leaves short labels floating away from their values. Opt out the same
    // way ActivityCategorySelector does — an inline box that the centred page still
    // centres, with its own contents left-aligned. The width cap keeps the user agent,
    // much the longest value here, wrapping at a readable measure instead of stretching
    // across a wide monitor.
    <dl className="m-0 inline-grid max-w-2xl grid-cols-[auto_1fr] gap-x-6 gap-y-2 text-left">
      {clientInfo.env != null && (
        <Row label="Environment">
          {clientInfo.env === expectedEnv ? (
            clientInfo.env
          ) : (
            <Warn
              title={`You are viewing the "${expectedEnv}" admin site. Non-production builds still read and write the production database.`}
            >
              {clientInfo.env} — not this environment
            </Warn>
          )}
        </Row>
      )}
      {clientInfo.origin != null && (
        <Row label="Loaded from">
          <span className="font-mono text-[0.9em]">{clientInfo.origin}</span>
        </Row>
      )}
      {clientInfo.apiUrl != null && (
        <Row label="API endpoint">
          <span className="font-mono text-[0.9em]">{clientInfo.apiUrl}</span>
        </Row>
      )}
      {clientInfo.profile != null && (
        <Row label="Kiosk profile">{clientInfo.profile}</Row>
      )}
      {clientInfo.pendingVersion != null && (
        <Row label="Update pending">
          <Warn title="The kiosk has seen a newer build but hasn't reloaded into it yet — usually because a scan or request is in progress. This is why its version can look stale.">
            waiting to reload into {clientInfo.pendingVersion.slice(0, 7)}
          </Warn>
        </Row>
      )}
      {clientInfo.userAgent != null && (
        <Row label="Browser">
          <span className="text-[0.9em]">{clientInfo.userAgent}</span>
        </Row>
      )}
      {clientInfo.screen != null && (
        <Row label="Screen">{clientInfo.screen}</Row>
      )}
      {clientInfo.displayMode != null && (
        <Row label="Display mode">
          {clientInfo.displayMode === "browser" ? (
            <Warn title="Running in a normal browser tab rather than as an installed kiosk app.">
              browser tab
            </Warn>
          ) : (
            clientInfo.displayMode
          )}
        </Row>
      )}
      {clientInfo.timezone != null && (
        <Row label="Time zone">{clientInfo.timezone}</Row>
      )}
      {skew != null && (
        <Row label="Clock">
          {Math.abs(skew) < CLOCK_SKEW_WARN_SECS ? (
            "in sync with the server"
          ) : (
            <Warn title="A kiosk clock this far out will start failing to authenticate: signed requests carry a timestamp the server only accepts within a narrow window.">
              {formatSkew(skew)}
            </Warn>
          )}
        </Row>
      )}
      {clientInfo.uptimeSecs != null && (
        <Row label="Page uptime">{formatSeconds(clientInfo.uptimeSecs)}</Row>
      )}
      {clientInfo.contactFailures != null && (
        <Row label="Failed check-ins">
          {clientInfo.contactFailures === 0 ? (
            "none since it last loaded"
          ) : (
            <Warn title="Requests that failed to reach the server since the kiosk page last loaded. A count that climbs while the kiosk still appears online points at a flaky network rather than a dead kiosk.">
              {clientInfo.contactFailures} since it last loaded
            </Warn>
          )}
        </Row>
      )}
      {clientInfo.updatedAt != null && (
        <Row label="Reported">
          {new Date(clientInfo.updatedAt * 1000).toLocaleString()}
        </Row>
      )}
    </dl>
  );
}
