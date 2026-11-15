#!/usr/bin/env python3
"""The parity pass (VER-30): Quire's reader beside omarky's, under the review themes.

    parity.py --omarky SHOTS [--out DIR] [--quire BIN] [--themes a,b] [--samples a,b]

SHOTS is omarky's `reader/tools/screenshot` output (`shots/<theme>/<sample>-1.png`), taken
with `--size wide --pages 1 --opt outline=false`. For each theme and sample this renders Quire
at the same page size, body size 14, the same Omarchy theme and no panels (`--scene parity`),
measures the ink in both, and writes DIR/index.html: the two pages side by side for a person
to sign off, with the measured offsets.

The measure: each band of rows holding ink in the page (a line of text, or a box such as a
code panel) gives its top and its left edge, relative to the page's top-left corner, since each
app draws its own bar above the page. The two lists of bands are aligned by their heights and
left edges, and the table gives the vertical drift (each matched band's offset less the page's
shift as a whole, the offset that lines up the most bands) and the left-edge difference.
"""

import argparse
import html
import os
import shutil
import subprocess
import sys
import tempfile

import numpy as np
from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "../../.."))
FIXTURES = os.path.join(REPO, "desktop/tests/fixtures")
THEMES = "gruvbox,tokyo-night,catppuccin-latte,rose-pine,flexoki-light,matte-black"
SAMPLES = "kitchen-sink,prose,alerts,code,tables,frontmatter,sample"

# Page areas in each app's 1400-wide window: omarky's header is 40 px over a 1000 px window.
# Quire's title bar is 42 px and a hairline, its status bar 28 px and a hairline, so its window
# is 1032 px for the same 960 px page.
OMARKY_PAGE = (40, 1000)
QUIRE_SIZE = (1400, 1032)
QUIRE_PAGE = (43, 1003)
FAR = 30


def render_quire(binary, theme, sample, out):
    """One Quire snapshot of `sample` under Omarchy theme `theme`, in a throwaway home."""
    home = tempfile.mkdtemp(prefix="quire-parity-")
    try:
        cur = os.path.join(home, ".config/omarchy/current/theme")
        os.makedirs(cur)
        shutil.copy(os.path.join(FIXTURES, "omarchy-themes", theme, "colors.toml"), cur)
        env = dict(os.environ, HOME=home, XDG_CONFIG_HOME=os.path.join(home, "cfg"),
                   XDG_DATA_HOME=os.path.join(home, "data"), XDG_STATE_HOME=os.path.join(home, "state"),
                   QUIRE_FOLDER=os.path.join(FIXTURES, "omarky"), QUIRE_NOTE=sample + ".md", RUNE_PERF_WARN="0")
        w, h = QUIRE_SIZE
        r = subprocess.run([binary, "--snap", out, "--size", f"{w}x{h}", "--scale", "1", "--scene", "parity"],
                           env=env, capture_output=True, text=True, timeout=180)
        if r.returncode != 0 or not os.path.exists(out):
            sys.exit(f"quire --snap failed for {theme}/{sample}:\n{r.stderr[-2000:]}")
    finally:
        shutil.rmtree(home, ignore_errors=True)


def bands(path, page):
    """The ink bands of the page area: (top, height, left) in page points."""
    a = np.asarray(Image.open(path).convert("RGB"), dtype=np.int16)
    top, bottom = page
    p = a[top:bottom]
    # The page's own colour: the commonest pixel of a strip down its left margin.
    strip = p[:, 8:40].reshape(-1, 3)
    vals, counts = np.unique(strip, axis=0, return_counts=True)
    bg = vals[counts.argmax()]
    ink = np.abs(p - bg).sum(axis=2) > 40
    rows = ink.any(axis=1)
    out = []
    y = 0
    while y < len(rows):
        if not rows[y]:
            y += 1
            continue
        start = y
        while y < len(rows) and rows[y]:
            y += 1
        xs = np.nonzero(ink[start:y].any(axis=0))[0]
        out.append((start, y - start, int(xs[0])))
    return out


