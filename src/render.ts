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

function star(favorite: FavoriteId, favorites: FavoriteId[]): string {
  const active = favorites.includes(favorite);
  const title = active ? "Não mostrar no menu bar" : "Mostrar no menu bar";
  return `<button class="star${active ? " active" : ""}" data-favorite="${favorite}" title="${title}">★</button>`;
}

function windowRow(label: string, percent: number, resetAt: string): string {
  return `
    <div class="window-row">
      <span class="window-label">${label}</span>
      ${limitBar(percent, true)}
      <span class="window-meta">${formatPercent(percent)} · ${formatResetCountdown(resetAt)}</span>
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

function grokSection(usage: ProviderUsage, favorites: FavoriteId[]): string {
  const limits = usage.grok;
  if (!limits) return "";
  const tier = limits.tier ? ` · ${limits.tier}` : "";
  return `
    <div class="limit">
      ${limitBar(limits.creditUsagePercent)}
      <div class="limit-meta">
        <span>${formatPercent(limits.creditUsagePercent)} do período semanal${tier}</span>
        <span>${formatResetCountdown(limits.periodEnd)}${star("grokWeekly", favorites)}</span>
      </div>
    </div>`;
}

function commandCodeSection(limits: CommandCodeLimits, favorites: FavoriteId[]): string {
  const badge = limits.plan
    ? `<span class="badge">${limits.plan}${limits.status ? ` · ${limits.status}` : ""}</span>`
    : "";

  const renew =
    limits.daysToRenew === null || limits.daysToRenew === undefined
      ? ""
      : limits.daysToRenew <= 0
        ? "renova hoje"
        : `renova em ${limits.daysToRenew} dias`;

  const requests = `${formatNumber(limits.requestsThisPeriod)} ${
    limits.periodBasis === "billing-period" ? "requests este mês" : "requests no período"
  }`;

  const windows = [
    limits.fiveHour ? windowRow("5h", limits.fiveHour.percentUsed, limits.fiveHour.resetAt) : "",
    limits.weekly ? windowRow("semanal", limits.weekly.percentUsed, limits.weekly.resetAt) : "",
  ].join("");

  return `
    <div class="limit">
      <div class="limit-meta top">
        ${badge}
        <span>${renew}${star("commandCodePlan", favorites)}</span>
      </div>
      ${limitBar(limits.usagePercent)}
      <div class="limit-meta">
        <span>${formatPercent(limits.usagePercent)} usado</span>
        <span>${requests}</span>
      </div>
      ${windows}
      <div class="limit-foot">saldo ${limits.creditsRemaining.toFixed(1)} de ${limits.creditsTotal.toFixed(0)} créditos</div>
    </div>`;
}

function openCodeGoSection(limits: OpenCodeGoLimits, favorites: FavoriteId[]): string {
  const weekly = limits.weekly
    ? `
      ${limitBar(limits.weekly.percent)}
      <div class="limit-meta">
        <span>${formatPercent(limits.weekly.percent)} do período semanal</span>
        <span>${formatResetCountdown(limits.weekly.resetsAt)}${star("openCodeGoWeekly", favorites)}</span>
      </div>`
    : "";

  const secondary = [
    limits.rolling ? windowRow("rolling", limits.rolling.percent, limits.rolling.resetsAt) : "",
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

function card(usage: ProviderUsage, favorites: FavoriteId[]): string {
  const showCost = usage.provider !== "grok";
  const updated = usage.lastRecordAt
    ? `atualizado ${formatRelativeTime(usage.lastRecordAt)}`
    : "sem dados";
  const costFavorite =
    usage.provider === "commandCode"
      ? star("commandCodeTodayCost", favorites)
      : usage.provider === "openCode"
        ? star("openCodeTodayCost", favorites)
        : "";
  return `
    <section class="card">
      <header class="card-head">
        <h2>${PROVIDER_LABEL[usage.provider]}</h2>
        <span class="card-updated">${updated}${costFavorite}</span>
      </header>
      ${statusNotice(usage)}
      ${usage.commandCode ? commandCodeSection(usage.commandCode, favorites) : ""}
      ${usage.openCodeGo ? openCodeGoSection(usage.openCodeGo, favorites) : ""}
      ${grokSection(usage, favorites)}
      ${rows(usage.today, showCost, "hoje")}
      ${rows(usage.last7d, showCost, "7 dias")}
      ${rows(usage.last30d, showCost, "30 dias")}
    </section>`;
}

export function panelHtml(snapshot: UsageSnapshot, favorites: FavoriteId[] = []): string {
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
        ${snapshot.providers.map((usage) => card(usage, favorites)).join("")}
      </main>
      <footer class="panel-foot">
        <span>gerado ${formatRelativeTime(snapshot.generatedAt)} · ★ escolhe o que vai pro menu bar</span>
        <button id="quit" title="Encerrar o Code Usage">sair</button>
      </footer>
    </div>`;
}

export function renderPanel(
  root: HTMLElement,
  snapshot: UsageSnapshot,
  favorites: FavoriteId[] = [],
): void {
  root.innerHTML = panelHtml(snapshot, favorites);
}
