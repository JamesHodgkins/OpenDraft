"""Export the app's icons as a library of SVG files to redraw by hand.

Every icon named in crates/od-ui/src/icons.rs becomes assets/icons/<name>.svg
(DRAW_LINE is draw-line.svg). Icons are named after the command or part of the
window they are for, so each can be redrawn on its own. The app reads that folder each time it
starts and draws a file in place of the icon's glyph, so an edited file shows
on the next launch.

The starting point for each file is the Lucide icon the glyph came from: the
original SVG from lucide-static 0.469.0 (strokes on a 24 x 24 grid), or, when
that cannot be downloaded, the glyph's outline traced from the bundled font.

A file that already exists is kept, so hand edits are never overwritten;
--force replaces them all. The index, assets/icons/README.md, is rewritten
every run.

    python scripts/export_icons.py [--force] [--offline]
"""

import argparse
import re
import shutil
import sys
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ICONS_RS = ROOT / "crates/od-ui/src/icons.rs"
FONT = ROOT / "crates/od-ui/assets/fonts/lucide.ttf"
LICENSE = ROOT / "crates/od-ui/assets/fonts/LICENSE-Lucide.txt"
OUT = ROOT / "assets/icons"
SOURCES = [ROOT / "crates"]
# The same release from two mirrors, tried in turn.
LUCIDE = [
    "https://cdn.jsdelivr.net/npm/lucide-static@0.469.0/icons/{}.svg",
    "https://unpkg.com/lucide-static@0.469.0/icons/{}.svg",
]


def file_name(const):
    return const.lower().replace("_", "-") + ".svg"


def read_icons():
    """(constant, codepoint, note) for each icon, in file order."""
    icons = []
    note = []
    for line in ICONS_RS.read_text(encoding="utf-8-sig").splitlines():
        line = line.strip()
        if line.startswith("///"):
            note.append(line[3:].strip())
            continue
        m = re.match(r"(\w+) = '\\u\{([0-9a-fA-F]+)\}';", line)
        if m:
            icons.append((m.group(1), int(m.group(2), 16), " ".join(note)))
        note = []
    return icons


def glyph_names():
    from fontTools.ttLib import TTFont

    return TTFont(str(FONT)).getBestCmap()


def download(glyph):
    failures = []
    for mirror in LUCIDE:
        url = mirror.format(glyph)
        try:
            with urllib.request.urlopen(url, timeout=20) as reply:
                body = reply.read().decode("utf-8")
        except Exception as why:  # noqa: BLE001 - try the next mirror
            failures.append(str(why))
            continue
        if "<svg" in body and "</svg>" in body:
            return body
        failures.append(f"{url} is not an SVG")
    raise ValueError("; ".join(failures))


def traced(glyph):
    """The glyph's outline from the font, filled, on a 24 x 24 grid."""
    from fontTools.pens.svgPathPen import SVGPathPen
    from fontTools.pens.transformPen import TransformPen
    from fontTools.ttLib import TTFont

    font = TTFont(str(FONT))
    glyphs = font.getGlyphSet()
    # Three decimals of a 24 unit grid are plenty, and easy to edit.
    pen = SVGPathPen(glyphs, ntos=lambda n: f"{n:.3f}".rstrip("0").rstrip("."))
    scale = 24 / font["head"].unitsPerEm
    # Font units run up from the baseline; SVG runs down from the top.
    glyphs[glyph].draw(TransformPen(pen, (scale, 0, 0, -scale, 0, 24)))
    return (
        f"<!-- Traced from lucide.ttf ({glyph}); lucide-static v0.469.0 - ISC -->\n"
        '<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" '
        'viewBox="0 0 24 24" fill="currentColor">\n'
        f'  <path d="{pen.getCommands()}" />\n'
        "</svg>\n"
    )


