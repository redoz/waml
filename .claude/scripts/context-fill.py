#!/usr/bin/env python3
"""
context-fill.py — report a Claude Code (sub)agent's current context-window fill,
read MECHANICALLY from its on-disk transcript (.jsonl).

Why this exists: an agent has no in-conversation API to read its own context. But each
(sub)agent's transcript is a JSONL file whose latest `type:"assistant"` entry carries a
`message.usage` object. The LIVE context size is the SUM of:

    input_tokens + cache_read_input_tokens + cache_creation_input_tokens

Raw `input_tokens` alone is ~nothing (often single digits) — almost all of the live
context lives in the cache fields, so you MUST sum all three or you under-report ~1000x.
The number is a one-turn-stale lower bound (it reflects the last COMPLETED assistant
turn, not the in-flight one) — fine for a between-task rotation check; leave headroom.

Interim spike tooling for the implement-orchestrator; this logic is destined to move
into `cargo dev` (the orchestrator subcommand). Single Python file (not the repo's
.sh/.ps1 pair) so the JSONL parsing lives in exactly one place and is callable
identically from bash or pwsh.

Usage:
  # fast path — discover the transcript once, then reuse it each check:
  python context-fill.py --transcript PATH [--window N | --model NAME] [--threshold PCT] [--pct]

  # discover own transcript by a unique marker the caller echoed earlier:
  python context-fill.py --marker STR [--projects-dir DIR] [--window N | --model NAME] [--threshold PCT]

Output: a JSON object on stdout (or just the number with --pct).
Exit codes: 0 = under threshold (keep going) | 10 = at/over threshold (ROTATE)
            2 = bad args / transcript not found | 3 = no usage entry found
"""
import argparse
import json
import os
import sys

# model-name substring -> context window (tokens)
WINDOWS = {
    "haiku": 200_000,
    "sonnet": 1_000_000,
    "opus": 1_000_000,
    "fable": 1_000_000,
}
DEFAULT_WINDOW = 1_000_000

# --- Cost estimation (the `--cost` / `--cost-footer` modes) -------------------
# Base per-MTok USD list price, keyed by model-name substring. HARD-CODED — update
# when Anthropic changes list prices (same maintenance burden as WINDOWS above).
# Cache + output are derived from the base INPUT price by fixed multipliers, so this
# table only carries (input, output). First substring match wins.
PRICES = {
    "fable": (10.0, 50.0),
    "haiku": (1.0, 5.0),
    "sonnet": (3.0, 15.0),
    "opus": (5.0, 25.0),
}
DEFAULT_PRICE = (5.0, 25.0)  # opus-tier — the session default when a turn has no model
# Cache billing multipliers vs the base input price (docs/superpowers prompt-caching):
# a 5-minute ephemeral write is 1.25x, a 1-hour write 2x, a read 0.1x.
CACHE_WRITE_5M_MULT = 1.25
CACHE_WRITE_1H_MULT = 2.0
CACHE_READ_MULT = 0.1


def window_for(model):
    if not model:
        return DEFAULT_WINDOW
    m = model.lower()
    for key, val in WINDOWS.items():
        if key in m:
            return val
    return DEFAULT_WINDOW


def prices_for(model):
    """(input_per_mtok, output_per_mtok) for `model`, by substring; opus-tier default."""
    m = (model or "").lower()
    for key, val in PRICES.items():
        if key in m:
            return val
    return DEFAULT_PRICE


