import { useAuth0 } from "@auth0/auth0-react";
import { getGraphQLEndpoint } from "../lib/api";
import { getAdminToken } from "../lib/adminToken";

export default function GraphiQLLink() {
  const { getAccessTokenSilently } = useAuth0();
  const graphiQLUrl = getGraphQLEndpoint();

  async function showAuthToken(event: React.MouseEvent<HTMLAnchorElement>) {
    event.preventDefault();

    // Prefer the stored seslogin token (email-code / passkey login); fall back
    // to Auth0. Mirrors AdminRelayEnvironment's token resolution.
    let authToken = getAdminToken() ?? "";
    if (!authToken) {
      try {
        authToken = await getAccessTokenSilently();
      } catch (err) {
        console.error("Failed to get Auth0 token:", err);
      }
    }

    const headers = {
      Authorization: authToken ? `Bearer ${authToken}` : "",
    };

    const confirmed = prompt(
      "Opening GraphiQL - copy/paste this into the headers field:",
      JSON.stringify(headers),
    );

    if (confirmed !== null) {
      window.open(graphiQLUrl, "_blank", "noopener,noreferrer");
    }
  }

  return (
    <a href={graphiQLUrl} target="_blank" onClick={showAuthToken}>
      GraphiQL
    </a>
  );
}
