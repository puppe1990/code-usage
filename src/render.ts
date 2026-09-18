import commandCodeMark from "../src-tauri/icons/command-code.svg?raw";
import grokMark from "../src-tauri/icons/grok.svg?raw";
import openCodeMark from "../src-tauri/icons/opencode.svg?raw";
import {
  formatCost,
  formatNumber,
  formatPercent,
  formatRelativeTime,
  formatResetCountdown,
  formatTokens,
} from "./format";
import type {
  CommandCodeLimits,
  FavoriteId,
  OpenCodeGoLimits,
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
};

const PROVIDER_MARK: Record<ProviderUsage["provider"], string> = {
  commandCode: commandCodeMark.trim(),
  grok: grokMark.trim(),
  openCode: openCodeMark.trim(),
};

function providerHeading(provider: ProviderUsage["provider"]): string {
  return `${PROVIDER_MARK[provider]}${PROVIDER_LABEL[provider]}`;
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
  const tier = limits.tier ? ` · ${limits.tier}` : "";
  if (limits.creditUsagePercent === null || limits.creditUsagePercent === undefined) {
    return `
    <div class="limit">
      <div class="limit-meta">
        <span>Uso semanal não informado pela conta${tier}</span>
        <span>${formatResetCountdown(limits.periodEnd)}</span>
      </div>
    </div>`;
  }
  return `
    <div class="limit">
      ${limitBar(limits.creditUsagePercent)}
      <div class="limit-meta">
        <span>${formatPercent(limits.creditUsagePercent)} do período semanal${tier}</span>
        <span>${formatResetCountdown(limits.periodEnd)}</span>
      </div>
    </div>`;
}

function commandCodeBadge(limits: CommandCodeLimits): string {
  if (!limits.plan) return "";
  return `<span class="badge">${limits.plan}${limits.status ? ` · ${limits.status}` : ""}</span>`;
}

function commandCodeRenewal(limits: CommandCodeLimits): string {
  if (limits.daysToRenew === null || limits.daysToRenew === undefined) return "";
  return limits.daysToRenew <= 0 ? "renova hoje" : `renova em ${limits.daysToRenew} dias`;
}

function commandCodeRequests(limits: CommandCodeLimits): string {
  const basis =
    limits.periodBasis === "billing-period" ? "requests este mês" : "requests no período";
  return `${formatNumber(limits.requestsThisPeriod)} ${basis}`;
}

function commandCodePrincipal(limits: CommandCodeLimits, requests: string): string {
  if (limits.weekly) {
    return `
      ${limitBar(limits.weekly.percentUsed)}
      <div class="limit-meta">
        <span>${formatPercent(limits.weekly.percentUsed)} do período semanal</span>
        <span>${formatResetCountdown(limits.weekly.resetAt)}</span>
      </div>`;
  }
  return `
      ${limitBar(limits.usagePercent)}
      <div class="limit-meta">
        <span>${formatPercent(limits.usagePercent)} usado</span>
        <span>${requests}</span>
      </div>`;
}

function commandCodeWindows(limits: CommandCodeLimits, requests: string): string {
  return [
    limits.fiveHour ? windowRow("5h", limits.fiveHour.percentUsed, limits.fiveHour.resetAt) : "",
    limits.weekly ? windowRowDetail("mensal", limits.usagePercent, requests) : "",
  ].join("");
}

function commandCodeSection(limits: CommandCodeLimits): string {
  const requests = commandCodeRequests(limits);

  return `
    <div class="limit">
      <div class="limit-meta top">
        ${commandCodeBadge(limits)}
        <span>${commandCodeRenewal(limits)}</span>
      </div>
      ${commandCodePrincipal(limits, requests)}
      ${commandCodeWindows(limits, requests)}
      <div class="limit-foot">saldo ${limits.creditsRemaining.toFixed(1)} de ${limits.creditsTotal.toFixed(0)} créditos</div>
    </div>`;
}

function openCodeGoSection(limits: OpenCodeGoLimits): string {
  const weekly = limits.weekly
    ? `
      ${limitBar(limits.weekly.percent)}
      <div class="limit-meta">
        <span>${formatPercent(limits.weekly.percent)} do período semanal</span>
        <span>${formatResetCountdown(limits.weekly.resetsAt)}</span>
      </div>`
    : "";

  const secondary = [
    limits.rolling ? windowRow("5h", limits.rolling.percent, limits.rolling.resetsAt) : "",
    limits.monthly ? windowRow("mensal", limits.monthly.percent, limits.monthly.resetsAt) : "",
  ].join("");

  if (!weekly && !secondary.trim()) return "";

  return `
    <div class="limit">
      <div class="limit-meta top">
        <span class="badge">OpenCode Go</span>
      </div>
      ${weekly}${secondary}
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

function card(
  usage: ProviderUsage,
  favorite: FavoriteId | null,
  expanded: ReadonlySet<ProviderId>,
): string {
  const showCost = usage.provider !== "grok";
  const isExpanded = expanded.has(usage.provider);
  const updated = usage.lastRecordAt
    ? `atualizado ${formatRelativeTime(usage.lastRecordAt)}`
    : "sem dados";
  return `
    <section class="card${isExpanded ? " expanded" : ""}" data-provider="${usage.provider}">
      <header class="card-head" data-collapse="${usage.provider}" role="button" tabindex="0" aria-expanded="${isExpanded}">
        <h2>${providerHeading(usage.provider)}</h2>
        <span class="card-updated">${updated}${star(usage.provider, favorite === usage.provider)}<span class="chevron" aria-hidden="true">▸</span></span>
      </header>
      ${statusNotice(usage)}
      ${usage.commandCode ? commandCodeSection(usage.commandCode) : ""}
      ${usage.openCodeGo ? openCodeGoSection(usage.openCodeGo) : ""}
      ${grokSection(usage)}
      <div class="card-body">
        ${rows(usage.today, showCost, "hoje")}
        ${rows(usage.last7d, showCost, "7 dias")}
        ${rows(usage.last30d, showCost, "30 dias")}
      </div>
    </section>`;
}

/** Renders the whole panel: one collapsible card per harness. */
export function panelHtml(
  snapshot: UsageSnapshot,
  favorite: FavoriteId | null = null,
  expanded: ReadonlySet<ProviderId> = new Set(),
  autostart = false,
): string {
  return `
    <div class="panel">
      <header class="panel-head">
        <span class="panel-title">Code Usage</span>
        <span class="panel-actions">
          <button id="refresh" title="Atualizar agora">⟳</button>
          <button id="close" title="Fechar">✕</button>
        </span>
      </header>
      <main class="cards">
        ${snapshot.providers.map((usage) => card(usage, favorite, expanded)).join("")}
      </main>
      <label class="toggle">
        <input type="checkbox" id="autostart"${autostart ? " checked" : ""}>
        <span>abrir ao iniciar o Mac</span>
      </label>
      <footer class="panel-foot">
        <span>gerado ${formatRelativeTime(snapshot.generatedAt)} · ★ escolhe o harness do menu bar</span>
        <button id="quit" title="Encerrar o Code Usage">sair</button>
      </footer>
    </div>`;
}

/** Renders the panel into `root`, restoring the caller's star and expanded cards. */
export function renderPanel(
  root: HTMLElement,
  snapshot: UsageSnapshot,
  favorite: FavoriteId | null = null,
  expanded: ReadonlySet<ProviderId> = new Set(),
  autostart = false,
): void {
  root.innerHTML = panelHtml(snapshot, favorite, expanded, autostart);
}
