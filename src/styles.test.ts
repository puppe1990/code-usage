import { describe, expect, it } from "vitest";
// @ts-expect-error frontend tsconfig has no Node types
import { readFileSync } from "node:fs";

const css = readFileSync(new URL("./styles.css", import.meta.url), "utf8") as string;

function rule(selector: string): string {
  const start = css.indexOf(`${selector} {`);
  expect(start, selector).toBeGreaterThan(-1);
  const open = css.indexOf("{", start);
  const close = css.indexOf("}", open);
  return css.slice(open + 1, close);
}

describe("card title alignment", () => {
  it("packs the harness mark and name to the left, with the star on the right", () => {
    expect(rule(".card-title-row")).not.toMatch(/justify-content:\s*space-between/);
    expect(rule(".card-head h2")).toMatch(/flex:\s*1/);
    expect(rule(".star")).toMatch(/margin-left:\s*auto/);
  });
});
