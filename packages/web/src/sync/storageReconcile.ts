// When the project's storages (re)load, keep the current selection only if it
// still exists in that project; otherwise fall back to the first available (or
// null if none). Prevents pushing to a stale storage after a project switch /
// sign-in, where the previously-selected storage id isn't valid anymore.
export function reconcileStorageId(current: string | null | undefined, list: { id: string }[]): string | null {
  if (current && list.some(s => s.id === current)) return current;
  return list[0]?.id ?? null;
}
