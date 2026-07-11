import { RelayEnvironmentProvider } from "react-relay";
import { useEffect, useMemo, useState, type ReactNode } from "react";
import { createKioskKeyGraphQLEnvironment } from "../../lib/environments";
import {
  buildSignedAuthHeader,
  getOrCreateKioskKey,
  type KioskKeyInfo,
} from "../lib/kioskKey";
import LoadingIndicator from "../../components/LoadingIndicator";

/**
 * Relay environment provider for a kiosk enrolled via public key. Loads the persisted
 * signing key, then signs every request with it. A 401 (session disabled / key expired)
 * calls onUnauthorized, which reverts the kiosk to the enrollment screen.
 */
export default function KioskKeyRelayEnvironment({
  profile,
  onUnauthorized,
  children,
}: {
  profile: string;
  onUnauthorized: () => void;
  children: ReactNode;
}) {
  const [keyInfo, setKeyInfo] = useState<KioskKeyInfo | null>(null);

  useEffect(() => {
    let cancelled = false;
    getOrCreateKioskKey(profile)
      .then((info) => {
        if (!cancelled) {
          setKeyInfo(info);
        }
      })
      .catch((err) => {
        console.error("Failed to load kiosk signing key:", err);
        onUnauthorized();
      });
    return () => {
      cancelled = true;
    };
  }, [profile, onUnauthorized]);

  const environment = useMemo(() => {
    if (!keyInfo) return null;
    return createKioskKeyGraphQLEnvironment(
      (body) => buildSignedAuthHeader(keyInfo, body),
      onUnauthorized,
    );
  }, [keyInfo, onUnauthorized]);

  if (!environment) {
    return <LoadingIndicator />;
  }

  return (
    <RelayEnvironmentProvider environment={environment}>
      {children}
    </RelayEnvironmentProvider>
  );
}