def sum_cost(path):
    """Walk EVERY assistant turn's usage and price each at ITS OWN model.

    Unlike `latest_usage` (a one-turn snapshot for the rotation check), this is the
    cumulative billed cost of the whole (sub)agent: each turn re-reads its context, so
    summing per-turn usage IS the spend. The model is read from the same `message`
    object as `usage`, so a stage is priced correctly even on a mixed-model transcript.
    Returns a dict, or None if the transcript can't be read.
    """
    tot = {"input": 0, "cache_read": 0, "cache_5m": 0, "cache_1h": 0, "output": 0}
    cost = 0.0
    models = {}
    turns = 0
    try:
        with open(path, encoding="utf-8") as fh:
            for line in fh:
                line = line.strip()
                if not line:
                    continue
                try:
                    obj = json.loads(line)
                except ValueError:
                    continue
                msg = obj.get("message") if isinstance(obj.get("message"), dict) else {}
                usage = msg.get("usage") or obj.get("usage")
                if not isinstance(usage, dict):
                    continue
                model = msg.get("model") or obj.get("model") or ""
                inp = int(usage.get("input_tokens", 0) or 0)
                read = int(usage.get("cache_read_input_tokens", 0) or 0)
                out = int(usage.get("output_tokens", 0) or 0)
                cc = usage.get("cache_creation")
                if isinstance(cc, dict):
                    c5 = int(cc.get("ephemeral_5m_input_tokens", 0) or 0)
                    c1 = int(cc.get("ephemeral_1h_input_tokens", 0) or 0)
                else:
                    # older transcripts: only the flat total, assume the 5m tier
                    c5 = int(usage.get("cache_creation_input_tokens", 0) or 0)
                    c1 = 0
                in_price, out_price = prices_for(model)
                cost += (
                    inp * in_price
                    + c5 * in_price * CACHE_WRITE_5M_MULT
                    + c1 * in_price * CACHE_WRITE_1H_MULT
                    + read * in_price * CACHE_READ_MULT
                    + out * out_price
                ) / 1_000_000.0
                tot["input"] += inp
                tot["cache_read"] += read
                tot["cache_5m"] += c5
                tot["cache_1h"] += c1
                tot["output"] += out
                if model:
                    models[model] = models.get(model, 0) + 1
                turns += 1
    except OSError:
        return None
    return {
        "cost_usd": round(cost, 4),
        "turns": turns,
        "models": models,
        "input_tokens": tot["input"],
        "cache_read_tokens": tot["cache_read"],
        "cache_write_tokens": tot["cache_5m"] + tot["cache_1h"],
        "output_tokens": tot["output"],
    }


def _humanize(n):
    """Compact token count: 1900000 -> '1.9M', 38000 -> '38k', 412 -> '412'."""
    n = int(n or 0)
    if n >= 1_000_000:
        return f"{n / 1_000_000:.1f}M".replace(".0M", "M")
    if n >= 1_000:
        return f"{n / 1_000:.0f}k"
    return str(n)


def cost_footer(c):
    """The ready-to-append Markdown `<sub>` footer line from a sum_cost() dict."""
    model = max(c["models"], key=c["models"].get) if c["models"] else "?"
    short = model.replace("claude-", "")
    return (
        f"<sub>⛽ this stage ≈ ${c['cost_usd']:.2f} · "
        f"{_humanize(c['output_tokens'])} out · "
        f"{_humanize(c['cache_read_tokens'])} cache-read · "
        f"{_humanize(c['cache_write_tokens'])} write · "
        f"{short} · self-metered</sub>"
    )


def default_projects_dir():
    base = os.environ.get("CLAUDE_CONFIG_DIR") or os.path.join(os.path.expanduser("~"), ".claude")
    return os.path.join(base, "projects")


def find_by_marker(marker, projects_dir):
    """The agent-*.jsonl containing `marker`, searched NEWEST-FIRST.

    The marker is unique to the caller, so the first match IS the caller's transcript;
    scanning by most-recent mtime reaches the actively-written file almost immediately
    instead of reading every (possibly hundreds of) old transcript. Filtering to the
    `agent-` prefix excludes the parent session transcript (which also mirrors the marker).
    """
    needle = marker.encode("utf-8", "ignore")
    candidates = []
    for root, _dirs, files in os.walk(projects_dir):
        for name in files:
            if name.startswith("agent-") and name.endswith(".jsonl"):
                path = os.path.join(root, name)
                try:
                    candidates.append((os.path.getmtime(path), path))
                except OSError:
                    continue
    for _mtime, path in sorted(candidates, reverse=True):
        try:
            with open(path, "rb") as fh:
                if needle in fh.read():
                    return path
        except OSError:
            continue
    return None


def latest_usage(path):
    """The last `usage` dict in the transcript, or None."""
    last = None
    try:
        with open(path, encoding="utf-8") as fh:
            for line in fh:
                line = line.strip()
                if not line:
                    continue
                try:
                    obj = json.loads(line)
                except ValueError:
                    continue
                msg = obj.get("message") if isinstance(obj.get("message"), dict) else {}
                usage = msg.get("usage") or obj.get("usage")
                if isinstance(usage, dict):
                    last = usage
    except OSError:
        return None
    return last


