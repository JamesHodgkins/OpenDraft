"""Generates ezdxf_layouts.dxf, a DXF with layouts and viewports from an
independent writer (ezdxf, AutoCAD 2018 format), for the reader tests in
ezdxf_fixture.rs.

    python -m pip install ezdxf
    python make_ezdxf_layouts.py

What is in it:
- model space: a line on layer "0" and a circle on layer "Walls";
- "Layout1": A4 landscape with margins left 8, bottom 6 (so the paper origin
  is the printable area's corner and the sheet starts at (-8, -6)), a
  viewport centred at (100, 80), 50 x 40, showing the model around (10, 5)
  at 1:2 (view height 80), with "Walls" frozen in it, and a line (10, 10)-(60, 10);
- "Plan": A3 with a locked viewport that is off, and a circle;
- a viewport looking from the side (skipped by the reader).
"""
import ezdxf

doc = ezdxf.new("R2018", setup=True)
msp = doc.modelspace()
doc.layers.add("Walls", color=1)
msp.add_line((0, 0), (100, 50))
msp.add_circle((10, 5), 20, dxfattribs={"layer": "Walls"})

layout1 = doc.layouts.get("Layout1")
layout1.page_setup(size=(297, 210), margins=(2, 4, 6, 8), units="mm")
viewport = layout1.add_viewport(
    center=(100, 80), size=(50, 40), view_center_point=(10, 5), view_height=80
)
viewport.frozen_layers = ["Walls"]
layout1.add_line((10, 10), (60, 10))
side = layout1.add_viewport(
    center=(200, 80), size=(30, 30), view_center_point=(0, 0), view_height=60
)
side.dxf.view_direction_vector = (0, 1, 0)

plan = doc.layouts.new("Plan")
plan.page_setup(size=(420, 297), margins=(0, 0, 0, 0), units="mm")
off = plan.add_viewport(
    center=(210, 148.5), size=(300, 200), view_center_point=(50, 25), view_height=400
)
off.dxf.status = 0
off.dxf.flags |= 0x4000  # locked
plan.add_circle((100, 100), 30)

doc.saveas("ezdxf_layouts.dxf")
