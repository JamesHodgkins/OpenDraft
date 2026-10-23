# Parking lot

Things we looked at and chose to leave for later. Move an item into `ui_plan.md` when work starts.

## Non-rectangular viewport clips

Today a viewport is always a rectangle (`center`, `width`, `height` in `od-core/src/layout.rs`). DXF files with other outlines import as rectangles with a warning.

Plan (medium size, about 8-10 files):

- **Model:** add `Viewport::clip: Option<Vec<Point>>`, a closed polygon on the sheet. `None` keeps the rectangle. `center`, `width` and `height` stay as the polygon's bounding box, so scale, zoom, pan and MVIEW keep working. Add point-in-polygon `contains_paper` and `Viewport::outline()`.
- **Plot (`od-core/src/plot.rs`):** clip each segment against the polygon edges and keep the inside pieces (works for concave outlines). Tests: triangle, concave L, circle.
- **Screen:** egui can only clip to rectangles, so clip each flattened path against the polygon with the same function as the plot. Filled shapes would break, but there are no solids or hatches yet.
- **Interaction:** picking and "inside the viewport" use point-in-polygon; draw the outline as the border.
- **Files:** `.odx` gets an optional field (no version bump). DXF write: clip entity (340 handle) with a closed `LWPOLYLINE`. DXF read: read it instead of warning. Check against an ezdxf-built file.
- **Commands:** VPCLIP (clip with a closed polyline or circle; Delete option goes back to a rectangle).

Suggested order:

1. Clipping end to end: model, screen, plot, picking, `.odx`, DXF, VPCLIP.
2. Later: MVIEW Polygonal and Object options, and grips to drag the outline's vertices (fiddly, least needed).

Not included: a clipped viewport inside a nested viewport; clipping of filled shapes (wait for hatches).

## Other layout leftovers

- Plot margins and linetypes.
- Annotative scales (needs text).
- Rotated and perspective viewports.
- The LAYOUT Set option.
- Open the R2000 export in AutoCAD (only checked with ezdxf so far).
- A ribbon Layout tab.

## Dimensions: outstanding

First slice is built (aligned, linear, radius, diameter, arc length; see `ui_plan.md`, "Dimensions"). Left for later, roughly in order of value:

- **Style table and dialog:** named `DimStyle`s in the drawing (today there is one current style, copied into each dimension), a DIMSTYLE dialog, and an option to update existing dimensions when a style changes.
- **DIMEDIT / editing text** of an existing dimension (new text, reset text position via `Dimension::reset_text`), and a Properties row for the text override.
- **Text entity and text styles:** replace the built-in stroke font (`dimension/font.rs`) with real fonts. Needed for MText, fixed text height and SHX/TrueType.
- **More dimension kinds:** baseline, continue, angular, ordinate, leader/multileader, jogged radius, and the `DIMARC` Partial and ambiguity-leader options.
- **Associativity:** `DIMASSOC` equivalent, a dependency graph from dimension points to source geometry, broken-reference handling and reassociation.
- **Style features not built:** alternate units, tolerances and limits, `DIMJUST` 3/4 (over extension line), `DIMTMOVE` text-follows for radial, oblique and text-angle options, frame around text (negative `DIMGAP`), arrowhead blocks, linetype/colour/lineweight overrides (`DIMCLRD`, `DIMLWD`...), `DIMSOXD`.
- **Annotative scales** and paper/model trans-space dimensions (needs text).
- **DXF:** write real `DIMENSION` entities (anonymous `*D` blocks, `DIMSTYLE` table, `ACAD` XDATA overrides, `DIMASSOC`) instead of exploding; read them (import currently skips dimensions). Check output in AutoCAD, not only ezdxf. The DWG/RealDWG adapter from the spec is not planned.
- **Plot:** arrowheads plot as outlines, not filled (plotting flattens wide polylines to single lines).
- **Performance (see `ui_plan.md` "Performance"):** the bounds of a dimension are now remembered, but `Dimension::picture()` is still rebuilt each frame the dimension is on screen (2,000 in view: 14 ms). Cache the picture per object if a drawing with very many dimensions feels slow.
- **Arc dimension text:** the symbol and text are laid on the tangent, so long text on a small radius looks stiff; consider text following the arc.
- **Verify in the real app:** radius, diameter, arc length and grip editing were only unit-tested (aligned and linear were seen on screen).

## Hatch gradients

Gradient hatches are read, kept and written (`HatchFill::Gradient`: raw name, angle, shift, one or two colours, tint; DXF 450-470 tail; `.odx` v11) but drawn as one flat colour, the first stop (`od-canvas/src/view/hatch_paint.rs`, `solid_color`). A solid hatch that carries a gradient tail with `450 = 0` loses the tail on import.

