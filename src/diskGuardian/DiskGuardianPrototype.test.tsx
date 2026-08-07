import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import DiskGuardianPrototype from "./DiskGuardianPrototype";

describe("DiskGuardianPrototype", () => {
  it("edits thresholds only in the local demo", () => {
    render(<DiskGuardianPrototype />);

    fireEvent.change(screen.getByLabelText("Warning threshold in gigabytes"), {
      target: { value: "24" },
    });
    fireEvent.change(screen.getByLabelText("Critical threshold in gigabytes"), {
      target: { value: "12" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Apply to this demo" }));

    expect(screen.getByRole("status").textContent).toBe(
      "Example updated locally: warning at 24 GB, critical at 12 GB. These values are not saved.",
    );
    expect(screen.getByText("Demo only · not persisted")).toBeTruthy();
  });

  it("rejects a critical threshold that is not below warning", () => {
    render(<DiskGuardianPrototype />);

    fireEvent.change(screen.getByLabelText("Warning threshold in gigabytes"), {
      target: { value: "10" },
    });
    fireEvent.change(screen.getByLabelText("Critical threshold in gigabytes"), {
      target: { value: "20" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Apply to this demo" }));

    expect(screen.getByRole("status").textContent).toBe(
      "Critical must be lower than warning.",
    );
  });

  it("requires the matching candidate confirmation before approval", () => {
    render(<DiskGuardianPrototype />);

    expect(screen.queryByText("Approved in demo")).toBeNull();

    fireEvent.click(
      screen.getByRole("button", { name: "Review action: Browser cache files" }),
    );

    const dialog = screen.getByRole("dialog", {
      name: "Approve “Browser cache files”?",
    });
    expect(within(dialog).getByText(/marks only this candidate/i)).toBeTruthy();
    expect(screen.queryByText("Approved in demo")).toBeNull();

    fireEvent.click(
      within(dialog).getByRole("button", { name: "Confirm demo approval" }),
    );

    const browserCard = screen
      .getByRole("heading", { name: "Browser cache files" })
      .closest("article");
    expect(browserCard).not.toBeNull();
    expect(within(browserCard!).getByText("Approved in demo")).toBeTruthy();

    const developerCard = screen
      .getByRole("heading", { name: "Old developer build output" })
      .closest("article");
    expect(developerCard).not.toBeNull();
    expect(within(developerCard!).queryByText("Approved in demo")).toBeNull();
  });

  it("leaves approval state unchanged when confirmation is cancelled", () => {
    render(<DiskGuardianPrototype />);

    fireEvent.click(
      screen.getByRole("button", { name: "Review action: Large Downloads to review" }),
    );
    const dialog = screen.getByRole("dialog", {
      name: "Approve “Large Downloads to review”?",
    });
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));

    expect(screen.queryByRole("dialog")).toBeNull();
    expect(screen.queryByText("Approved in demo")).toBeNull();
  });

  it("does not carry confirmation across concrete actions", () => {
    render(<DiskGuardianPrototype />);

    fireEvent.click(
      screen.getByRole("button", { name: "Review action: Browser cache files" }),
    );
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    fireEvent.click(
      screen.getByRole("button", { name: "Review action: Old developer build output" }),
    );

    const dialog = screen.getByRole("dialog", {
      name: "Approve “Old developer build output”?",
    });
    fireEvent.click(
      within(dialog).getByRole("button", { name: "Confirm demo approval" }),
    );

    const browserCard = screen
      .getByRole("heading", { name: "Browser cache files" })
      .closest("article");
    const developerCard = screen
      .getByRole("heading", { name: "Old developer build output" })
      .closest("article");
    expect(within(browserCard!).queryByText("Approved in demo")).toBeNull();
    expect(within(developerCard!).getByText("Approved in demo")).toBeTruthy();
  });
});
