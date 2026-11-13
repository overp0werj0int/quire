#!/usr/bin/env python3
"""Checks the behaviour files: every item has a known status and a verify method, IDs are
unique, and adapted items name a decision that exists. With --extract DIR it also checks that
every item ID in omarky's extraction (source/omarky-0.7.0, or --extract DIR) is present."""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ITEM = re.compile(r"^- \*\*([A-Z]+-\d+)\*\*\s*(.*)$")
STATUS = re.compile(r"^\[(in|quire|deferred|dropped|adapted: (\d{4}))\]\s*")
VERIFY = re.compile(r"^\[verify: (unit|state|geometry|golden|script|bench|manual|-)((?:, ?(?:unit|state|geometry|golden|script|bench|manual))*)\]\s*")


def main():
    decisions = {p.name[:4] for p in (ROOT / "decisions").glob("[0-9][0-9][0-9][0-9]-*.md")}
    seen, errors, counts = {}, [], {}
    for path in sorted(p for p in (ROOT / "behavior").glob("*.md") if p.name not in ("README.md", "verification.md")):
        for n, line in enumerate(path.read_text().splitlines(), 1):
            m = ITEM.match(line)
            if not m:
                continue
            ident, rest = m.groups()
            where = f"{path.name}:{n} {ident}"
            if ident in seen:
                errors.append(f"{where}: duplicate of {seen[ident]}")
            seen[ident] = where
            s = STATUS.match(rest)
            if not s:
                errors.append(f"{where}: missing or unknown status tag")
                continue
            status = s.group(1).split(":")[0]
            counts[status] = counts.get(status, 0) + 1
            if s.group(2) and s.group(2) not in decisions:
                errors.append(f"{where}: decision {s.group(2)} does not exist")
            v = VERIFY.match(rest[s.end():])
            if not v:
                errors.append(f"{where}: missing or unknown verify tag")
                continue
            none = v.group(1) == "-"
            if status in ("quire", "deferred", "dropped") and not none:
                errors.append(f"{where}: status {status} takes [verify: -]")
            if status in ("in", "adapted") and none:
                errors.append(f"{where}: status {status} needs a verify method")
            if not rest[s.end() + v.end():].strip():
                errors.append(f"{where}: no behaviour text")
    src = ROOT / "source" / "omarky-0.7.0"
    if "--extract" in sys.argv:
        src = Path(sys.argv[sys.argv.index("--extract") + 1])
    if src.is_dir():
        wanted = set()
        for p in src.glob("0[1-5]-*.md"):
            wanted |= set(re.findall(r"(?m)(?:\*\*|^)([A-Z]+-\d+)(?:\*\*|\.)", p.read_text()))
        deferred = (ROOT / "deferred").glob("*.md")
        present = set(seen) | {i for p in deferred for i in re.findall(r"\b([A-Z]+-\d+)\b", p.read_text())}
        for ident in sorted(wanted - present):
            errors.append(f"extract item {ident} is missing from the behaviour and deferred files")
    for e in errors:
        print(e)
    total = sum(counts.values())
    print(f"{total} items: " + ", ".join(f"{k} {v}" for k, v in sorted(counts.items())))
    sys.exit(1 if errors else 0)


main()
