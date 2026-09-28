import codexMark from "../src-tauri/icons/codex.svg?raw";
import commandCodeMark from "../src-tauri/icons/command-code.svg?raw";
import grokMark from "../src-tauri/icons/grok.svg?raw";
import openCodeMark from "../src-tauri/icons/opencode.svg?raw";
import {
  formatCost,
  formatPercent,
  formatRelativeTime,
  formatResetCountdown,
  formatTokens,
} from "./format";
import type {
  AccountEntry,
  AccountMenuState,
  CodexLimits,
  CodexWindow,
  CommandCodeLimits,
  FavoriteId,
  OpenCodeGoLimits,
  Appearance,
  PanelTab,
  ProviderId,
  ProviderUsage,
  TokenTotals,
  UsageSnapshot,
  UsageWindow,
} from "./types";

const PROVIDER_LABEL: Record<ProviderUsage["provider"], string> = {
  commandCode: "Command Code",
  grok: "Grok",
  openCode: "OpenCode",
  codex: "Codex",
};

const PROVIDER_MARK: Record<ProviderUsage["provider"], string> = {
  commandCode: commandCodeMark.trim(),
  grok: grokMark.trim(),
  openCode: openCodeMark.trim(),
  codex: codexMark.trim(),
};

export const DEFAULT_ORDER: ProviderId[] = ["commandCode", "grok", "openCode", "codex"];

const TAB_LABEL: Record<PanelTab, string> = {
  overview: "Overview",
  commandCode: "Code",
  grok: "Grok",
  openCode: "Open",
  codex: "Codex",
};

export function harnessOrder(order: readonly ProviderId[] = []): ProviderId[] {
  const seen = new Set<ProviderId>();
  const result: ProviderId[] = [];
  for (const id of order) {
    if (DEFAULT_ORDER.includes(id) && !seen.has(id)) {
      seen.add(id);
      result.push(id);
    }
  }
  for (const id of DEFAULT_ORDER) {
    if (!seen.has(id)) result.push(id);
  }
  return result;
}

export function moveHarnessTo(
  order: readonly ProviderId[],
  from: ProviderId,
  to: ProviderId,
): ProviderId[] {
  const ranked = harnessOrder(order);
  const fromIndex = ranked.indexOf(from);
  const toIndex = ranked.indexOf(to);
  if (fromIndex < 0 || toIndex < 0 || fromIndex === toIndex) return ranked;
  ranked.splice(fromIndex, 1);
  ranked.splice(toIndex, 0, from);
  return ranked;
}

const OVERVIEW_MARK = `<svg class="tab-icon" viewBox="0 0 16 16" aria-hidden="true"><rect x="1" y="1" width="6" height="6" rx="1.2" fill="currentColor"/><rect x="9" y="1" width="6" height="6" rx="1.2" fill="currentColor"/><rect x="1" y="9" width="6" height="6" rx="1.2" fill="currentColor"/><rect x="9" y="9" width="6" height="6" rx="1.2" fill="currentColor"/></svg>`;

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function providerHeading(provider: ProviderUsage["provider"]): string {
  return `${PROVIDER_MARK[provider]}${PROVIDER_LABEL[provider]}`;
}

function tabMark(id: PanelTab): string {
  if (id === "overview") return OVERVIEW_MARK;
  return PROVIDER_MARK[id].replace("provider-logo", "provider-logo tab-icon");
}

function tabsHtml(
  selected: PanelTab,
  hidden: ReadonlySet<ProviderId>,
  order: readonly ProviderId[],
): string {
  const items: PanelTab[] = ["overview", ...harnessOrder(order).filter((id) => !hidden.has(id))];
  return `<nav class="tabs">${items
    .map((id) => {
      const active = id === selected ? " active" : "";
      return `<button type="button" class="tab${active}" data-tab="${id}">${tabMark(id)}<span>${TAB_LABEL[id]}</span></button>`;
    })
    .join("")}</nav>`;
}

const APPEARANCE_ITEMS: { id: Appearance; label: string }[] = [
  { id: "dark", label: "Dark" },
  { id: "light", label: "Light" },
  { id: "translucent", label: "Translúcido" },
];

