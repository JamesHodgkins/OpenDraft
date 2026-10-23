"""Generates ezdxf_splines.dxf, splines from an independent writer (ezdxf,
AutoCAD 2018 format), for the reader tests in ezdxf_fixture.rs, and prints
points ezdxf's own evaluator puts on each curve so the tests can compare.

    python -m pip install ezdxf
    python make_ezdxf_splines.py
"""
import ezdxf
from ezdxf.math import closed_uniform_bspline

doc = ezdxf.new("R2018")
msp = doc.modelspace()

# 1: an open cubic by control points (clamped knots).
msp.add_open_spline([(0, 0), (2, 4), (5, -1), (8, 3), (10, 0)], degree=3)
# 2: a rational quadratic, a quarter of the unit circle.
msp.add_rational_spline(
    [(1, 0), (1, 1), (0, 1)], weights=[1, 0.5**0.5, 1], degree=2
)
# 3: a fit spline with the control frame ezdxf works out, as AutoCAD keeps both.
msp.add_cad_spline_control_frame([(0, 10), (3, 13), (6, 9), (9, 12)])
# 4: a closed cubic, its control points wrapped round (full knot form).
loop = msp.add_spline()
loop.apply_construction_tool(
    closed_uniform_bspline([(20, 0), (24, 0), (24, 4), (20, 4)], order=4)
)
loop.closed = True
# 5: a spline inside a block, placed once.
block = doc.blocks.new("WAVE")
block.add_open_spline([(0, 0), (1, 1), (2, -1), (3, 0)], degree=3)
msp.add_blockref("WAVE", (40, 0))

doc.saveas("ezdxf_splines.dxf")

for e in msp.query("SPLINE"):
    tool = e.construction_tool()
    t0, t1 = tool.knots()[tool.degree], tool.knots()[-tool.degree - 1]
    points = [tool.point(t0 + (t1 - t0) * i / 4) for i in range(5)]
    print([(round(p.x, 12), round(p.y, 12)) for p in points])
