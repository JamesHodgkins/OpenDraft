#!/usr/bin/env python3
"""End-to-end smoke test of OpenDraft on a Linux desktop (X11 or Wayland).

Run it inside a running display: scripts/linux_smoke.sh starts a virtual X
server, scripts/linux_smoke_wayland.sh a headless Wayland compositor (sway). It
starts the real program with a throwaway profile, home and runtime folder, and
drives it the way a person would, then reads the result back through the
automation socket (the same one the MCP server uses; OD_AUTOMATION=1 turns it
on without ticking the box in Settings):

  1. the program starts and the automation socket appears;
  2. the drawing named on the command line is open and has objects;
  3. the window draws something (the screen is not blank);
  4. typing L, Enter, two clicks and Enter draws one more line, and Ctrl+Z
     takes it back (keyboard, mouse, command, undo, repaint);
  5. `opendraft-mcp --check` reaches the running program.

A second scenario (--scenario home) starts the program with no drawing, as a
person does from the menu, and checks it starts on the Home page with nothing
open and ends with exit code 0 when Ctrl+Q is pressed. (The first scenario
cannot do that: a drawing imported from DXF counts as unsaved, so quitting asks
whether to save it.)

Screenshots, the program's log and a JSON report are left in --out. Only the
Python standard library is used, plus `xdotool` (X11) or `swaymsg`, `wtype` and
`grim` (Wayland) for input and screenshots.
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


# ------------------------------------------------------------------ pictures

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


def read_ppm(data):
    """Read a binary PPM (P6), which `grim -t ppm` writes."""
    fields = []
    position = 0
    while len(fields) < 4:
        while data[position:position + 1].isspace():
            position += 1
        if data[position:position + 1] == b"#":
            position = data.index(b"\n", position)
            continue
        end = position
        while not data[end:end + 1].isspace():
            end += 1
        fields.append(data[position:end])
        position = end
    magic, width, height, top = fields
    if magic != b"P6" or int(top) != 255:
        raise RuntimeError(f"unsupported picture: {magic!r} {top!r}")
    width, height = int(width), int(height)
    start = position + 1
    return width, height, data[start:start + width * height * 3]


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


# ------------------------------------------------------------- the displays

def run(command, check=True, **kwargs):
    return subprocess.run(command, check=check, capture_output=True, **kwargs)


class X11Session:
    """A virtual X server (Xvfb) with no window manager. Input is sent with
    xdotool; the screen is read from the server's framebuffer file."""

    name = "x11"
    has_input = True

    def __init__(self, args):
        self.fbdir = args.fbdir

    def environment(self, args):
        return {"WINIT_X11_SCALE_FACTOR": args.scale} if args.scale else {}

    def screenshot(self):
        if not self.fbdir:
            return None
        files = glob.glob(os.path.join(self.fbdir, "Xvfb_screen*"))
        return read_xwd(files[0]) if files else None

    def find_window(self):
        found = run(["xdotool", "search", "--name", WINDOW_TITLE], check=False, text=True).stdout.split()
        if not found:
            return None
        window = found[0]
        geometry = dict(line.split("=") for line in
                        run(["xdotool", "getwindowgeometry", "--shell", window], text=True).stdout.split())
        return {"id": window, **{k.lower(): int(geometry[k]) for k in ("X", "Y", "WIDTH", "HEIGHT")}}

    def focus(self, window):
        # With no window manager nothing gives a window the keyboard focus, as a
        # manager does when it is clicked: do it here, or the keys go nowhere.
        run(["xdotool", "windowfocus", window["id"]], check=False)

    def move(self, x, y):
        run(["xdotool", "mousemove", str(x), str(y)])

    def click(self):
        # Press, hold a moment, release, as a hand does: an instant click
        # arrives as one frame's press and release, which a program that
        # reads them frame by frame can miss.
        run(["xdotool", "mousedown", "1"])
        time.sleep(0.12)
        run(["xdotool", "mouseup", "1"])

    def type_text(self, text):
        run(["xdotool", "type", "--delay", "120", text])

    def key(self, chord):
        """A key or a chord such as "Return" or "ctrl+q"."""
        run(["xdotool", "key", chord])

    def end(self):
        """Ask the program to quit the way a person does: Ctrl+Q."""
        self.key("ctrl+q")

    def diagnostics(self):
        return run(["xdotool", "search", "--name", "."], check=False, text=True).stdout