function overviewMenu(
  autostart: boolean,
  settingsOpen: boolean,
  hidden: ReadonlySet<ProviderId>,
  appearance: Appearance,
  order: readonly ProviderId[],
): string {
  const ranked = harnessOrder(order);
  const settings = settingsOpen
    ? `<div class="settings">
        <label class="toggle"><input type="checkbox" id="autostart"${autostart ? " checked" : ""}><span>abrir ao iniciar o Mac</span></label>
        <p class="settings-label">Aparência</p>
        <div class="theme-picks">
          ${APPEARANCE_ITEMS.map(
            (item) =>
              `<button type="button" class="theme-pick${item.id === appearance ? " active" : ""}" data-appearance="${item.id}">${item.label}</button>`,
          ).join("")}
        </div>
        <p class="settings-label">Ordem</p>
        ${ranked
          .map((id, index) => {
            const checked = hidden.has(id) ? "" : " checked";
            const upOff = index === 0 ? " disabled" : "";
            const downOff = index === ranked.length - 1 ? " disabled" : "";
            return `<div class="order-row"><label class="toggle"><input type="checkbox" data-visible="${id}"${checked}><span>${PROVIDER_LABEL[id]}</span></label><span class="order-moves"><button type="button" class="order-move" data-move="up" data-provider="${id}"${upOff} aria-label="Subir">↑</button><button type="button" class="order-move" data-move="down" data-provider="${id}"${downOff} aria-label="Descer">↓</button></span></div>`;
          })
          .join("")}
      </div>`
    : "";

  return `
    <div class="menu">
      <button type="button" class="menu-row" id="refresh">Atualizar</button>
      <div class="menu-row${settingsOpen ? " open" : ""}" data-settings role="button" tabindex="0" aria-expanded="${settingsOpen}"><span>Ajustes</span><span class="chevron" aria-hidden="true">▸</span></div>
      ${settings}
      <button type="button" class="menu-row" id="quit" title="Encerrar o Code Usage">Sair</button>
    </div>`;
}

/** Plan or tier that goes with the account in the switch tooltip. */
function accountDetail(usage: ProviderUsage): string | null {
  if (usage.provider === "commandCode") return usage.commandCode?.plan ?? null;
  if (usage.provider === "grok") return usage.grok?.tier ?? null;
  if (usage.provider === "codex") return usage.codex?.plan ?? null;
  return null;
}

/** The harness name is already in the title, so the `OpenCode Go` credential shows up as `Go`. */
function accountLabel(usage: ProviderUsage): string {
  const account = usage.account ?? "";
  const label = PROVIDER_LABEL[usage.provider];

  return account.startsWith(label) ? account.slice(label.length).trim() : account;
}

/** Account behind the harness: its name is the switch button, the tooltip carries the plan. */
function accountSwitch(usage: ProviderUsage, open: boolean): string {
  if (!usage.account) return "";

  const title = [usage.account, accountDetail(usage), "trocar conta"].filter(Boolean).join(" · ");

  return `<button class="account${open ? " open" : ""}" data-accounts="${usage.provider}" title="${escapeHtml(title)}"><span class="account-name">${escapeHtml(accountLabel(usage))}</span><span class="account-caret" aria-hidden="true">▾</span></button>`;
}

/** What to do when the harness has no saved login yet. */
const ACCOUNT_HINT: Record<ProviderId, string> = {
  commandCode: "nenhuma conta salva — use ccs save <nome>",
  grok: "nenhum perfil salvo em ~/.grok/accounts",
  openCode: "nenhuma conta Go salva — use ocgs save <nome>",
  codex: "nenhuma conta salva — use codex-auth login",
};

function accountItem(provider: ProviderId, account: AccountEntry, disabled: boolean): string {
  const active = account.active ? '<span class="account-active">atual</span>' : "";
  return `<button class="account-item${account.active ? " active" : ""}" data-switch="${provider}" data-name="${escapeHtml(account.name)}"${disabled ? " disabled" : ""}><span>${escapeHtml(account.name)}</span>${active}</button>`;
}

/** Dropdown under the card header listing that harness logins; clicking one switches to it. */
function accountMenu(usage: ProviderUsage, menu: AccountMenuState | null): string {
  if (menu?.provider !== usage.provider) return "";

  const body = menu.error
    ? `<p class="account-error">${escapeHtml(menu.error)}</p>`
    : menu.accounts === undefined
      ? '<p class="account-hint">carregando…</p>'
      : menu.accounts.length === 0
        ? `<p class="account-hint">${escapeHtml(ACCOUNT_HINT[usage.provider])}</p>`
        : menu.accounts
            .map((account) => accountItem(usage.provider, account, menu.switching ?? false))
            .join("");
  const busy = menu.switching ? '<p class="account-hint">trocando…</p>' : "";

  return `<div class="account-menu" data-menu="${usage.provider}">${body}${busy}</div>`;
}

