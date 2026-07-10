import { Outlet, useLocation } from "react-router";
import { ErrorBoundary } from "react-error-boundary";
import TopBar from "../admin/components/TopBar";
import ClientVersionLabel from "../components/ClientVersionLabel";
import PageErrorFallback from "../components/PageErrorFallback";

export default function Layout() {
  const location = useLocation();

  return (
    <ErrorBoundary
      key={location.pathname}
      FallbackComponent={PageErrorFallback}
    >
      <TopBar username="" />
      <Outlet />
      <footer className="bg-surface-sunken p-2.5 text-xs text-ink-muted">
        NSW SES Volunteers &mdash; SES Activity v2 &mdash;{" "}
        <ClientVersionLabel />
      </footer>
    </ErrorBoundary>
  );
}
