import {
  Environment,
  Network,
  RecordSource,
  Store,
  type FetchFunction,
} from "relay-runtime";
import { fetchGraphQL } from "./graphql";
import { relayFieldLogger } from "./relayFieldLogger";

type GetTokenFn = () => Promise<string>;

/**
 * Creates a Relay environment for admin routes.
 * getToken is called per-request and returns the stored opaque seslogin token
 * (it rejects if there is none).
 */
export function createAdminGraphQLEnvironment(
  getToken: GetTokenFn,
  onTokenError: () => void,
  onUnauthorized: () => void,
): Environment {
  const _fetchGraphQL: FetchFunction = async (request, variables) => {
    let authToken: string;
    try {
      authToken = await getToken();
    } catch (err) {
      console.error("Failed to get auth token:", err);
      onTokenError();
      throw new Error("Failed to get auth token", { cause: err });
    }

    return await fetchGraphQL(`Bearer ${authToken}`, request, variables, () => {
      console.log("Unauthorized in admin section");
      onUnauthorized();
      throw new Error("Unauthorized from server");
    });
  };

  const environment = new Environment({
    network: Network.create(_fetchGraphQL),
    store: new Store(new RecordSource()),
    relayFieldLogger,
  });

  return environment;
}

/**
 * Creates a Relay environment for kiosk routes that uses the cached kiosk auth token
 */
export function createKioskGraphQLEnvironment(
  getScanToken: () => string | null,
  onUnauthorized: () => void,
): Environment {
  const _fetchGraphQL: FetchFunction = async (request, variables) => {
    const authToken = getScanToken();

    if (authToken == null) {
      onUnauthorized();
      throw new Error("Failed to get auth token");
    }

    return await fetchGraphQL(`Bearer ${authToken}`, request, variables, () => {
      console.log("Unauthorized in kiosk section");
      onUnauthorized();
      throw new Error("Unauthorized from server");
    });
  };

  const environment = new Environment({
    network: Network.create(_fetchGraphQL),
    store: new Store(new RecordSource(), { gcReleaseBufferSize: 0 }),
    relayFieldLogger,
  });

  return environment;
}

/**
 * Creates a Relay environment for a kiosk enrolled via public key. Each request is
 * signed: `getAuthHeader` is called with the serialized body and returns a full
 * `SLKey <fp>.<ts>.<sig>` Authorization header. A 401 (session disabled / key expired)
 * triggers onUnauthorized, which reverts the kiosk to the enrollment screen.
 */
export function createKioskKeyGraphQLEnvironment(
  getAuthHeader: (body: string) => Promise<string>,
  onUnauthorized: () => void,
): Environment {
  const _fetchGraphQL: FetchFunction = async (request, variables) => {
    return await fetchGraphQL(
      async (body) => {
        try {
          return await getAuthHeader(body);
        } catch (err) {
          console.error("Failed to sign kiosk request:", err);
          onUnauthorized();
          throw new Error("Failed to sign kiosk request", { cause: err });
        }
      },
      request,
      variables,
      () => {
        console.log("Unauthorized in kiosk key section");
        onUnauthorized();
        throw new Error("Unauthorized from server");
      },
    );
  };

  const environment = new Environment({
    network: Network.create(_fetchGraphQL),
    store: new Store(new RecordSource(), { gcReleaseBufferSize: 0 }),
    relayFieldLogger,
  });

  return environment;
}

/**
 * Creates a Relay environment for a member-facing period edit link.
 *
 * The `slp_` token comes from the URL fragment and is a capability, not an
 * identity: it authorises exactly one period, so there is nothing to refresh and
 * the token is fixed for the life of the page. A 401 means the link expired.
 */
export function createPeriodLinkGraphQLEnvironment(
  token: string,
  onUnauthorized: () => void,
): Environment {
  const _fetchGraphQL: FetchFunction = async (request, variables) => {
    return await fetchGraphQL(`Bearer ${token}`, request, variables, () => {
      console.log("Unauthorized period edit link");
      onUnauthorized();
      throw new Error("Unauthorized from server");
    });
  };

  return new Environment({
    network: Network.create(_fetchGraphQL),
    store: new Store(new RecordSource()),
    relayFieldLogger,
  });
}

export function createUnauthenticatedGraphQLEnvironment(): Environment {
  const _fetchGraphQL: FetchFunction = async (request, variables) => {
    return await fetchGraphQL(null, request, variables, () => {
      console.log("Unauthorized in unauthenticated environment");
    });
  };

  const environment = new Environment({
    network: Network.create(_fetchGraphQL),
    // note: no cache/store as this is intended for one-off requests without auth
    relayFieldLogger,
  });

  return environment;
}

export const unauthenticatedEnvironment =
  createUnauthenticatedGraphQLEnvironment();
