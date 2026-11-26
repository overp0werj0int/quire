#!/usr/bin/env python3
"""Reads the design system back after it was worked on, and writes the implementer's brief.

    python3 design/system/handoff.py EXPORT [-o HANDOFF.md]

EXPORT is what came back from Claude Design: an .html file, a .zip, or a folder. An edited
issues.json works too. It is compared with this folder's tokens.css and issues.json, and the
brief lists:

  - tokens whose value changed, with the file and line each lives at in the app
  - tokens that are new, or gone
  - the decision on each inconsistency, with the places to change
  - components that are new (no baseline data-component) or marked data-new

Nothing in the repository is changed. Without -o the brief is printed.
"""
import argparse, html, json, re, sys, zipfile
from pathlib import Path

HERE = Path(__file__).resolve().parent

def read_export(path: Path) -> str:
    """Every text file of the export, joined."""
    if path.is_dir():
        files = [p for p in sorted(path.rglob("*")) if p.suffix in (".html", ".css", ".json", ".md") and p.is_file()]
        return "\n".join(p.read_text(errors="replace") for p in files)
    if path.suffix == ".zip":
        with zipfile.ZipFile(path) as z:
            return "\n".join(z.read(n).decode(errors="replace") for n in z.namelist() if n.endswith((".html", ".css", ".json", ".md")))
    return path.read_text(errors="replace")

def tokens(css: str) -> dict:
    """{(selector, name): value} for every --q-* declaration, in CSS blocks or inline styles."""
    out = {}
    for sel, body in re.findall(r"([^{}]+)\{([^{}]*)\}", re.sub(r"/\*.*?\*/", "", css, flags=re.S)):
        for name, value in re.findall(r"(--q-[\w-]+)\s*:\s*([^;]+);", body):
            out[(" ".join(re.split(r"<[^>]*>", sel)[-1].split()), name)] = " ".join(value.split())
    return out

def decisions(text: str) -> dict:
    """{issue id: decision} from data-decision attributes, 'Decision:' lines or an issues.json."""
    out = {}
    for tag in re.findall(r"<[^>]*data-issue=[^>]*>", text):
        i, d = re.search(r'data-issue="([^"]+)"', tag), re.search(r'data-decision="([^"]*)"', tag)
        if i and d and d[1].strip():
            out[i[1]] = html.unescape(d[1]).strip()
    for i, body in re.findall(r'data-issue="([^"]+)".*?class="decision-text"[^>]*>(.*?)</span>', text, flags=re.S):
        d = html.unescape(re.sub(r"<[^>]+>", "", body)).strip()
        if d and d != "open":
            out.setdefault(i, d)
    for i, d in re.findall(r'"id":\s*"([A-Z]+-\d+)"[^{}]*?"decision":\s*"((?:[^"\\]|\\.)+)"', text, flags=re.S):
        out.setdefault(i, json.loads(f'"{d}"'))
    for i, d in re.findall(r"^###\s+([A-Z]+-\d+)\b.*?^Decision:\s*(.+?)$", text, flags=re.S | re.M):
        if d.strip() != "open":
            out.setdefault(i, d.strip())
    return out

