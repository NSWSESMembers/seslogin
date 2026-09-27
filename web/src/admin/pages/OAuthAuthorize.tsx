import { useState } from "react";
import { useSearchParams } from "react-router";
import { graphql, useMutation } from "react-relay";
import { useRetryableLazyLoadQuery } from "../../components/useRetryableLazyLoadQuery";
import { useUserInfo } from "../components/useUserInfo";
import { useNotify } from "../components/useNotify";
import {
  Panel,
  PanelBox,
  PanelTitle,
  PanelIntro,
  PanelMessage,
} from "../../components/ui/Panel";
import { Button } from "../../components/ui/Button";
import type { OAuthAuthorizeQuery } from "./__generated__/OAuthAuthorizeQuery.graphql";
import type { OAuthAuthorizeMutation } from "./__generated__/OAuthAuthorizeMutation.graphql";

/** Everything the consent screen needs out of the query string, once validated. */
interface AuthorizationParams {
  clientId: string;
  redirectUri: string;
  codeChallenge: string;
  codeChallengeMethod: string;
  scope: string | null;
  resource: string | null;
  state: string | null;
}

/**
 * Read and shape-check the OAuth params from the query string. Doesn't touch
 * the network — an unknown client or unregistered redirect_uri is caught
 * later, by the `oauthAuthorizationRequest` query itself.
 */
function parseParams(search: URLSearchParams): AuthorizationParams | null {
  const clientId = search.get("client_id");
  const redirectUri = search.get("redirect_uri");
  const codeChallenge = search.get("code_challenge");
  const codeChallengeMethod = search.get("code_challenge_method");
  const responseType = search.get("response_type");
  if (
    !clientId ||
    !redirectUri ||
    !codeChallenge ||
    !codeChallengeMethod ||
    responseType !== "code"
  ) {
    return null;
  }
  return {
    clientId,
    redirectUri,
    codeChallenge,
    codeChallengeMethod,
    scope: search.get("scope"),
    resource: search.get("resource"),
    state: search.get("state"),
  };
}

/** Append `error=access_denied` (+ `state`, if given) to an already-validated redirect_uri. */
function denialUrl(redirectUri: string, state: string | null): string {
  const url = new URL(redirectUri);
  url.searchParams.set("error", "access_denied");
  if (state) url.searchParams.set("state", state);
  return url.toString();
}

// Mounted at /admin/oauth/authorize. Reads the OAuth authorization request out
// of the query string and shows a consent screen — this is the browser-facing
// half of the MCP authorization flow; see CLAUDE.md and `api/src/oauth.rs`.
//
// The route sits inside AuthenticatedSession (see AdminApp.tsx) so a logged-out
// visit shows the ordinary login screen first: the URL and its query string
// never change, so once login succeeds this same component re-renders with
// the request still intact.
export default function OAuthAuthorize() {
  const [search] = useSearchParams();
  const params = parseParams(search);

  // Clickjacking guard: the site sends no frame-blocking headers, so refuse to
  // show an approve button inside someone else's frame.
  if (window.top !== window.self) {
    return (
      <Panel>
        <PanelBox>
          <PanelTitle>Open in a new tab</PanelTitle>
          <PanelMessage>
            For your security, this page can't be shown inside another site.
            Open it directly in your browser to continue.
          </PanelMessage>
        </PanelBox>
      </Panel>
    );
  }

  if (!params) {
    return (
      <Panel>
        <PanelBox>
          <PanelTitle>Invalid request</PanelTitle>
          <PanelMessage>
            This link is missing required parameters, or isn't an authorization
            request seslogin recognizes. Go back to the app or tool you started
            this from and try again.
          </PanelMessage>
        </PanelBox>
      </Panel>
    );
  }

  return <ConsentScreen {...params} />;
}

function ConsentScreen({
  clientId,
  redirectUri,
  codeChallenge,
  codeChallengeMethod,
  scope,
  resource,
  state,
}: AuthorizationParams) {
  const { notifyError } = useNotify();
  const user = useUserInfo();
  const [denying, setDenying] = useState(false);

  const data = useRetryableLazyLoadQuery<OAuthAuthorizeQuery>(
    graphql`
      query OAuthAuthorizeQuery($clientId: String!, $redirectUri: String!)
      @throwOnFieldError {
        oauthAuthorizationRequest(
          clientId: $clientId
          redirectUri: $redirectUri
        ) {
          clientName
          redirectHost
        }
      }
    `,
    { clientId, redirectUri },
  );

  const [commitMutation, isApproving] = useMutation<OAuthAuthorizeMutation>(
    graphql`
      mutation OAuthAuthorizeMutation(
        $clientId: String!
        $redirectUri: String!
        $codeChallenge: String!
        $codeChallengeMethod: String!
        $scope: String
        $resource: String
        $state: String
      ) {
        approveOauthAuthorization(
          clientId: $clientId
          redirectUri: $redirectUri
          codeChallenge: $codeChallenge
          codeChallengeMethod: $codeChallengeMethod
          scope: $scope
          resource: $resource
          state: $state
        )
      }
    `,
  );

  async function handleApprove() {
    try {
      const result = await new Promise<OAuthAuthorizeMutation["response"]>(
        (resolve, reject) => {
          commitMutation({
            variables: {
              clientId,
              redirectUri,
              codeChallenge,
              codeChallengeMethod,
              scope,
              resource,
              state,
            },
            onCompleted: resolve,
            onError: reject,
          });
        },
      );
      window.location.assign(result.approveOauthAuthorization);
    } catch (err) {
      notifyError(err, "Couldn't approve this request");
    }
  }

  function handleDeny() {
    setDenying(true);
    window.location.assign(denialUrl(redirectUri, state));
  }

  const { clientName, redirectHost } = data.oauthAuthorizationRequest;
  const busy = isApproving || denying;

  return (
    <Panel>
      <PanelBox>
        <PanelTitle>Connect {clientName}?</PanelTitle>
        <PanelIntro>
          <strong>{clientName}</strong> wants to connect to your seslogin
          account.
        </PanelIntro>
        <PanelMessage variant="warning">
          It will redirect to <strong>{redirectHost}</strong> once approved.
          Only approve this if you started this connection yourself, from that
          app or AI tool — don't approve it because a link or message told you
          to.
        </PanelMessage>
        <p className="my-4">
          This will let it act as you (<strong>{user.email}</strong>) in
          seslogin, with all of your permissions.
        </p>
        <div className="flex flex-row flex-wrap items-center gap-3">
          <Button
            variant="primary"
            size="panel"
            onClick={handleApprove}
            disabled={busy}
          >
            {isApproving ? "Approving…" : "Approve"}
          </Button>
          <Button
            variant="secondary"
            size="panel"
            onClick={handleDeny}
            disabled={busy}
          >
            Deny
          </Button>
        </div>
      </PanelBox>
    </Panel>
  );
}
