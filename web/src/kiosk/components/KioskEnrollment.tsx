import { useEffect } from "react";
import {
  Panel,
  PanelBox,
  PanelTitle,
  PanelIntro,
} from "../../components/ui/Panel";
import { Button } from "../../components/ui/Button";
import LoadingIndicator from "../../components/LoadingIndicator";
import { fetchKeySessionId } from "../lib/enrollmentKey";
import { useEnrollmentQr } from "../lib/useEnrollmentQr";
import { pollDelayMs } from "../lib/enrollPolling";

export default function KioskEnrollment({
  profile,
  onEnrolled,
  onUseCodeInstead,
}: {
  profile: string;
  onEnrolled: () => void;
  onUseCodeInstead: () => void;
}) {
  const { info, fingerprint, enrollUrl, qrDataUrl } = useEnrollmentQr(profile);

  useEffect(() => {
    if (info == null) return;

    let cancelled = false;
    let pollTimeout: number | null = null;
    const startedAt = Date.now();

    const runPoll = async () => {
      if (cancelled) return;
      let sessionId: string | null = null;
      try {
        sessionId = await fetchKeySessionId(info);
      } catch (err) {
        console.error("Enrollment poll failed:", err);
      }
      if (cancelled) return;
      if (sessionId != null) {
        onEnrolled();
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
    // `onEnrolled` is a stable useCallback from KioskEnvironment and `info` is set once
    // per profile, so this poll loop is set up once rather than on every render.
  }, [info, onEnrolled]);

  return (
    <Panel>
      <PanelBox>
        <PanelTitle>Enroll this kiosk</PanelTitle>
        <PanelIntro>
          Ask someone with administrator access to scan this QR code. It opens
          the enrollment page pre-filled for this device — once they save it,
          this screen will switch over automatically.
        </PanelIntro>

        {qrDataUrl && enrollUrl ? (
          <a
            href={enrollUrl}
            target="_blank"
            rel="noopener noreferrer"
            className="mx-auto mb-3 block w-fit"
          >
            <img
              src={qrDataUrl}
              alt="Kiosk enrollment QR code"
              className="size-70"
            />
          </a>
        ) : (
          <LoadingIndicator />
        )}

        {fingerprint && (
          <p className="mb-3 text-center font-mono text-xs break-all opacity-60">
            {fingerprint.slice(0, 16)}…
          </p>
        )}

        <Button type="button" size="panel" onClick={onUseCodeInstead}>
          Use a setup code instead
        </Button>
      </PanelBox>
    </Panel>
  );
}