class WaylandSession:
    """A headless sway; the screen is read with grim.

    Keyboard and mouse input is not sent. A seat only has a keyboard or a
    pointer while a device of that kind exists, and a headless sway has none;
    wtype and swaymsg make one for the length of one command, so the program
    sees its keyboard and pointer come and go and the keys are lost. Instead the
    program is shown, loaded, read through the automation socket and asked to
    close by the compositor, which is what a window manager's close button does.
    """

    name = "wayland"
    has_input = False

    def __init__(self, args):
        pass

    def environment(self, args):
        # winit uses Wayland when WAYLAND_DISPLAY is set and falls back to X11
        # when it is not; main() removes DISPLAY, so there is nothing to fall
        # back to and a pass means the Wayland path itself worked.
        return {}

    def screenshot(self):
        result = run(["grim", "-t", "ppm", "-"], check=False)
        return read_ppm(result.stdout) if result.returncode == 0 and result.stdout else None

    def _tree(self):
        return json.loads(run(["swaymsg", "-t", "get_tree", "-r"], text=True).stdout)

    def find_window(self):
        stack = [self._tree()]
        while stack:
            node = stack.pop()
            stack.extend(node.get("nodes", []) + node.get("floating_nodes", []))
            if node.get("name") == WINDOW_TITLE or node.get("app_id") == "opendraft-ci-smoke":
                rect = node["rect"]
                return {"id": node.get("id"), "x": rect["x"], "y": rect["y"],
                        "width": rect["width"], "height": rect["height"]}
        return None

    def focus(self, window):
        run(["swaymsg", f'[title="{WINDOW_TITLE}"] focus'], check=False)

    def move(self, x, y):
        run(["swaymsg", "seat", "-", "cursor", "set", str(x), str(y)])

    def click(self):
        run(["swaymsg", "seat", "-", "cursor", "press", "button1"])
        time.sleep(0.12)
        run(["swaymsg", "seat", "-", "cursor", "release", "button1"])

    def type_text(self, text):
        run(["wtype", "-d", "120", text])

    def key(self, chord):
        parts = chord.split("+")
        modifiers, key = parts[:-1], parts[-1]
        command = ["wtype"]
        for modifier in modifiers:
            command += ["-M", modifier]
        command += ["-k", key]
        for modifier in reversed(modifiers):
            command += ["-m", modifier]
        run(command)

    def end(self):
        """Ask the program to close its window, as a close button does."""
        run(["swaymsg", f'[title="{WINDOW_TITLE}"] kill'], check=False)

    def diagnostics(self):
        return run(["swaymsg", "-t", "get_tree"], check=False, text=True).stdout


def wait_for(predicate, seconds, step=0.25):
    end = time.time() + seconds
    while time.time() < end:
        value = predicate()
        if value:
            return value
        time.sleep(step)
    return None


# --------------------------------------------------------------- the driver

