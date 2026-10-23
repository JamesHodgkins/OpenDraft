#!/usr/bin/env python3
"""Draw OD Roman from its glyph files, to judge shapes and spacing.

    python scripts/od_roman_preview.py [font folder] [output folder] [--weight 0.10]

Writes to fonts/od_roman/preview/ (generated, safe to delete):

  overview.svg  every glyph in its advance box, at the default pen
  specimen.svg  sample lines set at the real advances, so you can see rhythm

and the same as .png when Pillow is installed. `--weight` is the pen as a
fraction of the cap height (default: font.cfg's `weight`).
"""
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from od_roman_svg import GLYPH_STROKE, circle_stroke, fmt, read_config, read_elements, view_box  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
SAMPLES = [
    "ABCDEFGHIJKLMNOPQRSTUVWXYZ",
    "abcdefghijklmnopqrstuvwxyz",
    "0123456789 .,:;!?&#()'\"*+-=_°",
    "THE QUICK BROWN FOX JUMPS OVER THE LAZY DOG",
    "the quick brown fox jumps over the lazy dog",
    "Pack my box with five dozen liquor jugs 1,234.56",
]


def flatten(stroke):
    """A stroke as a list of points, curves cut into short pieces."""
    pts, cur = [], None
    for seg in stroke:
        if seg[0] in "ML":
            cur = (seg[1], seg[2])
            pts.append(cur)
        else:
            p0, p1, p2, p3 = cur, seg[1:3], seg[3:5], seg[5:7]
            for i in range(1, 17):
                t, u = i / 16, 1 - i / 16
                pts.append(
                    tuple(
                        u**3 * p0[a] + 3 * u * u * t * p1[a] + 3 * u * t * t * p2[a] + t**3 * p3[a]
                        for a in (0, 1)
                    )
                )
            cur = p3
    return pts


def load(folder, cfg):
    """{codepoint: (advance width, [('line', points, weight) | ('dot', point, weight)])}"""
    pen = cfg["pen"]
    glyphs = {}
    for path in sorted((folder / "glyphs").glob("*.svg")):
        m = re.fullmatch(r"U\+([0-9A-Fa-f]{4,6})\.svg", path.name)
        if not m:
            continue
        text = path.read_text(encoding="utf8")
        width = view_box(text)[2]
        prims = []
        for e in (e for e in read_elements(text) if e["stroke"] == GLYPH_STROKE):
            weight = (e["width"] or pen) / pen
            if e["kind"] == "circle":
                cx, cy, r = e["data"]
                if e["dot"]:
                    prims.append(("dot", (cx, cy), weight))
                else:
                    prims.append(("line", flatten(circle_stroke(cx, cy, r)), weight))
            elif len(e["data"]) >= 2:
                prims.append(("line", flatten(e["data"]), weight))
        glyphs[int(m.group(1), 16)] = (width, prims)
    return glyphs


class Canvas:
    """Collects what to draw, in SVG units, and writes it as SVG or PNG."""

    def __init__(self, width, height):
        self.width, self.height = width, height
        self.items = []

    def rect(self, x, y, w, h, colour, fill=None):
        self.items.append(("rect", (x, y, w, h), colour, fill))

    def line(self, points, width, colour):
        self.items.append(("line", points, width, colour))

    def dot(self, point, radius, colour):
        self.items.append(("dot", point, radius, colour))

    def text(self, x, y, size, content, colour):
        self.items.append(("text", (x, y), size, content, colour))

    def svg(self):
        out = [
            f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {fmt(self.width)} {fmt(self.height)}"'
            f' width="{fmt(self.width * 4)}" height="{fmt(self.height * 4)}">',
            '<rect width="100%" height="100%" fill="#fff"/>',
        ]
        for item in self.items:
            if item[0] == "rect":
                (x, y, w, h), colour, fill = item[1:]
                out.append(
                    f'<rect x="{fmt(x)}" y="{fmt(y)}" width="{fmt(w)}" height="{fmt(h)}"'
                    f' fill="{fill or "none"}" stroke="{colour}" stroke-width="0.08"/>'
                )
            elif item[0] == "line":
                pts = " ".join(f"{fmt(x)},{fmt(y)}" for x, y in item[1])
                out.append(
                    f'<polyline points="{pts}" fill="none" stroke="{item[3]}" stroke-width="{fmt(item[2])}"'
                    ' stroke-linecap="round" stroke-linejoin="round"/>'
                )
            elif item[0] == "dot":
                out.append(f'<circle cx="{fmt(item[1][0])}" cy="{fmt(item[1][1])}" r="{fmt(item[2])}" fill="{item[3]}"/>')
            else:
                (x, y), size, content, colour = item[1:]
                out.append(
                    f'<text x="{fmt(x)}" y="{fmt(y)}" font-family="sans-serif" font-size="{fmt(size)}"'
                    f' fill="{colour}">{content.replace("&", "&amp;").replace("<", "&lt;")}</text>'
                )
        out.append("</svg>")
        return "\n".join(out) + "\n"

    def png(self, path, scale):
        from PIL import Image, ImageDraw

        image = Image.new("RGB", (int(self.width * scale), int(self.height * scale)), "white")
        draw = ImageDraw.Draw(image)
        p = lambda pt: (pt[0] * scale, pt[1] * scale)
        for item in self.items:
            if item[0] == "rect":
                (x, y, w, h), colour, fill = item[1:]
                draw.rectangle([x * scale, y * scale, (x + w) * scale, (y + h) * scale], outline=colour, fill=fill)
            elif item[0] == "line":
                width = max(1, round(item[2] * scale))
                pts = [p(q) for q in item[1]]
                draw.line(pts, fill=item[3], width=width, joint="curve")
                r = width / 2
                for x, y in (pts[0], pts[-1]):
                    draw.ellipse([x - r, y - r, x + r, y + r], fill=item[3])
            elif item[0] == "dot":
                x, y = p(item[1])
                r = max(1, item[2] * scale)
                draw.ellipse([x - r, y - r, x + r, y + r], fill=item[3])
            else:
                (x, y), size, content, colour = item[1:]
                draw.text(p((x, y - size)), content, fill=colour)
        image.save(path)


