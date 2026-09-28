export const PANEL_WIDTH = 360;
export const PANEL_MAX_HEIGHT = 820;
export const PANEL_MARGIN = 12;

export function panelWindowHeight(contentHeight: number): number {
  return Math.min(PANEL_MAX_HEIGHT, Math.max(Math.round(contentHeight) + PANEL_MARGIN, 160));
}
