import { describe, expect, it } from "vitest";
import { moveHarnessTo, panelHtml } from "./render";
import type { UsageSnapshot } from "./types";

const resetAt = new Date(Date.now() + 6 * 86_400_000 + 16 * 3_600_000).toISOString();
const goResetAt = new Date(Date.now() + 2 * 3_600_000 + 40 * 60_000).toISOString();

const snapshot: UsageSnapshot = {
  generatedAt: new Date().toISOString(),
  providers: [
    {
      provider: "commandCode",
      status: { state: "ok" },
      account: "matheuspuppe1whs",
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
      account: "ericasantiago240@gmail.com",
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
      account: "OpenCode Go",
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
    {
      provider: "codex",
      status: { state: "ok" },
      account: "puppeicaropuppe@gmail.com",
      today: {
        costUsd: 0,
        tokens: { input: 1000, output: 100, cacheRead: 800, cacheWrite: 0, reasoning: 0 },
        records: 2,
      },
      last7d: {
        costUsd: 0,
        tokens: { input: 3000, output: 300, cacheRead: 1800, cacheWrite: 100, reasoning: 40 },
        records: 2,
      },
      last30d: {
        costUsd: 0,
        tokens: { input: 3500, output: 350, cacheRead: 1800, cacheWrite: 100, reasoning: 40 },
        records: 3,
      },
      lastRecordAt: new Date().toISOString(),
      codex: {
        primary: { percentUsed: 18, windowMinutes: 300, resetsAt: goResetAt },
        secondary: { percentUsed: 4.6, windowMinutes: 10080, resetsAt: resetAt },
        monthly: { percentUsed: 12, windowMinutes: 43200, resetsAt: resetAt },
        plan: "plus",
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
    expect(html).toContain("45% · renova em 23 dias");
    expect(html).not.toContain("requests este mês");
    expect(html).not.toContain("créditos");
    expect(html).toContain("5h");
    expect(html).toContain("mensal");
    expect(html).toContain("reseta em");
  });

  it("shows the Command Code renewal on the monthly row, not the header", () => {
    const html = panelHtml(snapshot);
    const block = html.split('data-provider="commandCode"')[1].split("<section")[0];
    const head = block.slice(0, block.indexOf('class="limit"'));
    const monthly = block.slice(block.indexOf('<span class="window-label">mensal</span>'));

    expect(head).not.toContain("renova em");
    expect(monthly).toContain("renova em 23 dias");
    expect(monthly).not.toContain("requests");
  });

  it("renders the weekly window as the largest bar of the Command Code card", () => {
    const html = panelHtml(snapshot);
    const block = html.split('data-provider="commandCode"')[1].split("<section")[0];

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
    expect(html).toContain("77% do período semanal");
  });

  it("renders the Codex weekly window as the largest bar, with 5h and mensal below", () => {
    const codex = panelHtml(snapshot).split('<section class="card"').slice(1)[3];

    expect(codex).toContain('<span class="card-plan">plus</span>');
    expect(codex).toContain("5% do período semanal");
    expect(codex).toContain('class="limit-bar"><div class="limit-fill" style="width:4.6%');
    expect(codex).toContain('<span class="window-label">5h</span>');
    expect(codex).toContain('class="limit-bar small"><div class="limit-fill" style="width:18.0%');
    expect(codex).toContain('<span class="window-meta">18% · reseta em');
    expect(codex).toContain('<span class="window-label">mensal</span>');
    expect(codex).toContain('class="limit-bar small"><div class="limit-fill" style="width:12.0%');
    expect(codex).not.toContain('<span class="window-label">W</span>');
    expect(codex).not.toContain('<span class="window-label">M</span>');
  });

  it("omits the Codex mensal row when the plan has no monthly window", () => {
    const plus = structuredClone(snapshot);
    plus.providers[3].codex = {
      primary: { percentUsed: 18, windowMinutes: 300, resetsAt: goResetAt },
      secondary: { percentUsed: 4.6, windowMinutes: 10080, resetsAt: resetAt },
      plan: "plus",
      fetchedAt: new Date().toISOString(),
    };

    const codex = panelHtml(plus).split('<section class="card"').slice(1)[3];

    expect(codex).toContain("5% do período semanal");
    expect(codex).toContain('<span class="window-label">5h</span>');
    expect(codex).not.toContain('<span class="window-label">mensal</span>');
  });

  it("labels the single monthly window of a free Codex plan", () => {
    const free = structuredClone(snapshot);
    free.providers[3].codex = {
      primary: { percentUsed: 100, windowMinutes: 43200, resetsAt: resetAt },
      plan: "free",
      fetchedAt: new Date().toISOString(),
    };

    const codex = panelHtml(free).split('<section class="card"').slice(1)[3];

    expect(codex).toContain('<span class="card-plan">free</span>');
    expect(codex).toContain("100% usado");
    expect(codex).toContain('class="limit-bar"><div class="limit-fill" style="width:100.0%');
    expect(codex).not.toContain('<span class="window-label">5h</span>');
    expect(codex).not.toContain('<span class="window-label">W</span>');
    expect(codex).not.toContain('<span class="window-label">mensal</span>');
  });

  it("shows tokens instead of a price on the Codex card", () => {
    const codex = panelHtml(snapshot).split('<section class="card"').slice(1)[3];
    const body = codex.slice(codex.indexOf('<div class="card-body">'));

    expect(body).toContain("1.9k tokens");
    expect(body).toContain("2 turnos");
    expect(body).not.toContain("$");
  });

  it("leaves the Codex card without a limits block when the rollouts carry none", () => {
    const bare = structuredClone(snapshot);
    delete bare.providers[3].codex;

    const codex = panelHtml(bare).split('<section class="card"').slice(1)[3];

    expect(codex).not.toContain('class="limit"');
    expect(codex).toContain("1.9k tokens");
  });

  it("recognizes the active account when it omits the weekly percentage", () => {
    const switched = structuredClone(snapshot);
    switched.providers[1].grok!.creditUsagePercent = null;
    switched.providers[1].grok!.tier = "SuperGrok Plus";

    const html = panelHtml(switched);

    expect(html).toContain("Uso semanal não informado pela conta");
    expect(html).toContain('<span class="card-plan">SuperGrok Plus</span>');
    expect(html).not.toContain("77% do período semanal");
  });

  it("renders each harness mark in its own card header", () => {
    const html = panelHtml(snapshot);
    const cards = html.split('<section class="card"').slice(1);

    expect(cards).toHaveLength(4);
    for (const markup of cards) {
      expect(markup).toMatch(/<h2><svg [^>]*class="provider-logo"/);
    }
    expect(html.match(/class="provider-logo"/g) ?? []).toHaveLength(4);
  });

  it("renders one account switch per harness, with the plan in the tooltip", () => {
    const html = panelHtml(snapshot);

    expect(html.match(/class="account"/g) ?? []).toHaveLength(4);
    expect(html).toContain(
      '<button class="account" data-accounts="commandCode" title="matheuspuppe1whs · GOAT · trocar conta"><span class="account-name">matheuspuppe1whs</span><span class="account-caret" aria-hidden="true">▾</span></button>',
    );
    expect(html).toContain(
      '<button class="account" data-accounts="codex" title="puppeicaropuppe@gmail.com · plus · trocar conta"><span class="account-name">puppeicaropuppe@gmail.com</span>',
    );
    expect(html).toContain(
      '<button class="account" data-accounts="grok" title="ericasantiago240@gmail.com · SuperGrok · trocar conta"><span class="account-name">ericasantiago240@gmail.com</span>',
    );
    expect(html).toContain(
      '<button class="account" data-accounts="openCode" title="OpenCode Go · trocar conta"><span class="account-name">Go</span>',
    );
  });

  it("skips the switch when the harness reports no account", () => {
    const anonymous = structuredClone(snapshot);
    for (const provider of anonymous.providers) delete provider.account;

    expect(panelHtml(anonymous)).not.toContain('class="account"');
  });

  it("escapes the account in the tooltip", () => {
    const hostile = structuredClone(snapshot);
    hostile.providers[1].account = 'x"><script>alert(1)</script>';

    const html = panelHtml(hostile);

    expect(html).not.toContain("<script>");
    expect(html).toContain("&quot;&gt;&lt;script&gt;");
  });

  it("shows the switch in flight and locks the list while it runs", () => {
    const html = panelHtml(snapshot, null, new Set(), false, {
      provider: "grok",
      switching: true,
      accounts: [
        { name: "pessoal", active: true },
        { name: "trabalho", active: false },
      ],
    });

    expect(html).toContain('<p class="account-hint">trocando…</p>');
    expect(html.match(/disabled/g) ?? []).toHaveLength(2);
    expect(html).toContain('data-name="trabalho" disabled');
  });

  it("keeps the list clickable when no switch is in flight", () => {
    const html = panelHtml(snapshot, null, new Set(), false, {
      provider: "grok",
      accounts: [{ name: "pessoal", active: true }],
    });

    expect(html).not.toContain("disabled");
    expect(html).not.toContain("trocando…");
  });

  it("keeps the menu closed until the badge is clicked", () => {
    expect(panelHtml(snapshot)).not.toContain('class="account-menu"');
    expect(panelHtml(snapshot)).not.toContain('class="account open"');
  });

  it("lists the harness accounts in the open menu, marking the active one", () => {
    const html = panelHtml(snapshot, null, new Set(), false, {
      provider: "grok",
      accounts: [
        { name: "pessoal", active: true },
        { name: "trabalho", active: false },
      ],
    });

    expect(html).toContain('<button class="account open" data-accounts="grok"');
    expect(html).toContain('<div class="account-menu" data-menu="grok">');
    expect(html).toContain(
      '<button class="account-item active" data-switch="grok" data-name="pessoal"><span>pessoal</span><span class="account-active">atual</span></button>',
    );
    expect(html).toContain(
      '<button class="account-item" data-switch="grok" data-name="trabalho"><span>trabalho</span></button>',
    );
    expect(html.match(/class="account-item/g) ?? []).toHaveLength(2);
  });

  it("renders the menu only inside its own card", () => {
    const html = panelHtml(snapshot, null, new Set(), false, {
      provider: "openCode",
      accounts: [],
    });
    const cards = html.split('<section class="card"').slice(1);

    expect(html.match(/<div class="account-menu"/g) ?? []).toHaveLength(1);
    expect(cards[0]).not.toContain("account-menu");
    expect(cards[1]).not.toContain("account-menu");
    expect(cards[2]).toContain('<div class="account-menu" data-menu="openCode">');
  });

  it("shows a loading line while the account list is on its way", () => {
    const html = panelHtml(snapshot, null, new Set(), false, { provider: "grok" });

    expect(html).toContain('<p class="account-hint">carregando…</p>');
  });

  it("tells what to do when the harness has no saved login", () => {
    const commandCode = panelHtml(snapshot, null, new Set(), false, {
      provider: "commandCode",
      accounts: [],
    });
    const grok = panelHtml(snapshot, null, new Set(), false, { provider: "grok", accounts: [] });
    const codex = panelHtml(snapshot, null, new Set(), false, { provider: "codex", accounts: [] });

    expect(commandCode).toContain("use ccs save &lt;nome&gt;");
    expect(grok).toContain("nenhum perfil salvo em ~/.grok/accounts");
    expect(codex).toContain("use codex-auth login");
  });

  it("surfaces a switch failure inside the menu", () => {
    const html = panelHtml(snapshot, null, new Set(), false, {
      provider: "grok",
      accounts: [{ name: "pessoal", active: true }],
      error: 'conta "nope" não existe',
    });

    expect(html).toContain('<p class="account-error">conta &quot;nope&quot; não existe</p>');
    expect(html).not.toContain("account-item");
  });

  it("escapes the account name handed to the switch button", () => {
    const html = panelHtml(snapshot, null, new Set(), false, {
      provider: "commandCode",
      accounts: [{ name: 'a"><script>x</script>', active: false }],
    });

    expect(html).not.toContain("<script>");
    expect(html).toContain('data-name="a&quot;&gt;&lt;script&gt;x&lt;/script&gt;"');
  });

  it("collapses the harness cards unless they are expanded", () => {
    const collapsed = panelHtml(snapshot);

    expect(collapsed.match(/class="card expanded"/g) ?? []).toHaveLength(0);
    expect(collapsed.match(/class="cost-toggle"[^>]*aria-expanded="false"/g) ?? []).toHaveLength(4);

    const expanded = panelHtml(snapshot, null, new Set(["openCode"]));

    expect(expanded).toContain('class="card expanded" data-provider="openCode"');
    expect(expanded.match(/class="card expanded"/g) ?? []).toHaveLength(1);
    expect(expanded.match(/class="cost-toggle"[^>]*aria-expanded="true"/g) ?? []).toHaveLength(1);
  });

  it("keeps the totals inside the collapsible body of each card", () => {
    const cards = panelHtml(snapshot).split('<section class="card"').slice(1);

    expect(cards).toHaveLength(4);
    for (const markup of cards) {
      const body = markup.slice(markup.indexOf('<div class="card-body">'));

      expect(body).toContain("hoje");
      expect(body).toContain("7 dias");
      expect(body).toContain("30 dias");
    }
  });

  it("puts Custo on its own row, after the plan limits", () => {
    const cards = panelHtml(snapshot).split('<section class="card"').slice(1);

    expect(cards).toHaveLength(4);
    for (const markup of cards) {
      const head = markup.slice(0, markup.indexOf("cost-toggle"));
      const custoAt = markup.indexOf('class="cost-toggle"');
      const bodyAt = markup.indexOf('class="card-body"');

      expect(head).not.toContain("data-collapse");
      expect(head).not.toContain("chevron");
      expect(markup).toContain("Custo");
      expect(custoAt).toBeGreaterThan(markup.indexOf("card-head"));
      expect(bodyAt).toBeGreaterThan(custoAt);
    }
  });

  it("keeps the collapse control on the Custo row", () => {
    const html = panelHtml(snapshot, null, new Set(["grok"]));

    expect(html).toContain(
      'class="cost-toggle" data-collapse="grok" role="button" tabindex="0" aria-expanded="true"',
    );
    expect(html).toContain(
      'class="cost-toggle" data-collapse="commandCode" role="button" tabindex="0" aria-expanded="false"',
    );
    expect(html.match(/class="cost-toggle"/g) ?? []).toHaveLength(4);
  });

  it("renders Overview and one tab per harness", () => {
    const html = panelHtml(snapshot);

    expect(html).toContain('data-tab="overview"');
    expect(html).toContain('data-tab="commandCode"');
    expect(html).toContain('data-tab="grok"');
    expect(html).toContain('data-tab="openCode"');
    expect(html).toContain('data-tab="codex"');
    expect(html).toContain('class="tab active" data-tab="overview"');
    expect(html).not.toContain('class="panel-title"');
    expect(html.split('<section class="card"').slice(1)).toHaveLength(4);
  });

  it("shows only the selected harness on its tab", () => {
    const html = panelHtml(snapshot, null, new Set(), false, null, "grok");
    const cards = html.split('<section class="card"').slice(1);

    expect(html).toContain('class="tab active" data-tab="grok"');
    expect(cards).toHaveLength(1);
    expect(cards[0]).toContain('data-provider="grok"');
    expect(html).not.toContain('data-provider="commandCode"');
  });

  it("puts the plan on the same row as the updated time, like CodexBar", () => {
    const html = panelHtml(snapshot);
    const cards = html.split('<section class="card"').slice(1);
    const [command, grok, openCode, codex] = cards.map((markup) =>
      markup.slice(
        0,
        markup.indexOf('class="limit"') === -1 ? markup.length : markup.indexOf('class="limit"'),
      ),
    );

    for (const head of [command, grok, openCode, codex]) {
      const title = head.slice(head.indexOf("card-title-row"), head.indexOf("card-sub"));
      const sub = head.slice(head.indexOf("card-sub"));

      expect(title).not.toContain("card-plan");
      expect(sub).toContain("card-updated");
      expect(sub).toContain("card-plan");
    }

    expect(command).toContain('<span class="card-plan">GOAT · active</span>');
    expect(grok).toContain('<span class="card-plan">SuperGrok</span>');
    expect(openCode).toContain('<span class="card-plan">OpenCode Go</span>');
    expect(codex).toContain('<span class="card-plan">plus</span>');
  });

  it("puts the weekly heading above the bar and keeps SuperGrok out of that line", () => {
    const grok = panelHtml(snapshot).split('<section class="card"').slice(1)[1];
    const heading = grok.indexOf("77% do período semanal");
    const bar = grok.indexOf('class="limit-bar">');

    expect(heading).toBeGreaterThan(-1);
    expect(bar).toBeGreaterThan(heading);
    expect(grok.slice(heading, grok.indexOf("cost-toggle"))).not.toContain("SuperGrok");
  });

  it("puts Refresh and Settings at the bottom of Overview", () => {
    const html = panelHtml(snapshot);
    const cardsAt = html.lastIndexOf("card-body");
    const refreshAt = html.indexOf('id="refresh"');
    const settingsAt = html.indexOf("data-settings");

    expect(html).toContain("Atualizar");
    expect(html).toContain("Ajustes");
    expect(refreshAt).toBeGreaterThan(cardsAt);
    expect(settingsAt).toBeGreaterThan(refreshAt);
    expect(html).not.toContain('id="autostart"');
  });

  it("keeps autostart and harness visibility inside Settings", () => {
    const html = panelHtml(snapshot, null, new Set(), true, null, "overview", true);

    expect(html).toMatch(/id="autostart" checked/);
    expect(html).toContain("abrir ao iniciar o Mac");
    expect(html).toContain('data-visible="commandCode"');
    expect(html).toContain('data-visible="grok"');
    expect(html).toContain('data-visible="openCode"');
    expect(html).toContain('data-visible="codex"');
  });

  it("offers Dark, Light and Translúcido in Settings", () => {
    const html = panelHtml(snapshot, null, new Set(), false, null, "overview", true);

    expect(html).toContain('class="panel" data-theme="dark"');
    expect(html).toContain('data-appearance="dark"');
    expect(html).toContain('data-appearance="light"');
    expect(html).toContain('data-appearance="translucent"');
    expect(html).toContain(">Dark<");
    expect(html).toContain(">Light<");
    expect(html).toContain("Translúcido");
    expect(html).toContain('class="theme-pick active" data-appearance="dark"');
  });

  it("marks the selected appearance on the panel", () => {
    const html = panelHtml(
      snapshot,
      null,
      new Set(),
      false,
      null,
      "overview",
      true,
      new Set(),
      "light",
    );

    expect(html).toContain('class="panel" data-theme="light"');
    expect(html).toContain('class="theme-pick active" data-appearance="light"');
    expect(html).not.toContain('class="theme-pick active" data-appearance="dark"');
  });

  it("lists move controls for each harness in Settings", () => {
    const html = panelHtml(snapshot, null, new Set(), false, null, "overview", true);

    expect(html).toContain("Ordem");
    for (const id of ["commandCode", "grok", "openCode", "codex"]) {
      expect(html).toContain(`data-move="up" data-provider="${id}"`);
      expect(html).toContain(`data-move="down" data-provider="${id}"`);
    }
  });

  it("puts a drag handle on Overview cards and leaves the single-harness card still", () => {
    const overview = panelHtml(snapshot);
    const grokTab = panelHtml(snapshot, null, new Set(), false, null, "grok");

    expect(overview).toContain('data-drag="commandCode"');
    expect(overview).toContain('data-drag="grok"');
    expect(grokTab).not.toContain("data-drag=");
  });

  it("moves a harness to another slot in the order", () => {
    expect(
      moveHarnessTo(["commandCode", "grok", "openCode", "codex"], "commandCode", "openCode"),
    ).toEqual(["grok", "openCode", "commandCode", "codex"]);
    expect(moveHarnessTo(["commandCode", "grok", "openCode", "codex"], "codex", "grok")).toEqual([
      "commandCode",
      "codex",
      "grok",
      "openCode",
    ]);
  });

  it("reorders tabs and Overview cards from the settings order", () => {
    const html = panelHtml(
      snapshot,
      null,
      new Set(),
      false,
      null,
      "overview",
      false,
      new Set(),
      "dark",
      ["grok", "codex", "commandCode", "openCode"],
    );
    const tabs = [...html.matchAll(/data-tab="([^"]+)"/g)].map((match) => match[1]);
    const cards = [...html.matchAll(/class="card[^"]*" data-provider="([^"]+)"/g)].map(
      (match) => match[1],
    );

    expect(tabs).toEqual(["overview", "grok", "codex", "commandCode", "openCode"]);
    expect(cards).toEqual(["grok", "codex", "commandCode", "openCode"]);
  });

  it("hides a harness from Overview and from the tabs", () => {
    const html = panelHtml(
      snapshot,
      null,
      new Set(),
      false,
      null,
      "overview",
      false,
      new Set(["grok"]),
    );

    expect(html).not.toContain('data-tab="grok"');
    expect(html).not.toContain('data-provider="grok"');
    expect(html).toContain('data-tab="commandCode"');
    expect(html).toContain('data-provider="commandCode"');
    expect(html.split('<section class="card"').slice(1)).toHaveLength(3);
  });

  it("renders one star per harness in the card header", () => {
    const html = panelHtml(snapshot, null);

    for (const id of ["grok", "commandCode", "openCode", "codex"]) {
      expect(html).toContain(`data-favorite="${id}"`);
    }
    expect(html.match(/class="star/g) ?? []).toHaveLength(4);
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
