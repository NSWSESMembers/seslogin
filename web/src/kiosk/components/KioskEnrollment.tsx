import { useEffect, type ReactNode } from "react";
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

const STEPS: { icon: ReactNode; text: ReactNode }[] = [
  {
    icon: (
      <path d="M4 7V5a1 1 0 0 1 1-1h2M17 4h2a1 1 0 0 1 1 1v2M20 17v2a1 1 0 0 1-1 1h-2M7 20H5a1 1 0 0 1-1-1v-2M7 8h2v2H7zM15 8h2v2h-2zM7 14h2v2H7zM12 8v2M15 14h.01M15 17h2v2h-2zM12 14v6" />
    ),
    text: (
      <>
        Ask someone with administrator access to scan this code with their phone
        or computer camera.
      </>
    ),
  },
  {
    icon: (
      <path d="M9 12l2 2 4-4M7 4h10a2 2 0 0 1 2 2v13.5a.5.5 0 0 1-.74.44L15 18l-3 2-3-2-3.26 1.94A.5.5 0 0 1 5 19.5V6a2 2 0 0 1 2-2z" />
    ),
    text: (
      <>
        It opens the enrollment page, already filled in for this device — they
        just pick the location and name, then save.
      </>
    ),
  },
  {
    icon: (
      <path d="M4 12a8 8 0 0 1 14.5-4.5M20 12a8 8 0 0 1-14.5 4.5M8 7.5H4V3.5M16 16.5h4v4" />
    ),
    text: (
      <>
        This screen switches over automatically within a few seconds — no code
        to type.
      </>
    ),
  },
];

function StepIcon({ children }: { children: ReactNode }) {
  return (
    <svg
      aria-hidden="true"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.6}
      strokeLinecap="round"
      strokeLinejoin="round"
      className="size-6 shrink-0 text-accent"
    >
      {children}
    </svg>
  );
}

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
          Scan the QR code with a phone or computer that has administrator
          access to finish setting up this kiosk — no code to type.
        </PanelIntro>

        {qrDataUrl && enrollUrl ? (
          <a
            href={enrollUrl}
            target="_blank"
            rel="noopener noreferrer"
            className="mx-auto mb-5 block w-fit"
          >
            <img
              src={qrDataUrl}
              alt="Kiosk enrollment QR code"
              className="aspect-square w-70 max-w-full"
            />
          </a>
        ) : (
          <LoadingIndicator />
        )}

        {fingerprint && (
          <p className="mb-5 text-center font-mono text-xs break-all opacity-60">
            {fingerprint.slice(0, 16)}…
          </p>
        )}

        <ol className="mb-6 flex flex-col gap-4">
          {STEPS.map((step, index) => (
            <li key={index} className="flex items-start gap-3">
              <StepIcon>{step.icon}</StepIcon>
              <p className="m-0 text-sm text-ink">{step.text}</p>
            </li>
          ))}
        </ol>

        <Button type="button" size="panel" onClick={onUseCodeInstead}>
          Use a setup code instead
        </Button>
      </PanelBox>
    </Panel>
  );
}
