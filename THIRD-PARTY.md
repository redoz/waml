# Third-party notices

WAML is licensed under the MPL-2.0 (see [LICENSE](LICENSE)). Its binaries, the
published web editor, and every site produced by `waml export site` also embed
compiled code from the projects below. Their licences require that this notice
accompany those distributions, so it ships inside every exported site as
`THIRD-PARTY.md` alongside the editor.

The authoritative, machine-checked list of dependencies and their licences is
the lockfile plus [`deny.toml`](deny.toml); CI fails on any licence outside the
allow-list there. This file records the notices that must travel with the
distributed artifacts.

## makepad

The GPU UI framework the native and web editors are built on, used via a fork
([redoz/makepad](https://github.com/redoz/makepad), pinned by rev in the
workspace manifest). Upstream: <https://github.com/makepad/makepad>.

Licensed under the MIT License.

> Copyright (c) 2023 Makepad B.V.
>
> Permission is hereby granted, free of charge, to any person obtaining a copy
> of this software and associated documentation files (the "Software"), to deal
> in the Software without restriction, including without limitation the rights
> to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
> copies of the Software, and to permit persons to whom the Software is
> furnished to do so, subject to the following conditions:
>
> The above copyright notice and this permission notice shall be included in all
> copies or substantial portions of the Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
> IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
> FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
> AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
> LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
> OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
> SOFTWARE.

## merman

Mermaid diagram rendering, used by the Markdown reading view.
<https://crates.io/crates/merman>

Licensed under `MIT OR Apache-2.0`, used here under the MIT License, whose terms
are reproduced above.

## Rust crate dependencies

The remaining dependencies are permissively licensed (MIT, Apache-2.0,
BSD-2-Clause, BSD-3-Clause, ISC, Zlib, Unicode-3.0, MIT-0, BSL-1.0, CC0-1.0),
as enforced by `cargo deny check licenses` in CI. To reproduce the full list
with texts:

```bash
cargo install cargo-about && cargo about generate --format json
```

## Fonts

Web font subsets shipped with the editor are pruned from the makepad
distribution by `scripts/prune-web-fonts`; they carry their upstream licences,
which are MIT or SIL OFL.