function totalTokens(tokens: TokenTotals): number {
  return tokens.input + tokens.output + tokens.cacheRead + tokens.cacheWrite + tokens.reasoning;
}

function tokensLabel(tokens: TokenTotals): string {
  return `${formatTokens(totalTokens(tokens))} tokens`;
}

function limitBar(percent: number, small = false): string {
  const width = Math.max(0, Math.min(100, percent));
  return `<div class="limit-bar${small ? " small" : ""}"><div class="limit-fill" style="width:${width.toFixed(1)}%"></div></div>`;
}

function star(favorite: FavoriteId, selected: boolean): string {
  const title = selected ? "Tirar do menu bar" : "Mostrar no menu bar";
  return `<button class="star${selected ? " active" : ""}" data-favorite="${favorite}" title="${title}">★</button>`;
}

function windowRow(label: string, percent: number, resetAt: string): string {
  return windowRowDetail(label, percent, formatResetCountdown(resetAt));
}

function windowRowDetail(label: string, percent: number, detail: string): string {
  return `
    <div class="window-row">
      <span class="window-label">${label}</span>
      ${limitBar(percent, true)}
      <span class="window-meta">${formatPercent(percent)} · ${detail}</span>
    </div>`;
}

function rows(window: UsageWindow, showCost: boolean, label: string): string {
  const value = showCost ? formatCost(window.costUsd) : tokensLabel(window.tokens);
  const secondary = showCost ? tokensLabel(window.tokens) : `${window.records} turnos`;
  return `
    <div class="row">
      <span class="row-label">${label}</span>
      <span class="row-value">${value}</span>
      <span class="row-secondary">${secondary}</span>
    </div>`;
}

function grokSection(usage: ProviderUsage): string {
  const limits = usage.grok;
  if (!limits) return "";
  if (limits.creditUsagePercent === null || limits.creditUsagePercent === undefined) {
    return `
    <div class="limit">
      <div class="limit-heading">
        <span>Uso semanal não informado pela conta</span>
        <span>${formatResetCountdown(limits.periodEnd)}</span>
      </div>
    </div>`;
  }
  return `
    <div class="limit">
      <div class="limit-heading">
        <span>${formatPercent(limits.creditUsagePercent)} do período semanal</span>
        <span>${formatResetCountdown(limits.periodEnd)}</span>
      </div>
      ${limitBar(limits.creditUsagePercent)}
    </div>`;
}

function commandCodeRenewal(limits: CommandCodeLimits): string {
  if (limits.daysToRenew === null || limits.daysToRenew === undefined) return "";
  return limits.daysToRenew <= 0 ? "renova hoje" : `renova em ${limits.daysToRenew} dias`;
}

function commandCodePrincipal(limits: CommandCodeLimits): string {
  if (limits.weekly) {
    return `
      <div class="limit-heading">
        <span>${formatPercent(limits.weekly.percentUsed)} do período semanal</span>
        <span>${formatResetCountdown(limits.weekly.resetAt)}</span>
      </div>
      ${limitBar(limits.weekly.percentUsed)}`;
  }
  return `
      <div class="limit-heading">
        <span>${formatPercent(limits.usagePercent)} usado</span>
        <span>${commandCodeRenewal(limits)}</span>
      </div>
      ${limitBar(limits.usagePercent)}`;
}

function commandCodeWindows(limits: CommandCodeLimits): string {
  return [
    limits.fiveHour ? windowRow("5h", limits.fiveHour.percentUsed, limits.fiveHour.resetAt) : "",
    limits.weekly ? windowRowDetail("mensal", limits.usagePercent, commandCodeRenewal(limits)) : "",
  ].join("");
}

function commandCodeSection(limits: CommandCodeLimits): string {
  return `
    <div class="limit">
      ${commandCodePrincipal(limits)}
      ${commandCodeWindows(limits)}
    </div>`;
}