def uses(const):
    """Where the source uses the icon: ribbon and menu labels, else files."""
    labels, files = [], []
    pattern = re.compile(r'"([^"\n]+)",\s*icons::' + const + r"\b")
    for folder in SOURCES:
        for path in folder.rglob("*.rs"):
            text = path.read_text(encoding="utf-8-sig")
            if not re.search(r"icons::" + const + r"\b", text):
                continue
            found = pattern.findall(text)
            labels += found
            if not found:
                files.append(path.relative_to(ROOT).as_posix().split("/src/")[-1])
    return sorted(set(labels)), sorted(set(files))


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--force", action="store_true", help="replace edited files")
    parser.add_argument("--offline", action="store_true", help="trace from the font only")
    args = parser.parse_args()

    OUT.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(LICENSE, OUT / "LICENSE-Lucide.txt")
    cmap = glyph_names()
    rows = []
    written = kept = traced_count = 0
    for const, code, note in read_icons():
        glyph = cmap.get(code)
        if glyph is None:
            sys.exit(f"{const}: U+{code:04X} is not in lucide.ttf")
        target = OUT / file_name(const)
        if target.exists() and not args.force:
            kept += 1
        else:
            body = None
            if not args.offline:
                try:
                    body = download(glyph)
                except Exception as why:  # noqa: BLE001 - any failure falls back
                    print(f"{const}: download failed ({why}); tracing the font")
            if body is None:
                body = traced(glyph)
                traced_count += 1
            target.write_text(body, encoding="utf-8", newline="\n")
            written += 1
        labels, files = uses(const)
        rows.append((target.name, glyph, note, labels, files))

    lines = [
        "# Icon library",
        "",
        "Generated by `scripts/export_icons.py`; the table is rewritten each run,",
        "the SVG files are not (unless `--force`).",
        "",
        "Each file is drawn in the app in place of the icon's glyph, and the same",
        "drawings are built into the program (a rebuild picks up an edit). A debug",
        "build run from the source, a folder named by `OD_ICONS`, or an `icons`",
        "folder beside the program is read every time the app starts, so an edited",
        "file shows on the next launch, with no rebuild, and replaces the built-in",
        "drawing. A file that cannot be read keeps the built-in drawing, and the",
        "command line says which files were skipped.",
        "",
        "Drawing rules:",
        "",
        "- The drawing is fitted to a square at the icon's size (14 px in a ribbon",
        "  row, 32 px for a large button), so keep the artboard square.",
        "- Any size of square `viewBox` works, and so does any editor's SVG. An icon",
        "  whose colours are all grey or white (as Lucide's are) is *plain*: the app",
        "  takes only its shape and paints it in the theme's colour, so it follows",
        "  the theme and dims when the command is unavailable. An icon with any",
        "  clearly coloured part (the green points of `draw-line.svg`) is *coloured*:",
        "  it is drawn in its own colours and only fades when unavailable. Use",
        "  `currentColor` for a part that should be white. Keep the light parts light:",
        "  the ribbon is dark.",
        "- Each file is named after the command (`draw-line.svg` is Line, `modify-move.svg`",
        "  is Move) or the part of the window (`layer-panel-frozen.svg`) it is for, and",
        "  is used only there. To add an icon, add a line to `crates/od-ui/src/icons.rs`",
        "  and run the script again.",
        "",
        "The starting drawings are Lucide's (ISC licence, `LICENSE-Lucide.txt`).",
        "",
        "| Icon | File | Lucide | Used by |",
        "|---|---|---|---|",
    ]
    for name, glyph, note, labels, files in rows:
        used = ", ".join(labels + files) or "(not used)"
        if note:
            used += f" ({note})"
        lines.append(f"| ![]({name}) | `{name}` | {glyph} | {used} |")
    (OUT / "README.md").write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")
    print(
        f"{written} written ({traced_count} traced from the font), "
        f"{kept} kept as they were, index in {OUT.relative_to(ROOT).as_posix()}/README.md"
    )


if __name__ == "__main__":
    main()
