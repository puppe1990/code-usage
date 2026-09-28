import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { DEFAULT_ORDER, harnessOrder, moveHarnessTo, renderPanel } from "./render";
import type {
  AccountEntry,
  AccountMenuState,
  Appearance,
  FavoriteId,
  PanelTab,
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
let tab: PanelTab = "overview";
let settingsOpen = false;
let appearance: Appearance = "dark";
let order: ProviderId[] = [...DEFAULT_ORDER];
const hidden = new Set<ProviderId>();
const expanded = new Set<ProviderId>();

let accountMenu: ProviderId | null = null;
let accountSwitching: ProviderId | null = null;
const accountLists = new Map<ProviderId, AccountEntry[]>();
const accountErrors = new Map<ProviderId, string>();

function menuState(): AccountMenuState | null {
  if (!accountMenu) return null;

  return {
    provider: accountMenu,
    accounts: accountLists.get(accountMenu),
    switching: accountSwitching === accountMenu,
    error: accountErrors.get(accountMenu),
  };
}

function render(): void {
  if (!latest) return;

  try {
    renderPanel(
      root,
      latest,
      favorite,
      expanded,
      autostart,
      menuState(),
      tab,
      settingsOpen,
      hidden,
      appearance,
      order,
    );
  } catch (error) {
    // um render quebrado não pode deixar o painel congelado no desenho anterior
    console.error("failed to render the panel", error);
    const notice = document.createElement("div");
    notice.className = "loading";
    notice.textContent = `falha ao desenhar o painel: ${String(error)}`;
    root.replaceChildren(notice);
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
  accountSwitching = provider;
  render();

  try {
    accountLists.set(provider, await invoke<AccountEntry[]>("switch_account", { provider, name }));
  } catch (error) {
    accountErrors.set(provider, String(error));
  } finally {
    accountSwitching = null;
  }

  if (accountMenu === provider) render();
}

async function saveHidden(): Promise<void> {
  try {
    const saved = await invoke<ProviderId[]>("set_hidden", { hidden: [...hidden] });
    hidden.clear();
    for (const id of saved) hidden.add(id);
    if (favorite && hidden.has(favorite)) favorite = null;
  } catch (error) {
    console.error("failed to save hidden harnesses", error);
  }
  render();
}

function toggleHarnessVisible(id: ProviderId, visible: boolean): void {
  if (visible) {
    hidden.delete(id);
  } else {
    if (hidden.size >= 3) {
      render();
      return;
    }
    hidden.add(id);
    if (tab === id) tab = "overview";
  }
  render();
  void saveHidden();
}

async function saveOrder(): Promise<void> {
  try {
    order = await invoke<ProviderId[]>("set_order", { order });
  } catch (error) {
    console.error("failed to save harness order", error);
  }
  render();
}

function placeHarness(from: ProviderId, to: ProviderId): void {
  const next = moveHarnessTo(order, from, to);
  if (next.join() === harnessOrder(order).join()) return;
  order = next;
  render();
  void saveOrder();
}

function moveHarness(id: ProviderId, direction: -1 | 1): void {
  const ranked = harnessOrder(order);
  const index = ranked.indexOf(id);
  const next = index + direction;
  if (index < 0 || next < 0 || next >= ranked.length) return;
  const swap = ranked[index];
  ranked[index] = ranked[next];
  ranked[next] = swap;
  order = ranked;
  render();
  void saveOrder();
}

async function setAppearance(next: Appearance): Promise<void> {
  appearance = next;
  render();
  try {
    appearance = await invoke<Appearance>("set_theme", { theme: next });
  } catch (error) {
    console.error("failed to save theme", error);
  }
  render();
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

let draggingCard = false;

function dragCard(target: EventTarget | null): HTMLElement | null {
  if (!(target instanceof HTMLElement)) return null;
  if (
    target.closest(
      "button, input, a, label, [data-collapse], [data-accounts], [data-favorite], [data-menu], [data-tab], [data-settings], [data-move]",
    )
  ) {
    return null;
  }
  return target.closest(".card[data-provider]");
}

document.addEventListener("dragstart", (event) => {
  const card = dragCard(event.target);
  if (tab !== "overview" || !card?.dataset.provider || !event.dataTransfer) {
    event.preventDefault();
    return;
  }
  draggingCard = true;
  event.dataTransfer.effectAllowed = "move";
  event.dataTransfer.setData("text/plain", card.dataset.provider);
  card.classList.add("dragging");
});

document.addEventListener("dragover", (event) => {
  if (!(event.target instanceof HTMLElement) || tab !== "overview") return;
  const card = event.target.closest<HTMLElement>(".card[data-provider]");
  if (!card) return;
  event.preventDefault();
  if (event.dataTransfer) event.dataTransfer.dropEffect = "move";
  for (const other of root.querySelectorAll(".card.drop-target")) {
    if (other !== card) other.classList.remove("drop-target");
  }
  if (!card.classList.contains("dragging")) card.classList.add("drop-target");
});

document.addEventListener("drop", (event) => {
  if (!(event.target instanceof HTMLElement)) return;
  const card = event.target.closest<HTMLElement>(".card[data-provider]");
  const from = event.dataTransfer?.getData("text/plain") as ProviderId | undefined;
  const to = card?.dataset.provider;
  event.preventDefault();
  if (!from || !to) return;
  placeHarness(from, to as ProviderId);
});

document.addEventListener("dragend", () => {
  for (const card of root.querySelectorAll(".card.dragging, .card.drop-target")) {
    card.classList.remove("dragging", "drop-target");
  }
  window.setTimeout(() => {
    draggingCard = false;
  }, 0);
});

document.addEventListener("click", (event) => {
  if (draggingCard) return;
  const target = event.target as HTMLElement;
  const favorite = target.dataset.favorite as FavoriteId | undefined;

  const tabBtn = target.closest<HTMLElement>("[data-tab]");
  if (tabBtn?.dataset.tab) {
    tab = tabBtn.dataset.tab as PanelTab;
    accountMenu = null;
    settingsOpen = false;
    render();
    return;
  }

  const move = target.closest<HTMLElement>("[data-move]");
  if (move?.dataset.move && move.dataset.provider) {
    moveHarness(move.dataset.provider as ProviderId, move.dataset.move === "up" ? -1 : 1);
    return;
  }

  const appearanceBtn = target.closest<HTMLElement>("[data-appearance]");
  if (appearanceBtn?.dataset.appearance) {
    void setAppearance(appearanceBtn.dataset.appearance as Appearance);
    return;
  }

  if (target.closest("[data-settings]")) {
    settingsOpen = !settingsOpen;
    render();
    return;
  }

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
  if (!(target instanceof HTMLInputElement)) return;
  if (target.id === "autostart") {
    void updateAutostart(target.checked);
    return;
  }
  if (target.dataset.visible) {
    toggleHarnessVisible(target.dataset.visible as ProviderId, target.checked);
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

  const settings = event.target.closest<HTMLElement>("[data-settings]");
  if (settings) {
    event.preventDefault();
    settingsOpen = !settingsOpen;
    render();
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

void invoke<ProviderId[]>("get_order")
  .then((saved) => {
    order = harnessOrder(saved);
    render();
  })
  .catch((error) => console.error("failed to load harness order", error));

void invoke<Appearance>("get_theme")
  .then((saved) => {
    appearance = saved;
    render();
  })
  .catch((error) => console.error("failed to load theme", error));

void invoke<ProviderId[]>("get_hidden")
  .then((saved) => {
    hidden.clear();
    for (const id of saved) hidden.add(id);
    if (tab !== "overview" && hidden.has(tab)) tab = "overview";
    render();
  })
  .catch((error) => console.error("failed to load hidden harnesses", error));

void listen<UsageSnapshot>("usage-updated", (event) => {
  latest = event.payload;
  render();
}).catch((error) => {
  console.error("failed to subscribe to usage-updated", error);
});

void pull();
