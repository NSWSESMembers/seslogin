import "@testing-library/jest-dom/vitest";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { Suspense, act } from "react";
import UserEvent from "@testing-library/user-event";
import { render, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { RelayEnvironmentProvider } from "react-relay";
import { MockPayloadGenerator, createMockEnvironment } from "relay-test-utils";
import OAuthAuthorize from "./OAuthAuthorize";
import {
  NotifyContext,
  type NotifyContextValue,
} from "../components/useNotify";
import { UserInfoContext } from "../components/UserInfoContext";

// relay-test-utils reaches for jest.fn() internally; shim it for vitest.
(globalThis as unknown as { jest: { fn: typeof vi.fn } }).jest = { fn: vi.fn };

const notify: NotifyContextValue = {
  notify: vi.fn(),
  notifySuccess: vi.fn(),
  notifyError: vi.fn(),
  dismiss: vi.fn(),
};

const VALID_QUERY =
  "/admin/oauth/authorize" +
  "?response_type=code" +
  "&client_id=test-client-id" +
  "&redirect_uri=https%3A%2F%2Fclaude.ai%2Fcallback" +
  "&code_challenge=E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM" +
  "&code_challenge_method=S256" +
  "&state=xyz";

function renderAt(
  path: string,
  environment: ReturnType<typeof createMockEnvironment>,
) {
  return render(
    <RelayEnvironmentProvider environment={environment}>
      <NotifyContext.Provider value={notify}>
        <UserInfoContext.Provider
          value={{
            isLoaded: true,
            // Cast: only `.email` is read by this page.
            user: { email: "admin@example.com" } as never,
          }}
        >
          <MemoryRouter initialEntries={[path]}>
            <Suspense fallback={<p>Loading</p>}>
              <OAuthAuthorize />
            </Suspense>
          </MemoryRouter>
        </UserInfoContext.Provider>
      </NotifyContext.Provider>
    </RelayEnvironmentProvider>,
  );
}

let assignSpy: ReturnType<typeof vi.fn>;
let originalLocation: Location;

beforeEach(() => {
  // jsdom's window.location isn't configurable enough for vi.spyOn, so swap
  // the whole object out — same pattern as RelayErrorBoundary.test.tsx.
  assignSpy = vi.fn();
  originalLocation = window.location;
  Object.defineProperty(window, "location", {
    configurable: true,
    value: { ...originalLocation, assign: assignSpy },
  });
});

afterEach(() => {
  Object.defineProperty(window, "location", {
    configurable: true,
    value: originalLocation,
  });
});

describe("OAuthAuthorize", () => {
  it("shows an error and makes no query when the request is missing required params", () => {
    const environment = createMockEnvironment();
    const screen = renderAt(
      "/admin/oauth/authorize?client_id=abc",
      environment,
    );

    expect(screen.getByText("Invalid request")).toBeInTheDocument();
    expect(environment.mock.getAllOperations()).toHaveLength(0);
  });

  it("refuses to render the consent screen inside a frame", () => {
    const topSpy = vi.spyOn(window, "top", "get").mockReturnValue({} as Window);
    try {
      const environment = createMockEnvironment();
      const screen = renderAt(VALID_QUERY, environment);

      expect(screen.getByText("Open in a new tab")).toBeInTheDocument();
      expect(environment.mock.getAllOperations()).toHaveLength(0);
    } finally {
      topSpy.mockRestore();
    }
  });

  it("shows the client name and redirect host, then approves and follows the returned URL", async () => {
    const environment = createMockEnvironment();
    const screen = renderAt(VALID_QUERY, environment);

    act(() => {
      environment.mock.resolveMostRecentOperation((op) =>
        MockPayloadGenerator.generate(op, {
          OauthAuthorizationRequest: () => ({
            clientName: "Claude",
            redirectHost: "claude.ai",
          }),
        }),
      );
    });

    await screen.findByText("Connect Claude?");
    expect(screen.getByText("claude.ai")).toBeInTheDocument();
    expect(screen.getByText("admin@example.com")).toBeInTheDocument();

    const user = UserEvent.setup();
    await user.click(screen.getByRole("button", { name: "Approve" }));

    // `approveOauthAuthorization` returns a plain String scalar, so there's no
    // object type to hand MockPayloadGenerator a field resolver for — supply
    // the raw response instead.
    act(() => {
      environment.mock.resolveMostRecentOperation(() => ({
        data: {
          approveOauthAuthorization:
            "https://claude.ai/callback?code=abc123&state=xyz",
        },
      }));
    });

    await waitFor(() =>
      expect(assignSpy).toHaveBeenCalledWith(
        "https://claude.ai/callback?code=abc123&state=xyz",
      ),
    );
  });

  it("denying redirects to redirect_uri with access_denied, without calling the mutation", async () => {
    const environment = createMockEnvironment();
    const screen = renderAt(VALID_QUERY, environment);

    act(() => {
      environment.mock.resolveMostRecentOperation((op) =>
        MockPayloadGenerator.generate(op, {
          OauthAuthorizationRequest: () => ({
            clientName: "Claude",
            redirectHost: "claude.ai",
          }),
        }),
      );
    });

    await screen.findByText("Connect Claude?");

    const user = UserEvent.setup();
    await user.click(screen.getByRole("button", { name: "Deny" }));

    expect(assignSpy).toHaveBeenCalledTimes(1);
    const [url] = assignSpy.mock.calls[0] as [string];
    expect(url).toContain("https://claude.ai/callback");
    expect(url).toContain("error=access_denied");
    expect(url).toContain("state=xyz");
    // No mutation was ever sent for a deny.
    expect(environment.mock.getAllOperations()).toHaveLength(0);
  });
});
