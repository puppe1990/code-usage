import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { renderPanel } from "./render";
import type {
  AccountEntry,
  AccountMenuState,
  FavoriteId,
  ProviderId,
  UsageSnapshot,
} from "./types";
import "./styles.css";

function requireRoot(): HTMLDivElement {
  const element = document.querySelector<HTMLDivElement>("#app");
  if (!element) throw new Error("#app not found");
  return element;
}

const root = requireRoot();
let favorite: FavoriteId | null = null;
let latest: UsageSnapshot | null = null;
let autostart = false;
const expanded = new Set<ProviderId>();

let accountMenu: ProviderId | null = null;
const accountLists = new Map<ProviderId, AccountEntry[]>();
const accountErrors = new Map<ProviderId, string>();

function menuState(): AccountMenuState | null {
  if (!accountMenu) return null;

  return {
    provider: accountMenu,
    accounts: accountLists.get(accountMenu),
    error: accountErrors.get(accountMenu),
  };
}

function render(): void {
  if (latest) {
    renderPanel(root, latest, favorite, expanded, autostart, menuState());
  }
}

async function pull(): Promise<void> {
  try {
    const snapshot = await invoke<UsageSnapshot | null>("get_usage");
    if (snapshot) {
      latest = snapshot;
      render();
      return;
    }
    root.innerHTML = '<div class="loading">calculando usage…</div>';
  } catch (error) {
    root.innerHTML = `<div class="loading">falha ao ler usage: ${String(error)}</div>`;
  }
}

async function toggleFavorite(next: FavoriteId): Promise<void> {
  favorite = favorite === next ? null : next;
  render();

  try {
    favorite = await invoke<FavoriteId | null>("set_favorite", { favorite });
  } catch (error) {
    console.error("failed to save favorite", error);
  }
  render();
}

function toggleCollapse(provider: ProviderId): void {
  if (expanded.has(provider)) {
    expanded.delete(provider);
  } else {
    expanded.add(provider);
  }
  render();
}

function closeAccountMenu(): void {
  accountMenu = null;
  render();
}

function toggleAccountMenu(provider: ProviderId): void {
  if (accountMenu === provider) {
    closeAccountMenu();
    return;
  }

  accountMenu = provider;
  accountErrors.delete(provider);
  render();
  void loadAccounts(provider);
}

async function loadAccounts(provider: ProviderId): Promise<void> {
  try {
    accountLists.set(provider, await invoke<AccountEntry[]>("list_accounts", { provider }));
  } catch (error) {
    accountErrors.set(provider, String(error));
  }
  if (accountMenu === provider) render();
}

async function switchAccount(provider: ProviderId, name: string): Promise<void> {
  accountErrors.delete(provider);
  render();

  try {
    accountLists.set(provider, await invoke<AccountEntry[]>("switch_account", { provider, name }));
  } catch (error) {
    accountErrors.set(provider, String(error));
  }
  if (accountMenu === provider) render();
}

async function updateAutostart(enabled: boolean): Promise<void> {
  autostart = enabled;
  render();

  try {
    autostart = await invoke<boolean>("set_autostart", { enabled });
  } catch (error) {
    console.error("failed to set autostart", error);
  }
  render();
}

document.addEventListener("click", (event) => {
  const target = event.target as HTMLElement;
  const favorite = target.dataset.favorite as FavoriteId | undefined;

  if (favorite) {
    void toggleFavorite(favorite);
    return;
  }

  const item = target.closest<HTMLElement>("[data-switch]");
  if (item?.dataset.switch && item.dataset.name) {
    void switchAccount(item.dataset.switch as ProviderId, item.dataset.name);
    return;
  }

  const badge = target.closest<HTMLElement>("[data-accounts]");
  if (badge?.dataset.accounts) {
    toggleAccountMenu(badge.dataset.accounts as ProviderId);
    return;
  }

  if (accountMenu && !target.closest("[data-menu]")) closeAccountMenu();

  const header = target.closest<HTMLElement>("[data-collapse]");
  if (header?.dataset.collapse) {
    toggleCollapse(header.dataset.collapse as ProviderId);
    return;
  }
  if (target.id === "refresh") void invoke("refresh_now");
  if (target.id === "close") void invoke("hide_panel");
  if (target.id === "quit") void invoke("quit_app");
});

document.addEventListener("change", (event) => {
  const target = event.target;
  if (target instanceof HTMLInputElement && target.id === "autostart") {
    void updateAutostart(target.checked);
  }
});

document.addEventListener("keydown", (event) => {
  if (event.key === "Escape") {
    if (accountMenu) {
      closeAccountMenu();
      return;
    }
    void invoke("hide_panel");
    return;
  }
  if (event.key !== "Enter" && event.key !== " ") return;
  if (!(event.target instanceof HTMLElement)) return;

  const badge = event.target.closest<HTMLElement>("[data-accounts]");
  if (badge?.dataset.accounts) {
    event.preventDefault();
    toggleAccountMenu(badge.dataset.accounts as ProviderId);
    return;
  }

  const header = event.target.closest<HTMLElement>("[data-collapse]");
  if (!header?.dataset.collapse) return;

  event.preventDefault();
  toggleCollapse(header.dataset.collapse as ProviderId);
});

root.innerHTML = '<div class="loading">carregando usage…</div>';

void invoke<FavoriteId | null>("get_favorite")
  .then((saved) => {
    favorite = saved;
    render();
  })
  .catch((error) => console.error("failed to load favorite", error));

void invoke<boolean>("get_autostart")
  .then((enabled) => {
    autostart = enabled;
    render();
  })
  .catch((error) => console.error("failed to load autostart", error));

void listen<UsageSnapshot>("usage-updated", (event) => {
  latest = event.payload;
  render();
}).catch((error) => {
  console.error("failed to subscribe to usage-updated", error);
});

void pull();
