import { describe, expect, it } from "vitest";
import {
  formatCost,
  formatNumber,
  formatPercent,
  formatRelativeTime,
  formatResetCountdown,
  formatTokens,
} from "./format";

describe("formatCost", () => {
  it("renders zero without cents", () => {
    expect(formatCost(0)).toBe("$0");
  });

  it("renders two decimal places", () => {
    expect(formatCost(0.4242)).toBe("$0.42");
    expect(formatCost(1.034)).toBe("$1.03");
    expect(formatCost(12.4)).toBe("$12.40");
  });
});

describe("formatTokens", () => {
  it("keeps small numbers as-is", () => {
    expect(formatTokens(0)).toBe("0");
    expect(formatTokens(856)).toBe("856");
  });

  it("uses k and M suffixes", () => {
    expect(formatTokens(1000)).toBe("1.0k");
    expect(formatTokens(12500)).toBe("12.5k");
    expect(formatTokens(1200000)).toBe("1.2M");
  });
});

describe("formatPercent", () => {
  it("rounds to whole percent", () => {
    expect(formatPercent(46.4)).toBe("46%");
    expect(formatPercent(0)).toBe("0%");
  });
});

describe("formatNumber", () => {
  it("groups thousands with dots", () => {
    expect(formatNumber(0)).toBe("0");
    expect(formatNumber(42)).toBe("42");
    expect(formatNumber(7776)).toBe("7.776");
    expect(formatNumber(1234567)).toBe("1.234.567");
  });

  it("rounds fractional values", () => {
    expect(formatNumber(7775.6)).toBe("7.776");
  });
});

describe("formatRelativeTime", () => {
  const now = new Date("2026-09-17T18:00:00Z");

  it("describes recent timestamps", () => {
    expect(formatRelativeTime("2026-09-17T17:59:40Z", now)).toBe("agora");
    expect(formatRelativeTime("2026-09-17T17:30:00Z", now)).toBe("há 30 min");
    expect(formatRelativeTime("2026-09-17T15:00:00Z", now)).toBe("há 3 h");
    expect(formatRelativeTime("2026-09-15T18:00:00Z", now)).toBe("há 2 d");
  });
});

describe("formatResetCountdown", () => {
  const now = new Date("2026-09-17T18:00:00Z");

  it("counts down days and hours", () => {
    expect(formatResetCountdown("2026-09-24T12:00:00Z", now)).toBe("reseta em 6d 18h");
  });

  it("counts down hours and minutes", () => {
    expect(formatResetCountdown("2026-09-17T20:30:00Z", now)).toBe("reseta em 2h 30min");
  });

  it("counts down minutes", () => {
    expect(formatResetCountdown("2026-09-17T18:42:00Z", now)).toBe("reseta em 42min");
  });

  it("reports expired periods", () => {
    expect(formatResetCountdown("2026-09-16T18:00:00Z", now)).toBe("expirado");
  });
});
