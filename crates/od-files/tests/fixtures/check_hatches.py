"""Checks the HATCH records the app writes with ezdxf, an independent reader
that audits the structure the way a strict CAD program would.

    cargo test -p od-files --test hatch_fixtures     # leaves hatch_export.dxf
    python check_hatches.py target/tmp/hatch_export.dxf
"""
import sys

import ezdxf
from ezdxf.entities import boundary_paths as bp

path = sys.argv[1]
doc = ezdxf.readfile(path)  # strict: raises on structural errors
auditor = doc.audit()
print("version:", doc.dxfversion)
print("audit errors:", [e.message for e in auditor.errors])
print("audit fixes:", [e.message for e in auditor.fixes])
assert not auditor.errors, "the audit found errors"

hatches = [e for e in doc.modelspace() if e.dxftype() == "HATCH"]
for h in hatches:
    kinds = []
    for p in h.paths:
        if isinstance(p, bp.PolylinePath):
            kinds.append(f"polyline({len(p.vertices)}, closed={p.is_closed})")
        else:
            kinds.append("edges(" + ",".join(e.EDGE_TYPE for e in p.edges) + ")")
    print(
        h.dxf.pattern_name,
        "solid" if h.dxf.solid_fill else "pattern",
        "style", h.dxf.hatch_style,
        "paths", kinds,
        "lines", len(h.pattern.lines) if h.pattern else 0,
        "gradient", h.gradient.name if h.gradient else "-",
        "seeds", len(h.seeds),
    )
assert len(hatches) == 4, len(hatches)
first, dashed, user, gradient = hatches
# The clockwise half disc: an arc run clockwise, then the chord.
arc = first.paths[0].edges[0]
assert not arc.ccw and arc.EDGE_TYPE == "ArcEdge"
# ezdxf's own view of the same arc: counter-clockwise from 0 to 180 degrees.
assert abs(arc.start_angle - 0.0) < 1e-6 and abs(arc.end_angle - 180.0) < 1e-6, (
    arc.start_angle,
    arc.end_angle,
)
assert [type(p).__name__ for p in dashed.paths] == ["EdgePath", "PolylinePath"]
assert len(dashed.pattern.lines) == 1 and list(dashed.pattern.lines[0].dash_length_items) == [6.0, -2.0, 0.0, -2.0]
assert len(user.pattern.lines) == 2 and user.dxf.pattern_double == 1
assert gradient.gradient and gradient.gradient.name == "SPHERICAL"
print("OK")
