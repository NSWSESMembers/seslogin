import { useEffect, useState } from "react";
import QRCode from "qrcode";
import {
  Panel,
  PanelBox,
  PanelTitle,
  PanelIntro,
} from "../../components/ui/Panel";
import { Button } from "../../components/ui/Button";
import LoadingIndicator from "../../components/LoadingIndicator";
import { getGraphQLEndpoint } from "../../lib/api";
import {
  CLIENT_VERSION_HEADER,
  getCurrentClientVersion,
} from "../../lib/clientVersion";
import {
  buildSignedAuthHeader,
  getOrCreateKioskKey,
  type KioskKeyInfo,
} from "../lib/kioskKey";
import { pollDelayMs } from "../lib/enrollPolling";

// Re-publish the public key this often while the kiosk waits to be enrolled. The
// server keeps the pending record for 30 min, so 10 min keeps it comfortably alive.
const SUBMIT_INTERVAL_MS = 10 * 60 * 1000;

function baseHeaders(): Record<string, string> {
  return {
    "Content-Type": "application/json",
    [CLIENT_VERSION_HEADER]: getCurrentClientVersion(),
  };
}

/** Publish the public key as a pending enrollment (unauthenticated). */
async function submitEnrollmentKey(info: KioskKeyInfo): Promise<void> {
  const body = JSON.stringify({
    query:
      "mutation KioskSubmitEnrollmentKey($publicKey: String!) { submitEnrollmentKey(publicKey: $publicKey) }",
    variables: { publicKey: info.publicKeySpkiB64 },
  });
  await fetch(getGraphQLEndpoint(), {
    method: "POST",
    headers: baseHeaders(),
    body,
    cache: "no-store",
  });
}

/**
 * Attempt a signed request. Resolves true once the kiosk is enrolled (the signed
 * `session` query succeeds); false while still pending (a 401 is expected until then).
 */
async function pollEnrolled(info: KioskKeyInfo): Promise<boolean> {
  const body = JSON.stringify({
    query: "query KioskEnrollPoll { session { id } }",
  });
  const authorization = await buildSignedAuthHeader(info, body);
  const resp = await fetch(getGraphQLEndpoint(), {
    method: "POST",
    headers: { ...baseHeaders(), Authorization: authorization },
    body,
    cache: "no-store",
  });
  if (resp.status === 401) {
    return false;
  }
  if (!resp.ok) {
    return false;
  }
  const json = await resp.json();
  return Boolean(json?.data?.session?.id);
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
  const [qrDataUrl, setQrDataUrl] = useState<string | null>(null);
  const [enrollUrl, setEnrollUrl] = useState<string | null>(null);
  const [fingerprint, setFingerprint] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    let submitTimeout: number | null = null;
    let pollTimeout: number | null = null;
    const startedAt = Date.now();

    getOrCreateKioskKey(profile)
      .then(async (info) => {
        if (cancelled) return;
        setFingerprint(info.fingerprint);

        const enrollUrl = `${window.location.origin}/admin/sessions/enroll?fp=${info.fingerprint}`;
        setEnrollUrl(enrollUrl);
        QRCode.toDataURL(enrollUrl, { width: 320, margin: 2 })
          .then((url) => {
            if (!cancelled) setQrDataUrl(url);
          })
          .catch((err) => console.error("Failed to render QR code:", err));

        const runSubmit = async () => {
          if (cancelled) return;
          try {
            await submitEnrollmentKey(info);
          } catch (err) {
            console.error("Failed to submit enrollment key:", err);
          }
          if (!cancelled) {
            submitTimeout = window.setTimeout(runSubmit, SUBMIT_INTERVAL_MS);
          }
        };

        const runPoll = async () => {
          if (cancelled) return;
          let enrolled = false;
          try {
            enrolled = await pollEnrolled(info);
          } catch (err) {
            console.error("Enrollment poll failed:", err);
          }
          if (cancelled) return;
          if (enrolled) {
            onEnrolled();
            return;
          }
          pollTimeout = window.setTimeout(
            runPoll,
            pollDelayMs(Date.now() - startedAt),
          );
        };

        runSubmit();
        runPoll();
      })
      .catch((err) => console.error("Failed to load kiosk signing key:", err));

    return () => {
      cancelled = true;
      if (submitTimeout !== null) window.clearTimeout(submitTimeout);
      if (pollTimeout !== null) window.clearTimeout(pollTimeout);
    };
    // `onEnrolled` is a stable useCallback from KioskEnvironment, so this effect (and
    // its submit/poll loops) is set up once per profile, not on every render.
  }, [profile, onEnrolled]);

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
              className="h-70 w-70"
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
