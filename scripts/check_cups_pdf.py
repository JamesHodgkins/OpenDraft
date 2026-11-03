#!/usr/bin/env python3
"""Checks the PDF that cups-pdf wrote for the job in
crates/od-app/src/print_backend/cups_live_tests.rs: one A4 page, and the
100 mm line on it still 100 mm long, so the job was printed at its own size
and not fitted to the page.

Usage: check_cups_pdf.py DIRECTORY [DPI]   (needs pdftoppm, from poppler-utils)
"""
import glob
import os
import re
import subprocess
import sys
import tempfile

A4_POINTS = (595.28, 841.89)
LINE_MM = 100.0
PAGE_TOLERANCE_POINTS = 3.0
LINE_TOLERANCE_MM = 2.0


def ink_width_mm(pgm: bytes, dpi: float) -> float:
    """The width in millimetres of everything darker than mid grey in a binary
    PGM (P5) picture."""
    header = re.match(rb"P5\s+(?:#[^\n]*\n\s*)*(\d+)\s+(\d+)\s+255\s", pgm)
    if not header:
        raise ValueError("expected an 8-bit binary PGM")
    width, height = int(header.group(1)), int(header.group(2))
    pixels = pgm[header.end():]
    if len(pixels) != width * height:
        raise ValueError("the picture's size does not match its header")
    left, right = width, -1
    for row in range(height):
        line = pixels[row * width:(row + 1) * width]
        for column, value in enumerate(line):
            if value < 128:
                left = min(left, column)
                right = max(right, column)
    if right < 0:
        raise ValueError("nothing is drawn on the page")
    return (right - left + 1) / dpi * 25.4


def main() -> int:
    directory = sys.argv[1]
    dpi = float(sys.argv[2]) if len(sys.argv) > 2 else 100.0
    files = sorted(glob.glob(os.path.join(directory, "*.pdf")), key=os.path.getmtime)
    if not files:
        print(f"no PDF in {directory}")
        return 1
    path = files[-1]
    print(f"checking {path}")

    from pypdf import PdfReader

    reader = PdfReader(path)
    if len(reader.pages) != 1:
        print(f"expected 1 page, found {len(reader.pages)}")
        return 1
    box = reader.pages[0].mediabox
    size = (float(box.width), float(box.height))
    print(f"page {size[0]:.1f} x {size[1]:.1f} points")
    if any(abs(a - b) > PAGE_TOLERANCE_POINTS for a, b in zip(size, A4_POINTS)):
        print("the page is not A4")
        return 1

    with tempfile.TemporaryDirectory() as scratch:
        stem = os.path.join(scratch, "page")
        subprocess.run(
            ["pdftoppm", "-r", str(int(dpi)), "-gray", path, stem], check=True
        )
        rendered = glob.glob(stem + "*.pgm")
        if len(rendered) != 1:
            print(f"pdftoppm made {len(rendered)} pictures")
            return 1
        with open(rendered[0], "rb") as handle:
            width = ink_width_mm(handle.read(), dpi)
    print(f"the line is {width:.1f} mm wide, drawn as {LINE_MM:.0f} mm")
    if abs(width - LINE_MM) > LINE_TOLERANCE_MM:
        print("the job was scaled")
        return 1
    print("ok")
    return 0


if __name__ == "__main__":
    sys.exit(main())
