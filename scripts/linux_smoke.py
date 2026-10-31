#!/usr/bin/env python3
"""End-to-end smoke test of OpenDraft on a Linux desktop (X11).

Run it inside a running X server (scripts/linux_smoke.sh starts Xvfb). It
starts the real program with a throwaway profile, home and runtime folder,
and drives it the way a person would, then reads the result back through the
automation socket (the same one the MCP server uses):

  1. the program starts and the automation socket appears;
  2. the drawing named on the command line is open and has objects;
  3. the window draws something (the screen is not blank);
  4. typing L, Enter, two clicks and Enter draws one more line, and Ctrl+Z
     takes it back (keyboard, mouse, command, undo, repaint);
  5. `opendraft-mcp --check` reaches the running program;
  6. Ctrl+Q ends it with exit code 0.

Screenshots, the program's log and a JSON report are left in --out. Only the
Python standard library is used, plus `xdotool` for input.
"""

import argparse
import glob
import json
import os
import shutil
import socket
import struct
import subprocess
import sys
import tempfile
import time
import zlib
from collections import Counter

PROFILE = "ci-smoke"
WINDOW_TITLE = "OpenDraft ci-smoke"
HELLO_TIMEOUT = 20


# ---------------------------------------------------------------- reporting

class Report:
    def __init__(self):
        self.checks = []
        self.notes = {}

    def check(self, name, ok, detail=""):
        self.checks.append({"name": name, "ok": bool(ok), "detail": str(detail)})
        mark = "ok  " if ok else "FAIL"
        print(f"[{mark}] {name}" + (f": {detail}" if detail else ""), flush=True)
        return bool(ok)

    @property
    def failed(self):
        return [c for c in self.checks if not c["ok"]]


# --------------------------------------------------------- automation client

class Client:
    """The framed JSON protocol of od-api: a 4-byte big-endian length, then
    the message. See crates/od-api/src/frame.rs and handshake.rs."""

    def __init__(self, descriptor):
        self.sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.sock.settimeout(HELLO_TIMEOUT)
        self.sock.connect(descriptor["endpoint"])
        self.serial = 0
        self.send({
            "token": descriptor["token"],
            "api_versions": [1],
            "client_name": "linux-smoke",
        })
        self.hello = self.receive()
        if not self.hello.get("ok"):
            raise RuntimeError(f"hello refused: {self.hello}")

    def send(self, message):
        body = json.dumps(message).encode()
        self.sock.sendall(struct.pack(">I", len(body)) + body)

    def receive(self):
        header = self._exactly(4)
        (length,) = struct.unpack(">I", header)
        return json.loads(self._exactly(length))

    def _exactly(self, count):
        data = b""
        while len(data) < count:
            chunk = self.sock.recv(count - len(data))
            if not chunk:
                raise RuntimeError("the program closed the connection")
            data += chunk
        return data

    def call(self, operation, arguments=None, scope="drawing.read"):
        self.serial += 1
        self.send({
            "api_version": "1",
            "request_id": f"s{self.serial}",
            "deadline_ms": 10000,
            "scope": scope,
            "operation": operation,
            "arguments": arguments,
        })
        reply = self.receive()
        if not reply.get("ok"):
            raise RuntimeError(f"{operation} failed: {reply.get('error')}")
        return reply["result"]

    def close(self):
        try:
            self.sock.close()
        except OSError:
            pass


# ------------------------------------------------------------------ screen

def read_xwd(path):
    """Read an XWD file as Xvfb writes it with -fbdir: returns
    (width, height, rgb) with rgb the pixels as 3 bytes each, row by row."""
    with open(path, "rb") as handle:
        raw = handle.read()
    fields = struct.unpack(">25I", raw[:100])
    header_size, depth, width, height, byte_order, bpp, per_line = (
        fields[0], fields[3], fields[4], fields[5], fields[7], fields[11], fields[12])
    rmask, gmask, bmask, ncolors = fields[14], fields[15], fields[16], fields[19]
    if bpp != 32:
        raise RuntimeError(f"unsupported screen format: {bpp} bits per pixel, depth {depth}")
    start = header_size + ncolors * 12
    data = raw[start:start + per_line * height]
    if per_line != width * 4:
        data = b"".join(data[y * per_line:y * per_line + width * 4] for y in range(height))

    def byte_of(mask):
        # Pixels are 32-bit words in the server's byte order (0 is least
        # significant byte first); this is which byte of the word has `mask`.
        shift = (mask & -mask).bit_length() - 1
        return shift // 8 if byte_order == 0 else 3 - shift // 8

    rgb = bytearray(width * height * 3)
    rgb[0::3] = data[byte_of(rmask)::4]
    rgb[1::3] = data[byte_of(gmask)::4]
    rgb[2::3] = data[byte_of(bmask)::4]
    return width, height, bytes(rgb)


