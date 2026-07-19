import { Suspense } from "react";
import { useParams } from "react-router";
import { ErrorBoundary } from "react-error-boundary";
import { RelayEnvironmentProvider } from "react-relay";
import { unauthenticatedEnvironment } from "../lib/environments";
import LoadingIndicator from "../components/LoadingIndicator";
import PageErrorFallback from "../components/PageErrorFallback";
import SignOutSession from "./SignOutSession";

import "../global.css";
import "./style.css";

export default function SignOutPage() {
  const { personId } = useParams();

  return (
    <section className="action-panel">
      <div className="action-panel__panel signout-panel">
        <ErrorBoundary FallbackComponent={PageErrorFallback}>
          <Suspense fallback={<LoadingIndicator />}>
            {personId ? (
              <RelayEnvironmentProvider
                environment={unauthenticatedEnvironment}
              >
                <SignOutSession personId={personId} />
              </RelayEnvironmentProvider>
            ) : (
              <p className="action-panel__message action-panel__message--error">
                This link isn't valid.
              </p>
            )}
          </Suspense>
        </ErrorBoundary>
      </div>
    </section>
  );
}
