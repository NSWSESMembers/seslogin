import { Suspense } from "react";
import GraphiQLLink from "../../components/GraphiQLLink";
import ClientVersionLabel from "../../components/ClientVersionLabel";
import HelpLink from "../../components/HelpLink";
import LoadingIndicator from "../../components/LoadingIndicator";
import RelayErrorBoundary from "../../components/RelayErrorBoundary";
import HelpForm from "./HelpForm";

export default function Footer() {
  return (
    <footer className="bg-surface-sunken p-2.5 text-center text-xs text-ink-muted">
      NSW SES Volunteers &mdash; SES Activity v2 &mdash; <ClientVersionLabel />
      &mdash; <GraphiQLLink /> &mdash;{" "}
      {/* A signed-in admin gets the ticket form (when configured): it files
          from their verified address. */}
      <HelpLink>
        {(close) => (
          <RelayErrorBoundary resetKey="help-dialog">
            <Suspense fallback={<LoadingIndicator />}>
              <HelpForm onSent={close} onCancel={close} />
            </Suspense>
          </RelayErrorBoundary>
        )}
      </HelpLink>
    </footer>
  );
}
