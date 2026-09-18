import { describe, expect, it } from "vitest";
import { panelHtml } from "./render";
import type { UsageSnapshot } from "./types";

const resetAt = new Date(Date.now() + 6 * 86_400_000 + 16 * 3_600_000).toISOString();

const snapshot: UsageSnapshot = {
  generatedAt: new Date().toISOString(),
  providers: [
    {
      provider: "commandCode",
      status: { state: "ok" },
      today: {
        costUsd: 0.64,
        tokens: { input: 1000, output: 100, cacheRead: 0, cacheWrite: 0, reasoning: 0 },
        records: 3,
      },
      last7d: {
        costUsd: 10.35,
        tokens: { input: 2000, output: 200, cacheRead: 0, cacheWrite: 0, reasoning: 0 },
        records: 9,
      },
      last30d: {
        costUsd: 57.21,
        tokens: { input: 3000, output: 300, cacheRead: 0, cacheWrite: 0, reasoning: 0 },
        records: 30,
      },
      lastRecordAt: new Date().toISOString(),
      commandCode: {
        plan: "GOAT",
        planId: "individual-goat",
        status: "active",
        usagePercent: 44.93,
        creditsTotal: 70,
        creditsRemaining: 38.5,
        requestsThisPeriod: 7776,
        periodBasis: "billing-period",
        renewsAt: new Date(Date.now() + 23 * 86_400_000).toISOString(),
        daysToRenew: 23,
        fiveHour: { percentUsed: 3.09, used: 0.43, cap: 14, resetAt },
        weekly: { percentUsed: 1.85, used: 0.6, cap: 35, resetAt },
        fetchedAt: new Date().toISOString(),
      },
    },
    {
      provider: "grok",
      status: { state: "ok" },
      today: {
        costUsd: 0,
        tokens: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, reasoning: 0 },
        records: 2,
      },
      last7d: {
        costUsd: 0,
        tokens: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, reasoning: 0 },
        records: 2,
      },
      last30d: {
        costUsd: 0,
        tokens: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, reasoning: 0 },
        records: 2,
      },
      grok: {
        creditUsagePercent: 77,
        periodStart: new Date().toISOString(),
        periodEnd: resetAt,
        tier: "SuperGrok",
        fetchedAt: new Date().toISOString(),
      },
    },
    {
      provider: "openCode",
      status: { state: "notFound", path: "/tmp/missing.db" },
      today: {
        costUsd: 0,
        tokens: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, reasoning: 0 },
        records: 0,
      },
      last7d: {
        costUsd: 0,
        tokens: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, reasoning: 0 },
        records: 0,
      },
      last30d: {
        costUsd: 0,
        tokens: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, reasoning: 0 },
        records: 0,
      },
    },
  ],
};

describe("panelHtml", () => {
  it("renders the Command Code plan limits", () => {
    const html = panelHtml(snapshot);

    expect(html).toContain("GOAT · active");
    expect(html).toContain("renova em 23 dias");
    expect(html).toContain("45% usado");
    expect(html).toContain("7.776 requests este mês");
    expect(html).toContain("saldo 38.5 de 70 créditos");
    expect(html).toContain("5h");
    expect(html).toContain("semanal");
    expect(html).toContain("reseta em");
  });

  it("renders the local windows and grok limits", () => {
    const html = panelHtml(snapshot);

    expect(html).toContain("$0.64");
    expect(html).toContain("77% do período semanal · SuperGrok");
    expect(html).toContain("não encontrado em /tmp/missing.db");
  });
});
