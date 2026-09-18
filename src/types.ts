export type ProviderId = "commandCode" | "grok" | "openCode";

export interface TokenTotals {
  input: number;
  output: number;
  cacheRead: number;
  cacheWrite: number;
  reasoning: number;
}

export interface UsageWindow {
  costUsd: number;
  tokens: TokenTotals;
  records: number;
}

export type ProviderStatus =
  | { state: "ok" }
  | { state: "notFound"; path: string }
  | { state: "error"; message: string };

export interface GrokLimits {
  creditUsagePercent: number;
  periodStart: string;
  periodEnd: string;
  tier?: string | null;
  fetchedAt: string;
}

export interface WindowLimit {
  percentUsed: number;
  used: number;
  cap: number;
  resetAt: string;
}

export interface CommandCodeLimits {
  plan?: string | null;
  planId?: string | null;
  status?: string | null;
  usagePercent: number;
  creditsTotal: number;
  creditsRemaining: number;
  requestsThisPeriod: number;
  periodBasis?: string | null;
  renewsAt?: string | null;
  daysToRenew?: number | null;
  fiveHour?: WindowLimit | null;
  weekly?: WindowLimit | null;
  fetchedAt: string;
}

export interface ProviderUsage {
  provider: ProviderId;
  status: ProviderStatus;
  today: UsageWindow;
  last7d: UsageWindow;
  last30d: UsageWindow;
  lastRecordAt?: string | null;
  grok?: GrokLimits | null;
  commandCode?: CommandCodeLimits | null;
}

export interface UsageSnapshot {
  generatedAt: string;
  providers: ProviderUsage[];
}
