"""Generates ezdxf_dimensions.dxf, dimensions from an independent writer
(ezdxf, AutoCAD 2018 format), for the reader tests in ezdxf_fixture.rs. The
text of each says what the test expects of it.

    python -m pip install ezdxf
    python make_ezdxf_dimensions.py
"""
import ezdxf

doc = ezdxf.new("R2018", setup=True)
msp = doc.modelspace()

# 1: horizontal, 30 long, text moved to a user position.
msp.add_linear_dim(base=(15, 10), p1=(0, 0), p2=(30, 0)).render()
# 2: vertical, with typed text.
msp.add_linear_dim(
    base=(-10, 5), p1=(0, 0), p2=(0, 20), angle=90, text="<> TYP"
).render()
# 3: aligned along (40,0)-(70,40): 50 long.
msp.add_aligned_dim(p1=(40, 0), p2=(70, 40), distance=8).render()
# 4: a radius and a diameter of the circle at (100, 0) radius 10.
msp.add_radius_dim(center=(100, 0), radius=10, angle=45).render()
msp.add_diameter_dim(center=(100, 30), radius=10, angle=20).render()
# 5: angular from two lines (45 degrees) and from three points (90).
msp.add_angular_dim_2l(
    base=(5, 85), line1=((0, 80), (20, 80)), line2=((0, 80), (20, 100))
).render()
msp.add_angular_dim_3p(
    base=(5, 125), center=(0, 120), p1=(20, 120), p2=(0, 140)
).render()
# 6: ordinate, X of a feature at (30, 80).
msp.add_ordinate_x_dim(feature_location=(30, 80), offset=(0, 15)).render()
# 7: a dimension style of its own and a dimension that uses it.
style = doc.dimstyles.new("Big")
style.dxf.dimtxt = 6.0
style.dxf.dimdec = 1
msp.add_linear_dim(
    base=(15, -10), p1=(0, 0), p2=(30, 0), dimstyle="Big"
).render()
# 8: a dimension with an override of its style.
msp.add_linear_dim(
    base=(15, -30), p1=(0, 0), p2=(30, 0), override={"dimtxt": 5.0}
).render()

doc.saveas("ezdxf_dimensions.dxf")
