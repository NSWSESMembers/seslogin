// @vitest-environment jsdom

import { act, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { NotificationProvider } from "./Notifications";

function emitFieldError(operationName: string, errorCount = 1) {
  act(() => {
    window.dispatchEvent(
      new CustomEvent("graphql-field-error", {
        detail: { operationName, errorCount },
      }),
    );
  });
}

describe("NotificationProvider — GraphQL field errors", () => {
  it("shows one warning toast naming the operation", () => {
    render(<NotificationProvider>app</NotificationProvider>);

    emitFieldError("LocationPeriodsQuery");

    const alert = screen.getByRole("alert");
    expect(alert).toHaveTextContent(/may be incomplete/i);
    expect(alert).toHaveTextContent("Error occurred in: LocationPeriodsQuery");
  });

  it("dedupes repeated errors for the same operation", () => {
    render(<NotificationProvider>app</NotificationProvider>);

    emitFieldError("LocationPeriodsQuery");
    emitFieldError("LocationPeriodsQuery");

    expect(screen.getAllByRole("alert")).toHaveLength(1);
  });

  it("shows a separate toast for a different operation", () => {
    render(<NotificationProvider>app</NotificationProvider>);

    emitFieldError("LocationPeriodsQuery");
    emitFieldError("UserInfoQuery");

    expect(screen.getAllByRole("alert")).toHaveLength(2);
  });
});
