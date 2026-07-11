// Share moved to a top-bar button + modal dialog, so the rail now hosts only
// Inspect. (The rail itself is removed by a later spec.)
export type RightPanelId = "inspect";

export function createRightPanel() {
  let active = $state<RightPanelId | null>(null);
  return {
    get active() { return active; },
    open(id: RightPanelId) { active = id; },
    close() { active = null; },
  };
}
