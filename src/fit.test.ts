import { describe, expect, it } from "vitest";
import { PANEL_MAX_HEIGHT, panelWindowHeight } from "./fit";

describe("panelWindowHeight", () => {
  it("adds the panel margin around the content", () => {
    expect(panelWindowHeight(268)).toBe(280);
  });

  it("caps at the Overview height", () => {
    expect(panelWindowHeight(2000)).toBe(PANEL_MAX_HEIGHT);
  });

  it("does not shrink below a usable popover", () => {
    expect(panelWindowHeight(0)).toBe(160);
  });
});
