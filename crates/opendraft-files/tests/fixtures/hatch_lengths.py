"""Total stroke length of every pattern HATCH in a DXF, worked out by ezdxf's
own hatch-line generator, for comparing with this app's evaluator.

    python hatch_lengths.py private_material/HatchTest.dxf

Prints one line per pattern hatch: its index among the hatches, the number
of strokes and their total length.
"""
import sys

import ezdxf
from ezdxf.render import hatching

doc = ezdxf.readfile(sys.argv[1])
for i, h in enumerate(e for e in doc.modelspace() if e.dxftype() == "HATCH"):
    if h.dxf.solid_fill or h.pattern is None:
        continue
    count = 0
    total = 0.0
    try:
        for start, end in hatching.hatch_entity(h, jiggle_origin=False):
            count += 1
            total += (end - start).magnitude
    except hatching.HatchingError as err:
        print(i, "error", err)
        continue
    print(i, count, round(total, 6))
