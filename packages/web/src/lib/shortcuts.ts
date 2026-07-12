export type ShortcutId =
  | "tool.select"
  | "tool.add"
  | "tool.connect"
  | "selection.delete"
  | "hints.toggle";

export interface Shortcut {
  id: ShortcutId;
  /** KeyboardEvent.key values that trigger this action. */
  event: string[];
  /** Glyphs shown in the hint badge. */
  display: string[];
  /** Human label (for tooltips / aria). */
  label: string;
}

export const SHORTCUTS: readonly Shortcut[] = [
  { id: "tool.select", event: ["v"], display: ["V"], label: "Select & move" },
  { id: "tool.add", event: ["n"], display: ["N"], label: "Add object" },
  { id: "tool.connect", event: ["c"], display: ["C"], label: "Connect" },
  { id: "selection.delete", event: ["Delete", "Backspace"], display: ["⌫"], label: "Delete selection" },
  { id: "hints.toggle", event: ["?"], display: ["?"], label: "Toggle keyboard shortcuts" },
];

const byId = new Map<ShortcutId, Shortcut>(SHORTCUTS.map((s) => [s.id, s]));

export function shortcut(id: ShortcutId): Shortcut {
  const s = byId.get(id);
  if (!s) throw new Error(`unknown shortcut ${id}`);
  return s;
}

export function keyLabel(id: ShortcutId): string[] {
  return shortcut(id).display;
}

export function matchesShortcut(id: ShortcutId, e: KeyboardEvent): boolean {
  return shortcut(id).event.includes(e.key);
}
