#!/usr/bin/env python3
"""Update a row in the audit remediation ledger, and refresh the progress line.

    python scripts/audit-ledger.py A07 done "Fixed in <sha>: distinct wire tags."
    python scripts/audit-ledger.py --refresh
"""
import re
import sys
from pathlib import Path

LEDGER = Path(__file__).resolve().parent.parent / "docs/reviews/2026-08-21-audit-remediation.md"
STATUSES = ("open", "wip", "done", "decision", "wontfix")


def refresh(text: str) -> str:
    # Count only the findings table — the status-vocabulary table above it uses
    # the same `status` spelling and would otherwise inflate every total.
    findings = text.split("## Findings", 1)[1]
    counts = {s: len(re.findall(rf"\| `{s}` \|", findings)) for s in STATUSES}
    total = sum(counts.values())
    closed = counts["done"] + counts["wontfix"]
    bar = "".join("█" if i < round(closed / total * 24) else "░" for i in range(24))
    summary = (
        f"`{bar}` **{closed}/{total} closed** — "
        + ", ".join(f"{counts[s]} {s}" for s in STATUSES if counts[s])
    )
    return re.sub(
        r"(<!-- progress -->\n).*?(?=\n## Findings)",
        lambda m: m.group(1) + "\n" + summary + "\n",
        text,
        flags=re.S,
    )


def main() -> int:
    text = LEDGER.read_text(encoding="utf-8")
    if sys.argv[1:2] != ["--refresh"]:
        row_id, status, note = sys.argv[1], sys.argv[2], sys.argv[3]
        if status not in STATUSES:
            raise SystemExit(f"status must be one of {STATUSES}")
        pattern = rf"(\| {re.escape(row_id)} \| [CHML] \| [^|]+ \| )`\w+`( \| ).*?( \|\n)"
        text, n = re.subn(pattern, rf"\g<1>`{status}`\g<2>{note}\g<3>", text)
        if n != 1:
            raise SystemExit(f"{row_id}: matched {n} rows, expected 1")
    LEDGER.write_text(refresh(text), encoding="utf-8", newline="\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
