#!/usr/bin/env python3
"""Replay input in a headless Quire and print what you asked about: where focus is, where an
element sits, how far the page has scrolled, what is on disk. No window opens.

    tests/probe.py EVENTS [--print WHAT[,WHAT...]] [--at T] [--series T0:T1:STEP]
                   [--size WxH] [--scale N] [--theme light|dark] [--note FILE.md]
                   [--snap OUT.png] [--fast | --bin PATH]

Run it from `desktop/`. It drives the binary `./dev app` (or `cargo build`) makes,
target/debug/quire; `--fast` picks the one `./dev build` makes, target/fast/quire.

EVENTS is a JSON file of script events, or `-` for stdin, or a short form:

    tests/probe.py "tap:editor@0.1 key:End+cmd@0.4 type:hello_there@0.8 key:s+cmd@1.6"

    tap:KEY@T         a click on the element keyed KEY          tap:700,300@T   a click at a point
    hover:KEY@T       move the pointer over KEY                 hover:700,300@T move to a point
    key:NAME[+cmd][+shift][+alt]@T                               type:TEXT@T     (use _ for a space)
    scroll:DY,X,Y@T   a wheel turn (positive DY moves towards the note's top)
    drag:FROM>DX,DY[,DUR][+MOD]@T   a press at FROM (a KEY or X,Y), moved by DX,DY over DUR
                      seconds (default 0.3), then released; MOD is middle or a key modifier
    wheel:DX,DY[,X,Y][+MOD]@T       one raw wheel delta, at X,Y or where the pointer is
    pinch:DELTA[,X,Y]@T             a touchpad pinch (0.1 zooms in by about a tenth)

T is seconds on the app's own clock. `cmd` is Ctrl on Linux and Windows. Typed text arrives at
about 30 letters a second, so leave it time before the next event.

WHAT is any of (default `focus,doc-y`):

    focus             the focused element's key, and whether its ring shows
    rect:KEY          an element's rectangle [x, y, w, h] in points
    text:KEY          an element's text
    doc-y             the page's top edge, which tells you how far it has scrolled
    label:KEY         an element's accessible label (a map node's reads its text and state)
    scale:KEY         how much an element is drawn scaled (a map node's is the map's zoom)
    selected          the map's selected node, by key
    nodes             the map nodes built now, by key, in the order drawn: what culling kept
    keys              every keyed element with its rectangle: the way to find a KEY
    notes             the first 60 characters of each note on disk after the replay

--at takes the reading at that time; without it, 0.8 s after the last event.

--series samples the same replay at each time from T0 to T1 and prints one line per sample
with the change in doc-y since the one before. That shows a jump, a glide or a bounce, which
one reading cannot. Each sample is a fresh process: a 20-second script sampled 40 times takes
a few minutes.

--note opens a folder holding only that note. --folder opens a copy of a whole folder of notes,
and --open names the note in it to open, by its path inside the folder; opening a note opens the
folders above it. Without either the app writes its sample notes.

--state starts the app with that JSON as its state file, for settings: '{"settings":
{"panel_font": "mono"}}'. What it leaves out keeps its default.

Every run gets a fresh, empty home folder in the system's temporary folder, so it never reads
or changes your own notes and settings. Run one probe at a time: each is a whole copy of the
app.
"""
import argparse
import fcntl
import json
import os
import re
import shutil
import subprocess
import sys
import time

import tempfile

DESKTOP = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
LOCK_PATH = "/tmp/quire-run.lock"


def at(where):
    """A short form's place: X,Y in points, or an element's key."""
    if re.fullmatch(r"[0-9.]+,[0-9.]+", where):
        x, y = where.split(",")
        return {"x": float(x), "y": float(y)}
    return {"key": where}


