import { Suspense } from "react";
import { BrowserRouter, Routes, Route, Navigate } from "react-router";
import { ErrorBoundary } from "react-error-boundary";

import { lazyWithReload } from "./lib/lazyWithReload";
import LoadingIndicator from "./components/LoadingIndicator";
import PageErrorFallback from "./components/PageErrorFallback";

// Home is the landing page — keep it eager for fast first paint.
import HomeLayout from "./home/Layout";
import Home from "./home/Home";

// Admin and kiosk are mutually-exclusive areas, lazily loaded as separate chunks.
const AdminApp = lazyWithReload("admin", () => import("./admin/AdminApp"));
const KioskMain = lazyWithReload("kiosk", () => import("./kiosk/KioskMain"));
const StatusDemo = lazyWithReload(
  "kiosk",
  () => import("./kiosk/pages/StatusDemo"),
);
const PeriodEdit = lazyWithReload(
  "period",
  () => import("./period/PeriodEdit"),
);

export default function Router() {
  return (
    <BrowserRouter>
      {/* Backstop for routes with no error boundary of their own (/scan,
          /kiosk/status-demo, *) and for a throw before an area's own boundary
          mounts. Every area below installs a more specific boundary of its own;
          this one only sees what escapes those. */}
      <ErrorBoundary FallbackComponent={PageErrorFallback}>
        <Suspense fallback={<LoadingIndicator />}>
          <Routes>
            {/* Home routes - no auth required */}
            <Route path="/" element={<HomeLayout />}>
              <Route index element={<Home />} />
            </Route>

            {/* Admin routes - auth required at /admin/* */}
            <Route path="/admin/*" element={<AdminApp />} />

            {/* Kiosk routes - auth required at /kiosk */}
            <Route path="/scan" element={<Navigate to="/kiosk" replace />} />
            <Route path="/kiosk/status-demo" element={<StatusDemo />} />
            <Route path="/kiosk" element={<KioskMain />} />
            <Route path="/kiosk/:profile" element={<KioskMain />} />

            {/* Member edit link - authenticated by the slp_ token in the URL
                fragment, which browsers never send to the server */}
            <Route path="/period" element={<PeriodEdit />} />

            {/* Catch all */}
            <Route path="*" element={<h1>Not Found</h1>} />
          </Routes>
        </Suspense>
      </ErrorBoundary>
    </BrowserRouter>
  );
}
