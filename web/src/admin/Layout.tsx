import { useAuth0 } from "@auth0/auth0-react";
import { Suspense, startTransition, useEffect, useState } from "react";
import { ErrorBoundary } from "react-error-boundary";
import AdminContent from "./components/AdminContent";
import LoadingIndicator from "../components/LoadingIndicator";
import PageErrorFallback from "../components/PageErrorFallback";
import { Auth0Provider } from "@auth0/auth0-react";
import SettingsProvider from "./components/SettingsProvider";
import "./style.css";
import { UserInfoProvider } from "./components/UserInfoProvider";
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
    <div id="admin">
      <ErrorBoundary FallbackComponent={PageErrorFallback}>
        <AdminAuthProvider />
      </ErrorBoundary>
    </div>
  );
}

function AdminAuthProvider() {
  return (
    <Auth0Provider
      domain="auth.seslogin.com"
      clientId="5dECCcUEKvNNVpp0cyss4O4nI4RkrJaw"
      authorizationParams={{
        redirect_uri:
          typeof window !== "undefined" ? window.location.origin : "",
        audience: "https://api.seslogin.com",
      }}
      cacheLocation="localstorage"
    >
      <LoginRequired />
    </Auth0Provider>
  );
}

type AuthState = "loading" | "authenticated" | "unauthenticated";

function LoginRequired() {
  const { isAuthenticated, loginWithPopup, logout, isLoading } = useAuth0();
  console.log("Auth0 loading:", isLoading, "authenticated:", isAuthenticated);

  const hasStoredToken = getAdminToken() !== null;

  const [authState, setAuthState] = useState<AuthState>(
    isLoading
      ? "loading"
      : isAuthenticated || hasStoredToken
        ? "authenticated"
        : "unauthenticated",
  );
  const [accessDenied, setAccessDenied] = useState(false);
  const [isLoggingIn, setIsLoggingIn] = useState(false);
  const [loggingOut, setLoggingOut] = useState(false);
  const [loginError, setLoginError] = useState<string | null>(null);

  useEffect(() => {
    if (authState === "loading" && !isLoading) {
      startTransition(() => {
        if (isAuthenticated || getAdminToken() !== null) {
          setAuthState("authenticated");
        } else {
          setAuthState("unauthenticated");
        }
      });
    }
  }, [isLoading, isAuthenticated, authState]);

  function onUnauthorized() {
    // Distinguish which credential the rejected request used. AdminRelay-
    // Environment prefers the stored seslogin token and only falls back to
    // Auth0 when there's none, so a present stored token means our own token
    // was rejected. This only fires on a definitive 401 — transient 5xx /
    // network failures never reach here, so we never drop a valid token over a
    // blip.
    if (getAdminToken() !== null) {
      // Our seslogin token expired or was revoked: discard it and return the
      // user to the login window with a clear message.
      clearAdminToken();
      setAccessDenied(false);
      setLoginError(
        "Your session has expired or is no longer valid, please login again.",
      );
      setAuthState("unauthenticated");
    } else {
      // Auth0-authenticated request was rejected (e.g. the email isn't a
      // registered admin). Preserve the existing Auth0 behavior unchanged.
      clearAdminToken();
      setAuthState("unauthenticated");
      setAccessDenied(true);
    }
  }

  function onTokenError() {
    setAccessDenied(false);
    setLoginError(
      "Failed to get an auth token - your session may have expired. Please try logging in again.",
    );
    setAuthState("unauthenticated");
  }

  async function onLogin() {
    setAccessDenied(false);
    setLoginError(null);
    setIsLoggingIn(true);

    try {
      await loginWithPopup();
    } catch {
      setLoginError(
        "Login did not complete. Try again to receive a fresh email code.",
      );
    } finally {
      setIsLoggingIn(false);
      setAuthState("authenticated");
    }
  }

  function onNewTokenReceived(token: string) {
    setAdminToken(token);
    setAccessDenied(false);
    setLoginError(null);
    startTransition(() => {
      setAuthState("authenticated");
    });
  }

  async function onLogout() {
    // Show the loading indicator (not the login page) for the whole logout so
    // we don't briefly mount AdminLoginPage — which would kick off a wasteful
    // passkey autofill / BeginPasskeyLogin before the Auth0 redirect navigates
    // away.
    setLoggingOut(true);
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
    setAuthState("unauthenticated");
    setAccessDenied(false);
    logout({
      logoutParams: {
        returnTo: `${window.location.origin}/`,
      },
    });
  }

  if (authState === "loading" || loggingOut) {
    return <LoadingIndicator />;
  }

  if (authState !== "authenticated") {
    return (
      <AdminLoginPage
        onLogin={onLogin}
        onLogout={onLogout}
        isLoading={isLoggingIn}
        errorMessage={loginError}
        showUnauthorizedMessage={accessDenied}
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
              <PasskeyEnrollPrompt>
                <AdminContent onLogout={onLogout}>
                  <Suspense fallback={<LoadingIndicator />}>
                    <Outlet />
                  </Suspense>
                </AdminContent>
              </PasskeyEnrollPrompt>
            </UserInfoProvider>
          </Suspense>
        </ErrorBoundary>
      </AdminRelayEnvironment>
    </SettingsProvider>
  );
}