def shift(a, b):
    """The page's shift as a whole: the offset that lines up the most bands of a and b."""
    best = (0, 0)
    for off in {y - x[0] for x in a for y, *_ in b}:
        hits = sum(any(abs(q[0] - o[0] - off) <= 2 and abs(q[1] - o[1]) <= 2 for q in b) for o in a)
        best = max(best, (hits, -abs(off)))
    return -best[1] if best[0] else 0


def align(a, b, off):
    """Pairs of indices into a and b, matched by height and left edge (a sequence alignment).
    Lines of plain text all look alike, so a pair also costs its distance from the page's
    shift `off`: a run of lines pairs up line for line, and a real drift still shows."""
    gap = 24.0
    n, m = len(a), len(b)

    def pair(x, y):
        # A band more than FAR off the page's shift is a different band (Quire's file name
        # against omarky's card border), so it stays unmatched rather than reading as a drift.
        if abs(y[0] - x[0] - off) > FAR:
            return float("inf")
        return abs(x[1] - y[1]) + abs(x[2] - y[2]) / 4 + abs(y[0] - x[0] - off) / 8

    cost = np.zeros((n + 1, m + 1))
    cost[:, 0] = np.arange(n + 1) * gap
    cost[0, :] = np.arange(m + 1) * gap
    for i in range(1, n + 1):
        for j in range(1, m + 1):
            match = pair(a[i - 1], b[j - 1])
            cost[i, j] = min(cost[i - 1, j - 1] + match, cost[i - 1, j] + gap, cost[i, j - 1] + gap)
    pairs = []
    i, j = n, m
    while i > 0 and j > 0:
        match = pair(a[i - 1], b[j - 1])
        if cost[i, j] == cost[i - 1, j - 1] + match:
            pairs.append((i - 1, j - 1))
            i, j = i - 1, j - 1
        elif cost[i, j] == cost[i - 1, j] + gap:
            i -= 1
        else:
            j -= 1
    return pairs[::-1]


def compare(o_path, q_path):
    o, q = bands(o_path, OMARKY_PAGE), bands(q_path, QUIRE_PAGE)
    # The page's shift as a whole, so a line one app draws and the other does not (Quire's
    # file name over the page) moves no other row.
    base = shift(o, q)
    pairs = align(o, q, base)
    dys = [q[j][0] - o[i][0] for i, j in pairs]
    rows = [(o[i], q[j], dy - base, q[j][2] - o[i][2]) for (i, j), dy in zip(pairs, dys)]
    drift = [abs(r[2]) for r in rows]
    left = [abs(r[3]) for r in rows]
    return {
        "omarky": len(o), "quire": len(q), "matched": len(pairs), "offset": base,
        "drift_max": max(drift, default=0), "drift_median": float(np.median(drift)) if drift else 0.0,
        "left_max": max(left, default=0), "rows": rows,
    }


