"""Shared by the OD Roman scripts: reading the SVG a glyph is drawn in, and
writing a glyph file.

A glyph file (fonts/od_roman/glyphs/U+0041.svg) is an SVG whose viewBox is
the glyph's advance box: the width is the advance, the left edge the origin.
Every file has the same height and baseline (fonts/od_roman/font.cfg), so a
y value means the same in all of them. What counts as the glyph:

  * red (stroke #f00) <path>s, drawn with round caps: one stroke each;
  * red <circle class="dot">: a dot, one pen laid down;
  * any other red <circle>: a ring.

A stroke's `stroke-width` is its weight: the font's `pen` is a weight of 1.0,
half of it a stroke drawn at half the pen. Anything not red (the guides, the
background) is ignored.
"""
import re
from pathlib import Path

KAPPA = 0.5522847498
TOKEN = re.compile(r"[A-Za-z]|[-+]?(?:\d+\.?\d*|\.\d+)(?:[eE][-+]?\d+)?")
GLYPH_STROKE = "#f00"


def parse_path(d):
    """Path data as one stroke: [('M',x,y), ('L',x,y), ('C',x1,y1,x2,y2,x,y)]."""
    toks = TOKEN.findall(d)
    out, k, cmd = [], 0, None
    x = y = sx = sy = 0.0
    prev_c2 = None
    arity = {"M": 2, "L": 2, "H": 1, "V": 1, "C": 6, "S": 4}
    while k < len(toks):
        if toks[k].isalpha():
            cmd = toks[k]
            k += 1
            if cmd in "Zz":
                if (x, y) != (sx, sy):
                    out.append(("L", sx, sy))
                x, y = sx, sy
                prev_c2 = None
            continue
        up = cmd.upper()
        if up not in arity:
            raise ValueError(f"unsupported path command {cmd!r}")
        v = [float(t) for t in toks[k : k + arity[up]]]
        k += arity[up]
        rel = cmd.islower()
        ox, oy = (x, y) if rel else (0.0, 0.0)
        if up in "ML":
            x, y = ox + v[0], oy + v[1]
            if up == "M":
                sx, sy = x, y
                cmd = "l" if rel else "L"
            out.append(("M" if up == "M" and not out else "L", x, y))
            prev_c2 = None
        elif up == "H":
            x = ox + v[0]
            out.append(("L", x, y))
            prev_c2 = None
        elif up == "V":
            y = oy + v[0]
            out.append(("L", x, y))
            prev_c2 = None
        elif up == "C":
            c1 = (ox + v[0], oy + v[1])
            c2 = (ox + v[2], oy + v[3])
            x, y = ox + v[4], oy + v[5]
            out.append(("C", *c1, *c2, x, y))
            prev_c2 = c2
        else:  # S: first control is the reflection of the last second control
            c1 = (2 * x - prev_c2[0], 2 * y - prev_c2[1]) if prev_c2 else (x, y)
            c2 = (ox + v[0], oy + v[1])
            x, y = ox + v[2], oy + v[3]
            out.append(("C", *c1, *c2, x, y))
            prev_c2 = c2
    return out


def circle_stroke(cx, cy, r):
    """A ring as one closed stroke of four cubics."""
    k = KAPPA * r
    return [
        ("M", cx + r, cy),
        ("C", cx + r, cy + k, cx + k, cy + r, cx, cy + r),
        ("C", cx - k, cy + r, cx - r, cy + k, cx - r, cy),
        ("C", cx - r, cy - k, cx - k, cy - r, cx, cy - r),
        ("C", cx + k, cy - r, cx + r, cy - k, cx + r, cy),
    ]


def sample(stroke):
    """Points along a stroke, enough for a bounding box."""
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


def style_of(attrs):
    """The presentation attributes of an element, `style` included."""
    out = {}
    for m in re.finditer(r'(?<![\w:-])([a-z-]+)="([^"]*)"', attrs):
        out[m.group(1)] = m.group(2)
    for part in out.get("style", "").split(";"):
        if ":" in part:
            key, value = part.split(":", 1)
            out[key.strip()] = value.strip()
    return out


def number(text, default=None):
    m = re.match(r"\s*([-+]?(?:\d+\.?\d*|\.\d+)(?:[eE][-+]?\d+)?)", text or "")
    return float(m.group(1)) if m else default


def read_elements(text):
    """Every <path> and <circle> in the file, in order, as dicts: id, group
    (the innermost <g>'s id), kind, data (a stroke, or [cx, cy, r]), stroke
    (colour, lower case), width (stroke-width), dot (class="dot")."""
    found, groups = [], []
    for m in re.finditer(r"<g\b([^>]*)>|</g>|<(path|circle)\b([^>]*?)/?>", text):
        if m.group(0) == "</g>":
            if groups:
                groups.pop()
        elif m.group(1) is not None:
            gid = re.search(r'\bid="([^"]*)"', m.group(1))
            groups.append(gid.group(1) if gid else None)
        else:
            st = style_of(m.group(3))
            if m.group(2) == "path":
                kind, data = "path", parse_path(st.get("d", ""))
            else:
                kind = "circle"
                data = [number(st.get(k), 0.0) for k in ("cx", "cy", "r")]
            found.append(
                {
                    "id": st.get("id"),
                    "group": groups[-1] if groups else None,
                    "kind": kind,
                    "data": data,
                    "stroke": (st.get("stroke") or "").lower() or None,
                    "width": number(st.get("stroke-width"), None),
                    "dot": "dot" in st.get("class", "").split(),
                }
            )
    return found


