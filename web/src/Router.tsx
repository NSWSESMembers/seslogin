import { Suspense } from "react";
import {
  BrowserRouter,
  Routes,
  Route,
  Navigate,
  useLocation,
} from "react-router";
import { ErrorBoundary } from "react-error-boundary";

import { lazyWithReload } from "./lib/lazyWithReload";
import LoadingIndicator from "./components/LoadingIndicator";
import PageErrorFallback from "./components/PageErrorFallback";

// Home is the landing page — keep it eager for fast first paint.
import HomeLayout from "./home/Layout";
import Home from "./home/Home";

// Admin and kiosk are mutually-exclusive areas, lazily loaded as separate chunks.
const AdminApp = lazyWithReload("admin", () => import("./admin/AdminApp"));
const EnrollApp = lazyWithReload("enroll", () => import("./enroll/EnrollApp"));
const KioskMain = lazyWithReload("kiosk", () => import("./kiosk/KioskMain"));
const StatusDemo = lazyWithReload("demo", () => import("./demo/StatusDemo"));
const TimeEntryDemo = lazyWithReload(
  "demo",
  () => import("./demo/TimeEntryDemo"),
);
const CategoryButtonDemo = lazyWithReload(
  "demo",
  () => import("./demo/CategoryButtonDemo"),
);
const PeriodEdit = lazyWithReload(
  "period",
  () => import("./period/PeriodEdit"),
);

/** Preserves `?fp=...` while sending an old enrollment link to its new home. */
function RedirectToEnroll() {
  const location = useLocation();
  return (
    <Navigate to={{ pathname: "/enroll", search: location.search }} replace />
  );
}

export default function Router() {
  return (
    <BrowserRouter>
      {/* Backstop for routes with no error boundary of their own (/scan,
          /demo/*, *) and for a throw before an area's own boundary
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

            {/* Kiosk enrollment - auth required, but a standalone mobile-first
                page rather than part of the admin dashboard (see EnrollApp.tsx).
                The old path redirects for any enrollment link/QR code shown by a
                kiosk that hasn't reloaded since this moved. */}
            <Route path="/enroll" element={<EnrollApp />} />
            <Route
              path="/admin/sessions/enroll"
              element={<RedirectToEnroll />}
            />

            {/* Kiosk routes - auth required at /kiosk */}
            <Route path="/scan" element={<Navigate to="/kiosk" replace />} />
            <Route path="/kiosk" element={<KioskMain />} />
            <Route path="/kiosk/:profile" element={<KioskMain />} />

            {/* Component demos - no auth, no data. Deliberately outside /kiosk
                so they can never be caught by kiosk enrolment or session
                handling, even though the components they exercise are kiosk
                ones. */}
            <Route path="/demo/status" element={<StatusDemo />} />
            <Route path="/demo/time" element={<TimeEntryDemo />} />
            <Route
              path="/demo/category/:id?"
              element={<CategoryButtonDemo />}
            />

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
