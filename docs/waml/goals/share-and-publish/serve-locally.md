# Serve Locally

**Goal:** One command serves the editor on the loopback interface against a
local directory. A reader opens a bundle in a browser with no build step and
with no published site.

**Why:** This command removes the difference between the desktop form and the
published form. It uses the same web artifact and the same views, but it reads
and writes the files of the author.

**Done when:** The command serves the embedded web editor on the loopback
interface. The command gives an operations interface for a selected directory.
An edit in the browser writes to disk through that interface. The command binds
to the loopback interface by default; a wider bind is opt-in and warns.

**Status:** implemented
**MVP:** no

## Notes

- The web form now writes to disk through this command. `waml serve <dir>`
  loads the directory, mints a random token, prints one URL —
  `http://127.0.0.1:<port>/?api=/api&token=<token>` — and opens it in the
  default browser unless `--no-open` is passed. The editor boots from
  `GET /api/bundle` and saves through `POST /api/documents`.
- The served editor and the published editor are the same embedded artifact;
  `serve` mounts it unless `--api-only` is passed, in which case only the API
  runs.
- The command binds to `127.0.0.1` by default. `--bind-all` binds `0.0.0.0`
  instead — it prints a warning naming the exposure, and the token is still
  required for every `/api` request. `--port` picks the port (`0` for an
  ephemeral one, printed once bound).
- Writes go through two guarded routes: `POST /api/ops` (semantic edits,
  validated as one batch) and `POST /api/documents` (baseline-guarded raw
  text, the route the editor's save uses). Both are all-or-nothing — a
  candidate that fails revalidation writes nothing — and both require the
  token, the `X-Waml-Client: 1` header, and (when present) a matching
  `Origin`.
