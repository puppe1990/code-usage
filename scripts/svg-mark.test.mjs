import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { renderMark } from "./svg-mark.mjs";

/// Rasterizes markup exactly the way `generate-icons.mjs` rasterizes a mark file.
function render(body, { height = 32, ...options } = {}) {
  const dir = mkdtempSync(join(tmpdir(), "svg-mark-"));
  const path = join(dir, "mark.svg");

  writeFileSync(
    path,
    `<svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 32 32">${body}</svg>`,
  );
  return renderMark(path, height, options);
}

function alphaAt(mark, x, y) {
  return mark.rgba[(y * mark.width + x) * 4 + 3];
}

describe("renderMark", () => {
  it("paints a stroked outline instead of the filled shape", () => {
    // a line across the middle of the viewBox, 8 units thick
    const mark = render('<path d="M0 16H32" stroke="#000" stroke-width="8"/>');

    expect(alphaAt(mark, 16, 12)).toBe(255);
    expect(alphaAt(mark, 16, 16)).toBe(255);
    expect(alphaAt(mark, 16, 11)).toBe(0);
    expect(alphaAt(mark, 16, 21)).toBe(0);
  });

  it("rounds the caps at both ends of an open stroke", () => {
    const mark = render(
      '<path d="M8 16H24" stroke="#000" stroke-linecap="round" stroke-width="8"/>',
    );

    expect(alphaAt(mark, 8, 16)).toBe(255);
    expect(alphaAt(mark, 4, 16)).toBe(255);
    expect(alphaAt(mark, 3, 16)).toBe(0);
  });

  it("draws the closing segment of a stroked subpath", () => {
    const mark = render('<path d="M8 8H24V24H8Z" stroke="#000" stroke-width="4"/>');

    expect(alphaAt(mark, 8, 16), "the left edge only exists through `Z`").toBe(255);
    expect(alphaAt(mark, 16, 16), "the outline stays hollow").toBe(0);
  });

  it("drops a path that is filled with `none` and has no stroke", () => {
    expect(() => render('<path d="M8 8H24V24H8Z" fill="none"/>')).toThrow(
      "no paintable paths found in the mark",
    );
  });

  it("keeps skipping the black tile behind a brand mark", () => {
    const mark = render(
      '<path d="M0 0H32V32H0Z" fill="#000"/><path d="M8 8H24V24H8Z" fill="#fff" fill-rule="evenodd"/>',
    );

    expect(alphaAt(mark, 2, 2)).toBe(0);
    expect(alphaAt(mark, 16, 16)).toBe(255);
  });

  it("keeps the fills the caller asks to skip", () => {
    // the OpenCode tray mark: the light square goes away, the frame around it stays
    const mark = render(
      '<path d="M0 0H32V32H0Z" fill="#CFCECD"/><path d="M8 8H24V24H8Z" fill="#211E1E"/>',
      { skip: ["#CFCECD"] },
    );

    expect(alphaAt(mark, 2, 2)).toBe(0);
    expect(alphaAt(mark, 16, 16)).toBe(255);
  });
});
