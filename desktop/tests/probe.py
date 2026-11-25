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
    rtap:KEY@T        a right-click, the same way
    hover:KEY@T       move the pointer over KEY                 hover:700,300@T move to a point
    key:NAME[+cmd][+shift][+alt]@T                               type:TEXT@T     (use _ for a space)
    scroll:DY,X,Y@T   a wheel turn (positive DY moves towards the note's top)
    drag:FROM>DX,DY[,DUR][+MOD]@T   a press at FROM (a KEY or X,Y), moved by DX,DY over DUR
                      seconds (default 0.3), then released; MOD is middle or a key modifier
    wheel:DX,DY[,X,Y][+MOD]@T       one raw wheel delta, at X,Y or where the pointer is; MOD is a
                      key modifier or the touchpad phase (began, moved, ended)
    pinch:DELTA[,X,Y]@T             a touchpad pinch (0.1 zooms in by about a tenth)
    swipe:DX,DY[,X,Y][,N]@T         a two-finger touchpad swipe: N frames (default 8) of DX,DY
                      16 ms apart, then the fingers lift, which throws the page or the map on
    drop:FILE[,FILE...]>WHERE@T     image files from another app, held over WHERE (a KEY or
                      X,Y) for 0.4 s, then dropped; FILE is a path from desktop/
    pastepng:FILE@T   a paste of a copied image: FILE's PNG bytes on the clipboard
    pick:FILE@T       FILE picked in the open file dialog
    write:NAME@T      another app writes NAME in the notes folder (its text from --outside)
    delete:NAME@T     another app deletes NAME from the notes folder

T is seconds on the app's own clock. `cmd` is Ctrl on Linux and Windows. Typed text arrives at
about 30 letters a second, so leave it time before the next event.

A replay with `write` or `delete` runs on the wall clock (Rune's --realtime), so the app's
watcher sees the file change as it would one a sync tool made: it looks every 1.5 s, so leave
2 s after the change before the next event.

WHAT is any of (default `focus,doc-y`):

    focus             the focused element's key, and whether its ring shows
    rect:KEY          an element's rectangle [x, y, w, h] in points
    text:KEY          an element's text, or the text inside it joined by spaces
    doc-y             the page's top edge, which tells you how far it has scrolled
    label:KEY         an element's accessible label (a map node's reads its text and state)
    role:KEY          an element's accessible role
    scale:KEY         how much an element is drawn scaled (a map node's is the map's zoom)
    opacity:KEY       how opaque an element is drawn: sample it with --series to see a fade
    color:KEY         an element's text colour, or that of the first text inside it
    contrast:KEY      the text inside an element against the fills behind it (WCAG 4.5:1) and
                      the marks drawn there, such as underlines and icons (3:1): how many it
                      measured, or what falls short
    selected          the map's selected node, by key
    nodes             the map nodes built now, by key, in the order drawn: what culling kept
    keys              every keyed element with its rectangle: the way to find a KEY
    notes             the first 60 characters of each note on disk after the replay
    state:KEY         a top-level value in the state file the app saved, as JSON
    file:GLOB         the files in the notes folder that match GLOB, with their sizes

--at takes the reading at that time; without it, 0.8 s after the last event.

--series samples the same replay at each time from T0 to T1 and prints one line per sample
with the change in doc-y since the one before. That shows a jump, a glide or a bounce, which
one reading cannot. Each sample is a fresh process: a 20-second script sampled 40 times takes
a few minutes.

--note opens a folder holding only that note. --folder opens a copy of a whole folder of notes,
and --open names the note in it to open, by its path inside the folder; opening a note opens the
folders above it. Without either the app writes its sample notes.

--outside NAME=FILE gives the text a `write:NAME` event writes: FILE's bytes. Repeat it for
more files.

--scene NAME starts the app in one of its scenes: `note` opens the note in Note mode, as a
file of its own.

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
        m = re.fullmatch(r"(tap|rtap|hover|key|type|pastepng|paste|pick|scroll|drag|wheel|pinch|swipe|drop|write|delete):(.*)@([0-9.]+)", tok)
        if not m:
            raise SystemExit("probe: cannot read %r (see --help)" % tok)
        kind, body, t = m.group(1), m.group(2), float(m.group(3))
        if kind in ("tap", "rtap", "hover"):
            events.append(dict({"t": t, "type": {"tap": "tap", "rtap": "secondary-click", "hover": "move"}[kind]}, **at(body)))
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
            phases = [m for m in mods if m in ("began", "moved", "ended")]
            ev = dict({"t": t, "type": "wheel", "dx": float(dx), "dy": float(dy)}, **{m: True for m in mods if m not in phases})
            if phases:
                ev["phase"] = phases[0]
            if xy:
                ev.update(x=float(xy[0]), y=float(xy[1]))
            events.append(ev)
        elif kind == "swipe":
            parts = body.split(",")
            dx, dy = float(parts[0]), float(parts[1])
            xy = {"x": float(parts[2]), "y": float(parts[3])} if len(parts) >= 4 else {}
            n = int(parts[4]) if len(parts) == 5 else int(parts[2]) if len(parts) == 3 else 8
            for i in range(n):
                events.append(dict({"t": round(t + i * 0.016, 4), "type": "wheel", "dx": dx, "dy": dy, "phase": "began" if i == 0 else "moved"}, **xy))
            events.append(dict({"t": round(t + n * 0.016, 4), "type": "wheel", "dx": 0.0, "dy": 0.0, "phase": "ended"}, **xy))
        elif kind == "pinch":
            delta, *xy = body.split(",")
            ev = {"t": t, "type": "magnify", "delta": float(delta)}
            if xy:
                ev.update(x=float(xy[0]), y=float(xy[1]))
            events.append(ev)
        elif kind == "type":
            events.append({"t": t, "type": "type", "text": body.replace("_", " ")})
        elif kind == "drop":
            files, _, where = body.partition(">")
            spec = {"paths": [os.path.abspath(f) for f in files.split(",")], "hover": 0.4}
            place = at(where)
            spec.update({"key": place["key"]} if "key" in place else {"at": [place["x"], place["y"]]})
            events.append({"t": t, "drop_files": spec})
        elif kind == "pick":
            events.append({"t": t, "type": "file-dialog", "path": os.path.abspath(body)})
        elif kind in ("write", "delete"):
            events.append({"t": t, "type": "mark", "name": "probe-%s:%s" % (kind, body)})
        elif kind == "pastepng":
            events.append({"t": t, "type": "paste", "png": os.path.abspath(body)})
        elif kind == "paste":
            events.append({"t": t, "type": "paste", "text": body.replace("_", " ").replace("\\n", "\n")})
        else:
            dy, x, y = (body.split(",") + ["700", "400"])[:3]
            events.append({"t": t, "type": "scroll", "dy": float(dy), "x": float(x), "y": float(y)})
    return events


def probe_timeout(extra=0.0):
    raw = os.environ.get("QUIRE_PROBE_TIMEOUT", "45")
    base = max(0.0, float(raw))
    return None if base == 0.0 else base + extra


def run_outside(cmd, env, notes, outside, timeout=None):
    """A --realtime run that acts on its `probe-write:NAME` and `probe-delete:NAME` marks as
    the app reaches them, writing `outside[NAME]` to the notes folder or deleting NAME."""
    import selectors
    with tempfile.TemporaryFile("w+") as err:
        p = subprocess.Popen(cmd + ["--realtime"], env=env, stdout=subprocess.PIPE, stderr=err, text=True, start_new_session=True)
        sel = selectors.DefaultSelector()
        sel.register(p.stdout, selectors.EVENT_READ)
        deadline = (time.monotonic() + timeout) if timeout else None
        try:
            while True:
                wait = None if deadline is None else max(0.0, deadline - time.monotonic())
                if deadline is not None and wait <= 0.0:
                    try:
                        os.killpg(os.getpgid(p.pid), 9)
                    except ProcessLookupError:
                        pass
                    p.wait()
                    raise SystemExit("probe: timed out after %.0fs" % timeout)
                if not sel.select(timeout=min(0.5, wait) if wait is not None else 0.5):
                    if p.poll() is not None:
                        break
                    continue
                line = p.stdout.readline()
                if not line:
                    break
                m = re.match(r"rune-mark probe-(write|delete):(\S+) ", line)
                if not m:
                    continue
                path = os.path.join(notes, m.group(2))
                if m.group(1) == "delete":
                    os.remove(path)
                    continue
                if m.group(2) not in outside:
                    p.kill()
                    raise SystemExit("probe: no text for write:%s (see --outside)" % m.group(2))
                with open(path + ".probe-tmp", "wb") as f:
                    f.write(outside[m.group(2)])
                os.replace(path + ".probe-tmp", path)
            p.wait(timeout=max(1.0, deadline - time.monotonic()) if deadline else None)
        except subprocess.TimeoutExpired:
            try:
                os.killpg(os.getpgid(p.pid), 9)
            except ProcessLookupError:
                pass
            p.wait()
            raise SystemExit("probe: timed out after %.0fs" % timeout)
        finally:
            sel.close()
        err.seek(0)
        return subprocess.CompletedProcess(cmd, p.returncode, "", err.read())


def replay(binary, events, at, size, scale, theme, work, snap=None, note=None, folder=None, open_note=None, state=None, env_add=None, extra=None, outside=None, scene=None):
    """One headless run; returns the element tree (a list of dicts) and the home folder.
    `extra` maps paths in the notes folder, or in the home folder for one starting `~/`, to the
    bytes written there beside `note`; `outside` maps them to the bytes a `write` event writes
    mid-run."""
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
    if scene:
        cmd += ["--scene", scene]
    for attempt in range(6):
        shutil.rmtree(home, ignore_errors=True)
        os.makedirs(home)
        env = dict(os.environ, HOME=home, XDG_CONFIG_HOME=home + "/.config", XDG_DATA_HOME=home + "/.local/share", RUNE_PERF_WARN="0")
        env.update(env_add or {})
        if note:
            notes = os.path.join(work, "notes")
            shutil.rmtree(notes, ignore_errors=True)
            os.makedirs(notes)
            shutil.copy(note, notes)
            for rel, data in (extra or {}).items():
                path = os.path.join(home, rel[2:]) if rel.startswith("~/") else os.path.join(notes, rel)
                os.makedirs(os.path.dirname(path), exist_ok=True)
                with open(path, "wb") as f:
                    f.write(data)
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
        timed = any(e.get("type") == "mark" and e.get("name", "").startswith(("probe-write:", "probe-delete:")) for e in events)
        if timed and not (note or folder):
            raise SystemExit("probe: write and delete need --note or --folder")
        limit = probe_timeout(extra=at if timed else 0.0)
        with open(LOCK_PATH, "w") as lf:
            fcntl.flock(lf, fcntl.LOCK_EX)
            try:
                if timed:
                    r = run_outside(cmd, env, notes, outside or {}, timeout=limit)
                else:
                    try:
                        r = subprocess.run(cmd, env=env, capture_output=True, text=True, timeout=limit)
                    except subprocess.TimeoutExpired:
                        raise SystemExit("probe: timed out after %.0fs" % limit)
            finally:
                fcntl.flock(lf, fcntl.LOCK_UN)
        if not r.returncode:
            return json.load(open(tree))["elements"], home
        if "Out of Memory" not in r.stderr:  # the GPU's memory runs out now and then: retry that, nothing else
            break
        time.sleep(3)
    raise SystemExit("probe: the run failed: %s" % (r.stderr.strip().splitlines() or ["?"])[-1])


def luminance(c):
    lin = [v / 12.92 if v <= 0.04045 else ((v + 0.055) / 1.055) ** 2.4 for v in c[:3]]
    return 0.2126 * lin[0] + 0.7152 * lin[1] + 0.0722 * lin[2]


def ratio(a, b):
    la, lb = luminance(a) + 0.05, luminance(b) + 0.05
    return max(la, lb) / min(la, lb)


def contrast(els, key):
    """Each text and mark drawn inside the element `key`, against what is behind it: the fills
    of the elements it sits in, laid over each other from `key` down. A mark is an icon, a
    shape's stroke, as a map's branch with its node's underline is, or a filled element with
    nothing in it drawn straight on `key`'s own fill, as the root's underline is. A map card draws its block as Preview does, so a quote's bar in it
    is the block's decoration, not a mark, and the selection's glass is a surface under a node.
    Returns (shortfalls, texts, marks, lowest text, lowest mark)."""
    top = next((e for e in els if e.get("key") == key), None)
    if not top:
        return None
    rgba = lambda h: [int(h[i:i + 2], 16) / 255 for i in (1, 3, 5, 7)]
    over = lambda a, b: [a[i] * a[3] + b[i] * (1 - a[3]) for i in range(3)] + [1.0]
    at = {e["path"]: e for e in els}
    parents = {e["path"].rsplit("/", 1)[0] for e in els}
    low, texts, marks = [], [], []
    for e in els:
        path = e.get("path", "")
        if not path.startswith(top["path"] + "/"):
            continue
        tail = path[len(top["path"]):].split("/")
        chain = [at[top["path"] + "/".join(tail[:k])] for k in range(1, len(tail) + 1) if top["path"] + "/".join(tail[:k]) in at]
        if any(a.get("opacity", 1.0) == 0 for a in chain + [top]):
            continue
        back = rgba(top.get("fill") or "#000000ff")
        for a in chain[:-1]:
            back = over(rgba(a.get("fill") or "#00000000"), back)
        if e.get("text") is not None and e.get("color"):
            back = over(rgba(e.get("fill") or "#00000000"), back)
            kind, ink, need, into = "text", rgba(e["color"]), 4.5, texts
        elif e.get("role") == "icon" and e.get("color"):
            back = over(rgba(e.get("fill") or "#00000000"), back)
            kind, ink, need, into = "icon", rgba(e["color"]), 3.0, marks
        elif e.get("stroke") and path not in parents:
            back = over(rgba(e.get("fill") or "#00000000"), back)
            kind, ink, need, into = "line", rgba(e["stroke"]), 3.0, marks
        elif path not in parents and e.get("key") != "map-glass" and e.get("fill") and rgba(e["fill"])[3] > 0 and over(rgba(e["fill"]), back) != back and not any(rgba(a.get("fill") or "#00000000")[3] > 0 or a.get("key", "").startswith("map-card:") for a in chain[1:-1]):
            kind, ink, need, into = "mark", rgba(e["fill"]), 3.0, marks
        else:
            continue
        r = ratio(over(ink, back), back)
        into.append(r)
        if r < need:
            low.append("%s %s %s %.2f:1 on #%02x%02x%02x" % (kind, path[len(top["path"]) + 1:], repr(e["text"]) if kind == "text" else e.get("color") or e.get("stroke") or e.get("fill"), r, *[round(v * 255) for v in back[:3]]))
    return low, len(texts), len(marks), min(texts, default=None), min(marks, default=None)


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
        if e and e.get("text") is None:
            inner = [d["text"] for d in els if d.get("text") is not None and d.get("path", "").startswith(e.get("path", "") + "/")]
            return "%s: %r" % (what, " ".join(inner) if inner else None)
        return "%s: %r" % (what, e.get("text") if e else None)
    if what.startswith("label:"):
        e = next((e for e in els if e.get("key") == what[6:]), None)
        return "%s: %r" % (what, e.get("label") if e else None)
    if what.startswith("role:"):
        e = next((e for e in els if e.get("key") == what[5:]), None)
        return "%s: %s" % (what, e.get("role") if e else None)
    if what.startswith("scale:"):
        e = next((e for e in els if e.get("key") == what[6:]), None)
        return "%s: %s" % (what, e.get("scale") if e else None)
    if what.startswith("color:"):
        e = next((e for e in els if e.get("key") == what[6:]), None)
        if e and e.get("color") is None:
            e = next((d for d in els if d.get("color") and d.get("text") is not None and d.get("path", "").startswith(e.get("path", "") + "/")), None)
        return "%s: %s" % (what, e.get("color") if e else None)
    if what.startswith("contrast:"):
        got = contrast(els, what[9:])
        if got is None:
            return "%s: None" % what
        low, texts, marks = got[:3]
        return "%s: %s" % (what, "; ".join(low) if low else "%d texts at 4.5:1 or more, %d marks at 3:1 or more" % (texts, marks))
    if what.startswith("opacity:"):
        e = next((e for e in els if e.get("key") == what[8:]), None)
        return "%s: %s" % (what, e.get("opacity") if e else None)
    if what == "selected":
        e = next((e for e in els if e.get("itemSelected") and e.get("key", "").startswith("map-node:")), None)
        return "selected: %s" % (e.get("key") if e else None)
    if what == "nodes":
        return "nodes: %s" % " ".join(e["key"][9:] for e in els if e.get("key", "").startswith("map-node:"))
    if what.startswith("file:"):
        import glob
        notes = os.path.join(os.path.dirname(home), "notes")
        hits = sorted(glob.glob(os.path.join(notes, what[5:])))
        return "%s: %s" % (what, ", ".join("%s (%d bytes)" % (os.path.relpath(h, notes), os.path.getsize(h)) for h in hits) or "none")
    if what.startswith("state:"):
        import glob
        found = glob.glob(os.path.join(os.path.dirname(home), "state.json")) + glob.glob(os.path.join(home, ".config", "*", "state.json"))
        saved = json.load(open(found[0])) if found else {}
        return "%s: %s" % (what, json.dumps(saved.get(what[6:])) if found else None)
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
    ap.add_argument("--outside", action="append", default=[])
    ap.add_argument("--scene")
    a = ap.parse_args()
    outside = {}
    for spec in a.outside:
        name, _, path = spec.partition("=")
        outside[name] = open(path, "rb").read()
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
                els, home = replay(a.bin, events, t, a.size, a.scale, a.theme, a.work, note=a.note, folder=a.folder, open_note=a.open, state=a.state, outside=outside, scene=a.scene)
                y = (rect(els, "doc-col") or [0, None])[1]
                move = "" if prev is None or y is None else "  moved %+.1f" % (y - prev)
                rest = "  ".join(answer(w, els, home) for w in whats if w != "doc-y")
                print("t %.3f  doc-y %s%s  %s" % (t, y, move, rest))
                prev, t = y, t + step
        else:
            els, home = replay(a.bin, events, a.at, a.size, a.scale, a.theme, a.work, snap=a.snap, note=a.note, folder=a.folder, open_note=a.open, state=a.state, outside=outside, scene=a.scene)
            for w in whats:
                print(answer(w, els, home))
    finally:
        shutil.rmtree(a.work, ignore_errors=True)


if __name__ == "__main__":
    main()