def parse_events(arg):
    if arg == "-":
        return json.load(sys.stdin)
    if os.path.exists(arg):
        d = json.load(open(arg))
        return d["events"] if isinstance(d, dict) else d
    events = []
    for tok in arg.split():
        m = re.fullmatch(r"(tap|hover|key|type|scroll|drag|wheel|pinch):(.*)@([0-9.]+)", tok)
        if not m:
            raise SystemExit("probe: cannot read %r (see --help)" % tok)
        kind, body, t = m.group(1), m.group(2), float(m.group(3))
        if kind in ("tap", "hover"):
            events.append(dict({"t": t, "type": "tap" if kind == "tap" else "move"}, **at(body)))
        elif kind == "key":
            name, *mods = body.split("+")
            events.append(dict({"t": t, "type": "key", "name": name}, **{m: True for m in mods}))
        elif kind == "drag":
            body, *mods = body.split("+")
            src, _, by = body.partition(">")
            dx, dy, dur = (by.split(",") + ["0.3"])[:3]
            ev = {"t": t, "type": "drag", "dx": float(dx), "dy": float(dy), "dur": float(dur)}
            ev.update(at(src))
            for m in mods:
                ev.update({"button": "middle"} if m == "middle" else {m: True})
            events.append(ev)
        elif kind == "wheel":
            body, *mods = body.split("+")
            dx, dy, *xy = body.split(",")
            ev = dict({"t": t, "type": "wheel", "dx": float(dx), "dy": float(dy)}, **{m: True for m in mods})
            if xy:
                ev.update(x=float(xy[0]), y=float(xy[1]))
            events.append(ev)
        elif kind == "pinch":
            delta, *xy = body.split(",")
            ev = {"t": t, "type": "magnify", "delta": float(delta)}
            if xy:
                ev.update(x=float(xy[0]), y=float(xy[1]))
            events.append(ev)
        elif kind == "type":
            events.append({"t": t, "type": "type", "text": body.replace("_", " ")})
        else:
            dy, x, y = (body.split(",") + ["700", "400"])[:3]
            events.append({"t": t, "type": "scroll", "dy": float(dy), "x": float(x), "y": float(y)})
    return events


def replay(binary, events, at, size, scale, theme, work, snap=None, note=None, folder=None, open_note=None, state=None):
    """One headless run; returns the element tree (a list of dicts) and the home folder."""
    home = os.path.join(work, "home")
    script = os.path.join(work, "events.json")
    tree = os.path.join(work, "tree.json")
    last = max([e.get("t", 0) for e in events] or [0])
    at = last + 0.8 if at is None else at
    with open(script, "w") as f:
        json.dump({"events": events + [{"t": max(at, last) + 0.1, "type": "mark", "name": "probe-end"}]}, f)
    cmd = [binary, "--script", script, "--tree", tree, "--at", "%.3f" % at, "--size", size, "--scale", str(scale)]
    if theme:
        cmd += ["--theme", theme]
    if snap:
        cmd += ["--snap", snap]
    for attempt in range(6):
        shutil.rmtree(home, ignore_errors=True)
        os.makedirs(home)
        env = dict(os.environ, HOME=home, XDG_CONFIG_HOME=home + "/.config", XDG_DATA_HOME=home + "/.local/share", RUNE_PERF_WARN="0")
        if note:
            notes = os.path.join(work, "notes")
            shutil.rmtree(notes, ignore_errors=True)
            os.makedirs(notes)
            shutil.copy(note, notes)
            env.update(QUIRE_FOLDER=notes, QUIRE_NOTE=os.path.basename(note))
        elif folder:
            name = os.path.basename(os.path.normpath(folder)) or "notes"
            notes = os.path.join(work, name)
            shutil.rmtree(notes, ignore_errors=True)
            shutil.copytree(folder, notes)
            env.update(QUIRE_FOLDER=notes)
            if open_note:
                env.update(QUIRE_NOTE=open_note)
        if state:
            with open(os.path.join(work, "state.json"), "w") as f:
                f.write(state)
            env.update(QUIRE_STATE=os.path.join(work, "state.json"))
        with open(LOCK_PATH, "w") as lf:
            fcntl.flock(lf, fcntl.LOCK_EX)
            try:
                r = subprocess.run(cmd, env=env, capture_output=True, text=True)
            finally:
                fcntl.flock(lf, fcntl.LOCK_UN)
        if not r.returncode:
            return json.load(open(tree))["elements"], home
        if "Out of Memory" not in r.stderr:  # the GPU's memory runs out now and then: retry that, nothing else
            break
        time.sleep(3)
    raise SystemExit("probe: the run failed: %s" % (r.stderr.strip().splitlines() or ["?"])[-1])


def rect(els, key):
    e = next((e for e in els if e.get("key") == key), None)
    return e.get("rect") if e else None


