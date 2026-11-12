"""Generates ezdxf_sample.dxf, a realistic DXF from an independent writer
(ezdxf, AutoCAD 2018 format), for the reader tests in ezdxf_fixture.rs.

    python -m pip install ezdxf
    python make_ezdxf_sample.py
"""
import ezdxf

doc = ezdxf.new("R2018", setup=True)
msp = doc.modelspace()

doc.layers.add("Walls", color=1, linetype="DASHED", lineweight=50)
doc.layers.add("Frozen", color=3).freeze()
doc.layers.add("Locked", color=5).lock()
doc.layers.add("Off", color=2).off()
doc.layers.add("Café", color=4)
true = doc.layers.add("TrueColor")
true.rgb = (10, 200, 100)

msp.add_line((0, 0), (10, 0), dxfattribs={"layer": "Walls"})  # 1 line
msp.add_circle((5, 5), 2, dxfattribs={"layer": "Walls", "color": 2})  # 2 circle
msp.add_arc((0, 0), 5, 0, 90, dxfattribs={"layer": "Café"})  # 3 arc
# 4: a polyline with one bulged segment, 5: a closed rectangle
msp.add_lwpolyline([(0, 0, 0, 0, 1), (4, 0, 0, 0, 0), (4, 3, 0, 0, 0)], format="xyseb")
msp.add_lwpolyline([(0, 0), (6, 0), (6, 2), (0, 2)], close=True, dxfattribs={"layer": "Locked"})
# 6: an old-style 2D polyline
msp.add_polyline2d([(0, 0), (3, 0), (3, 3)], close=False, dxfattribs={"layer": "Frozen"})
# 7: attributes set on the entity
line = msp.add_line((0, 1), (5, 1), dxfattribs={"linetype": "CENTER", "lineweight": 35, "color": 5})
# 8: true colour on the entity
tc = msp.add_line((0, 2), (5, 2))
tc.rgb = (255, 128, 0)
# 9: on a layer that is never declared in the table
msp.add_line((0, 3), (5, 3), dxfattribs={"layer": "Undeclared"})
# things the app cannot read yet (5 objects)
msp.add_text("hello", dxfattribs={"layer": "0"})
msp.add_spline([(0, 0), (1, 2), (2, 0), (3, 2)])
msp.add_ellipse((0, 0), major_axis=(3, 0), ratio=0.5)
block = doc.blocks.new("DOOR")
block.add_line((0, 0), (1, 0))  # the block's one object
msp.add_blockref("DOOR", (20, 20))
msp.add_blockref("DOOR", (30, 20))
# a paper space object (1)
doc.layout("Layout1").add_line((0, 0), (1, 1))

doc.saveas("ezdxf_sample.dxf")