function openCodeGoSection(limits: OpenCodeGoLimits): string {
  const weekly = limits.weekly
    ? `
      <div class="limit-heading">
        <span>${formatPercent(limits.weekly.percent)} do período semanal</span>
        <span>${formatResetCountdown(limits.weekly.resetsAt)}</span>
      </div>
      ${limitBar(limits.weekly.percent)}`
    : "";

  const secondary = [
    limits.rolling ? windowRow("5h", limits.rolling.percent, limits.rolling.resetsAt) : "",
    limits.monthly ? windowRow("mensal", limits.monthly.percent, limits.monthly.resetsAt) : "",
  ].join("");

  if (!weekly && !secondary.trim()) return "";

  return `
    <div class="limit">
      ${weekly}${secondary}
    </div>`;
}

type CodexSlot = "5h" | "weekly" | "monthly";

/** Codex labels every window by its own length: the free plan reports a single monthly one. */
function codexSlot(window: CodexWindow, fallback: CodexSlot): CodexSlot {
  const minutes = window.windowMinutes;
  if (minutes === null || minutes === undefined) return fallback;
  if (minutes <= 6 * 60) return "5h";
  if (minutes <= 7 * 24 * 60) return "weekly";
  return "monthly";
}

function codexBuckets(limits: CodexLimits): {
  fiveHour?: CodexWindow;
  weekly?: CodexWindow;
  monthly?: CodexWindow;
} {
  const buckets: { fiveHour?: CodexWindow; weekly?: CodexWindow; monthly?: CodexWindow } = {};
  const slots: Array<[CodexWindow | null | undefined, CodexSlot]> = [
    [limits.primary, "5h"],
    [limits.secondary, "weekly"],
    [limits.monthly, "monthly"],
  ];
  for (const [window, fallback] of slots) {
    if (!window) continue;
    const slot = codexSlot(window, fallback);
    if (slot === "5h") buckets.fiveHour ??= window;
    else if (slot === "weekly") buckets.weekly ??= window;
    else buckets.monthly ??= window;
  }
  return buckets;
}

function codexReset(window: CodexWindow): string {
  return window.resetsAt ? formatResetCountdown(window.resetsAt) : "sem reset informado";
}

function codexSection(limits: CodexLimits): string {
  const { fiveHour, weekly, monthly } = codexBuckets(limits);
  const heading = weekly
    ? `
      <div class="limit-heading">
        <span>${formatPercent(weekly.percentUsed)} do período semanal</span>
        <span>${codexReset(weekly)}</span>
      </div>
      ${limitBar(weekly.percentUsed)}`
    : monthly
      ? `
      <div class="limit-heading">
        <span>${formatPercent(monthly.percentUsed)} usado</span>
        <span>${codexReset(monthly)}</span>
      </div>
      ${limitBar(monthly.percentUsed)}`
      : "";
  const rows = [
    fiveHour ? windowRowDetail("5h", fiveHour.percentUsed, codexReset(fiveHour)) : "",
    weekly && monthly ? windowRowDetail("mensal", monthly.percentUsed, codexReset(monthly)) : "",
  ].join("");
  if (!heading.trim() && !rows.trim()) return "";

  return `
    <div class="limit">
      ${heading}${rows}
    </div>`;
}

function statusNotice(usage: ProviderUsage): string {
  if (usage.status.state === "notFound") {
    return `<div class="notice">não encontrado em ${usage.status.path}</div>`;
  }
  if (usage.status.state === "error") {
    return `<div class="notice">erro: ${usage.status.message}</div>`;
  }
  return "";
}

function planLabel(usage: ProviderUsage): string | null {
  if (usage.commandCode?.plan) {
    const status = usage.commandCode.status ? ` · ${usage.commandCode.status}` : "";
    return `${usage.commandCode.plan}${status}`;
  }
  if (usage.grok?.tier) return usage.grok.tier;
  if (usage.openCodeGo) return "OpenCode Go";
  if (usage.codex?.plan) return usage.codex.plan;
  return null;
}

function costToggle(provider: ProviderId, isExpanded: boolean): string {
  return `<div class="cost-toggle" data-collapse="${provider}" role="button" tabindex="0" aria-expanded="${isExpanded}"><span>Custo</span><span class="chevron" aria-hidden="true">▸</span></div>`;
}

