#!/usr/bin/env python3
"""Run a command in its own process group with stall and wall-clock bounds.

Used by `desktop/dev test` so a hung test cannot stall forever. Lines from libtest saying
`test ... has been running for over 60 seconds` are recorded as suspected stalls and do NOT
reset the stall timer.

Environment:
    QUIRE_TEST_STALL_SECS   seconds without real progress before aborting (default 120, 0 = off)
    QUIRE_TEST_MAX_SECS     total wall-clock ceiling in seconds (default 900, 0 = off)
"""
import os
import re
import selectors
import signal
import subprocess
import sys
import time


STALL_RE = re.compile(r"^test (\S+) has been running for over \d+ seconds")
START_RE = re.compile(r"^test (\S+) \.\.\.\s*$")
DONE_RE = re.compile(r"^test (\S+) \.\.\. (ok|FAILED|ignored)\b")


def env_secs(name, default):
    raw = os.environ.get(name)
    if raw is None or raw == "":
        return float(default)
    val = float(raw)
    return max(0.0, val)


def terminate_group(proc):
    try:
        pgid = os.getpgid(proc.pid)
    except ProcessLookupError:
        return
    for sig, wait in ((signal.SIGTERM, 2.0), (signal.SIGKILL, 2.0)):
        try:
            os.killpg(pgid, sig)
        except ProcessLookupError:
            return
        deadline = time.monotonic() + wait
        while time.monotonic() < deadline:
            if proc.poll() is not None:
                return
            time.sleep(0.05)


def main():
    if len(sys.argv) < 2:
        sys.stderr.write("usage: run-bounded.py COMMAND [ARGS...]\n")
        return 2
    stall_limit = env_secs("QUIRE_TEST_STALL_SECS", 120)
    max_limit = env_secs("QUIRE_TEST_MAX_SECS", 900)

    proc = subprocess.Popen(
        sys.argv[1:],
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        start_new_session=True,
        bufsize=0,
    )
    sel = selectors.DefaultSelector()
    sel.register(proc.stdout, selectors.EVENT_READ)

    start = time.monotonic()
    last_progress = start
    buf = b""
    in_flight = []
    slow = []

    def handle_line(line):
        nonlocal last_progress
        text = line.decode("utf-8", errors="replace")
        sys.stdout.write(text)
        sys.stdout.flush()
        stripped = text.strip()
        m = STALL_RE.match(stripped)
        if m:
            name = m.group(1)
            if name not in slow:
                slow.append(name)
            return
        last_progress = time.monotonic()
        m = DONE_RE.match(stripped)
        if m:
            name = m.group(1)
            if name in in_flight:
                in_flight.remove(name)
            if name in slow:
                slow.remove(name)
            return
        m = START_RE.match(stripped)
        if m:
            name = m.group(1)
            if name not in in_flight:
                in_flight.append(name)

    try:
        while True:
            now = time.monotonic()
            reason = None
            if stall_limit > 0 and now - last_progress >= stall_limit:
                reason = "stalled for %.0fs without progress (QUIRE_TEST_STALL_SECS=%.0f)" % (now - last_progress, stall_limit)
            elif max_limit > 0 and now - start >= max_limit:
                reason = "exceeded wall-clock limit of %.0fs (QUIRE_TEST_MAX_SECS=%.0f)" % (max_limit, max_limit)
            if reason is not None:
                suspects = slow or in_flight
                terminate_group(proc)
                sys.stderr.write("\ndev: aborted: %s\n" % reason)
                if suspects:
                    sys.stderr.write("dev: active/stalled test(s): %s\n" % ", ".join(suspects))
                sys.stderr.flush()
                return 124

            events = sel.select(timeout=0.5)
            if not events:
                if proc.poll() is not None:
                    break
                continue
            chunk = os.read(proc.stdout.fileno(), 4096)
            if not chunk:
                break
            buf += chunk
            while b"\n" in buf:
                line, buf = buf.split(b"\n", 1)
                handle_line(line + b"\n")
            # Flush partial line like `test foo ... ` so in_flight tracks it under --test-threads=1.
            if buf:
                partial = buf.decode("utf-8", errors="replace").strip()
                m = START_RE.match(partial)
                if m and m.group(1) not in in_flight:
                    in_flight.append(m.group(1))
                    last_progress = time.monotonic()
        if buf:
            handle_line(buf)
        return proc.wait()
    except KeyboardInterrupt:
        terminate_group(proc)
        return 130
    finally:
        sel.close()


if __name__ == "__main__":
    sys.exit(main())
