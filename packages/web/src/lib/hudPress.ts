// Press-ripple for .hud-btn: on pointerdown set the reveal origin (--ox/--oy)
// from the click point and add `.down` so the ::after frame reveals and the
// glowpulse fires; remove it on release. CSS lives in atlas-components.css.
export function hudPress(node: HTMLElement) {
  function down(e: PointerEvent) {
    const r = node.getBoundingClientRect();
    node.style.setProperty("--ox", `${(((e.clientX - r.left) / r.width) * 100).toFixed(1)}%`);
    node.style.setProperty("--oy", `${(((e.clientY - r.top) / r.height) * 100).toFixed(1)}%`);
    node.classList.remove("down");
    void node.offsetWidth; // reflow to restart the animation
    node.classList.add("down");
  }
  function up() {
    node.classList.remove("down");
  }
  node.addEventListener("pointerdown", down);
  node.addEventListener("pointerup", up);
  node.addEventListener("pointercancel", up);
  return {
    destroy() {
      node.removeEventListener("pointerdown", down);
      node.removeEventListener("pointerup", up);
      node.removeEventListener("pointercancel", up);
    },
  };
}
