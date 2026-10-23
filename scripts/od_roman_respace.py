#!/usr/bin/env python3
"""Change the side margins of glyph files: widen each artboard and slide the
drawing, without touching the shapes.

    python scripts/od_roman_respace.py A-Z --by 45 --min 120
    python scripts/od_roman_respace.py "aeo" --by 30
    python scripts/od_roman_respace.py U+0057 --left 20 --right 20 --min 0

Which glyphs: characters and `X-Y` ranges (`A-Z`), or `U+XXXX`, in one argument.
Margins are in font units (thousandths of the cap height), measured from the
drawing's ink to the artboard edge: each side becomes max(old + --by, --min),
and --left / --right add to one side only. Use a negative --by to tighten.

Rewrites each file through the glyph template, so only the red drawing is kept
(guides are redrawn, anything else in the file is dropped); a file with no
drawing is skipped. Run od_roman_preview.py afterwards to look at the result.
"""
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from od_roman_svg import (  # noqa: E402
    GLYPH_STROKE,
    circle_stroke,
    glyph_file_name,
    glyph_svg,
    read_config,
    read_elements,
    sample,
    view_box,
)

ROOT = Path(__file__).resolve().parent.parent
FOLDER = ROOT / "fonts" / "od_roman"


def codepoints(spec):
    out = []
    for token in re.findall(r"U\+[0-9A-Fa-f]{4,6}|.-.|.", spec):
        if token.upper().startswith("U+") and len(token) > 2:
            out.append(int(token[2:], 16))
        elif len(token) == 3 and token[1] == "-":
            out += range(ord(token[0]), ord(token[2]) + 1)
        else:
            out += [ord(c) for c in token]
    return out


def option(name, default=0.0):
    if name in sys.argv:
        return float(sys.argv[sys.argv.index(name) + 1])
    return default


def main():
    values = {sys.argv[i + 1] for i, a in enumerate(sys.argv[:-1]) if a.startswith("--")}
    args = [a for a in sys.argv[1:] if not a.startswith("--") and a not in values]
    if not args:
        raise SystemExit(__doc__)
    by, left_by, right_by, minimum = option("--by"), option("--left"), option("--right"), option("--min", 0.0)
    cfg = read_config(FOLDER / "font.cfg")
    cap, pen = cfg["cap_height"], cfg["pen"]
    unit = lambda font_units: font_units / 1000.0 * cap

    for cp in codepoints(args[0]):
        path = FOLDER / "glyphs" / glyph_file_name(cp)
        if not path.exists():
            print(f"skip U+{cp:04X}: no file")
            continue
        text = path.read_text(encoding="utf8")
        width = view_box(text)[2]
        elements = []
        pts = []
        for e in (e for e in read_elements(text) if e["stroke"] == GLYPH_STROKE):
            weight = (e["width"] or pen) / pen
            if e["kind"] == "circle":
                cx, cy, r = e["data"]
                if e["dot"]:
                    elements.append(("dot", (cx, cy), weight))
                    pts.append((cx, cy))
                else:
                    elements.append(("ring", (cx, cy, r), weight))
                    pts += sample(circle_stroke(cx, cy, r))
            elif len(e["data"]) >= 2:
                elements.append(("path", e["data"], weight))
                pts += sample(e["data"])
        if not pts:
            print(f"skip U+{cp:04X}: nothing drawn")
            continue
        x0, x1 = min(p[0] for p in pts), max(p[0] for p in pts)
        new_left = max(x0 + unit(by + left_by), unit(minimum))
        new_right = max((width - x1) + unit(by + right_by), unit(minimum))
        shift = new_left - x0

        def moved(kind, data, weight):
            if kind == "path":
                return kind, [(s[0], *[v + shift if i % 2 == 0 else v for i, v in enumerate(s[1:])]) for s in data], weight
            return kind, (data[0] + shift, *data[1:]), weight

        new_width = new_left + (x1 - x0) + new_right
        path.write_text(glyph_svg(cp, new_width, cfg, [moved(*e) for e in elements]), encoding="utf8", newline="\n")
        f = lambda v: round(v / cap * 1000)
        print(f"{chr(cp)!r} U+{cp:04X}: margins {f(x0)}/{f(width - x1)} -> {f(new_left)}/{f(new_right)}, advance {f(width)} -> {f(new_width)}")


if __name__ == "__main__":
    main()
