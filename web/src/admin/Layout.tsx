import { Suspense, startTransition, useState } from "react";
import { ErrorBoundary } from "react-error-boundary";
import AdminContent from "./components/AdminContent";
import LoadingIndicator from "../components/LoadingIndicator";
import PageErrorFallback from "../components/PageErrorFallback";
import SettingsProvider from "./components/SettingsProvider";
import { UserInfoProvider } from "./components/UserInfoProvider";
import { NotificationProvider } from "./components/Notifications";
import AdminRelayEnvironment from "./components/AdminRelayEnvironment";
import { Outlet } from "react-router";
import AdminLoginPage from "./components/AdminLoginPage";
import PasskeyEnrollPrompt from "./components/PasskeyEnrollPrompt";
import { clearPasskeyLoginSession } from "../lib/passkey";
import {
  getAdminToken,
  setAdminToken,
  clearAdminToken,
} from "../lib/adminToken";
import { getGraphQLEndpoint } from "../lib/api";
import {
  getCurrentClientVersion,
  CLIENT_VERSION_HEADER,
} from "../lib/clientVersion";

export default function Layout() {
  return (
    <div>
      <ErrorBoundary FallbackComponent={PageErrorFallback}>
        <LoginRequired />
      </ErrorBoundary>
    </div>
  );
}

// Admin auth relies solely on our own opaque seslogin token (issued by the
// email-code and passkey login flows) stored in localStorage. The view is a
// single state machine so invalid flag combinations can't occur.
type Status =
  | { kind: "authenticated" }
  | { kind: "loggingOut" }
  | { kind: "unauthenticated"; error: string | null };

function LoginRequired() {
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
            [CLIENT_VERSION_HEADER]: getCurrentClientVersion(),
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
        <ErrorBoundary FallbackComponent={PageErrorFallback}>
          <Suspense fallback={<LoadingIndicator />}>
            <UserInfoProvider>
              <NotificationProvider>
                <PasskeyEnrollPrompt>
                  <AdminContent onLogout={onLogout}>
                    <Suspense fallback={<LoadingIndicator />}>
                      <Outlet />
                    </Suspense>
                  </AdminContent>
                </PasskeyEnrollPrompt>
              </NotificationProvider>
            </UserInfoProvider>
          </Suspense>
        </ErrorBoundary>
      </AdminRelayEnvironment>
    </SettingsProvider>
  );
}
