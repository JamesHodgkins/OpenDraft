#!/usr/bin/env python3
"""Build the runtime font crates/od-core/assets/fonts/od_roman.odf from the
glyph files in fonts/od_roman/glyphs/ (one SVG per glyph, see od_roman_svg.py
for what counts as the glyph) and fonts/od_roman/font.cfg.

    python scripts/od_roman_import.py [font folder] [odf]

A glyph file's viewBox width is its advance and its left edge the origin; every
file has the same height and baseline, set in font.cfg. The file name is the
codepoint: `U+0041.svg`. Output coordinates are thousandths of the cap height,
y up, the baseline at 0.

Prints what it found wrong: a file name that is not a codepoint, a glyph with
nothing drawn, ink outside the advance box or closer to its edge than half the
default pen (a heavier pen would overhang the neighbour), a viewBox of another
height, and which ASCII characters have no glyph.
"""
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from od_roman_svg import (  # noqa: E402
    GLYPH_STROKE,
    circle_stroke,
    read_config,
    read_elements,
    sample,
    view_box,
)

ROOT = Path(__file__).resolve().parent.parent
UNIT = 1000.0


def num(v):
    return str(int(round(v)))


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    folder = Path(args[0]) if args else ROOT / "fonts" / "od_roman"
    out = Path(args[1]) if len(args) > 1 else ROOT / "crates/od-core/assets/fonts/od_roman.odf"
    cfg = read_config(folder / "font.cfg")
    cap, baseline, pen = cfg["cap_height"], cfg["baseline"], cfg["pen"]
    half_pen_units = cfg["weight"] / 2.0
    warnings, glyphs = [], {}

    for path in sorted((folder / "glyphs").glob("*.svg")):
        m = re.fullmatch(r"U\+([0-9A-Fa-f]{4,6})\.svg", path.name)
        if not m:
            warnings.append(f"{path.name}: not named U+XXXX.svg; skipped")
            continue
        cp = int(m.group(1), 16)
        text = path.read_text(encoding="utf8")
        _, _, width, height = view_box(text)
        if abs(height - cfg["view_height"]) > 1e-6:
            warnings.append(f"{path.name}: viewBox height {height} is not font.cfg's {cfg['view_height']}")
        strokes, pts = [], []
        for e in (e for e in read_elements(text) if e["stroke"] == GLYPH_STROKE):
            weight = round((e["width"] or pen) / pen, 3)
            if e["kind"] == "circle":
                cx, cy, r = e["data"]
                if e["dot"]:
                    strokes.append(("dot", (cx, cy), weight))
                    pts.append((cx, cy))
                else:
                    ring = circle_stroke(cx, cy, r)
                    strokes.append(("stroke", ring, weight))
                    pts += sample(ring)
            elif len(e["data"]) < 2:
                warnings.append(f"{path.name}: a path with nothing in it")
            else:
                strokes.append(("stroke", e["data"], weight))
                pts += sample(e["data"])
        if not strokes and cp != 0x20:
            warnings.append(f"{path.name}: nothing drawn (a red path or circle is the glyph)")
        if pts:
            left = min(p[0] for p in pts) / cap * UNIT
            right = (width - max(p[0] for p in pts)) / cap * UNIT
            name = f"{chr(cp)!r} (U+{cp:04X})" if chr(cp).isprintable() else f"U+{cp:04X}"
            if min(left, right) < 0:
                warnings.append(f"{name}: ink outside its advance box by {-min(left, right):.0f}")
            elif min(left, right) < half_pen_units:
                warnings.append(
                    f"{name}: only {min(left, right):.0f} between ink and the box edge"
                    f" (half the default pen is {half_pen_units:.0f})"
                )
        glyphs[cp] = (width, strokes, pts)

    if 0x20 not in glyphs:
        raise SystemExit("no U+0020.svg: the space needs a file (an empty glyph as wide as the space)")

    def body(cp):
        width, strokes, _ = glyphs[cp]
        sx = lambda x: x / cap * UNIT
        sy = lambda y: (baseline - y) / cap * UNIT
        lines = [f"glyph {cp:04X} adv={num(width / cap * UNIT)}"]
        for kind, data, weight in strokes:
            w = "" if abs(weight - 1.0) < 1e-3 else f"w={weight:g} "
            if kind == "dot":
                lines.append(f"  dot {w}{num(sx(data[0]))} {num(sy(data[1]))}")
            else:
                parts = []
                for seg in data:
                    pairs = [f"{num(sx(seg[i]))} {num(sy(seg[i + 1]))}" for i in range(1, len(seg), 2)]
                    parts.append(f"{seg[0]} " + " ".join(pairs))
                lines.append(f"  stroke {w}" + "  ".join(parts))
        return lines

    depth = lambda cp: (max(p[1] for p in glyphs[cp][2]) - baseline) / cap * UNIT
    descender = num(max((depth(ord(c)) for c in "gjpqy" if ord(c) in glyphs and glyphs[ord(c)][2]), default=340))
    space = glyphs[0x20][0] / cap * UNIT
    lines = [
        "odroman 1",
        f"metrics unit={int(UNIT)} cap={int(UNIT)} xheight={num(cfg['x_height'] / cap * UNIT)}"
        f" descender={descender} space={num(space)} weight={num(cfg['weight'])}"
        f" bold={cfg['bold']:g} italic={cfg['italic']:g}",
    ]
    for cp in sorted(glyphs):
        lines += body(cp)
    out.write_text("\n".join(lines) + "\n", encoding="utf8", newline="\n")

    missing = "".join(chr(c) for c in range(33, 127) if c not in glyphs)
    print(f"wrote {out} ({len(glyphs)} glyphs)")
    for w in warnings:
        print("warning:", w)
    print("ASCII with no glyph:", missing or "none")

    if "--no-preview" not in sys.argv:
        # The viewer and the overview are cheap, and are how a change is checked.
        from od_roman_preview import generate
        from od_roman_viewer import write_viewer

        preview = folder / "preview"
        generate(folder, preview)
        page = write_viewer(folder, preview / "index.html", warnings)
        print(f"viewer: {page.resolve().as_uri()}")


if __name__ == "__main__":
    main()
