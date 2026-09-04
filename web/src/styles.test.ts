import { describe, expect, it } from "vitest";
import { scanViewProps, type ScreenPosition } from "./styles";

// The kiosk keeps every scan screen mounted so it can animate, so the only thing
// standing between an operator's Tab key and four invisible screens is that
// off-centre screens come back inert. Pin that here rather than in each screen's
// own test, since the point of the helper is that no screen decides it locally.
describe("scanViewProps", () => {
  it("marks off-screen positions inert and the centre one not", () => {
    const positions: ScreenPosition[] = ["offLeft", "center", "offRight"];
    expect(positions.map((p) => scanViewProps(p).inert)).toEqual([
      true,
      false,
      true,
    ]);
  });

  it("carries the position's transform and any extra classes", () => {
    const { className } = scanViewProps("offRight", "flex flex-col");
    expect(className).toContain("translate-x-full");
    expect(className).toContain("transition-transform");
    expect(className).toContain("flex flex-col");
  });

  it("does not leave a trailing space when given no extra classes", () => {
    expect(scanViewProps("center").className).toBe(
      scanViewProps("center").className.trim(),
    );
  });
});
