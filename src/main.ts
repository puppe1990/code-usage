import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { renderPanel } from "./render";
import type { FavoriteId, UsageSnapshot } from "./types";
import "./styles.css";

function requireRoot(): HTMLDivElement {
  const element = document.querySelector<HTMLDivElement>("#app");
  if (!element) throw new Error("#app not found");
  return element;
}

const root = requireRoot();
let favorite: FavoriteId | null = null;
let latest: UsageSnapshot | null = null;

function render(): void {
  if (latest) {
    renderPanel(root, latest, favorite);
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

document.addEventListener("click", (event) => {
  const target = event.target as HTMLElement;
  const favorite = target.dataset.favorite as FavoriteId | undefined;

  if (favorite) {
    void toggleFavorite(favorite);
    return;
  }
  if (target.id === "refresh") void invoke("refresh_now");
  if (target.id === "close") void invoke("hide_panel");
  if (target.id === "quit") void invoke("quit_app");
});

document.addEventListener("keydown", (event) => {
  if (event.key === "Escape") void invoke("hide_panel");
});

root.innerHTML = '<div class="loading">carregando usage…</div>';

void invoke<FavoriteId | null>("get_favorite")
  .then((saved) => {
    favorite = saved;
    render();
  })
  .catch((error) => console.error("failed to load favorite", error));

void listen<UsageSnapshot>("usage-updated", (event) => {
  latest = event.payload;
  render();
}).catch((error) => {
  console.error("failed to subscribe to usage-updated", error);
});

void pull();