def diagnose(args, work, shot, app, display):
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
    try:
        lines.append(display.diagnostics())
    except Exception as error:  # noqa: BLE001
        lines.append(f"(could not list them: {error!r})")
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
    summary = {"label": args.label, "session": args.session, "scale": args.scale,
               "checks": report.checks, "notes": report.notes}
    with open(os.path.join(args.out, f"{args.label}-report.json"), "w") as handle:
        json.dump(summary, handle, indent=2)
    if report.failed:
        print(f"\n{len(report.failed)} check(s) failed. The program's log ({log_path}), last lines:")
        with open(log_path, errors="replace") as handle:
            print("".join(handle.readlines()[-40:]))
    shutil.rmtree(work, ignore_errors=True)
    print(f"\n{len(report.checks) - len(report.failed)} of {len(report.checks)} checks passed ({args.label}).")
    return 1 if report.failed else 0


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--binary", required=True, help="path of the opendraft program")
    parser.add_argument("--mcp", help="path of opendraft-mcp (default: next to the program)")
    parser.add_argument("--sample", required=True, help="a drawing to open")
    parser.add_argument("--out", required=True, help="folder for screenshots, log and report")
    parser.add_argument("--session", choices=["x11", "wayland"], default="x11")
    parser.add_argument("--fbdir", default=os.environ.get("SMOKE_FBDIR"),
                        help="the Xvfb -fbdir, for screenshots (X11)")
    parser.add_argument("--scale", help="set WINIT_X11_SCALE_FACTOR (for example 2; X11)")
    parser.add_argument("--label", default="smoke", help="names the artifacts")
    parser.add_argument("--scenario", choices=["drawing", "home"], default="drawing")
    args = parser.parse_args()

    display = X11Session(args) if args.session == "x11" else WaylandSession(args)
    os.makedirs(args.out, exist_ok=True)
    report = Report()
    work = tempfile.mkdtemp(prefix="od-smoke-")
    home = os.path.join(work, "home")
    data = os.path.join(work, "data")
    runtime = os.path.join(work, "run")
    for folder in (home, data, runtime):
        os.makedirs(folder)
    os.chmod(runtime, 0o700)
    sample = os.path.join(work, os.path.basename(args.sample))
    shutil.copy(args.sample, sample)

    env = dict(os.environ)
    env.update({
        "HOME": home,
        "XDG_DATA_HOME": data,
        "XDG_CONFIG_HOME": os.path.join(home, ".config"),
        "XDG_CACHE_HOME": os.path.join(home, ".cache"),
        "XDG_RUNTIME_DIR": runtime,
        "OPENDRAFT_PROFILE": PROFILE,
        "LIBGL_ALWAYS_SOFTWARE": "1",
        "RUST_BACKTRACE": "1",
        "RUST_LOG": "info",
        # Turns the automation socket on without ticking the box in Settings.
        "OD_AUTOMATION": "1",
    })
    env.update(display.environment(args))
    if args.session == "wayland":
        # The program's own runtime folder is private to the test, so the
        # compositor's socket is linked into it, and X11 is out of reach.
        env.pop("DISPLAY", None)
        wayland = os.environ["WAYLAND_DISPLAY"]
        os.symlink(os.path.join(os.environ["XDG_RUNTIME_DIR"], wayland), os.path.join(runtime, wayland))

    log_path = os.path.join(args.out, f"{args.label}-app.log")
    log = open(log_path, "wb")
    command = [args.binary] + ([sample] if args.scenario == "drawing" else [])
    app = subprocess.Popen(command, env=env, stdout=log, stderr=subprocess.STDOUT)
    client = None

    def shot(name):
        picture = display.screenshot()
        if picture is None:
            return None
        write_png(os.path.join(args.out, f"{args.label}-{name}.png"), *picture)
        return picture

    try:
        # 1. The program starts and the automation socket appears.
        pattern = os.path.join(runtime, "OpenDraft", "automation", "*.json")
        descriptor_file = wait_for(
            lambda: (glob.glob(pattern) or [None])[0] if app.poll() is None else "gone", 90)
        if descriptor_file in (None, "gone"):
            code = app.poll()
            report.check("the program starts and offers the automation socket", False,
                         f"it exited with code {code}" if descriptor_file == "gone" else "no socket after 90 s")
            diagnose(args, work, shot, app, display)
            return finish(report, app, log, log_path, args, work)
        report.check("the program starts and offers the automation socket", True)
        with open(descriptor_file) as handle:
            descriptor = json.load(handle)
        client = Client(descriptor)
        report.check("the hello is accepted with a read-only grant", client.hello["scopes"] == ["drawing.read"],
                     client.hello.get("scopes"))

        doc_id = None
        before = 0
        if args.scenario == "home":
            # 2. Nothing is open: the program starts on the Home page.
            time.sleep(3)
            nothing = client.call("documents.list")["documents"]
            report.check("it starts on the Home page with no drawing open", nothing == [], json.dumps(nothing)[:160])

            def count():
                return 0
        else:
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
            report.check("a screenshot could be taken", False, "the display gave no picture")

        # 4. Keyboard and mouse.
        window = wait_for(display.find_window, 30)
        if not window:
            report.check("the window is on screen", False, f"no window named {WINDOW_TITLE!r}")
        else:
            report.notes["window"] = window
            report.check("the window is on screen and a sensible size",
                         window["width"] >= 600 and window["height"] >= 400,
                         f"{window['width']}x{window['height']}")
            cx = window["x"] + window["width"] // 2
            cy = window["y"] + int(window["height"] * 0.55)
            if display.has_input:
                display.move(cx, cy)
                display.focus(window)
                time.sleep(0.5)
            else:
                report.notes["input"] = "not exercised on this session"
            if args.scenario == "drawing" and display.has_input:
                display.type_text("L")
                display.key("Return")
                time.sleep(1.0)
                display.move(cx - 120, cy - 40)
                time.sleep(0.4)
                display.click()
                time.sleep(0.6)
                shot("first-point")
                display.move(cx + 140, cy + 60)
                time.sleep(0.4)
                display.click()
                time.sleep(0.6)
                display.key("Return")
                drawn = wait_for(lambda: count() == before + 1 or None, 15)
                report.check("typing L, two clicks and Enter draws one line", bool(drawn),
                             f"{before} -> {count()} objects")
                time.sleep(1.0)
                second = shot("drawn")
                if first and second and (first[0], first[1]) == (second[0], second[1]):
                    changed = differing_pixels(first[2], second[2], first[0])
                    report.check("the new line is painted", changed > 50, f"{changed} pixels changed")
                if drawn:
                    newest = client.call("entities.query", {"document_id": doc_id, "limit": 1000})["entities"][-1]
                    report.check("the new object is a line", str(newest.get("kind", "")).lower() == "line",
                                 newest.get("kind"))
                display.key("ctrl+z")
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

        # 6. The end of the run.
        client.close()
        client = None
        if args.scenario == "home":
            # Open asks the desktop for a file dialog. This session has no
            # desktop portal and no zenity, so none can be shown and the status
            # bar should say why (see picker.rs); the picture is kept to look at.
            if display.has_input:
                display.key("ctrl+o")
                time.sleep(2)
                shot("open-without-a-dialog")
            way = "Ctrl+Q" if display.has_input else "closing the window"
            display.end()
            try:
                code = app.wait(timeout=20)
                report.check(f"{way} ends the program with exit code 0", code == 0, f"exit code {code}")
            except subprocess.TimeoutExpired:
                shot("quit-stuck")
                report.check(f"{way} ends the program with exit code 0", False, "still running after 20 s")
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


if __name__ == "__main__":
    sys.exit(main())
