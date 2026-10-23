"""Checks the DXF (R2000) the app writes for a drawing with layouts, using
ezdxf as an independent, strict reader. Not AutoCAD itself.

    cargo run -p od-files --example export_layouts -- out.dxf
    python check_layouts.py out.dxf
"""
import sys

import ezdxf

path = sys.argv[1]
# strict read first: any structural problem raises
doc = ezdxf.readfile(path)
print("version:", doc.dxfversion, "(", doc.acad_release, ")")
auditor = doc.audit()
print("audit errors:", [(e.code, e.message) for e in auditor.errors])
print("audit fixes:", [(e.code, e.message) for e in auditor.fixes])
print("layouts:", doc.layouts.names_in_taborder())

msp = doc.modelspace()
print("model entities:", sorted(e.dxftype() for e in msp))
for e in msp:
    if e.dxftype() == "LWPOLYLINE":
        print("  lwpolyline:", e.dxf.layer, "closed" if e.closed else "open", "pts", list(e.get_points("xyseb")), "lw", e.dxf.get("lineweight"), "rgb", e.dxf.get("true_color"), "lt", e.dxf.get("linetype"))
for name in ("Plan", "Detail"):
    lay = doc.layouts.get(name)
    dl = lay.dxf_layout.dxf
    print(f"layout {name}: paper {dl.paper_width} x {dl.paper_height} tab {dl.taborder}")
    print("  entities:", sorted(e.dxftype() for e in lay))
    for vp in lay.viewports():
        scale = vp.dxf.height / vp.dxf.view_height
        print(f"  viewport id={vp.dxf.id} status={vp.dxf.status} center={tuple(vp.dxf.center)[:2]} size=({vp.dxf.width},{vp.dxf.height}) scale={scale:.4g} view_center={tuple(vp.dxf.view_center_point)[:2]} frozen={vp.frozen_layers} flags={vp.dxf.flags}")
layers = {l.dxf.name: (l.dxf.color, l.dxf.get("lineweight"), l.dxf.get("true_color")) for l in doc.layers}
print("layers:", layers)
assert not auditor.errors, "the audit found errors"
assert doc.layouts.names_in_taborder() == ["Model", "Plan", "Detail"]
plan = doc.layouts.get("Plan")
main = [v for v in plan.viewports() if v.dxf.id == 2][0]
assert abs(main.dxf.height / main.dxf.view_height - 0.02) < 1e-9
assert main.frozen_layers == ["Walls"] and main.dxf.flags & 0x4000
print("OK")