def write_png(path, width, height, rgb):
    def chunk(kind, data):
        body = kind + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body) & 0xFFFFFFFF)

    rows = b"".join(b"\x00" + rgb[y * width * 3:(y + 1) * width * 3] for y in range(height))
    png = (b"\x89PNG\r\n\x1a\n"
           + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0))
           + chunk(b"IDAT", zlib.compress(rows, 6))
           + chunk(b"IEND", b""))
    with open(path, "wb") as handle:
        handle.write(png)


def screen_stats(width, height, rgb):
    triples = Counter(zip(rgb[0::3], rgb[1::3], rgb[2::3]))
    total = width * height
    (_, top), = triples.most_common(1)
    return {"colours": len(triples), "dominant_share": top / total}


def differing_pixels(a, b, width):
    """How many pixels differ between two RGB buffers of the same size."""
    if len(a) != len(b):
        return -1
    row = width * 3
    count = 0
    for start in range(0, len(a), row):
        if a[start:start + row] != b[start:start + row]:
            ra, rb = a[start:start + row], b[start:start + row]
            count += sum(1 for i in range(0, row, 3) if ra[i:i + 3] != rb[i:i + 3])
    return count


# --------------------------------------------------------------- the driver

def xdotool(*args, check=True):
    return subprocess.run(["xdotool", *args], check=check, capture_output=True, text=True)


def wait_for(predicate, seconds, step=0.25):
    end = time.time() + seconds
    while time.time() < end:
        value = predicate()
        if value:
            return value
        time.sleep(step)
    return None