def answer(what, els, home):
    if what == "focus":
        f = next((e for e in els if e.get("focused")), None)
        return "focus: %s%s" % ((f.get("key"), " (ring shown)" if f.get("focusVisible") else " (no ring)") if f else ("nothing", ""))
    if what.startswith("rect:"):
        return "%s: %s" % (what, rect(els, what[5:]))
    if what.startswith("text:"):
        e = next((e for e in els if e.get("key") == what[5:]), None)
        return "%s: %r" % (what, e.get("text") if e else None)
    if what.startswith("label:"):
        e = next((e for e in els if e.get("key") == what[6:]), None)
        return "%s: %r" % (what, e.get("label") if e else None)
    if what.startswith("scale:"):
        e = next((e for e in els if e.get("key") == what[6:]), None)
        return "%s: %s" % (what, e.get("scale") if e else None)
    if what == "selected":
        e = next((e for e in els if e.get("itemSelected") and e.get("key", "").startswith("map-node:")), None)
        return "selected: %s" % (e.get("key") if e else None)
    if what == "nodes":
        return "nodes: %s" % " ".join(e["key"][9:] for e in els if e.get("key", "").startswith("map-node:"))
    if what == "doc-y":
        r = rect(els, "doc-col")
        return "doc-y: %s" % (r[1] if r else None)
    if what == "keys":
        return "\n".join("%-28s %s" % (e["key"], e.get("rect")) for e in els if e.get("key"))
    if what == "notes":
        out = []
        work = os.path.dirname(home)
        roots = [os.path.join(home, "Documents")]
        for entry in sorted(os.listdir(work)):
            p = os.path.join(work, entry)
            if entry != "home" and os.path.isdir(p):
                roots.append(p)
        for top in roots:
            for dp, ds, fs in sorted(os.walk(top)):
                ds[:] = [d for d in ds if not d.startswith(".")]
                for f in sorted(fs):
                    if f.endswith(".md"):
                        out.append("%-24s %r" % (os.path.relpath(os.path.join(dp, f), top), open(os.path.join(dp, f)).read()[:60]))
        return "\n".join(out) or "notes: none found"
    raise SystemExit("probe: don't know %r (see --help)" % what)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("events")
    ap.add_argument("--at", type=float)
    ap.add_argument("--size", default="1280x800")
    ap.add_argument("--scale", default="1")
    ap.add_argument("--theme")
    ap.add_argument("--bin", default=os.environ.get("QUIRE_BIN"))
    ap.add_argument("--fast", action="store_true")
    ap.add_argument("--print", dest="what", default="focus,doc-y")
    ap.add_argument("--series")
    ap.add_argument("--snap")
    ap.add_argument("--note")
    ap.add_argument("--folder")
    ap.add_argument("--open")
    ap.add_argument("--state")
    a = ap.parse_args()
    a.bin = a.bin or os.path.join(DESKTOP, "target", "fast" if a.fast else "debug", "quire")
    if not os.path.exists(a.bin):
        raise SystemExit("probe: no binary at %s. Build it with `./dev %s`." % (a.bin, "build" if a.fast else "app"))
    a.work = tempfile.mkdtemp(prefix="quire-probe-")
    events = parse_events(a.events)
    whats = [w for w in a.what.split(",") if w]
    try:
        if a.series:
            t0, t1, step = [float(v) for v in a.series.split(":")]
            prev, t = None, t0
            while t <= t1 + 1e-9:
                els, home = replay(a.bin, events, t, a.size, a.scale, a.theme, a.work, note=a.note, folder=a.folder, open_note=a.open, state=a.state)
                y = (rect(els, "doc-col") or [0, None])[1]
                move = "" if prev is None or y is None else "  moved %+.1f" % (y - prev)
                rest = "  ".join(answer(w, els, home) for w in whats if w != "doc-y")
                print("t %.3f  doc-y %s%s  %s" % (t, y, move, rest))
                prev, t = y, t + step
        else:
            els, home = replay(a.bin, events, a.at, a.size, a.scale, a.theme, a.work, snap=a.snap, note=a.note, folder=a.folder, open_note=a.open, state=a.state)
            for w in whats:
                print(answer(w, els, home))
    finally:
        shutil.rmtree(a.work, ignore_errors=True)


if __name__ == "__main__":
    main()
