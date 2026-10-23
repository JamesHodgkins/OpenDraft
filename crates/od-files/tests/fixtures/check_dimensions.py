"""Checks the dimensions the app writes with ezdxf, an independent reader that
audits the structure the way a strict CAD program would.

    cargo run -p od-files --example export_dimensions -- out.dxf
    python check_dimensions.py out.dxf
"""
import sys

import ezdxf

path = sys.argv[1]
doc = ezdxf.readfile(path)  # strict: raises on structural errors
auditor = doc.audit()
print("version:", doc.dxfversion)
print("audit errors:", [e.message for e in auditor.errors])
print("audit fixes:", [e.message for e in auditor.fixes])
assert not auditor.errors, "the audit found errors"

kinds = {}
for e in doc.modelspace():
    if e.dxftype() != "DIMENSION":
        continue
    kinds.setdefault(e.dimtype, 0)
    kinds[e.dimtype] += 1
    block = doc.blocks.get(e.dxf.geometry)
    assert len(block) > 0, f"{e.dxf.geometry} is empty"
    assert doc.dimstyles.has_entry(e.dxf.dimstyle), e.dxf.dimstyle
    measured = e.get_measurement()
    if not isinstance(measured, float):
        measured = tuple(round(v, 4) for v in measured)
    else:
        measured = round(measured, 4)
    print(e.dimtype, e.dxf.geometry, e.dxf.dimstyle, len(block), "pieces,", "measurement", measured)
# 0 rotated, 1 aligned, 2 two-line angular, 3 diameter, 4 radius, 5 3-point
# angular, 6/70 ordinate X.
print("kinds:", kinds)
assert set(kinds) >= {0, 1, 2, 3, 4, 5, 6}, kinds
print("OK")