def view_box(text):
    """(min x, min y, width, height) of the root <svg>."""
    m = re.search(r'viewBox="([^"]*)"', text)
    if not m:
        raise ValueError("no viewBox")
    parts = [float(v) for v in re.split(r"[\s,]+", m.group(1).strip())]
    if len(parts) != 4:
        raise ValueError("bad viewBox")
    return tuple(parts)


def read_config(path):
    """font.cfg: `key value` lines, `#` comments."""
    cfg = {}
    for line in Path(path).read_text(encoding="utf8").splitlines():
        line = line.split("#")[0].strip()
        if line:
            key, value = line.split(None, 1)
            cfg[key] = float(value)
    return cfg


def fmt(v):
    text = f"{v:.3f}".rstrip("0").rstrip(".")
    return "0" if text in ("-0", "") else text


def path_data(stroke):
    parts = []
    for seg in stroke:
        pairs = " ".join(f"{fmt(seg[i])},{fmt(seg[i + 1])}" for i in range(1, len(seg), 2))
        parts.append(f"{seg[0]}{pairs}")
    return " ".join(parts)


def glyph_file_name(cp):
    return f"U+{cp:04X}.svg"


def glyph_svg(cp, width, cfg, elements):
    """The text of a glyph file: an empty template (guides only) when
    `elements` is empty. Each element is ('path', stroke, weight),
    ('dot', (cx, cy), weight) or ('ring', (cx, cy, r), weight)."""
    height = cfg["view_height"]
    guides = "".join(
        f'    <path d="M0,{fmt(y)} L{fmt(width)},{fmt(y)}"/>\n'
        for y in (cfg["baseline"] - cfg["cap_height"], cfg["baseline"] - cfg["x_height"], cfg["baseline"])
    )
    pen = cfg["pen"]
    body = ""
    for kind, data, weight in elements:
        style = f'style="fill:none;stroke:{GLYPH_STROKE};stroke-width:{fmt(pen * weight)}px;stroke-linecap:round;stroke-linejoin:round"'
        if kind == "path":
            body += f'  <path d="{path_data(data)}" {style}/>\n'
        elif kind == "dot":
            body += f'  <circle class="dot" cx="{fmt(data[0])}" cy="{fmt(data[1])}" r="{fmt(pen * weight / 2)}" {style}/>\n'
        else:
            body += f'  <circle cx="{fmt(data[0])}" cy="{fmt(data[1])}" r="{fmt(data[2])}" {style}/>\n'
    shown = chr(cp) if 32 < cp < 0x10FFFF and chr(cp).isprintable() and chr(cp) not in "<>&" else ""
    return (
        '<?xml version="1.0" encoding="UTF-8"?>\n'
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {fmt(width)} {fmt(height)}"'
        f' width="{fmt(width)}" height="{fmt(height)}">\n'
        f"  <title>U+{cp:04X} {shown}</title>\n"
        '  <g id="guides" style="fill:none;stroke:#09f;stroke-width:0.05px">\n'
        f"{guides}  </g>\n"
        f"{body}</svg>\n"
    )


def load_glyph(path, cfg):
    """A glyph file as (advance width, elements, ink x0, ink x1). `elements` is
    what glyph_svg takes; the ink extent is None when nothing is drawn."""
    text = Path(path).read_text(encoding="utf8")
    width = view_box(text)[2]
    pen = cfg["pen"]
    elements, pts = [], []
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
        return width, elements, None, None
    return width, elements, min(p[0] for p in pts), max(p[0] for p in pts)


def set_margins(path, cp, cfg, left, right):
    """Rewrite a glyph file so its ink is `left` and `right` from the artboard
    edges (font units, thousandths of the cap height), sliding the drawing.
    Returns (old left, old right, old advance, new advance) in font units, or
    None for a glyph with nothing drawn."""
    width, elements, x0, x1 = load_glyph(path, cfg)
    if x0 is None:
        return None
    cap = cfg["cap_height"]
    new_left, new_right = left / 1000.0 * cap, right / 1000.0 * cap
    shift = new_left - x0

    def moved(kind, data, weight):
        if kind == "path":
            return kind, [(s[0], *[v + shift if i % 2 == 0 else v for i, v in enumerate(s[1:])]) for s in data], weight
        return kind, (data[0] + shift, *data[1:]), weight

    new_width = new_left + (x1 - x0) + new_right
    Path(path).write_text(glyph_svg(cp, new_width, cfg, [moved(*e) for e in elements]), encoding="utf8", newline="\n")
    font = lambda v: v / cap * 1000.0
    return font(x0), font(width - x1), font(width), font(new_width)
