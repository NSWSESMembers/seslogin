import { Suspense } from "react";
import { BrowserRouter, Routes, Route } from "react-router";

import { lazyWithReload } from "./lib/lazyWithReload";
import LoadingIndicator from "./components/LoadingIndicator";

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

export default function Router() {
  return (
    <BrowserRouter>
      <Suspense fallback={<LoadingIndicator />}>
        <Routes>
          {/* Home routes - no auth required */}
          <Route path="/" element={<HomeLayout />}>
            <Route index element={<Home />} />
          </Route>

          {/* Admin routes - auth required at /admin/* */}
          <Route path="/admin/*" element={<AdminApp />} />

          {/* Kiosk routes - auth required at /kiosk */}
          <Route path="/scan" element={<KioskMain />} />
          <Route path="/scan/status-demo" element={<StatusDemo />} />
          <Route path="/kiosk" element={<KioskMain />} />
          <Route path="/kiosk/:profile" element={<KioskMain />} />

          {/* Catch all */}
          <Route path="*" element={<h1>Not Found</h1>} />
        </Routes>
      </Suspense>
    </BrowserRouter>
  );
}