def place(canvas, prims, ox, oy, pen, colour="#000"):
    """Draw a glyph with its origin at (ox, oy) of the canvas."""
    for kind, data, weight in prims:
        if kind == "dot":
            canvas.dot((ox + data[0], oy + data[1]), pen * weight / 2, colour)
        else:
            canvas.line([(ox + x, oy + y) for x, y in data], pen * weight, colour)


def main():
    values = {sys.argv[i + 1] for i, a in enumerate(sys.argv[:-1]) if a.startswith("--")}
    args = [a for a in sys.argv[1:] if not a.startswith("--") and a not in values]
    weight = float(sys.argv[sys.argv.index("--weight") + 1]) if "--weight" in sys.argv else None
    folder = Path(args[0]) if args else ROOT / "fonts" / "od_roman"
    generate(folder, Path(args[1]) if len(args) > 1 else folder / "preview", weight)


def generate(folder, out, weight=None):
    """Write overview.svg and specimen.svg (and .png with Pillow) to `out`."""
    cfg = read_config(folder / "font.cfg")
    glyphs = load(folder, cfg)
    pen_units = (weight if weight is not None else cfg["weight"] / 1000.0) * cfg["cap_height"]
    cap, base, xh, height = cfg["cap_height"], cfg["baseline"], cfg["x_height"], cfg["view_height"]
    out.mkdir(parents=True, exist_ok=True)

    # Overview: every glyph in its advance box, rows wrapped at a fixed width.
    gap, row_w, label = 1.5, 330.0, 3.0
    x = y = 0.0
    placed = []
    for cp in sorted(glyphs):
        width = glyphs[cp][0]
        if x + width > row_w and x > 0:
            x, y = 0.0, y + height + label + gap
        placed.append((cp, x, y))
        x += width + gap
    canvas = Canvas(row_w, y + height + label + gap)
    for cp, ox, oy in placed:
        width, prims = glyphs[cp]
        canvas.rect(ox, oy, width, height, "#bbb")
        for line_y in (base - cap, base - xh, base):
            canvas.line([(ox, oy + line_y), (ox + width, oy + line_y)], 0.05, "#7cf")
        place(canvas, [(k, d if k == "dot" else [(a, b) for a, b in d], w) for k, d, w in prims], ox, oy, pen_units)
        shown = chr(cp) if chr(cp).isprintable() else ""
        canvas.text(ox, oy + height + label - 0.6, 2.4, f"{shown} {cp:04X}".strip(), "#555")
    (out / "overview.svg").write_text(canvas.svg(), encoding="utf8", newline="\n")
    overview = canvas

    # Specimen: lines of text at the real advances.
    line_h, margin = height * 1.05, 4.0
    widest = max(sum(glyphs[ord(c)][0] if ord(c) in glyphs else 0 for c in text) for text in SAMPLES)
    spec = Canvas(widest + 2 * margin, margin * 2 + line_h * len(SAMPLES))
    for i, text in enumerate(SAMPLES):
        ox, oy = margin, margin + i * line_h - (base - cap) + 2.0
        for c in text:
            if ord(c) in glyphs:
                width, prims = glyphs[ord(c)]
                place(spec, prims, ox, oy, pen_units)
                ox += width
    (out / "specimen.svg").write_text(spec.svg(), encoding="utf8", newline="\n")
    print(f"wrote {out / 'overview.svg'} and {out / 'specimen.svg'} ({len(glyphs)} glyphs, pen {pen_units / cap:.3f} of the cap height)")
    try:
        overview.png(out / "overview.png", 6)
        spec.png(out / "specimen.png", 8)
        print("and the same as .png")
    except ImportError:
        print("(install Pillow for .png copies)")


if __name__ == "__main__":
    main()