def seed_settings(data_home):
    """Turn automation on in the profile's saved settings, as ticking the
    box in Settings would. eframe keeps them in app.ron (a RON map of strings),
    in $XDG_DATA_HOME/<profile name, lower case, without spaces>."""
    folder = os.path.join(data_home, WINDOW_TITLE.lower().replace(" ", ""))
    os.makedirs(folder, exist_ok=True)
    settings = json.dumps({"automation_enabled": True})
    escaped = settings.replace("\\", "\\\\").replace('"', '\\"')
    with open(os.path.join(folder, "app.ron"), "w") as handle:
        handle.write('{\n    "settings": "%s",\n}\n' % escaped)
    return folder


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--binary", required=True, help="path of the opendraft program")
    parser.add_argument("--mcp", help="path of opendraft-mcp (default: next to the program)")
    parser.add_argument("--sample", required=True, help="a drawing to open")
    parser.add_argument("--out", required=True, help="folder for screenshots, log and report")
    parser.add_argument("--fbdir", default=os.environ.get("SMOKE_FBDIR"),
                        help="the Xvfb -fbdir, for screenshots")
    parser.add_argument("--scale", help="set WINIT_X11_SCALE_FACTOR (for example 2)")
    parser.add_argument("--label", default="smoke", help="names the artifacts")
    args = parser.parse_args()

    os.makedirs(args.out, exist_ok=True)
    report = Report()
    work = tempfile.mkdtemp(prefix="od-smoke-")
    home = os.path.join(work, "home")
    data = os.path.join(work, "data")
    run = os.path.join(work, "run")
    for folder in (home, data, run):
        os.makedirs(folder)
    os.chmod(run, 0o700)
    sample = os.path.join(work, os.path.basename(args.sample))
    shutil.copy(args.sample, sample)

    env = dict(os.environ)
    env.update({
        "HOME": home,
        "XDG_DATA_HOME": data,
        "XDG_CONFIG_HOME": os.path.join(home, ".config"),
        "XDG_CACHE_HOME": os.path.join(home, ".cache"),
        "XDG_RUNTIME_DIR": run,
        "OPENDRAFT_PROFILE": PROFILE,
        "LIBGL_ALWAYS_SOFTWARE": "1",
        "RUST_BACKTRACE": "1",
        "RUST_LOG": "info",
    })
    if args.scale:
        env["WINIT_X11_SCALE_FACTOR"] = args.scale
    seed_settings(data)

    log_path = os.path.join(args.out, f"{args.label}-app.log")
    log = open(log_path, "wb")
    app = subprocess.Popen([args.binary, sample], env=env, stdout=log, stderr=subprocess.STDOUT)
    client = None
    shots = {}
    cx = cy = None

    def shot(name):
        if not args.fbdir:
            return None
        files = glob.glob(os.path.join(args.fbdir, "Xvfb_screen*"))
        if not files:
            return None
        width, height, rgb = read_xwd(files[0])
        write_png(os.path.join(args.out, f"{args.label}-{name}.png"), width, height, rgb)
        shots[name] = (width, height, rgb)
        return shots[name]

    try:
        # 1. The program starts and the automation socket appears.
        pattern = os.path.join(run, "OpenDraft", "automation", "*.json")
        descriptor_file = wait_for(
            lambda: (glob.glob(pattern) or [None])[0] if app.poll() is None else "gone", 90)
        if descriptor_file in (None, "gone"):
            code = app.poll()
            report.check("the program starts and offers the automation socket", False,
                         f"it exited with code {code}" if descriptor_file == "gone" else "no socket after 90 s")
            diagnose(args, work, shot, app)
            return finish(report, app, log, log_path, args, work)
        report.check("the program starts and offers the automation socket", True)
        with open(descriptor_file) as handle:
            descriptor = json.load(handle)
        client = Client(descriptor)
        report.check("the hello is accepted with a read-only grant", client.hello["scopes"] == ["drawing.read"],
                     client.hello.get("scopes"))

        # 2. The drawing named on the command line is open.
        def documents():
            found = client.call("documents.list")["documents"]
            return found if found else None
        docs = wait_for(documents, 60)
        sample_name = os.path.splitext(os.path.basename(args.sample))[0]
        report.check("the drawing on the command line is open",
                     bool(docs) and sample_name in json.dumps(docs), json.dumps(docs)[:200] if docs else "none")
        if not docs:
            return finish(report, app, log, log_path, args, work)
        doc_id = docs[0]["document_id"]

        def count():
            return len(client.call("entities.query", {"document_id": doc_id, "limit": 1000})["entities"])
        before = count()
        report.check("the drawing has objects", before > 0, f"{before} objects")

        # 3. The window draws something.
        time.sleep(3)
        first = shot("opened")
        if first:
            stats = screen_stats(*first)
            report.notes["screen"] = stats
            report.check("the screen shows a drawn window, not a blank one",
                         stats["colours"] >= 16 and stats["dominant_share"] < 0.9,
                         f"{stats['colours']} colours, {stats['dominant_share']:.0%} the commonest")
        else:
            report.check("a screenshot could be taken", False, "no framebuffer file (pass --fbdir)")

        # 4. Keyboard and mouse: draw a line, then undo it.
        window = wait_for(lambda: (xdotool("search", "--name", WINDOW_TITLE, check=False).stdout.split() or [None])[0], 30)
        if not window:
            report.check("the window is on screen", False, f"no window named {WINDOW_TITLE!r}")
        else:
            geometry = dict(line.split("=") for line in
                            xdotool("getwindowgeometry", "--shell", window).stdout.split())
            x, y, w, h = (int(geometry[k]) for k in ("X", "Y", "WIDTH", "HEIGHT"))
            report.notes["window"] = {"x": x, "y": y, "width": w, "height": h}
            report.check("the window is on screen and a sensible size", w >= 600 and h >= 400, f"{w}x{h}")
            cx, cy = x + int(w * 0.5), y + int(h * 0.55)
            # No window manager: the window under the pointer gets the keys.
            xdotool("mousemove", str(cx), str(cy))
            time.sleep(0.5)
            xdotool("type", "--delay", "120", "L")
            xdotool("key", "Return")
            time.sleep(1.0)
            xdotool("mousemove", str(cx - 120), str(cy - 40))
            time.sleep(0.4)
            xdotool("click", "1")
            time.sleep(0.6)
            xdotool("mousemove", str(cx + 140), str(cy + 60))
            time.sleep(0.4)
            xdotool("click", "1")
            time.sleep(0.6)
            xdotool("key", "Return")
            drawn = wait_for(lambda: count() == before + 1 or None, 15)
            report.check("typing L, two clicks and Enter draws one line", bool(drawn), f"{before} -> {count()} objects")
            time.sleep(1.0)
            second = shot("drawn")
            if first and second:
                changed = differing_pixels(first[2], second[2], first[0])
                report.check("the new line is painted", changed > 50, f"{changed} pixels changed")
            if drawn:
                newest = client.call("entities.query", {"document_id": doc_id, "limit": 1000})["entities"][-1]
                report.check("the new object is a line", str(newest.get("kind", "")).lower() == "line", newest.get("kind"))
            xdotool("key", "ctrl+z")
            undone = wait_for(lambda: count() == before or None, 15)
            report.check("Ctrl+Z takes it back", bool(undone), f"now {count()} objects")

        # 5. The MCP sidecar reaches the running program.
        mcp = args.mcp or os.path.join(os.path.dirname(os.path.abspath(args.binary)), "opendraft-mcp")
        if os.path.exists(mcp):
            result = subprocess.run([mcp, "--check"], env=env, capture_output=True, text=True, timeout=60)
            with open(os.path.join(args.out, f"{args.label}-mcp-check.txt"), "w") as handle:
                handle.write(result.stdout + "\n--- stderr ---\n" + result.stderr)
            report.check("opendraft-mcp --check reaches the running program", result.returncode == 0,
                         (result.stdout.strip().splitlines() or ["(no output)"])[0][:160])
        else:
            report.check("opendraft-mcp is next to the program", False, mcp)

        # 6. A clean exit.
        client.close()
        client = None
        if cx is not None:
            xdotool("mousemove", str(cx), str(cy))
        xdotool("key", "ctrl+q")
        try:
            code = app.wait(timeout=20)
            report.check("Ctrl+Q ends the program with exit code 0", code == 0, f"exit code {code}")
        except subprocess.TimeoutExpired:
            shot("quit-stuck")
            report.check("Ctrl+Q ends the program with exit code 0", False, "still running after 20 s")
    except Exception as error:  # noqa: BLE001 - the report must still be written
        report.check("the run finished without an error", False, repr(error))
        try:
            shot("error")
        except Exception:  # noqa: BLE001
            pass
    finally:
        if client:
            client.close()
    return finish(report, app, log, log_path, args, work)