def page(results, out):
    css = """
body{margin:0;padding:24px 16px;font:14px/1.45 system-ui,sans-serif;background:#f6f5f2;color:#222}
h1{font-size:20px;margin:0 0 4px}h2{font-size:17px;margin:36px 0 8px}h3{font-size:14px;margin:18px 0 6px}
p.note{color:#555;max-width:70ch}
.pair{display:grid;grid-template-columns:1fr 1fr;gap:12px;max-width:1500px}
.pair figure{margin:0}.pair img{width:100%;border:1px solid #ccc;display:block}
figcaption{font-size:12px;color:#666;margin-top:3px}
table{border-collapse:collapse;font:12px ui-monospace,monospace;margin:6px 0 0}
td,th{padding:1px 8px;text-align:right;border-bottom:1px solid #e3e1dc}
.bad{color:#b3261e;font-weight:600}
details{margin:4px 0 0}
table.sum td:first-child,table.sum th:first-child{text-align:left}
@media (prefers-color-scheme:dark){body{background:#1c1c1c;color:#ddd}p.note,figcaption{color:#aaa}td,th{border-color:#333}}
"""
    h = [f"<!doctype html><meta charset=utf-8><title>Parity pass</title><style>{css}</style>",
         "<h1>Parity pass: omarky and Quire</h1>",
         "<p class=note>Each pair is one sample under one Omarchy theme: omarky on the left, Quire on the "
         "right, 1400 px wide, body size 14, no side panels. Each app draws its own bar above the page. "
         "Drift is how far a matched line or box sits from where the page's shift as a whole (the offset "
         "that lines up the most bands, given per pair) puts it (Quire less omarky, px); left is the difference in its left edge. Drift over 2 px or a left "
         "edge over 1 px is marked.</p>"]
    h.append("<table class=sum><tr><th>sample</th><th>theme</th><th>bands o/q</th><th>matched</th>"
             "<th>offset</th><th>drift max</th><th>drift median</th><th>left max</th></tr>")
    for (sample, theme), r in results:
        bad = lambda v, lim: f"<td class=bad>{v}</td>" if v > lim else f"<td>{v}</td>"
        h.append(f"<tr><td><a href='#{sample}-{theme}'>{sample}</a></td><td>{theme}</td>"
                 f"<td>{r['omarky']}/{r['quire']}</td><td>{r['matched']}</td><td>{r['offset']}</td>"
                 f"{bad(r['drift_max'], 2)}<td>{r['drift_median']:.1f}</td>{bad(r['left_max'], 1)}</tr>")
    h.append("</table>")
    last = None
    for (sample, theme), r in results:
        if sample != last:
            h.append(f"<h2>{html.escape(sample)}</h2>")
            last = sample
        h.append(f"<h3 id='{sample}-{theme}'>{theme}: drift max {r['drift_max']} px, left max {r['left_max']} px</h3>")
        h.append(f"<div class=pair><figure><img loading=lazy src='omarky/{theme}/{sample}.png'><figcaption>omarky</figcaption></figure>"
                 f"<figure><img loading=lazy src='quire/{theme}/{sample}.png'><figcaption>Quire</figcaption></figure></div>")
        h.append("<details><summary>Matched bands</summary><table><tr><th>omarky top</th><th>h</th><th>left</th>"
                 "<th>Quire top</th><th>h</th><th>left</th><th>drift</th><th>left Δ</th></tr>")
        for o, q, dy, dx in r["rows"]:
            cls = lambda v, lim: " class=bad" if abs(v) > lim else ""
            h.append(f"<tr><td>{o[0]}</td><td>{o[1]}</td><td>{o[2]}</td><td>{q[0]}</td><td>{q[1]}</td><td>{q[2]}</td>"
                     f"<td{cls(dy, 2)}>{dy}</td><td{cls(dx, 1)}>{dx}</td></tr>")
        h.append("</table></details>")
    with open(out, "w") as f:
        f.write("\n".join(h))


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--omarky", required=True, help="omarky's shots folder (shots/<theme>/<sample>-1.png)")
    ap.add_argument("--out", default=os.path.join(REPO, "desktop/target/parity"))
    ap.add_argument("--quire", default=os.path.join(REPO, "desktop/target/debug/quire"))
    ap.add_argument("--themes", default=THEMES)
    ap.add_argument("--samples", default=SAMPLES)
    a = ap.parse_args()
    results = []
    for sample in a.samples.split(","):
        for theme in a.themes.split(","):
            src = os.path.join(a.omarky, theme, f"{sample}-1.png")
            if not os.path.exists(src):
                sys.exit(f"no omarky shot {src}")
            o = os.path.join(a.out, "omarky", theme, f"{sample}.png")
            q = os.path.join(a.out, "quire", theme, f"{sample}.png")
            os.makedirs(os.path.dirname(o), exist_ok=True)
            os.makedirs(os.path.dirname(q), exist_ok=True)
            shutil.copy(src, o)
            render_quire(a.quire, theme, sample, q)
            r = compare(o, q)
            results.append(((sample, theme), r))
            print(f"{sample:13} {theme:17} bands {r['omarky']:3}/{r['quire']:3} matched {r['matched']:3} offset {r['offset']:4} "
                  f"drift max {r['drift_max']:4} median {r['drift_median']:4.1f} left max {r['left_max']:3}")
    page(results, os.path.join(a.out, "index.html"))
    print(os.path.join(a.out, "index.html"))


if __name__ == "__main__":
    main()
