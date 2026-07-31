import "@testing-library/jest-dom/vitest";
import { describe, it, expect, vi } from "vitest";
import UserEvent from "@testing-library/user-event";
import type { UserEvent as UserEventType } from "@testing-library/user-event";
import { render, screen, waitFor } from "@testing-library/react";
import ScanScreenAdjust from "./ScanScreenAdjust";
import type { TransactionSignedOut } from "../ScanState";

function makeTransaction(hoursAgo: number): TransactionSignedOut {
  return {
    uuid: "tx-1",
    status: "SIGNED_OUT",
    periodId: "period-1",
    person: { id: "person-1", firstName: "Random", lastName: "Guy" },
    startTime: new Date(Date.now() - hoursAgo * 60 * 60 * 1000),
    categoryId: "RX2bfpU6ppvV",
    adjusted: false,
    quickPickSkipped: false,
  };
}

function renderAdjust(
  transaction: TransactionSignedOut,
  onSubmit: () => void,
  easyTimeEntry: boolean = false,
) {
  return render(
    <ScanScreenAdjust
      screenPosition="center"
      uuid={transaction.uuid}
      transaction={transaction}
      onEditCategory={() => {}}
      onSubmit={onSubmit}
      isSubmitting={false}
      easyTimeEntry={easyTimeEntry}
      newCategories={false}
    />,
  );
}

async function enterTime(
  user: UserEventType,
  time: string,
  am: boolean = true,
) {
  if (time.length !== 5 || !time.includes(":")) {
    throw new Error("Time must be in HH:MM format");
  }

  await user.click(screen.getByRole("button", { name: am ? "AM" : "PM" }));

  for (const char of time) {
    if (char === ":") {
      continue;
    }
    const button = screen.getByRole("button", { name: char });
    await user.click(button);
  }
  await user.click(screen.getByRole("button", { name: "Confirm" }));
}

describe("ScanScreenAdjust", () => {
  it("submits immediately for a period under 12 hours", async () => {
    const onSubmit = vi.fn();
    const user = UserEvent.setup();
    renderAdjust(makeTransaction(1), onSubmit);

    await user.click(screen.getByRole("button", { name: "Submit" }));

    expect(onSubmit).toHaveBeenCalledOnce();
    expect(
      screen.queryByRole("heading", { name: "Long session" }),
    ).not.toBeInTheDocument();
  });

  it("asks for confirmation before submitting a period over 12 hours", async () => {
    const onSubmit = vi.fn();
    const user = UserEvent.setup();
    renderAdjust(makeTransaction(13), onSubmit);

    await user.click(screen.getByRole("button", { name: "Submit" }));

    expect(
      screen.getByRole("heading", { name: "Long session" }),
    ).toBeInTheDocument();
    expect(onSubmit).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "Confirm" }));

    expect(onSubmit).toHaveBeenCalledOnce();
  });

  it("does not submit if the long-period confirmation is cancelled", async () => {
    const onSubmit = vi.fn();
    const user = UserEvent.setup();
    renderAdjust(makeTransaction(13), onSubmit);

    await user.click(screen.getByRole("button", { name: "Submit" }));
    await user.click(screen.getByRole("button", { name: "Cancel" }));

    expect(
      screen.queryByRole("heading", { name: "Long session" }),
    ).not.toBeInTheDocument();
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("does not submit if the end time is before the start time", async () => {
    const onSubmit = vi.fn();
    const user = UserEvent.setup();
    renderAdjust(makeTransaction(0), onSubmit, true);

    const [startTimeEdit, endTimeEdit] = screen.getAllByRole("button", {
      name: "Edit",
    });

    await user.click(startTimeEdit);
    await enterTime(user, "11:11");

    await user.click(endTimeEdit);
    await enterTime(user, "11:10");

    await user.click(screen.getByRole("button", { name: "Submit" }));

    await waitFor(() =>
      expect(
        screen.getByText("Error: End time cannot be before start time."),
      ).toBeInTheDocument(),
    );
    expect(onSubmit).not.toHaveBeenCalled();
  });
});
