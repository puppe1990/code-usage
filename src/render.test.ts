import { describe, expect, it } from "vitest";
import { panelHtml } from "./render";
import type { UsageSnapshot } from "./types";

const resetAt = new Date(Date.now() + 6 * 86_400_000 + 16 * 3_600_000).toISOString();
const goResetAt = new Date(Date.now() + 2 * 3_600_000 + 40 * 60_000).toISOString();

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
      status: { state: "ok" },
      today: {
        costUsd: 2.95,
        tokens: { input: 500, output: 50, cacheRead: 0, cacheWrite: 0, reasoning: 0 },
        records: 5,
      },
      last7d: {
        costUsd: 0,
        tokens: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, reasoning: 0 },
        records: 5,
      },
      last30d: {
        costUsd: 0,
        tokens: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, reasoning: 0 },
        records: 5,
      },
      lastRecordAt: new Date().toISOString(),
      openCodeGo: {
        rolling: { percent: 11, status: "ok", resetsAt: goResetAt },
        weekly: { percent: 21, status: "ok", resetsAt: resetAt },
        monthly: { percent: 10, status: "ok", resetsAt: resetAt },
        fetchedAt: new Date().toISOString(),
      },
    },
  ],
};

describe("panelHtml", () => {
  it("renders the Command Code plan limits", () => {
    const html = panelHtml(snapshot);

    expect(html).toContain("GOAT · active");
    expect(html).toContain("renova em 23 dias");
    expect(html).toContain("2% do período semanal");
    expect(html).toContain("45% · 7.776 requests este mês");
    expect(html).toContain("saldo 38.5 de 70 créditos");
    expect(html).toContain("5h");
    expect(html).toContain("mensal");
    expect(html).toContain("reseta em");
  });

  it("renders the weekly window as the largest bar of the Command Code card", () => {
    const html = panelHtml(snapshot);
    const block = html.slice(html.indexOf("Command Code"), html.indexOf("Grok"));

    expect(block).toContain('class="limit-bar"><div class="limit-fill" style="width:1.9%');
    expect(block).toContain('<span class="window-label">5h</span>');
    expect(block).toContain('<span class="window-label">mensal</span>');
    expect(block).not.toContain('<span class="window-label">semanal</span>');
  });

  it("renders the OpenCode Go windows", () => {
    const html = panelHtml(snapshot);

    expect(html).toContain("OpenCode Go");
    expect(html).toContain('<span class="window-label">5h</span>');
    expect(html).toContain("11%");
    expect(html).toContain("21%");
    expect(html).toContain("mensal");
    expect(html).toContain("10%");
  });

  it("renders the weekly window as the largest bar of the OpenCode card", () => {
    const html = panelHtml(snapshot);
    const goBlock = html.slice(html.indexOf("OpenCode Go"));

    expect(goBlock).toContain('class="limit-bar"><div class="limit-fill" style="width:21.0%');
    expect(goBlock).toContain("21% do período semanal");
    expect(goBlock).toContain('<span class="window-label">5h</span>');
    expect(goBlock).toContain('<span class="window-label">mensal</span>');
    expect(goBlock).not.toContain('<span class="window-label">semanal</span>');
    expect(goBlock).not.toContain(
      'class="limit-bar small"><div class="limit-fill" style="width:21.0%',
    );
  });

  it("renders the local windows and grok limits", () => {
    const html = panelHtml(snapshot);

    expect(html).toContain("$0.64");
    expect(html).toContain("$2.95");
    expect(html).toContain("77% do período semanal · SuperGrok");
  });

  it("recognizes the active account when it omits the weekly percentage", () => {
    const switched = structuredClone(snapshot);
    switched.providers[1].grok!.creditUsagePercent = null;
    switched.providers[1].grok!.tier = "SuperGrok Plus";

    const html = panelHtml(switched);

    expect(html).toContain("Uso semanal não informado pela conta · SuperGrok Plus");
    expect(html).not.toContain("77% do período semanal");
  });

  it("renders each harness mark in its own card header", () => {
    const html = panelHtml(snapshot);
    const cards = html.split('<section class="card"').slice(1);

    expect(cards).toHaveLength(3);
    for (const markup of cards) {
      expect(markup).toMatch(/<h2><svg [^>]*class="provider-logo"/);
    }
    expect(html.match(/class="provider-logo"/g) ?? []).toHaveLength(3);
  });

  it("collapses the harness cards unless they are expanded", () => {
    const collapsed = panelHtml(snapshot);

    expect(collapsed.match(/class="card expanded"/g) ?? []).toHaveLength(0);
    expect(collapsed.match(/aria-expanded="false"/g) ?? []).toHaveLength(3);

    const expanded = panelHtml(snapshot, null, new Set(["openCode"]));

    expect(expanded).toContain('class="card expanded" data-provider="openCode"');
    expect(expanded.match(/class="card expanded"/g) ?? []).toHaveLength(1);
    expect(expanded.match(/aria-expanded="true"/g) ?? []).toHaveLength(1);
  });

  it("keeps the totals inside the collapsible body of each card", () => {
    const cards = panelHtml(snapshot).split('<section class="card"').slice(1);

    expect(cards).toHaveLength(3);
    for (const markup of cards) {
      const body = markup.slice(markup.indexOf('<div class="card-body">'));

      expect(body).toContain("hoje");
      expect(body).toContain("7 dias");
      expect(body).toContain("30 dias");
    }
  });

  it("renders the launch at login toggle", () => {
    expect(panelHtml(snapshot)).toContain('id="autostart"');
    expect(panelHtml(snapshot)).not.toContain("checked");
    expect(panelHtml(snapshot, null, new Set(), true)).toMatch(/id="autostart" checked/);
  });

  it("renders one star per harness in the card header", () => {
    const html = panelHtml(snapshot, null);

    for (const id of ["grok", "commandCode", "openCode"]) {
      expect(html).toContain(`data-favorite="${id}"`);
    }
    expect(html.match(/class="star/g) ?? []).toHaveLength(3);
    expect(html).not.toContain('class="star active"');
  });

  it("marks only the selected harness as active", () => {
    const html = panelHtml(snapshot, "openCode");

    expect(html.match(/class="star active"/g) ?? []).toHaveLength(1);
    expect(html).toContain('class="star active" data-favorite="openCode"');
    expect(html).toContain('class="star" data-favorite="grok"');
    expect(html).toContain('class="star" data-favorite="commandCode"');
  });

  it("renders a notice when a provider is missing", () => {
    const missing: UsageSnapshot = {
      generatedAt: new Date().toISOString(),
      providers: [
        {
          provider: "openCode",
          status: { state: "notFound", path: "/tmp/missing.db" },
          today: snapshot.providers[2].today,
          last7d: snapshot.providers[2].last7d,
          last30d: snapshot.providers[2].last30d,
        },
      ],
    };

    expect(panelHtml(missing)).toContain("não encontrado em /tmp/missing.db");
  });
});
