import { useEffect, useState } from "react";
import {
  Panel,
  PanelBox,
  PanelIntro,
  PanelTitle,
} from "../../components/ui/Panel";
import { describeClockOffset, useClockSkew } from "../lib/clockSkew";

const TIME_FORMAT: Intl.DateTimeFormatOptions = {
  weekday: "short",
  day: "numeric",
  month: "short",
  year: "numeric",
  hour: "numeric",
  minute: "2-digit",
};

function formatTime(ms: number): string {
  return new Date(ms).toLocaleString("en-AU", TIME_FORMAT);
}

/**
 * Covers the kiosk while the server refuses its requests because the computer's clock
 * is wrong (see lib/clockSkew.ts). Rendered over the kiosk rather than instead of it,
 * so whatever is underneath keeps retrying and the notice goes away by itself once a
 * request gets through.
 */
export default function KioskClockSkewNotice() {
  const skew = useClockSkew();
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    if (skew === null) return;
    const id = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(id);
  }, [skew]);

  if (skew === null) {
    return null;
  }

  return (
    <div
      className="fixed inset-0 z-100 overflow-auto"
      role="alertdialog"
      aria-modal="true"
      aria-labelledby="kiosk-clock-skew-title"
    >
      <Panel>
        <PanelBox>
          <div id="kiosk-clock-skew-title">
            <PanelTitle>This computer's clock is wrong</PanelTitle>
          </div>
          <PanelIntro>
            Members can't sign in or out here until the computer's date and time
            are correct. The kiosk is still set up — there's no need to scan a
            QR code.
          </PanelIntro>

          <dl className="mb-6 grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-lg text-ink">
            <dt className="opacity-70">This computer</dt>
            <dd className="m-0 font-semibold" data-testid="clock-skew-device">
              {formatTime(now)}
            </dd>
            <dt className="opacity-70">Correct time</dt>
            <dd className="m-0 font-semibold" data-testid="clock-skew-server">
              {formatTime(now + skew.offsetMs)}
            </dd>
            <dt className="opacity-70">Out by</dt>
            <dd className="m-0 font-semibold">
              {describeClockOffset(skew.offsetMs)}
            </dd>
          </dl>

          <ol className="m-0 flex list-decimal flex-col gap-2 pl-5 text-ink">
            <li>
              Open the computer's date and time settings and turn on{" "}
              <b>Set time automatically</b> (on Windows, also press{" "}
              <b>Sync now</b>), or set the correct time by hand.
            </li>
            <li>This screen goes away by itself within a minute.</li>
            <li>
              If the clock is wrong again after every restart, the computer's
              clock battery may need replacing.
            </li>
          </ol>
        </PanelBox>
      </Panel>
    </div>
  );
}
