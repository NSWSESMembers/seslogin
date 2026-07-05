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
      <footer className="bg-[#dddddd] p-2.5 text-[12px] text-[#333333]">
        NSW SES Volunteers &mdash; SES Activity v2 &mdash;{" "}
        <ClientVersionLabel />
      </footer>
    </ErrorBoundary>
  );
}