function card(
  usage: ProviderUsage,
  favorite: FavoriteId | null,
  expanded: ReadonlySet<ProviderId>,
  menu: AccountMenuState | null,
  draggable: boolean,
): string {
  const showCost = usage.provider !== "grok" && usage.provider !== "codex";
  const isExpanded = expanded.has(usage.provider);
  const updated = usage.lastRecordAt
    ? `atualizado ${formatRelativeTime(usage.lastRecordAt)}`
    : "sem dados";
  const plan = planLabel(usage);
  const planHtml = plan ? `<span class="card-plan">${escapeHtml(plan)}</span>` : "";
  const accountHtml = accountSwitch(usage, menu?.provider === usage.provider);
  return `
    <section class="card${isExpanded ? " expanded" : ""}" data-provider="${usage.provider}">
      <header class="card-head">
        <div class="card-title-row">
          ${draggable ? `<button type="button" class="drag-handle" data-drag="${usage.provider}" aria-label="Reordenar">⋮⋮</button>` : ""}
          <h2>${providerHeading(usage.provider)}</h2>
          ${star(usage.provider, favorite === usage.provider)}
        </div>
        <div class="card-sub">
          <span class="card-updated">${updated}</span>
          ${planHtml}
        </div>
        ${accountHtml}
      </header>
      ${accountMenu(usage, menu)}
      ${statusNotice(usage)}
      ${usage.commandCode ? commandCodeSection(usage.commandCode) : ""}
      ${usage.openCodeGo ? openCodeGoSection(usage.openCodeGo) : ""}
      ${usage.codex ? codexSection(usage.codex) : ""}
      ${grokSection(usage)}
      ${costToggle(usage.provider, isExpanded)}
      <div class="card-body">
        ${rows(usage.today, showCost, "hoje")}
        ${rows(usage.last7d, showCost, "7 dias")}
        ${rows(usage.last30d, showCost, "30 dias")}
      </div>
    </section>`;
}

/** Renders the whole panel: Overview of every harness, or one harness tab. */
export function panelHtml(
  snapshot: UsageSnapshot,
  favorite: FavoriteId | null = null,
  expanded: ReadonlySet<ProviderId> = new Set(),
  autostart = false,
  menu: AccountMenuState | null = null,
  tab: PanelTab = "overview",
  settingsOpen = false,
  hidden: ReadonlySet<ProviderId> = new Set(),
  appearance: Appearance = "dark",
  order: readonly ProviderId[] = [],
): string {
  const ranked = harnessOrder(order);
  const rank = new Map(ranked.map((id, index) => [id, index]));
  const shown = snapshot.providers
    .filter((usage) => !hidden.has(usage.provider))
    .sort((left, right) => (rank.get(left.provider) ?? 99) - (rank.get(right.provider) ?? 99));
  const visible = tab === "overview" ? shown : shown.filter((usage) => usage.provider === tab);

  return `
    <div class="panel" data-theme="${appearance}">
      <header class="panel-head">
        ${tabsHtml(tab, hidden, ranked)}
      </header>
      <main class="cards${tab === "overview" ? "" : " single"}">
        ${visible.map((usage) => card(usage, favorite, expanded, menu, tab === "overview")).join("")}
      </main>
      ${
        tab === "overview"
          ? overviewMenu(autostart, settingsOpen, hidden, appearance, ranked)
          : `<footer class="panel-foot">
        <span>gerado ${formatRelativeTime(snapshot.generatedAt)} · ★ escolhe o harness do menu bar</span>
        <button id="quit" title="Encerrar o Code Usage">sair</button>
      </footer>`
      }
    </div>`;
}

/** Renders the panel into `root`, restoring the caller's star, cards and open account menu. */
export function renderPanel(
  root: HTMLElement,
  snapshot: UsageSnapshot,
  favorite: FavoriteId | null = null,
  expanded: ReadonlySet<ProviderId> = new Set(),
  autostart = false,
  menu: AccountMenuState | null = null,
  tab: PanelTab = "overview",
  settingsOpen = false,
  hidden: ReadonlySet<ProviderId> = new Set(),
  appearance: Appearance = "dark",
  order: readonly ProviderId[] = [],
): void {
  root.innerHTML = panelHtml(
    snapshot,
    favorite,
    expanded,
    autostart,
    menu,
    tab,
    settingsOpen,
    hidden,
    appearance,
    order,
  );
}
