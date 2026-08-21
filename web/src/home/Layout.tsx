import { Outlet, useLocation } from "react-router";
import TopBar from "../admin/components/TopBar";
import ClientVersionLabel from "../components/ClientVersionLabel";
import PageErrorBoundary from "../components/PageErrorBoundary";
import HomeEnvironmentProbe from "./HomeEnvironmentProbe";

export default function Layout() {
  const location = useLocation();

  return (
    <PageErrorBoundary resetKey={location.pathname}>
      <HomeEnvironmentProbe />
      <TopBar username="" />
      <Outlet />
      <footer className="bg-surface-sunken p-2.5 text-xs text-ink-muted">
        NSW SES Volunteers &mdash; SES Activity v2 &mdash;{" "}
        <ClientVersionLabel />
      </footer>
    </PageErrorBoundary>
  );
}
