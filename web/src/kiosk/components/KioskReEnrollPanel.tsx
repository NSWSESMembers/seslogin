import { useEffect, useState } from "react";
import { Button } from "../../components/ui/Button";
import { fetchKeySessionId } from "../lib/enrollmentKey";
import { pollDelayMs } from "../lib/enrollPolling";
import { useEnrollmentQr } from "../lib/useEnrollmentQr";
import useKioskEnvironment from "./useKioskEnvironment";

/**
 * The enrollment QR code, on demand, from inside the kiosk status dialog — the way to
 * rescue a kiosk that was enrolled as the wrong session or at the wrong location
 * without having to identify the device from the admin side first. Enrolling it again
 * releases the key from the kiosk it is enrolled as now (see `enrollSession`), so the
 * code works whatever state that kiosk is in and needs no clean-up beforehand.
 *
 * Hidden behind a button because showing it publishes this device's public key to the
 * server; there's no reason to do that every time someone opens the status dialog.
 *
 * Deliberately renders no link (unlike the full enrollment screen): a kiosk that
 * navigates away from /kiosk needs someone with browser chrome to rescue it.
 */
export default function KioskReEnrollPanel({
  currentKioskName,
}: {
  currentKioskName: string | null;
}) {
  const { profile, authMode, onKeyEnrolled } = useKioskEnvironment();
  const [shown, setShown] = useState(false);
  const { info, fingerprint, qrDataUrl, error } = useEnrollmentQr(
    profile,
    shown,
  );

  // A code-authenticated kiosk keeps using its stored JWT — and so keeps showing the
  // old kiosk — until we notice the key has been enrolled and hand over to it. Kiosks
  // already signing with the key need no handover: their next session refresh resolves
  // whichever session now holds the key.
  useEffect(() => {
    if (!shown || info == null || authMode !== "jwt") return;

    let cancelled = false;
    let pollTimeout: number | null = null;
    const startedAt = Date.now();

    const runPoll = async () => {
      if (cancelled) return;
      let sessionId: string | null = null;
      try {
        sessionId = await fetchKeySessionId(info);
      } catch (err) {
        console.error("Re-enrollment poll failed:", err);
      }
      if (cancelled) return;
      if (sessionId != null) {
        onKeyEnrolled();
        return;
      }
      pollTimeout = window.setTimeout(
        runPoll,
        pollDelayMs(Date.now() - startedAt),
      );
    };

    runPoll();

    return () => {
      cancelled = true;
      if (pollTimeout !== null) window.clearTimeout(pollTimeout);
    };
  }, [shown, info, authMode, onKeyEnrolled]);

  if (!shown) {
    return (
      <div className="border-t border-line pt-4">
        <Button variant="secondary" onClick={() => setShown(true)}>
          Re-enroll this kiosk
        </Button>
      </div>
    );
  }

  return (
    <div className="flex flex-col items-center gap-3 border-t border-line pt-4">
      <p className="m-0 text-center text-sm">
        Have an administrator scan this to enroll this device as a different
        kiosk. That replaces{" "}
        {currentKioskName ? <b>{currentKioskName}</b> : "the current kiosk"},
        which stops working here — its past activity is kept.
      </p>

      {qrDataUrl ? (
        <img
          src={qrDataUrl}
          alt="Kiosk enrollment QR code"
          className="size-50"
        />
      ) : (
        !error && <p className="m-0 text-sm opacity-60">Generating code…</p>
      )}

      {error && (
        <p className="m-0 text-center text-sm text-red-700 dark:text-red-400">
          {error}
        </p>
      )}

      {fingerprint && (
        <p className="m-0 text-center font-mono text-xs break-all opacity-60">
          {fingerprint.slice(0, 16)}…
        </p>
      )}

      <Button variant="secondary" onClick={() => setShown(false)}>
        Hide code
      </Button>
    </div>
  );
}
