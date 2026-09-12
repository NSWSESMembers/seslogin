import { useEffect, type ReactNode } from "react";
import {
  Panel,
  PanelBox,
  PanelTitle,
  PanelIntro,
} from "../../components/ui/Panel";
import { Button } from "../../components/ui/Button";
import LoadingIndicator from "../../components/LoadingIndicator";
import { FingerprintChip } from "../../components/FingerprintChip";
import { fetchKeySessionId } from "../lib/enrollmentKey";
import { useEnrollmentQr } from "../lib/useEnrollmentQr";
import { pollDelayMs } from "../lib/enrollPolling";

const STEPS: { icon: ReactNode; text: ReactNode }[] = [
  {
    icon: (
      <path d="M3 9V5a2 2 0 0 1 2-2h4M15 3h4a2 2 0 0 1 2 2v4M21 15v4a2 2 0 0 1-2 2h-4M9 21H5a2 2 0 0 1-2-2v-4M7 7h3v3H7zM14 7h3v3h-3zM7 14h3v3H7zM14.5 14.5h.01M17 14.5h.01M14.5 17h.01M17 17h.01" />
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
      <path d="M13.5 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8.5zM13.5 3v5.5H19M8.5 12h7M9 16.5l2 2 4-4" />
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
      <path d="M3 12a9 9 0 0 1 15.4-6.4L21 8M21 3v5h-5M21 12a9 9 0 0 1-15.4 6.4L3 16M3 21v-5h5" />
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
          <p className="mb-5 text-center text-xs opacity-60">
            <FingerprintChip fingerprint={fingerprint} className="text-xs" />
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
