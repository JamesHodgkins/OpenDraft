"""Generates ezdxf_attributes.dxf, a DXF from an independent writer (ezdxf,
AutoCAD 2018 format) with attributed blocks, for the reader tests in
attribute_fixtures.rs.

    python -m pip install ezdxf
    python make_ezdxf_attributes.py

Contents (the Rust test names the same things):
  block TITLE   a line and ATTDEFs NAME (plain, with prompt), REVISION
                (invisible), STATUS (constant "DRAFT"), CHECKED (verify),
                SCALE (preset), LOCKED (lock position), HEADING (centred)
  block DUP     two ATTDEFs that share the tag N
  inserts       TITLE plain, TITLE turned 30 degrees at scale 2, TITLE
                mirrored, DUP with two values
  model space   one loose ATTDEF
"""
import ezdxf
from ezdxf.enums import TextEntityAlignment

doc = ezdxf.new("R2018", setup=True)
msp = doc.modelspace()

title = doc.blocks.new("TITLE", base_point=(0, 0))
title.add_line((0, 0), (40, 0))
title.add_attdef("NAME", (2, 4), "none", dxfattribs={"height": 2.5, "prompt": "Name?"})
title.add_attdef("REVISION", (2, 8), "A", dxfattribs={"height": 2.5, "flags": 1, "prompt": "Revision?"})
title.add_attdef("STATUS", (2, 12), "DRAFT", dxfattribs={"height": 2.5, "flags": 2, "prompt": "Status?"})
title.add_attdef("CHECKED", (2, 16), "no", dxfattribs={"height": 2.5, "flags": 4, "prompt": "Checked?"})
title.add_attdef("SCALE", (2, 20), "1:1", dxfattribs={"height": 2.5, "flags": 8, "prompt": "Scale?"})
locked = title.add_attdef("LOCKED", (2, 24), "pinned", dxfattribs={"height": 2.5, "prompt": "Locked?"})
locked.dxf.lock_position = 1
heading = title.add_attdef("HEADING", (20, 30), "Heading", dxfattribs={"height": 3, "prompt": "Heading?"})
heading.set_placement((20, 30), align=TextEntityAlignment.MIDDLE_CENTER)

dup = doc.blocks.new("DUP", base_point=(0, 0))
dup.add_attdef("N", (0, 0), "one", dxfattribs={"height": 2, "prompt": "First?"})
dup.add_attdef("N", (0, 4), "two", dxfattribs={"height": 2, "prompt": "Second?"})

plain = msp.add_blockref("TITLE", (100, 50))
plain.add_auto_attribs({"NAME": "Plan A", "REVISION": "B", "CHECKED": "yes", "SCALE": "1:50",
                        "LOCKED": "pinned", "HEADING": "Ground floor"})

turned = msp.add_blockref("TITLE", (200, 50), dxfattribs={"rotation": 30, "xscale": 2, "yscale": 2})
turned.add_auto_attribs({"NAME": "Turned", "REVISION": "C", "CHECKED": "no", "SCALE": "1:20",
                         "LOCKED": "pinned", "HEADING": "Roof"})

flipped = msp.add_blockref("TITLE", (300, 50), dxfattribs={"xscale": -1})
flipped.add_auto_attribs({"NAME": "Flipped", "REVISION": "D", "CHECKED": "no", "SCALE": "1:1",
                          "LOCKED": "pinned", "HEADING": "Mirror"})

twins = msp.add_blockref("DUP", (0, 100))
# add_auto_attribs keys by tag, so both values are added by hand.
twins.add_attrib("N", "first", (0, 0), dxfattribs={"height": 2})
twins.add_attrib("N", "second", (0, 4), dxfattribs={"height": 2})

msp.add_attdef("LOOSE", (50, 100), "stray", dxfattribs={"height": 2, "prompt": "Loose?"})

doc.saveas("ezdxf_attributes.dxf")
auditor = doc.audit()
print("errors:", len(auditor.errors), "fixes:", len(auditor.fixes))
