# AGENTS

- Screenshot a running window (native px, HiDPI-correct via PrintWindow): `pwsh -File scripts/capture-window.ps1 -Out shot.png [-Process waml-editor]`
- Use ASD-STE100 Simplified Technical English.
- ALWAYS pass `-Title <slug>` when you launch the editor (`run.ps1`), with no
  exception — several editors run at once and an unlabelled window cannot be
  told apart. The slug is short kebab-case that names the work, not the fixture:
  `-Title edge-routing-fix`, not `-Title mini`. Add `-Color <hex>` when you open
  more than one window in the same task.

@RTK.md