def diagnose(args, work, shot, app):
    """Leave what is needed to understand a program that did not come up: the
    screen, the folders it should have written to, the windows and the
    process."""
    try:
        shot("failed-start")
    except Exception as error:  # noqa: BLE001
        print(f"no screenshot: {error!r}")
    lines = []
    for folder in ("run", "data"):
        lines.append(f"--- find {folder}")
        for root, _dirs, files in os.walk(os.path.join(work, folder)):
            lines.append(root)
            lines.extend("    " + name for name in files)
    lines.append("--- windows")
    lines.append(xdotool("search", "--name", ".", check=False).stdout)
    lines.append(f"--- process: pid {app.pid}, returncode {app.poll()}")
    try:
        with open(f"/proc/{app.pid}/status") as handle:
            lines.append(handle.read())
    except OSError:
        lines.append("(no /proc entry: it has gone)")
    with open(os.path.join(args.out, f"{args.label}-diagnosis.txt"), "w") as handle:
        handle.write("\n".join(lines))
    print("\n".join(lines))


def finish(report, app, log, log_path, args, work):
    if app.poll() is None:
        app.terminate()
        try:
            app.wait(timeout=10)
        except subprocess.TimeoutExpired:
            app.kill()
    log.close()
    summary = {"label": args.label, "scale": args.scale, "checks": report.checks, "notes": report.notes}
    with open(os.path.join(args.out, f"{args.label}-report.json"), "w") as handle:
        json.dump(summary, handle, indent=2)
    if report.failed:
        print(f"\n{len(report.failed)} check(s) failed. The program's log ({log_path}), last lines:")
        with open(log_path, errors="replace") as handle:
            print("".join(handle.readlines()[-40:]))
    shutil.rmtree(work, ignore_errors=True)
    print(f"\n{len(report.checks) - len(report.failed)} of {len(report.checks)} checks passed ({args.label}).")
    return 1 if report.failed else 0


if __name__ == "__main__":
    sys.exit(main())
