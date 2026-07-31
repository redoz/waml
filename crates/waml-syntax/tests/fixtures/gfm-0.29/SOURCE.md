# Source

- Specification revision tag: 0.29.0.gfm.13
- URL: https://raw.githubusercontent.com/github/cmark-gfm/0.29.0.gfm.13/test/spec.txt
- Imported (UTC): 2026-07-31
- SHA-256: `7D8E5814BEFEC287AC116786D81FF14E0ADC9B13295B4494649E995408FD871C`
- Import command:

```powershell
rtk proxy pwsh -NoProfile -Command "Invoke-WebRequest https://raw.githubusercontent.com/github/cmark-gfm/0.29.0.gfm.13/test/spec.txt -OutFile crates/waml-syntax/tests/fixtures/gfm-0.29/spec.txt; Get-FileHash -Algorithm SHA256 crates/waml-syntax/tests/fixtures/gfm-0.29/spec.txt"
```

The GitHub Flavored Markdown specification is licensed under CC-BY-SA-4.0. `LICENSE` contains the license text.
