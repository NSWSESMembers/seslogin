import { Suspense, useMemo, useState } from "react";
import { RelayEnvironmentProvider } from "react-relay";
import { useLocation } from "react-router";
import { ErrorBoundary } from "react-error-boundary";

import { createPeriodLinkGraphQLEnvironment } from "../lib/environments";
import LoadingIndicator from "../components/LoadingIndicator";
import {
  Panel,
  PanelBox,
  PanelIntro,
  PanelTitle,
} from "../components/ui/Panel";
import PeriodEditForm from "./PeriodEditForm";

/**
 * Shown for anything that means "this link won't work": no token in the URL, an
 * expired or unknown token, or a period that has since been removed.
 *
 * Deliberately not `PageErrorFallback` — its "Show details" button would put raw
 * GraphQL error text in front of a member, and the server keeps that text uniform
 * precisely so it reveals nothing. There is also no "Try again": retrying an
 * expired link never succeeds, so the only useful next step is a fresh link.
 */
function LinkProblem({ children }: { children: string }) {
  return (
    <Panel>
      <PanelBox>
        <PanelTitle>Link not valid</PanelTitle>
        <PanelIntro>{children}</PanelIntro>
        <p className="text-ink-muted">
          Ask your unit to send you a new link, then try again.
        </p>
      </PanelBox>
    </Panel>
  );
}

/**
 * Member-facing single-period edit page, reached at `/period#<token>`.
 *
 * The `slp_` token lives in the URL fragment on purpose: browsers never send a
 * fragment to the server, so it stays out of access logs and `Referer` headers in
 * a way a query parameter would not.
 */
export default function PeriodEdit() {
  const { hash } = useLocation();

  // Capture the token once. The environment is keyed to it, so re-reading a
  // changing hash would tear down the Relay store mid-edit for no benefit.
  const [token] = useState(() => {
    try {
      return decodeURIComponent(hash.replace(/^#/, "")).trim();
    } catch {
      // A malformed percent-escape is just a broken link.
      return "";
    }
  });

  const [expired, setExpired] = useState(false);

  const environment = useMemo(
    () =>
      token
        ? createPeriodLinkGraphQLEnvironment(token, () => setExpired(true))
        : null,
    [token],
  );

  if (!token) {
    return (
      <LinkProblem>
        This link is incomplete — it's missing the part that identifies your
        time entry. Copying the whole link from the original message usually
        fixes it.
      </LinkProblem>
    );
  }

  if (expired || !environment) {
    return (
      <LinkProblem>
        This link has expired or is no longer valid. Edit links last 48 hours.
      </LinkProblem>
    );
  }

  return (
    <RelayEnvironmentProvider environment={environment}>
      {/* Both boundaries sit inside the provider so the environment stays stable
          across a suspend/retry cycle. */}
      <ErrorBoundary
        fallback={
          <LinkProblem>
            We couldn't load your time entry. This can happen if the link has
            expired, or if something went wrong on our end.
          </LinkProblem>
        }
      >
        <Suspense fallback={<LoadingIndicator />}>
          <PeriodEditForm />
        </Suspense>
      </ErrorBoundary>
    </RelayEnvironmentProvider>
  );
}
