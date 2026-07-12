const KEY = "uaml:show-shortcuts";

function load(): boolean {
  try {
    return globalThis.localStorage?.getItem(KEY) === "1";
  } catch {
    return false;
  }
}

function save(v: boolean): void {
  try {
    globalThis.localStorage?.setItem(KEY, v ? "1" : "0");
  } catch {
    // ignore (private mode / unavailable)
  }
}

let show = $state(load());

export const hints = {
  get show(): boolean {
    return show;
  },
  set show(v: boolean) {
    show = v;
    save(v);
  },
  toggle(): void {
    this.show = !show;
  },
};
