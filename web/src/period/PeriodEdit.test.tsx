import "@testing-library/jest-dom/vitest";
import {
  describe,
  it,
  expect,
  vi,
  beforeAll,
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

describe("PeriodEdit", () => {
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
    expect(screen.getByLabelText<HTMLSelectElement>("Activity").value).toBe(
      "cat-training",
    );
    expect(seenAuthHeaders).toContain(`Bearer ${TOKEN}`);
  });

  it("omits retired activities but keeps the entry's own", async () => {
    renderAt(`/period#${TOKEN}`);

    await screen.findByLabelText("Activity");
    expect(screen.getByRole("option", { name: "Storm" })).toBeInTheDocument();
    expect(
      screen.queryByRole("option", { name: "Retired Activity" }),
    ).not.toBeInTheDocument();
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

    await user.selectOptions(
      await screen.findByLabelText("Activity"),
      "cat-storm",
    );
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

  // An entry with no end time is someone who forgot to sign out. The page has to
  // ask for the missing finish time rather than inviting a review of times that
  // aren't there — and the confirmation has to match how they arrived, even
  // though the entry is complete by the time they see it.
  describe("when the entry was never signed out", () => {
    const OPEN_PERIOD_RESPONSE = {
      data: {
        ...PERIOD_RESPONSE.data,
        linkedPeriod: { ...PERIOD_RESPONSE.data.linkedPeriod, endTime: null },
      },
    };

    function serveOpenPeriod() {
      server.use(
        relayEndpoint.query("PeriodEditFormQuery", () =>
          HttpResponse.json(OPEN_PERIOD_RESPONSE),
        ),
      );
    }

    it("asks for the finish time instead of a review", async () => {
      serveOpenPeriod();
      renderAt(`/period#${TOKEN}`);

      expect(
        await screen.findByText("You didn't sign out"),
      ).toBeInTheDocument();
      expect(
        screen.getByText(/won't count towards your hours/i),
      ).toBeInTheDocument();
      // The end field is the ask, so it is labelled as a question and starts empty.
      const end = screen.getByLabelText<HTMLInputElement>(
        "What time did you finish?",
      );
      expect(end.value).toBe("");
      expect(
        screen.getByRole("button", { name: "Save finish time" }),
      ).toBeInTheDocument();
      // The complete-case wording must not leak through.
      expect(
        screen.queryByText("Check your time entry"),
      ).not.toBeInTheDocument();
    });

    it("confirms the finish time was recorded", async () => {
      serveOpenPeriod();
      server.use(
        relayEndpoint.mutation("PeriodEditFormMutation", (req) =>
          HttpResponse.json({
            data: {
              updatePeriodTimeCategory: {
                id: "period-123",
                startTime: req.variables.startTime,
                endTime: req.variables.endTime,
                category: { id: "cat-training", name: "Training" },
              },
            },
          }),
        ),
      );

      const user = UserEvent.setup();
      renderAt(`/period#${TOKEN}`);

      const end = await screen.findByLabelText("What time did you finish?");
      await user.type(end, "2026-03-04T12:00");
      await user.click(
        screen.getByRole("button", { name: "Save finish time" }),
      );

      expect(
        await screen.findByText(/your finish time has been recorded/i),
      ).toBeInTheDocument();
      // Not the complete-case confirmation, even though the entry now has both
      // times — the variant is fixed at load, not derived from what was saved.
      expect(
        screen.queryByText(/your time entry has been updated/i),
      ).not.toBeInTheDocument();
    });
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
});
