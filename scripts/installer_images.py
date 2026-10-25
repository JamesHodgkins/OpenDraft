#!/usr/bin/env python3
"""Make the Windows installer's two pictures from their PNG sources.

The installer (installer/windows/main.wxs) wants 24-bit BMP files, which are
what is committed: installer/windows/banner.bmp (493 x 58, the top of every page
after the first) and installer/windows/dialog.bmp (493 x 312, the Welcome and
Finish pages). Edit the PNGs in installer/windows/source/, then run this and
commit both. Needs Pillow (`pip install pillow`).

Where the installer draws its own text matters: on the banner keep x 0-418
clear for the title and description (logo in the last ~72 px); on the dialog
picture the title is at about x 180-490, y 22-50 and the body at x 180-478,
y 108-138. Check the result by building the installer and opening it (`msiexec
/i` with no `/qn`), then cancel.
"""
from pathlib import Path

from PIL import Image

HERE = Path(__file__).resolve().parent.parent / "installer" / "windows"
PICTURES = {"banner": (493, 58), "dialog": (493, 312)}

for name, size in PICTURES.items():
    source = HERE / "source" / f"{name}.png"
    image = Image.open(source).convert("RGB")
    if image.size != size:
        raise SystemExit(f"{source} is {image.size[0]}x{image.size[1]}; it must be {size[0]}x{size[1]}")
    target = HERE / f"{name}.bmp"
    image.save(target, "BMP")
    print(target)
