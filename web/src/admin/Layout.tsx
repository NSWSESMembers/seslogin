import { Suspense } from "react";
import { ErrorBoundary } from "react-error-boundary";
import { Outlet } from "react-router";
import AdminContent from "./components/AdminContent";
import AuthenticatedSession from "./components/AuthenticatedSession";
import { useLogout } from "./components/useLogout";
import LoadingIndicator from "../components/LoadingIndicator";
import PageErrorFallback from "../components/PageErrorFallback";
import PasskeyEnrollPrompt from "./components/PasskeyEnrollPrompt";

export default function Layout() {
  return (
    <div>
      <ErrorBoundary FallbackComponent={PageErrorFallback}>
        <AuthenticatedSession>
          <PasskeyEnrollPrompt>
            <AdminShell />
          </PasskeyEnrollPrompt>
        </AuthenticatedSession>
      </ErrorBoundary>
    </div>
  );
}

function AdminShell() {
  const onLogout = useLogout();

  return (
    <AdminContent onLogout={onLogout}>
      <Suspense fallback={<LoadingIndicator />}>
        <Outlet />
      </Suspense>
    </AdminContent>
  );
}
