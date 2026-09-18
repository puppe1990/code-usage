import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { renderPanel } from "./render";
import type { UsageSnapshot } from "./types";
import "./styles.css";

function requireRoot(): HTMLDivElement {
  const element = document.querySelector<HTMLDivElement>("#app");
  if (!element) throw new Error("#app not found");
  return element;
}

const root = requireRoot();

async function pull(): Promise<void> {
  try {
    const snapshot = await invoke<UsageSnapshot | null>("get_usage");
    if (snapshot) {
      renderPanel(root, snapshot);
      return;
    }
    root.innerHTML = '<div class="loading">calculando usage…</div>';
  } catch (error) {
    root.innerHTML = `<div class="loading">falha ao ler usage: ${String(error)}</div>`;
  }
}

document.addEventListener("click", (event) => {
  const target = event.target as HTMLElement;
  if (target.id === "refresh") void invoke("refresh_now");
  if (target.id === "close") void invoke("hide_panel");
  if (target.id === "quit") void invoke("quit_app");
});

document.addEventListener("keydown", (event) => {
  if (event.key === "Escape") void invoke("hide_panel");
});

root.innerHTML = '<div class="loading">carregando usage…</div>';

void listen<UsageSnapshot>("usage-updated", (event) => renderPanel(root, event.payload)).catch(
  (error) => {
    console.error("failed to subscribe to usage-updated", error);
  },
);

void pull();
