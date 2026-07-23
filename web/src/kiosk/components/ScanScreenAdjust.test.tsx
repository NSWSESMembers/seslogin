import "@testing-library/jest-dom/vitest";
import { describe, it, expect, vi } from "vitest";
import UserEvent from "@testing-library/user-event";
import { render, screen } from "@testing-library/react";
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
  };
}

function renderAdjust(transaction: TransactionSignedOut, onSubmit: () => void) {
  return render(
    <ScanScreenAdjust
      screenPosition="center"
      uuid={transaction.uuid}
      transaction={transaction}
      onEditCategory={() => {}}
      onSubmit={onSubmit}
      isSubmitting={false}
      easyTimeEntry={false}
      newCategories={false}
    />,
  );
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
});
