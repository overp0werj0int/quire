#!/usr/bin/env python3
"""Run behaviour cases: each replays input in a headless Quire on one note and checks the note
saved on disk, and any probe readings, against what the case expects.

    tests/cases.py [FILTER...] [--fast | --bin PATH] [--keep]

Run it from `desktop/`. Cases live in tests/cases/**/*.case; FILTER keeps the cases whose path
contains any of the words. A case file:

    Tab on a heading adds a child and Enter commits it.
    note: Plan.md
    state: {"mode": "map"}
    events: key:m+cmd@0.3 key:Tab@0.8 type:Budget@1.0 key:Enter@1.6 key:s+cmd@2.0
    expect: focus: note-keys (no ring)
    --- before
    # Plan
    --- after
    # Plan

    ## Budget

The first line says what the case checks. `note` names the file (default Note.md), `state`
is the state file as for probe.py --state, `events` is probe.py's short form (it may run over
several `events:` lines), `size` is WxH, and each `expect` is one line probe.py would print for
that reading (`focus`, `rect:KEY`, `text:KEY`, `doc-y`, `label:KEY`, `scale:KEY`,
`opacity:KEY`, `selected`, `nodes`), compared exactly. `at` takes the readings at that time instead of 0.8 s
after the last event, to read the app mid-gesture: a drag's drop zone before the release, or a
node part way through a glide. Each `env` line is NAME=VALUE for the app's environment, such
as QUIRE_REDUCED_MOTION=1. With `password`, the note on disk must be a locked note (SYNC.md),
and its text opened with that password is what `--- after` holds. The text after
`--- before` is the note as it starts. The text after `--- after` is the note expected on disk
after the replay; without that section the note must be unchanged. Each line of a section is a
line of the note, ended by a newline, so both notes end in one.

Each case is a whole copy of the app, run one at a time. --keep leaves each run's folder.
"""
import argparse
import difflib
import glob
import os
import shutil
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.dont_write_bytecode = True
import probe  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
READINGS = ("focus", "rect:", "text:", "doc-y", "label:", "scale:", "opacity:", "selected", "nodes")


class Case:
    def __init__(self, path):
        self.path = path
        self.name = os.path.relpath(path, os.path.join(HERE, "cases"))
        self.note, self.state, self.size, self.password, self.at = "Note.md", None, "1280x800", None, None
        self.events, self.expect, self.before, self.after, self.env = [], [], None, None, {}
        text = open(path).read()
        lines = (text[:-1] if text.endswith("\n") else text).split("\n")
        self.says = lines[0]
        section, body = None, []
        for line in lines[1:]:
            if line in ("--- before", "--- after"):
                self._close(section, body)
                section, body = line[4:], []
            elif section:
                body.append(line)
            elif line.strip():
                key, _, value = line.partition(":")
                value = value.strip()
                if key == "note":
                    self.note = value
                elif key == "state":
                    self.state = value
                elif key == "size":
                    self.size = value
                elif key == "password":
                    self.password = value
                elif key == "at":
                    self.at = float(value)
                elif key == "env":
                    name, _, v = value.partition("=")
                    self.env[name.strip()] = v.strip()
                elif key == "events":
                    self.events.append(value)
                elif key == "expect":
                    self.expect.append(value)
                else:
                    raise SystemExit("%s: unknown line %r" % (self.name, line))
        self._close(section, body)
        if self.before is None or not self.events:
            raise SystemExit("%s: a case needs events and a --- before section" % self.name)

    def _close(self, section, body):
        if section:
            setattr(self, section, "".join(line + "\n" for line in body))


def unlock(text, password):
    """A locked note's text (SYNC.md, "Locked notes"), or a line saying it is not locked."""
    import base64
    import hashlib

    from cryptography.hazmat.primitives.ciphers.aead import AESGCM

    lines = text.split("\n")
    head = lines[0].split()
    if head[:3] != ["%QUIRE-LOCKED-NOTE", "v1", "pbkdf2-sha256"]:
        return "(not locked)\n" + text
    key = hashlib.pbkdf2_hmac("sha256", password.encode(), base64.b64decode(head[4]), int(head[3]), 64)[:32]
    blob = base64.b64decode("".join(lines[3:]))
    return AESGCM(key).decrypt(blob[6:18], blob[18:], None).decode()


def reading(line):
    # A key may hold a colon (`map-node:12`), never a space: the reading ends at the first ": ".
    what = line.split(": ", 1)[0]
    if not what.startswith(READINGS):
        raise SystemExit("cases: cannot read an expectation from %r" % line)
    return what


def run(case, binary, keep):
    work = tempfile.mkdtemp(prefix="quire-case-")
    try:
        src = os.path.join(work, "src")
        os.makedirs(src)
        with open(os.path.join(src, case.note), "w") as f:
            f.write(case.before)
        events = probe.parse_events(" ".join(case.events))
        els, home = probe.replay(binary, events, case.at, case.size, 1, None, work, note=os.path.join(src, case.note), state=case.state, env_add=case.env)
        problems = []
        saved = open(os.path.join(work, "notes", case.note)).read()
        if case.password:
            saved = unlock(saved, case.password)
        want = case.before if case.after is None else case.after
        if saved != want:
            diff = difflib.unified_diff(want.splitlines(True), saved.splitlines(True), "expected", "on disk")
            problems.append("the note on disk differs:\n" + "".join(diff).rstrip("\n"))
        for line in case.expect:
            got = probe.answer(reading(line), els, home)
            if got != line:
                problems.append("expected %r, got %r" % (line, got))
        return problems
    finally:
        if keep:
            print("  kept %s" % work)
        else:
            shutil.rmtree(work, ignore_errors=True)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("filters", nargs="*")
    ap.add_argument("--bin", default=os.environ.get("QUIRE_BIN"))
    ap.add_argument("--fast", action="store_true")
    ap.add_argument("--keep", action="store_true")
    a = ap.parse_args()
    binary = a.bin or os.path.join(probe.DESKTOP, "target", "fast" if a.fast else "debug", "quire")
    if not os.path.exists(binary):
        raise SystemExit("cases: no binary at %s" % binary)
    paths = sorted(glob.glob(os.path.join(HERE, "cases", "**", "*.case"), recursive=True))
    cases = [Case(p) for p in paths if not a.filters or any(w in p for w in a.filters)]
    failed = 0
    for case in cases:
        problems = run(case, binary, a.keep)
        print("%s  %s  (%s)" % ("FAIL" if problems else "ok  ", case.name, case.says))
        for p in problems:
            print("    " + p.replace("\n", "\n    "))
        failed += bool(problems)
    print("%d cases, %d failed" % (len(cases), failed))
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
