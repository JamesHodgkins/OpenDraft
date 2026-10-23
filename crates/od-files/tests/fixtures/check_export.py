"""Checks the DXF the app writes with ezdxf, an independent reader that
audits the structure the way a strict CAD program would.

    cargo run -p od-files --example export_sample -- out.dxf
    python check_export.py out.dxf
"""
import sys

import ezdxf
from ezdxf import recover

path = sys.argv[1]
doc = ezdxf.readfile(path)  # strict: raises on structural errors
auditor = doc.audit()
print("version:", doc.dxfversion)
print("audit errors:", [e.message for e in auditor.errors])
print("audit fixes:", [e.message for e in auditor.fixes])
print("layers:", sorted(layer.dxf.name for layer in doc.layers))
for e in doc.modelspace():
    print(" ", e.dxftype(), e.dxf.layer, "color", e.dxf.get("color", "-"), "ltype", e.dxf.get("linetype", "-"))
assert not auditor.errors, "the audit found errors"
print("OK")
