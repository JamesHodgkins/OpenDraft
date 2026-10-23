#!/usr/bin/env python3
"""One-off: split the original design sheet (private_material/OD Roman.svg: every
glyph overlaid in one cell, with blue guides naming each advance box) into the
per-glyph files in fonts/od_roman/glyphs/, and write fonts/od_roman/font.cfg.

    python scripts/od_roman_split_sheet.py [--force]

It refuses to touch a folder that already has glyph files unless told to:
after the split the files are the source, and edits to them are the point.

A glyph takes its own guide pair as its advance box, else the default pair;
a narrow glyph with none is fitted to its ink plus FIT_BEARING each side; the
digits all take the default box so columns of numbers line up.
"""
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from od_roman_svg import (  # noqa: E402
    GLYPH_STROKE,
    circle_stroke,
    fmt,
    glyph_file_name,
    glyph_svg,
    read_elements,
    sample,
)

ROOT = Path(__file__).resolve().parent.parent
SHEET = ROOT / "private_material" / "OD Roman.svg"
OUT = ROOT / "fonts" / "od_roman"
GUIDE_STROKE = "#00f"
DOT_MAX_RADIUS = 0.06  # in cap heights: a circle this small is a dot
FIT_BEARING = 115.0  # font units
FIT_BELOW = 0.5  # of the default box
SPACE = 556.0  # font units
SHEET_PEN = 0.1


def codepoint(ident):
    """Codepoint named by an element or group id of the sheet, or None."""
    if not ident:
        return None
    m = re.match(r"U-([0-9A-Fa-f]{4})", ident)
    if m:
        return int(m.group(1), 16)
    m = re.fullmatch(r"_(\d)", ident)
    if m:
        return ord(m.group(1))
    m = re.fullmatch(r"([A-Za-z])\d*", ident)
    if m:
        return ord(m.group(1))
    return None


def main():
    force = "--force" in sys.argv
    glyph_dir = OUT / "glyphs"
    if glyph_dir.exists() and any(glyph_dir.glob("*.svg")) and not force:
        raise SystemExit(f"{glyph_dir} already has glyph files; they are the source now (--force to overwrite)")
    elements = read_elements(SHEET.read_text(encoding="utf8"))

    horizontal, boxes = {}, {}
    for e in elements:
        if e["stroke"] != GUIDE_STROKE or e["kind"] != "path" or not e["id"]:
            continue
        x, y = e["data"][0][1], e["data"][0][2]
        if e["id"] in ("baseline", "x-height", "cap-height"):
            horizontal[e["id"]] = y
        elif e["id"].endswith(("-left", "-right")):
            stem, side = e["id"].rsplit("-", 1)
            boxes.setdefault(stem, {})[side] = x
    for need in ("baseline", "x-height", "cap-height"):
        if need not in horizontal:
            raise SystemExit(f"the sheet has no blue guide named {need}")
    if "default" not in boxes:
        raise SystemExit("the sheet has no default-left / default-right guides")
    baseline, x_line, cap_line = horizontal["baseline"], horizontal["x-height"], horizontal["cap-height"]
    cap = baseline - cap_line
    default = boxes["default"]
    unit_of = lambda x: x / cap * 1000.0

    own = {}
    for stem, box in boxes.items():
        cp = codepoint(stem) if stem != "default" else None
        if cp is not None:
            own[cp] = box

    drawn = {}
    for e in elements:
        if e["stroke"] != GLYPH_STROKE:
            continue
        cp = codepoint(e["id"]) or (codepoint(e["group"]) if not e["id"] else None)
        if cp is None:
            print(f"warning: element {e['id']!r} names no glyph; skipped")
            continue
        drawn.setdefault(cp, []).append(e)

    cfg = {
        "cap_height": round(cap, 3),
        "baseline": baseline,
        "x_height": round(baseline - x_line, 3),
        "pen": SHEET_PEN,
        "view_height": 26.0,
        "default_advance": round(default["right"] - default["left"], 3),
        "weight": 100.0,
        "bold": 1.6,
        "italic": 15.0,
    }
    glyph_dir.mkdir(parents=True, exist_ok=True)
    how = {}
    for cp, items in sorted(drawn.items()):
        strokes, dots, rings, pts = [], [], [], []
        for e in items:
            weight = round((e["width"] or SHEET_PEN) / SHEET_PEN, 2)
            if e["kind"] == "circle":
                cx, cy, r = e["data"]
                if r <= DOT_MAX_RADIUS * cap:
                    dots.append(((cx, cy), weight))
                    pts.append((cx, cy))
                else:
                    rings.append(((cx, cy, r), weight))
                    pts += sample(circle_stroke(cx, cy, r))
            else:
                strokes.append((e["data"], weight))
                pts += sample(e["data"])
        x0, x1 = min(p[0] for p in pts), max(p[0] for p in pts)
        ch = chr(cp)
        if cp in own:
            left, right, kind = own[cp]["left"], own[cp]["right"], "own box"
        elif ch.isdigit():
            left, right, kind = default["left"], default["right"], "digit, default box"
        elif unit_of(x1 - x0) < FIT_BELOW * unit_of(default["right"] - default["left"]):
            gap = FIT_BEARING / 1000.0 * cap
            left, right, kind = x0 - gap, x1 + gap, "fitted"
        else:
            left, right, kind = default["left"], default["right"], "default box"
        how[cp] = kind
        shift = lambda seg: (seg[0], *[v - left if i % 2 == 0 else v for i, v in enumerate(seg[1:])])
        out = [("path", [shift(s) for s in stroke], w) for stroke, w in strokes]
        out += [("dot", (c[0] - left, c[1]), w) for c, w in dots]
        out += [("ring", (c[0] - left, c[1], c[2]), w) for c, w in rings]
        (glyph_dir / glyph_file_name(cp)).write_text(glyph_svg(cp, right - left, cfg, out), encoding="utf8", newline="\n")

    space_width = SPACE / 1000.0 * cap
    (glyph_dir / glyph_file_name(0x20)).write_text(glyph_svg(0x20, space_width, cfg, []), encoding="utf8", newline="\n")

    (OUT / "font.cfg").write_text(
        "# OD Roman: what is the same for every glyph. Lengths are SVG units, the same\n"
        "# as in the glyph files in glyphs/ (each one's viewBox width is its advance).\n"
        f"cap_height {fmt(cfg['cap_height'])}      # baseline to the top of a capital\n"
        f"baseline {fmt(cfg['baseline'])}        # y of the baseline in every glyph file\n"
        f"x_height {fmt(cfg['x_height'])}       # baseline to the top of a lowercase x\n"
        f"view_height {fmt(cfg['view_height'])}      # height of every glyph file's viewBox\n"
        f"pen {fmt(cfg['pen'])}              # the stroke-width of a stroke of weight 1.0\n"
        f"default_advance {fmt(cfg['default_advance'])}  # width a new glyph file starts at\n"
        "weight 100              # default pen width, thousandths of the cap height\n"
        "bold 1.6                # what bold multiplies the pen by\n"
        "italic 15               # the slant italic adds, degrees\n",
        encoding="utf8",
        newline="\n",
    )
    counts = {k: sum(1 for v in how.values() if v == k) for k in sorted(set(how.values()))}
    print(f"wrote {len(how) + 1} glyph files and font.cfg to {OUT}")
    print("spacing:", ", ".join(f"{n} {k}" for k, n in counts.items()))


if __name__ == "__main__":
    main()
