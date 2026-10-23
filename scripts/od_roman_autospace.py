#!/usr/bin/env python3
"""Space every glyph listed in fonts/od_roman/spacing.cfg by its edge shapes.

    python scripts/od_roman_autospace.py [--dry-run]

The table gives each glyph the kind of edge on its left and right (stem, round,
open, diagonal, ...) and each kind a margin; see the file for the reasoning.
The tool sets each listed glyph's margins to exactly that (the ink's
centre-line to the artboard edge, in font units), sliding the drawing without
touching the shapes, and gives the ten digits one shared advance. Glyphs that
are not listed are left alone, so a hand-spaced glyph stays as it is.

It rewrites the listed glyph files through the template: keep nothing in them
but the red drawing. Run od_roman_import.py afterwards to rebuild and look at
the viewer. --dry-run prints what would change and writes nothing.
"""
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from od_roman_svg import glyph_file_name, load_glyph, read_config, set_margins  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
FOLDER = ROOT / "fonts" / "od_roman"


def read_table(path):
    edges, glyphs, extra = {}, {}, {"lowercase": 1.0, "digits": None}
    for n, line in enumerate(Path(path).read_text(encoding="utf8").splitlines(), 1):
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        parts = line.split()
        if parts[0] == "edge" and len(parts) == 3:
            edges[parts[1]] = float(parts[2])
        elif parts[0] in extra and len(parts) == 2:
            extra[parts[0]] = float(parts[1])
        elif len(parts) == 3:
            who = int(parts[0][2:], 16) if parts[0].upper().startswith("U+") and len(parts[0]) > 2 else None
            if who is None:
                if len(parts[0]) != 1:
                    raise SystemExit(f"{path}:{n}: {parts[0]!r} is not one character or U+XXXX")
                who = ord(parts[0])
            glyphs[who] = (parts[1], parts[2])
        else:
            raise SystemExit(f"{path}:{n}: cannot read {line!r}")
    for cp, kinds in glyphs.items():
        for kind in kinds:
            if kind not in edges:
                raise SystemExit(f"U+{cp:04X}: no edge kind {kind!r} (have {sorted(edges)})")
    return edges, glyphs, extra


def main():
    dry = "--dry-run" in sys.argv
    cfg = read_config(FOLDER / "font.cfg")
    cap = cfg["cap_height"]
    edges, glyphs, extra = read_table(FOLDER / "spacing.cfg")
    rows, missing = [], []

    def glyph_path(cp):
        return FOLDER / "glyphs" / glyph_file_name(cp)

    for cp, (left_kind, right_kind) in sorted(glyphs.items()):
        path = glyph_path(cp)
        if not path.exists():
            missing.append(chr(cp))
            continue
        scale = extra["lowercase"] if chr(cp).islower() else 1.0
        left, right = edges[left_kind] * scale, edges[right_kind] * scale
        if dry:
            width, _, x0, x1 = load_glyph(path, cfg)
            if x0 is None:
                continue
            old = (x0 / cap * 1000, (width - x1) / cap * 1000, width / cap * 1000)
            new = (left, right, left + (x1 - x0) / cap * 1000 + right)
            rows.append((cp, *old, *new))
        else:
            done = set_margins(path, cp, cfg, left, right)
            if done:
                rows.append((cp, done[0], done[1], done[2], left, right, done[3]))

    if extra["digits"] is not None:
        digits = [ord(c) for c in "0123456789" if glyph_path(ord(c)).exists()]
        inks = {}
        for cp in digits:
            width, _, x0, x1 = load_glyph(glyph_path(cp), cfg)
            if x0 is not None:
                inks[cp] = (x1 - x0) / cap * 1000, width, x0
        if inks:
            box = max(w for w, _, _ in inks.values()) + 2 * extra["digits"]
            for cp, (ink, width, x0) in sorted(inks.items()):
                left = (box - ink) / 2
                if dry:
                    rows.append((cp, x0 / cap * 1000, 0, width / cap * 1000, left, box - ink - left, box))
                else:
                    done = set_margins(glyph_path(cp), cp, cfg, left, box - ink - left)
                    rows.append((cp, done[0], done[1], done[2], left, box - ink - left, done[3]))

    for cp, ol, orr, ow, nl, nr, nw in rows:
        print(f"{chr(cp)!r:6} U+{cp:04X}  margins {ol:4.0f}/{orr:4.0f} -> {nl:4.0f}/{nr:4.0f}   advance {ow:5.0f} -> {nw:5.0f}")
    if missing:
        print("not drawn yet (listed in spacing.cfg):", "".join(missing))
    print(("would change" if dry else "changed") + f" {len(rows)} glyphs; the rest are untouched")


if __name__ == "__main__":
    main()
