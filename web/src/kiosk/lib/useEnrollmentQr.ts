import { useEffect, useState } from "react";
import QRCode from "qrcode";
import { getOrCreateKioskKey, type KioskKeyInfo } from "./kioskKey";
import {
  ENROLL_SUBMIT_INTERVAL_MS,
  enrollUrlForFingerprint,
  submitEnrollmentKey,
} from "./enrollmentKey";

export type EnrollmentQr = {
  /** This device's signing key, once loaded — the handle for signed requests. */
  info: KioskKeyInfo | null;
  fingerprint: string | null;
  /** The admin page the QR code points at. Never render this as a link on a kiosk. */
  enrollUrl: string | null;
  qrDataUrl: string | null;
  /** Set when the key, the QR render, or publishing the key failed. */
  error: string | null;
};

const IDLE: EnrollmentQr = {
  info: null,
  fingerprint: null,
  enrollUrl: null,
  qrDataUrl: null,
  error: null,
};

function message(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

/**
 * Load (or create) this kiosk's signing key, publish it as a pending enrollment, and
 * render the QR code an admin scans to enroll it — re-publishing periodically so the
 * code stays scannable for as long as it is on screen.
 *
 * Set `enabled` false to hold off entirely: nothing is generated and nothing is sent
 * to the server until the caller actually wants to show a code.
 */
export function useEnrollmentQr(
  profile: string,
  enabled: boolean = true,
): EnrollmentQr {
  const [state, setState] = useState<EnrollmentQr>(IDLE);

  useEffect(() => {
    if (!enabled) {
      setState(IDLE);
      return;
    }

    let cancelled = false;
    let submitTimeout: number | null = null;

    getOrCreateKioskKey(profile)
      .then((info) => {
        if (cancelled) return;
        const enrollUrl = enrollUrlForFingerprint(info.fingerprint);
        setState((prev) => ({
          ...prev,
          info,
          fingerprint: info.fingerprint,
          enrollUrl,
        }));

        QRCode.toDataURL(enrollUrl, { width: 320, margin: 2 })
          .then((qrDataUrl) => {
            if (!cancelled) setState((prev) => ({ ...prev, qrDataUrl }));
          })
          .catch((err) => {
            console.error("Failed to render QR code:", err);
            if (!cancelled) {
              setState((prev) => ({
                ...prev,
                error: `Couldn't draw the QR code: ${message(err)}`,
              }));
            }
          });

        const runSubmit = async () => {
          if (cancelled) return;
          try {
            await submitEnrollmentKey(info);
            if (!cancelled) setState((prev) => ({ ...prev, error: null }));
          } catch (err) {
            console.error("Failed to submit enrollment key:", err);
            if (!cancelled) {
              setState((prev) => ({
                ...prev,
                error: `Couldn't publish this device's key: ${message(err)}`,
              }));
            }
          }
          if (!cancelled) {
            submitTimeout = window.setTimeout(
              runSubmit,
              ENROLL_SUBMIT_INTERVAL_MS,
            );
          }
        };

        runSubmit();
      })
      .catch((err) => {
        console.error("Failed to load kiosk signing key:", err);
        if (!cancelled) {
          setState((prev) => ({
            ...prev,
            error: `Couldn't load this device's key: ${message(err)}`,
          }));
        }
      });

    return () => {
      cancelled = true;
      if (submitTimeout !== null) window.clearTimeout(submitTimeout);
    };
  }, [profile, enabled]);

  return state;
}