def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("export", type=Path)
    ap.add_argument("-o", "--out", type=Path)
    a = ap.parse_args()
    if not a.export.exists():
        sys.exit(f"no such file or folder: {a.export}")
    text = read_export(a.export)
    base_css = (HERE / "tokens.css").read_text()
    issues = {i["id"]: i for i in json.loads((HERE / "issues.json").read_text())["issues"]}
    scale = json.loads((HERE / "tokens.json").read_text())
    src = {t["name"]: f'{t["src"]} ({t["note"]})' for g, v in scale.items() if isinstance(v, list) for t in v if isinstance(t, dict) and "src" in t}

    old, new = tokens(base_css), tokens(text)
    lines = ["# Design handoff", "", f"Read from `{a.export}` against `design/system`. Paths are under `desktop/src` unless they say otherwise.", ""]

    if not new:
        lines += ["## Tokens", "", "No `--q-*` variables were found in the export, so token changes could not be read. Compare the design by eye, or export it as HTML.", ""]
    else:
        changed = [(k, old[k], new[k]) for k in old if k in new and old[k] != new[k]]
        added = [k for k in new if k not in old and not any(k[1] == o[1] and new[k] == old[o] for o in old)]
        gone = sorted({k[1] for k in old} - {k[1] for k in new})
        lines += ["## Tokens changed", ""]
        lines += [f"- `{n}` in `{s}`: `{o}` → `{v}`" + (f". Lives at {src[n]}." if n in src else ". A theme colour: `theme.rs`, `pal()` for Paper and Ink, `THEMES` for the rest.") for (s, n), o, v in changed] or ["None."]
        lines += ["", "## Tokens added", ""] + ([f"- `{n}` in `{s}`: `{new[(s, n)]}`" for s, n in added] or ["None."])
        if gone:
            lines += ["", "## Tokens no longer in the design", ""] + [f"- `{n}`" + (f", was at {src[n]}" if n in src else "") for n in gone]
        lines.append("")

    got = decisions(text)
    lines += ["## Decisions", ""]
    for i, d in got.items():
        it = issues.get(i)
        if not it:
            lines += [f"### {i} (not in issues.json)", "", d, ""]
            continue
        lines += [f"### {i} · {it['title']}", "", f"**Decision:** {d}", "", f"Proposal was: {it['propose']}", "", "Where:"]
        lines += [f"- {v['l']}: {v['v']}" + (f" — `{v['at']}`" if v["at"] else "") for v in it["variants"]]
        if it["status"] == "reported":
            lines.append("- This finding was not re-read against the code. Read the lines first.")
        lines.append("")
    if not got:
        lines += ["No decisions were found. Each one goes in its card's `data-decision` attribute, its \"Decision\" line, or the `decision` field of `issues.json`.", ""]
    undecided = [i for i in issues if i not in got]
    lines += [f"Still open: {len(undecided)} of {len(issues)}" + (": " + ", ".join(undecided) if undecided and got else "") + ".", ""]

    base_html = (HERE / "quire-design-system.html").read_text()
    known = set(re.findall(r'data-component="([^"]+)"', base_html))
    seen = re.findall(r'data-component="([^"]+)"', text)
    fresh = sorted({c for c in seen if c not in known})
    marked = [re.sub(r"\s+", " ", t)[:200] for t in re.findall(r"<[^>]*\bdata-new\b[^>]*>", text)]
    lines += ["## New in the design", ""]
    lines += [f"- component `{c}`" for c in fresh] + [f"- marked new: `{t}`" for t in marked] or ["Nothing marked. New screens or components that carry no `data-component` or `data-new` are not listed here: look at the design itself."]
    missing = sorted(known - set(seen)) if seen else []
    if missing:
        lines += ["", "## Components no longer in the design", ""] + [f"- `{c}`" for c in missing]
    lines += ["", "## For the implementer", "",
              "Read `ARCHITECTURE.md` and its \"Rules for changes\" first. Change a token where it lives and nowhere else; where a value is a bare number at several call sites (see SP-07 in `ISSUES.md`), give it a name as part of the change.",
              "Check each change in the running app with `desktop/tests/probe.py`, and add a case under `desktop/tests/cases/` for anything a user can do.",
              "When a decision changes what a spec file under `docs/reader-writer/behavior/` says, update that file and run `python3 docs/reader-writer/tools/check-specs.py`.",
              "After the code changes, bring `scales.json` and `issues.json` up to date and run `python3 design/system/build.py`.", ""]
    out = "\n".join(lines)
    if a.out:
        a.out.write_text(out)
        print(f"wrote {a.out}")
    else:
        print(out)

if __name__ == "__main__":
    main()