def main():
    # The cost footer carries a non-ASCII glyph (⛽) bound for GitHub Markdown; force UTF-8
    # so it survives a Windows cp1252 console (this script runs from both bash and pwsh).
    try:
        sys.stdout.reconfigure(encoding="utf-8")
    except Exception:
        pass
    ap = argparse.ArgumentParser(description="Report a (sub)agent's context-window fill from its transcript.")
    grp = ap.add_mutually_exclusive_group(required=True)
    grp.add_argument("--transcript", help="explicit path to the agent-*.jsonl transcript")
    grp.add_argument("--marker", help="unique string the caller echoed; finds the agent-*.jsonl containing it")
    ap.add_argument("--projects-dir", default=default_projects_dir(),
                    help="root to search for --marker (default: ~/.claude/projects)")
    ap.add_argument("--window", type=int, help="context window in tokens (overrides --model)")
    ap.add_argument("--model", help="model name; maps to a window (haiku=200k, sonnet/opus/fable=1M)")
    ap.add_argument("--threshold", type=float, help="fill%% at/above which to ROTATE (sets exit code 10)")
    ap.add_argument("--max-tokens", type=int,
                    help="absolute context_tokens at/above which to ROTATE (e.g. 100000). A quality-zone cap; "
                         "more robust than --threshold%% since the 'still-sharp' zone is ~absolute, not a window fraction")
    ap.add_argument("--pct", action="store_true", help="print only the fill percentage")
    ap.add_argument("--cost", action="store_true",
                    help="print the cumulative cost estimate as JSON (per-turn, model-aware) instead of context fill")
    ap.add_argument("--cost-footer", action="store_true",
                    help="print the ready-to-append Markdown <sub> cost footer line")
    args = ap.parse_args()

    path = args.transcript
    if args.marker:
        path = find_by_marker(args.marker, args.projects_dir)
        if not path:
            print(json.dumps({"error": "no agent-*.jsonl found containing marker", "marker": args.marker}),
                  file=sys.stderr)
            return 2
    if not path or not os.path.exists(path):
        print(json.dumps({"error": "transcript not found", "path": path}), file=sys.stderr)
        return 2

    # Cost modes short-circuit before the rotation snapshot — they sum the WHOLE transcript.
    if args.cost or args.cost_footer:
        c = sum_cost(path)
        if not c or c["turns"] == 0:
            print(json.dumps({"error": "no usage entries in transcript", "path": path}), file=sys.stderr)
            return 3
        print(cost_footer(c) if args.cost_footer else json.dumps(c, indent=2))
        return 0

    usage = latest_usage(path)
    if not usage:
        print(json.dumps({"error": "no usage entry in transcript", "path": path}), file=sys.stderr)
        return 3

    inp = int(usage.get("input_tokens", 0) or 0)
    cache_read = int(usage.get("cache_read_input_tokens", 0) or 0)
    cache_creation = int(usage.get("cache_creation_input_tokens", 0) or 0)
    ctx = inp + cache_read + cache_creation
    window = args.window if args.window else window_for(args.model)
    pct = round(100.0 * ctx / window, 1) if window else 0.0
    over_pct = args.threshold is not None and pct >= args.threshold
    over_abs = args.max_tokens is not None and ctx >= args.max_tokens
    rotate = over_pct or over_abs

    out = {
        "transcript": path,
        "input_tokens": inp,
        "cache_read_input_tokens": cache_read,
        "cache_creation_input_tokens": cache_creation,
        "output_tokens": int(usage.get("output_tokens", 0) or 0),
        "context_tokens": ctx,
        "window": window,
        "pct": pct,
    }
    if args.threshold is not None:
        out["threshold_pct"] = args.threshold
    if args.max_tokens is not None:
        out["max_tokens"] = args.max_tokens
    if args.threshold is not None or args.max_tokens is not None:
        out["should_rotate"] = rotate

    print(pct if args.pct else json.dumps(out, indent=2))
    return 10 if rotate else 0


if __name__ == "__main__":
    sys.exit(main())
