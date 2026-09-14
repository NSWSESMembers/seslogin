import "@testing-library/jest-dom/vitest";
import {
  describe,
  it,
  expect,
  vi,
  beforeAll,
  beforeEach,
  afterEach,
  afterAll,
} from "vitest";
import UserEvent from "@testing-library/user-event";
import { render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router";
import { setupServer } from "msw/node";
import { graphql, HttpResponse } from "msw";

import { getGraphQLEndpoint } from "../lib/api";
import PeriodEdit from "./PeriodEdit";

const TOKEN = "slp_testtoken";

// 2026-03-04 09:00 -> 12:00, expressed in local time so the datetime-local
// inputs round-trip regardless of the machine's zone.
const START = new Date(2026, 2, 4, 9, 0, 0);
const END = new Date(2026, 2, 4, 12, 0, 0);

const PERIOD_RESPONSE = {
  data: {
    linkedPeriod: {
      id: "period-123",
      startTime: Math.floor(START.getTime() / 1000),
      endTime: Math.floor(END.getTime() / 1000),
      category: { id: "cat-training", name: "Training" },
      person: { firstName: "Jamie", lastName: "Smith" },
      location: { name: "Example Unit" },
    },
    categories: [
      { id: "cat-training", name: "Training", enabled: true },
      { id: "cat-storm", name: "Storm", enabled: true },
      { id: "cat-retired", name: "Retired Activity", enabled: false },
    ],
  },
};

const relayEndpoint = graphql.link(getGraphQLEndpoint());

/** Captures the Authorization header of every intercepted GraphQL call. */
const seenAuthHeaders: string[] = [];

const server = setupServer(
  relayEndpoint.query("PeriodEditFormQuery", ({ request }) => {
    seenAuthHeaders.push(request.headers.get("Authorization") ?? "");
    return HttpResponse.json(PERIOD_RESPONSE);
  }),
);

beforeAll(() => server.listen());
afterEach(() => {
  server.resetHandlers();
  seenAuthHeaders.length = 0;
});
afterAll(() => server.close());

function renderAt(path: string) {
  return render(
    <MemoryRouter initialEntries={[path]}>
      <Routes>
        <Route path="/period" element={<PeriodEdit />} />
      </Routes>
    </MemoryRouter>,
  );
}

/**
 * The category id the form would submit. The Activity control is a `Combobox`,
 * whose visible box holds the category *name* while the id rides a hidden input.
 */
function submittedCategoryId(): string | null {
  return (
    document.querySelector<HTMLInputElement>(
      'input[type="hidden"][name="category"]',
    )?.value ?? null
  );
}

describe("PeriodEdit", () => {
  beforeEach(() => {
    // Above the `md` breakpoint, so the Activity field is the typeahead. The
    // narrow viewport gets the native picker instead — pinned at the end.
    window.innerWidth = 1024;
  });

  it("refuses to call the API when the link has no token", async () => {
    const requests = vi.fn();
    server.events.on("request:start", requests);

    renderAt("/period");

    expect(await screen.findByText("Link not valid")).toBeInTheDocument();
    expect(screen.getByText(/this link is incomplete/i)).toBeInTheDocument();
    expect(requests).not.toHaveBeenCalled();

    server.events.removeListener("request:start", requests);
  });

  it("sends the token as a bearer header and prefills the form", async () => {
    renderAt(`/period#${TOKEN}`);

    const start = await screen.findByLabelText<HTMLInputElement>("Start time");
    expect(start.value).toBe("2026-03-04T09:00");
    expect(screen.getByLabelText<HTMLInputElement>("End time").value).toBe(
      "2026-03-04T12:00",
    );
    expect(screen.getByLabelText<HTMLInputElement>("Activity").value).toBe(
      "Training",
    );
    expect(submittedCategoryId()).toBe("cat-training");
    expect(seenAuthHeaders).toContain(`Bearer ${TOKEN}`);
  });

  it("frames a still-open entry as a forgotten sign-out", async () => {
    server.use(
      relayEndpoint.query("PeriodEditFormQuery", () =>
        HttpResponse.json({
          data: {
            linkedPeriod: {
              ...PERIOD_RESPONSE.data.linkedPeriod,
              endTime: null,
            },
            categories: PERIOD_RESPONSE.data.categories,
          },
        }),
      ),
    );

    renderAt(`/period#${TOKEN}`);

    expect(await screen.findByText("Forgot to sign out?")).toBeInTheDocument();
    expect(
      screen.getByText(/don't have a sign-out time recorded/i),
    ).toBeInTheDocument();
    expect(screen.getByLabelText<HTMLInputElement>("End time").value).toBe("");
  });

  it("omits retired activities", async () => {
    const user = UserEvent.setup();
    renderAt(`/period#${TOKEN}`);

    await user.click(await screen.findByLabelText("Activity"));
    expect(screen.getByRole("option", { name: "Storm" })).toBeInTheDocument();
    expect(
      screen.queryByRole("option", { name: "Retired Activity" }),
    ).not.toBeInTheDocument();
  });

  it("keeps a retired activity offered when it is the entry's own", async () => {
    server.use(
      relayEndpoint.query("PeriodEditFormQuery", () =>
        HttpResponse.json({
          data: {
            linkedPeriod: {
              ...PERIOD_RESPONSE.data.linkedPeriod,
              category: { id: "cat-retired", name: "Retired Activity" },
            },
            categories: PERIOD_RESPONSE.data.categories,
          },
        }),
      ),
    );

    const user = UserEvent.setup();
    renderAt(`/period#${TOKEN}`);

    // Correcting a time must not force a category change, and the retired option
    // must stay choosable — not merely visible — so it can be picked again after
    // the member clears the box.
    const activity = await screen.findByLabelText<HTMLInputElement>("Activity");
    expect(activity.value).toBe("Retired Activity");
    await user.click(activity);
    const option = screen.getByRole("option", { name: "Retired Activity" });
    expect(option).not.toHaveAttribute("aria-disabled");
  });

  it("blocks submission when the end time precedes the start", async () => {
    const user = UserEvent.setup();
    renderAt(`/period#${TOKEN}`);

    const end = await screen.findByLabelText("End time");
    await user.clear(end);
    await user.type(end, "2026-03-04T08:00");

    expect(
      screen.getByText("The end time must come after the start time"),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
  });

  it("saves, thanks the member, and offers a further change", async () => {
    const variables = vi.fn();
    server.use(
      relayEndpoint.mutation("PeriodEditFormMutation", (req) => {
        variables(req.variables);
        return HttpResponse.json({
          data: {
            updatePeriodTimeCategory: {
              id: "period-123",
              startTime: req.variables.startTime,
              endTime: req.variables.endTime,
              category: { id: "cat-storm", name: "Storm" },
            },
          },
        });
      }),
    );

    const user = UserEvent.setup();
    renderAt(`/period#${TOKEN}`);

    // Drive the typeahead the way a member would: type a fragment, take the
    // match. This is the whole point of the control on a 171-activity list.
    const activity = await screen.findByLabelText("Activity");
    await user.click(activity);
    await user.type(activity, "storm");
    await user.keyboard("{Enter}");
    expect(submittedCategoryId()).toBe("cat-storm");

    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(await screen.findByText("Thank you")).toBeInTheDocument();
    await waitFor(() => expect(variables).toHaveBeenCalled());
    expect(variables.mock.calls[0][0]).toEqual({
      id: "period-123",
      startTime: Math.floor(START.getTime() / 1000),
      endTime: Math.floor(END.getTime() / 1000),
      categoryId: "cat-storm",
    });
    // The edit link must never rewrite the comment; the server rejects it, so
    // the page must not send one at all.
    expect(variables.mock.calls[0][0]).not.toHaveProperty("comment");
    expect(screen.getByText("Storm")).toBeInTheDocument();
    // The location is on the confirmation so the member can tell at a glance
    // which entry they just corrected.
    expect(screen.getByText("Location")).toBeInTheDocument();
    expect(screen.getByText("Example Unit")).toBeInTheDocument();

    await user.click(
      screen.getByRole("button", { name: "Make another change" }),
    );
    expect(screen.getByLabelText("Start time")).toBeInTheDocument();
  });

  // Mutation errors are shown verbatim on purpose: unlike the deliberately
  // uniform auth error, they are actionable validation messages.
  it("surfaces a save failure inline and stays on the form", async () => {
    server.use(
      relayEndpoint.mutation("PeriodEditFormMutation", () =>
        HttpResponse.json({
          errors: [{ message: "A time entry cannot be longer than 24 hours" }],
        }),
      ),
    );

    const user = UserEvent.setup();
    renderAt(`/period#${TOKEN}`);

    await screen.findByLabelText("Activity");
    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(
      await screen.findByText(/cannot be longer than 24 hours/i),
    ).toBeInTheDocument();
    expect(screen.queryByText("Thank you")).not.toBeInTheDocument();
  });

  it("shows an expiry message when the token is rejected", async () => {
    server.use(
      relayEndpoint.query("PeriodEditFormQuery", () =>
        HttpResponse.json(
          { errors: [{ message: "Invalid or expired token" }] },
          { status: 401 },
        ),
      ),
    );

    renderAt(`/period#${TOKEN}`);

    expect(await screen.findByText("Link not valid")).toBeInTheDocument();
    expect(
      screen.getByText(/expired or is no longer valid/i),
    ).toBeInTheDocument();
    expect(
      screen.queryByText("Invalid or expired token"),
    ).not.toBeInTheDocument();
  });

  // This page is reached from an emailed link, so most visits are on a phone,
  // where the OS picker beats a typeahead with a virtual keyboard over it.
  it("uses the native picker on a phone-width viewport", async () => {
    window.innerWidth = 375;
    renderAt(`/period#${TOKEN}`);

    const activity =
      await screen.findByLabelText<HTMLSelectElement>("Activity");
    expect(activity.tagName).toBe("SELECT");
    expect(activity.value).toBe("cat-training");
    expect(screen.getByRole("option", { name: "Storm" })).toBeInTheDocument();
  });

  it("shows a load-problem message, not raw GraphQL text, when a field fails to resolve", async () => {
    // Same shape as a dangling person/category reference: data present, but one
    // field errored. @throwOnFieldError turns this into a thrown error instead of
    // silently handing the form a null it doesn't expect.
    vi.spyOn(console, "error").mockImplementation(() => {});
    server.use(
      relayEndpoint.query("PeriodEditFormQuery", () =>
        HttpResponse.json({
          data: {
            linkedPeriod: {
              ...PERIOD_RESPONSE.data.linkedPeriod,
              category: null,
            },
            categories: PERIOD_RESPONSE.data.categories,
          },
          errors: [
            {
              message: "Category with ID cat-training missing",
              path: ["linkedPeriod", "category"],
            },
          ],
        }),
      ),
    );

    renderAt(`/period#${TOKEN}`);

    expect(await screen.findByText("Link not valid")).toBeInTheDocument();
    expect(
      screen.getByText(/couldn't load your time entry/i),
    ).toBeInTheDocument();
    expect(
      screen.queryByText("Category with ID cat-training missing"),
    ).not.toBeInTheDocument();
  });
});
