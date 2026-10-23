#!/usr/bin/env python3
"""Start a new glyph file, already the right height and with the guides in.

    python scripts/od_roman_new_glyph.py A                 # a character
    python scripts/od_roman_new_glyph.py U+00D8 --width 15 # or a codepoint

Writes fonts/od_roman/glyphs/U+XXXX.svg as wide as `default_advance` in
font.cfg unless --width says otherwise (SVG units). Draw the glyph as red
(#f00) paths with round caps, stroke-width = the pen for a normal stroke, and
resize the artboard (the viewBox width) to set the advance. Refuses to
overwrite a glyph that exists.
"""
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from od_roman_svg import glyph_file_name, glyph_svg, read_config  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
FOLDER = ROOT / "fonts" / "od_roman"


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    if not args:
        raise SystemExit(__doc__)
    width = None
    if "--width" in sys.argv:
        width = float(sys.argv[sys.argv.index("--width") + 1])
        args = [a for a in args if a != sys.argv[sys.argv.index("--width") + 1]]
    which = args[0]
    m = re.fullmatch(r"(?:U\+)?([0-9A-Fa-f]{4,6})", which)
    cp = int(m.group(1), 16) if m and (which.upper().startswith("U+") or len(which) > 1) else None
    if cp is None:
        if len(which) != 1:
            raise SystemExit(f"{which!r} is neither a single character nor U+XXXX")
        cp = ord(which)
    cfg = read_config(FOLDER / "font.cfg")
    path = FOLDER / "glyphs" / glyph_file_name(cp)
    if path.exists():
        raise SystemExit(f"{path} already exists")
    path.parent.mkdir(parents=True, exist_ok=True)
    width = width or cfg["default_advance"]
    path.write_text(glyph_svg(cp, width, cfg, []), encoding="utf8", newline="\n")
    print(f"wrote {path} ({width} wide)")


if __name__ == "__main__":
    main()
