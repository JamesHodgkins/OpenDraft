"""Checks the attributed blocks the app writes with ezdxf, an independent
reader that audits the structure the way a strict CAD program would.

    cargo run -p od-files --example export_attributes -- out.dxf
    python check_attributes.py out.dxf
"""
import sys

import ezdxf

doc = ezdxf.readfile(sys.argv[1])  # strict: raises on structural errors
auditor = doc.audit()
print("version:", doc.dxfversion)
print("audit errors:", [e.message for e in auditor.errors])
assert not auditor.errors, "the audit found errors"

blocks = {b.name: b for b in doc.blocks if not b.name.startswith("*")}
for name in ("TITLE", "DUP"):
    assert name in blocks, f"block {name} is missing"
tags = [e.dxf.tag for e in blocks["TITLE"] if e.dxftype() == "ATTDEF"]
print("TITLE attributes:", tags)
assert tags == ["NAME", "REVISION", "STATUS", "CHECKED", "SCALE", "LOCKED", "HEADING"]

inserts = [e for e in doc.modelspace() if e.dxftype() == "INSERT"]
assert len(inserts) == 4
for insert in inserts:
    owners = {a.dxf.owner for a in insert.attribs}
    assert owners <= {insert.dxf.handle}, "an ATTRIB is not owned by its INSERT"
    print(insert.dxf.name, insert.dxf.insert, [(a.dxf.tag, a.dxf.text) for a in insert.attribs])
# The constant attribute is never written on an insert.
assert all(a.dxf.tag != "STATUS" for i in inserts for a in i.attribs)
print("OK")
