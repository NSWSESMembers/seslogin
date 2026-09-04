import { Suspense, startTransition, useState, type ReactNode } from "react";
import SettingsProvider from "./SettingsProvider";
import { UserInfoProvider } from "./UserInfoProvider";
import { NotificationProvider } from "./Notifications";
import AdminRelayEnvironment from "./AdminRelayEnvironment";
import AdminLoginPage from "./AdminLoginPage";
import { LogoutContext } from "./useLogout";
import LoadingIndicator from "../../components/LoadingIndicator";
import RelayErrorBoundary from "../../components/RelayErrorBoundary";
import { clearPasskeyLoginSession } from "../../lib/passkey";
import {
  getAdminToken,
  setAdminToken,
  clearAdminToken,
} from "../../lib/adminToken";
import { getGraphQLEndpoint } from "../../lib/api";
import { clientHeaders } from "../../lib/clientInfo";

// Admin auth relies solely on our own opaque seslogin token (issued by the
// email-code and passkey login flows) stored in localStorage. The view is a
// single state machine so invalid flag combinations can't occur.
type Status =
  | { kind: "authenticated" }
  | { kind: "loggingOut" }
  | { kind: "unauthenticated"; error: string | null };

/**
 * Login gate plus the provider stack every authenticated seslogin-token area needs:
 * settings, the token-bearing Relay environment, user info, and toast notifications.
 * Shared by the full admin dashboard (`admin/Layout.tsx`) and any other top-level area
 * that reuses the same admin session — e.g. the standalone kiosk enrollment page — so
 * the two can't drift on how login, token refresh, or logout behave.
 *
 * Deliberately stops short of anything about page chrome (menus, title bars) or
 * passkey nudges: those are a per-area choice layered on top by the caller.
 */
export default function AuthenticatedSession({
  children,
}: {
  children: ReactNode;
}) {
  const [status, setStatus] = useState<Status>(() =>
    getAdminToken()
      ? { kind: "authenticated" }
      : { kind: "unauthenticated", error: null },
  );

  // The server rejected our token (expired or revoked). Discard it and send
  // the user back to the login window with a clear message. This only fires on
  // a definitive 401 — transient 5xx / network failures never reach here, so we
  // never drop a still-valid token over a blip.
  function onUnauthorized() {
    clearAdminToken();
    setStatus({
      kind: "unauthenticated",
      error:
        "Your session has expired or is no longer valid, please login again.",
    });
  }

  // Relay couldn't obtain a token to send (getToken threw because there's no
  // stored token). This shouldn't normally happen: the authenticated tree only
  // mounts when a token exists, so reaching here means the token vanished
  // mid-session — an unexpected state rather than ordinary expiry. Hence the
  // more generic wording vs. onUnauthorized's "session expired" message.
  function onTokenError() {
    setStatus({
      kind: "unauthenticated",
      error:
        "An unexpected error occurred while fetching an auth token. Please log in again.",
    });
  }

  function onNewTokenReceived(token: string) {
    setAdminToken(token);
    startTransition(() => {
      setStatus({ kind: "authenticated" });
    });
  }

  async function onLogout() {
    // Switch to the loading view immediately so we don't briefly mount
    // AdminLoginPage (which would kick off a wasteful passkey autofill /
    // BeginPasskeyLogin) while the logout request is in flight.
    setStatus({ kind: "loggingOut" });
    const token = getAdminToken();
    if (token) {
      try {
        await fetch(getGraphQLEndpoint(), {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            Authorization: `Bearer ${token}`,
            ...clientHeaders(),
          },
          body: JSON.stringify({ query: "mutation { logout }" }),
          cache: "no-store",
        });
      } catch {
        // Ignore — token will expire via TTL regardless
      }
      clearAdminToken();
    }
    clearPasskeyLoginSession();
    window.location.href = "/";
  }

  if (status.kind === "loggingOut") {
    return <LoadingIndicator />;
  }

  if (status.kind === "unauthenticated") {
    return (
      <AdminLoginPage
        errorMessage={status.error}
        onNewTokenReceived={onNewTokenReceived}
      />
    );
  }

  return (
    <SettingsProvider>
      <AdminRelayEnvironment
        onTokenError={onTokenError}
        onUnauthorized={onUnauthorized}
      >
        {/* canRetry: the one query reachable here before the caller's own
            boundary takes over — UserInfoProvider's — threads
            useRelayRetryFetchKey() into its useLazyLoadQuery call. Anything the
            caller renders that reads passkeys off the same query (e.g.
            PasskeyEnrollPrompt) must stay below UserInfoProvider (and below
            NotificationProvider, whose useNotify it may call). */}
        <RelayErrorBoundary canRetry>
          <Suspense fallback={<LoadingIndicator />}>
            <UserInfoProvider>
              <NotificationProvider>
                <LogoutContext value={onLogout}>{children}</LogoutContext>
              </NotificationProvider>
            </UserInfoProvider>
          </Suspense>
        </RelayErrorBoundary>
      </AdminRelayEnvironment>
    </SettingsProvider>
  );
}
