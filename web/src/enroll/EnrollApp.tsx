import { Suspense } from "react";
import { ErrorBoundary } from "react-error-boundary";
import AuthenticatedSession from "../admin/components/AuthenticatedSession";
import LoadingIndicator from "../components/LoadingIndicator";
import PageErrorFallback from "../components/PageErrorFallback";
import RelayErrorBoundary from "../components/RelayErrorBoundary";
import SessionEnroll from "./SessionEnroll";

/**
 * Standalone entry point for kiosk enrollment (`/enroll?fp=...`), reached by scanning
 * the QR code a kiosk shows during setup — almost always on a phone. It shares the
 * admin dashboard's login and session plumbing (`AuthenticatedSession`: the same
 * seslogin token, Relay environment, user info, notifications), but skips the
 * dashboard's chrome entirely — no menu bar, submenu, title bar, or location
 * interstitial — and the passkey-enrollment nudge, so scanning the code goes straight
 * to one focused, mobile-sized form. `SessionEnroll` supplies its own full-screen
 * card layout (matching the login screen it may have just been shown), so there's
 * nothing to wrap it in here beyond the error/suspense boundaries every query needs.
 */
export default function EnrollApp() {
  return (
    <ErrorBoundary FallbackComponent={PageErrorFallback}>
      <AuthenticatedSession>
        <RelayErrorBoundary canRetry>
          <Suspense fallback={<LoadingIndicator />}>
            <SessionEnroll />
          </Suspense>
        </RelayErrorBoundary>
      </AuthenticatedSession>
    </ErrorBoundary>
  );
}
