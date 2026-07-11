export type RightPanelId = "inspect" | "share";

export function createRightPanel() {
  let active = $state<RightPanelId | null>(null);
  return {
    get active() { return active; },
    open(id: RightPanelId) { active = id; },
    close() { active = null; },
  };
}