Plan (medium size, mostly in `od-canvas`):

- **Colour per vertex:** the solid mesh (`hatch_cache.rs`, `SolidMesh`) is already triangulated relative to the hatch's middle. Add a gradient coordinate `g` per vertex from the hatch's extent and the gradient angle, and colour each vertex from it. egui interpolates linearly between vertices, so a coarse mesh draws curved profiles badly: subdivide the triangles (or add a grid of interior points) when the kind is not linear.
- **Kinds to draw:** LINEAR first (two colours, `shift` moves the centre, one-colour gradients blend toward white or black by `tint`), then CYLINDER, INVCYLINDER, SPHERICAL, INVSPHERICAL, HEMISPHERICAL, INVHEMISPHERICAL, CURVED, INVCURVED. The specification says not to map them all to linear. The profiles need AutoCAD output to calibrate (see below); until then they should say they are approximate.
- **Plot:** `PlotFill` is one flat colour. Draw a gradient as bands (several fills) or write the first colour and note it.
- **Files:** keep a `450 = 0` tail (and `GradientColor1ACI`/`GradientColor2ACI` XDATA) on solid hatches so a round trip is lossless; read the hatch background colour (`HPBACKGROUNDCOLOR`, XDATA) at the same time.
- **Properties and HATCH:** gradient kind, angle, shift, the two colours, tint, centred; a Gradient option in HATCH and HATCHEDIT (HATCHEDIT refuses gradients today).
- **Checking:** needs a fixture from AutoCAD with each kind at known colours, angle 0 and 45 degrees, centred and not, saved as DXF and exported as an image, so the profiles can be compared by eye and by sampled pixels. We have none yet. ezdxf can write the records but not tell us what AutoCAD draws.

Not included: transparency (needs a document-wide attribute) and mesh/gradient fills inside blocks.

## Linetypes: `.lin` files and what follows

The table, dashed drawing, scales, DXF/DWG patterns and the Linetype Manager are built (see `ui_plan.md`, "Linetypes", and `private_material/autocad_linetype_reference_guide.md`, which is the rulebook). Load in the Manager offers only our stock library. Left for later:

- **`.lin` loading (next, self-contained):** a pure parser in `od-core/src/linetype/lin.rs` (header `*NAME,description`, pattern line `A,len,len,...`, `;` comments, 2 to 12 elements, first not a gap, 47-character description, names unique ignoring case) that returns records or per-entry problems, tested on text only. Then a file picker on the Manager's Load button (list the file's linetypes, pick, load; an existing name is replaced, as AutoCAD's reload does) and a `-LINETYPE` Load option. Test against the user's own `acad.lin` and `acadiso.lin`, which also settles the metric and ISO numbers (the guide still marks those as checked only against a 2017 snapshot). Complex entries (`["GAS",STANDARD,S=.1,...]`, shapes from `.shx`) are read as their dashes and reported, as DXF import does. Export (`.lin` out) and Reload/Rename in the Manager belong here too.
- **Current linetype (`CELTYPE`):** new objects are always ByLayer. Needs a current linetype in the editor or document and every object-creation site to take it; also `CELTSCALE` and `PLINEGEN` as the defaults for new objects (today new objects have scale 1 and generation off). The Linetype Manager would get Current and a Set current button.
- **ByBlock:** `Attr<T>` has only ByLayer and Set; DXF import turns ByBlock into ByLayer. Needs the value, propagation through nested inserts for linetype, colour and lineweight together, and ByBlock in the lists. Linetypes still refer to names, not `LinetypeId`s; move to `EntityLinetype { ByLayer, ByBlock, Named(id) }` at the same time.
- **Complex linetypes:** text and shape elements are dropped (drawn as plain dashes with a warning). Keeping them for a round trip needs a decoration on each pattern element (`ComplexDecoration`, raw flags) and DXF groups 74/75/340/46/50/44/45/9; drawing them needs the text and shape engines. DWG decodes the complex flag but not the extra fields.
- **Not yet dashed:** block contents, dimensions and hatch outlines draw solid (`HPLINETYPE` governs hatches in AutoCAD).
- **Scale rules:** `PSLTSCALE` and `MSLTSCALE` (need layout viewports and annotative scales); DWG polyline Linetype Generation flag (LWPOLYLINE flag 256) is not decoded.
- **Fitting arithmetic:** how end dashes take up slack, and how a closed shape is phased, is our own choice meeting AutoCAD's documented rules. Lock it down with AutoCAD fixtures at several lengths (lines, arcs, circles, closed polylines), as the guide's acceptance tests describe.
- **Performance:** dashes are rebuilt every paint; cache per object if a large dashed drawing feels slow (the hatch cache is the model).
