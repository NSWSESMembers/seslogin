import { lazy, Suspense } from "react";
import Router from "./Router";
import "./app.css";

// Dev server only: a constant `false` in production builds, so the chunk is never emitted.
const DevBar = import.meta.env.DEV ? lazy(() => import("./dev/DevBar")) : null;

export default function App() {
  return (
    <>
      <Router />
      {DevBar && (
        <Suspense>
          <DevBar />
        </Suspense>
      )}
    </>
  );
}
