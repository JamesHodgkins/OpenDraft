#!/usr/bin/env python3
"""Write a self-contained HTML viewer for OD Roman: fonts/od_roman/preview/index.html.

    python scripts/od_roman_viewer.py [font folder] [output file]

od_roman_import.py runs this after every build. The page has no external
files or libraries, so it opens straight from disk. It shows:

  * your own text, set at the real advances, with sliders for the pen weight
    (percent of the cap height) and tracking, and bold / italic / guide /
    advance-box switches;
  * every glyph in its advance box, with a warning mark where the build
    found a problem; click one for its margins and a large drawing;
  * the build report: warnings and the ASCII characters still missing.

The pen, bold factor and slant are the font's own (font.cfg), so what the page
draws is what the app draws.
"""
import json
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from od_roman_preview import load  # noqa: E402
from od_roman_svg import read_config  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent


def glyph_data(folder, cfg):
    cap, pen = cfg["cap_height"], cfg["pen"]
    half_pen = cfg["weight"] / 2.0
    out = []
    for cp, (width, prims) in sorted(load(folder, cfg).items()):
        xs = []
        shapes = []
        for kind, data, weight in prims:
            if kind == "dot":
                xs.append(data[0])
                shapes.append(["d", [round(data[0], 3), round(data[1], 3)], round(weight, 3)])
            else:
                xs += [p[0] for p in data]
                shapes.append(["l", [[round(x, 3), round(y, 3)] for x, y in data], round(weight, 3)])
        g = {"cp": cp, "adv": round(width, 3), "shapes": shapes, "warn": []}
        if xs:
            left = min(xs) / cap * 1000.0
            right = (width - max(xs)) / cap * 1000.0
            g["margins"] = [round(left), round(right)]
            if min(left, right) < 0:
                g["warn"].append(f"ink outside its advance box by {-min(left, right):.0f}")
            elif min(left, right) < half_pen:
                g["warn"].append(f"only {min(left, right):.0f} from the box edge (half the default pen is {half_pen:.0f})")
        elif cp != 0x20:
            g["warn"].append("nothing drawn")
        out.append(g)
    return out


def write_viewer(folder, out, report=None):
    """Write the viewer to `out`. `report` is the build's warning lines, if any."""
    folder, out = Path(folder), Path(out)
    cfg = read_config(folder / "font.cfg")
    glyphs = glyph_data(folder, cfg)
    have = {g["cp"] for g in glyphs}
    data = {
        "cap": cfg["cap_height"],
        "base": cfg["baseline"],
        "xh": cfg["x_height"],
        "vh": cfg["view_height"],
        "pen": cfg["pen"],
        "weight": cfg["weight"] / 10.0,
        "bold": cfg["bold"],
        "italic": cfg["italic"],
        "glyphs": glyphs,
        "missing": "".join(chr(c) for c in range(33, 127) if c not in have),
        "report": report or [],
        "built": time.strftime("%Y-%m-%d %H:%M"),
    }
    text = json.dumps(data, separators=(",", ":"), ensure_ascii=True).replace("<", "\\u003c")
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(PAGE.replace("/*DATA*/null", text), encoding="utf8", newline="\n")
    return out


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    folder = Path(args[0]) if args else ROOT / "fonts" / "od_roman"
    out = Path(args[1]) if len(args) > 1 else folder / "preview" / "index.html"
    print(f"wrote {write_viewer(folder, out)}")


PAGE = r"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>OD Roman viewer</title>
<style>
:root { --bg:#fff; --panel:#f4f5f7; --ink:#111; --weak:#666; --line:#d4d7dc; --guide:#3a9bff; --box:#b9bec6; --warn:#d33; --accent:#0a66d6; }
@media (prefers-color-scheme: dark) {
  :root { --bg:#17181a; --panel:#202225; --ink:#f1f1f1; --weak:#9a9ea5; --line:#363a40; --guide:#4aa3ff; --box:#4a4f57; --warn:#ff6b6b; --accent:#6aa9ff; }
}
* { box-sizing: border-box; }
body { margin:0; font:14px/1.45 system-ui, sans-serif; background:var(--bg); color:var(--ink); }
header, section { padding:12px 20px; }
header { display:flex; gap:16px; align-items:baseline; flex-wrap:wrap; border-bottom:1px solid var(--line); }
h1 { font-size:20px; margin:0; } h2 { font-size:15px; margin:0 0 8px; }
.weak { color:var(--weak); }
#controls { background:var(--panel); border-bottom:1px solid var(--line); display:grid; gap:10px; }
#controls .row { display:flex; gap:18px; flex-wrap:wrap; align-items:center; }
textarea { width:100%; font:15px/1.4 ui-monospace, monospace; background:var(--bg); color:var(--ink); border:1px solid var(--line); border-radius:6px; padding:6px 8px; }
button { font:inherit; background:var(--bg); color:var(--ink); border:1px solid var(--line); border-radius:6px; padding:3px 9px; cursor:pointer; }
button:hover { border-color:var(--accent); }
label { display:inline-flex; gap:6px; align-items:center; }
input[type=range] { width:160px; }
#sampleWrap { overflow-x:auto; }
#sample { display:block; color:var(--ink); }
#grid { display:flex; flex-wrap:wrap; gap:6px; align-items:flex-end; }
.cell { border:1px solid var(--line); border-radius:6px; padding:3px 3px 1px; background:var(--panel); cursor:pointer; text-align:center; font-size:11px; color:var(--weak); }
.cell:hover, .cell.on { border-color:var(--accent); }
.cell.bad { border-color:var(--warn); }
.cell svg { display:block; height:104px; width:auto; color:var(--ink); }
#detail { display:grid; grid-template-columns:minmax(220px, 420px) 1fr; gap:20px; align-items:start; }
#detail svg { width:100%; height:auto; color:var(--ink); background:var(--panel); border:1px solid var(--line); border-radius:6px; }
.warn { color:var(--warn); }
ul { margin:0; padding-left:18px; }
@media (max-width: 700px) { #detail { grid-template-columns:1fr; } }
</style>
</head>
<body>
<header>
  <h1>OD Roman</h1>
  <span class="weak" id="meta"></span>
</header>

<section id="controls">
  <textarea id="text" rows="3" spellcheck="false" aria-label="Text to set"></textarea>
  <div class="row" id="presets"></div>
  <div class="row">
    <label>Size <input type="range" id="size" min="8" max="120" step="1"> <output id="sizeOut"></output> px cap height</label>
    <label>Weight <input type="range" id="weight" min="1" max="40" step="0.5"> <output id="weightOut"></output>% of cap height</label>
    <label>Tracking <input type="range" id="track" min="-100" max="300" step="5"> <output id="trackOut"></output></label>
    <label><input type="checkbox" id="bold"> Bold</label>
    <label><input type="checkbox" id="italic"> Italic</label>
    <label><input type="checkbox" id="guides"> Guides</label>
    <label><input type="checkbox" id="boxes"> Advance boxes</label>
  </div>
</section>

<section>
  <div id="sampleWrap"><svg id="sample" role="img" aria-label="The text, set in OD Roman"></svg></div>
  <div class="weak" id="missingNote"></div>
</section>

<section>
  <h2>Glyphs <span class="weak" id="count"></span></h2>
  <div id="grid"></div>
</section>

<section id="detail"></section>

<section>
  <h2>Build report</h2>
  <ul id="report"></ul>
</section>

<script>
const DATA = /*DATA*/null;
const PRESETS = {
  "Capitals": "ABCDEFGHIJKLMNOPQRSTUVWXYZ\nTHE QUICK BROWN FOX JUMPS OVER THE LAZY DOG",
  "Lowercase": "abcdefghijklmnopqrstuvwxyz\nthe quick brown fox jumps over the lazy dog",
  "Numbers": "0123456789\n1,234.56  1111 0000  3.14159  12:30  12'-6\"  45°",
  "Symbols": "!?&#()'\"*+-=_.,:;°",
  "Drawing notes": "SECTION A-A\nSCALE 1:50    DO NOT SCALE FROM DRAWING\nAll dimensions in mm unless noted",
  "Pangram": "Pack my box with five dozen liquor jugs"
};
const $ = (id) => document.getElementById(id);
const by = new Map(DATA.glyphs.map((g) => [g.cp, g]));
const state = { text: PRESETS["Capitals"], size: 18, weight: DATA.weight, track: 0, bold: false, italic: false, guides: false, boxes: false, pick: 65 };

function penWidth(extra) {
  const pen = DATA.cap * Math.min(40, Math.max(1, state.weight)) / 100 * (state.bold ? DATA.bold : 1);
  return Math.min(pen * extra, DATA.cap * 0.4 * DATA.bold);
}

// One glyph as SVG, its origin at x on the baseline of a canvas whose top is y=0.
function glyphSvg(g, x, slant) {
  let out = "";
  for (const s of g.shapes) {
    const w = penWidth(s[2]);
    if (s[0] === "d") out += `<circle cx="${s[1][0]}" cy="${s[1][1]}" r="${w / 2}" fill="currentColor"/>`;
    else out += `<polyline points="${s[1].map((p) => p.join(",")).join(" ")}" fill="none" stroke="currentColor" stroke-width="${w}" stroke-linecap="round" stroke-linejoin="round"/>`;
  }
  let t = `translate(${x} 0)`;
  if (slant) { const k = Math.tan(DATA.italic * Math.PI / 180); t += ` matrix(1 0 ${-k} 1 ${k * DATA.base} 0)`; }
  return `<g transform="${t}">${out}</g>`;
}

function guideLines(x0, x1) {
  const ys = [DATA.base - DATA.cap, DATA.base - DATA.xh, DATA.base];
  return ys.map((y) => `<line x1="${x0}" x2="${x1}" y1="${y}" y2="${y}" stroke="var(--guide)" stroke-width="0.06"/>`).join("");
}

function renderSample() {
  const lines = state.text.split("\n");
  const track = state.track / 1000 * DATA.cap;
  const space = by.get(32) ? by.get(32).adv : DATA.cap * 0.55;
  const pitch = DATA.vh * 1.05, pad = 3;
  let widest = 0, body = "", missing = new Set();
  lines.forEach((line, i) => {
    let x = pad;
    const top = pad + i * pitch - (DATA.base - DATA.cap) + 2;
    let inner = "";
    for (const ch of line) {
      const g = by.get(ch.codePointAt(0));
      if (!g) { missing.add(ch); x += space + track; continue; }
      if (state.boxes) inner += `<rect x="${x}" y="0" width="${g.adv}" height="${DATA.vh}" fill="none" stroke="var(--box)" stroke-width="0.06"/>`;
      inner += glyphSvg(g, x, state.italic);
      x += g.adv + track;
    }
    if (state.guides) inner = guideLines(pad, x) + inner;
    body += `<g transform="translate(0 ${top})">${inner}</g>`;
    widest = Math.max(widest, x);
  });
  const w = Math.max(widest + pad, 40), h = pad * 2 + pitch * lines.length;
  // A fixed size, not the width of the page: `size` is the cap height in pixels.
  const k = state.size / DATA.cap;
  $("sample").setAttribute("viewBox", `0 0 ${w} ${h}`);
  $("sample").setAttribute("width", Math.round(w * k));
  $("sample").setAttribute("height", Math.round(h * k));
  $("sample").innerHTML = body;
  $("missingNote").textContent = missing.size ? `Not in the font yet: ${[...missing].join(" ")}` : "";
}

function cellSvg(g) {
  const w = g.adv, h = DATA.vh;
  return `<svg viewBox="0 0 ${w} ${h}" aria-hidden="true">
    <rect x="0" y="0" width="${w}" height="${h}" fill="none" stroke="var(--box)" stroke-width="0.12"/>
    ${guideLines(0, w)}${glyphSvg(g, 0, false)}</svg>`;
}

function renderGrid() {
  $("grid").innerHTML = DATA.glyphs.map((g) => {
    const ch = g.cp > 32 ? String.fromCodePoint(g.cp) : "space";
    return `<div class="cell${g.warn.length ? " bad" : ""}${g.cp === state.pick ? " on" : ""}" data-cp="${g.cp}" title="${g.warn.join("; ")}">
      ${cellSvg(g)}${g.warn.length ? "⚠ " : ""}${ch} ${g.cp.toString(16).toUpperCase().padStart(4, "0")}</div>`;
  }).join("");
  $("count").textContent = `(${DATA.glyphs.length})`;
}

function renderDetail() {
  const g = by.get(state.pick);
  if (!g) { $("detail").innerHTML = ""; return; }
  const pad = 2, w = g.adv + pad * 2;
  const big = `<svg viewBox="${-pad} 0 ${w} ${DATA.vh}">
    <rect x="0" y="0" width="${g.adv}" height="${DATA.vh}" fill="none" stroke="var(--box)" stroke-width="0.1"/>
    ${guideLines(-pad, g.adv + pad)}${glyphSvg(g, 0, state.italic)}</svg>`;
  const f = (v) => Math.round(v / DATA.cap * 1000);
  const ch = g.cp > 32 ? String.fromCodePoint(g.cp) : "space";
  $("detail").innerHTML = `<div>${big}</div><div>
    <h2>${ch} <span class="weak">U+${g.cp.toString(16).toUpperCase().padStart(4, "0")}</span></h2>
    <p>Advance <b>${f(g.adv)}</b> (${g.adv} units)${g.margins ? `<br>Margins <b>${g.margins[0]}</b> left, <b>${g.margins[1]}</b> right, from the ink to the box edge` : ""}<br>
    ${g.shapes.length} stroke${g.shapes.length === 1 ? "" : "s"}; weights ${[...new Set(g.shapes.map((s) => s[2]))].join(", ") || "none"}</p>
    ${g.warn.length ? `<p class="warn">⚠ ${g.warn.join("<br>⚠ ")}</p>` : `<p class="weak">No warnings.</p>`}
    <p class="weak">Spacing is the width of the glyph's artboard in fonts/od_roman/glyphs/U+${g.cp.toString(16).toUpperCase().padStart(4, "0")}.svg.</p></div>`;
}

function renderReport() {
  const items = [...DATA.report];
  if (DATA.missing) items.push(`ASCII with no glyph: ${DATA.missing}`);
  $("report").innerHTML = items.length ? items.map((t) => `<li>${t.replace(/[<&]/g, (c) => ({ "<": "&lt;", "&": "&amp;" })[c])}</li>`).join("") : "<li>Nothing to report.</li>";
}

function syncControls() {
  $("text").value = state.text;
  $("size").value = state.size; $("sizeOut").textContent = state.size;
  $("weight").value = state.weight; $("weightOut").textContent = state.weight.toFixed(1);
  $("track").value = state.track; $("trackOut").textContent = state.track;
  for (const k of ["bold", "italic", "guides", "boxes"]) $(k).checked = state[k];
}

function update() { syncControls(); renderSample(); renderDetail(); }

$("meta").textContent = `built ${DATA.built} · cap height ${DATA.cap} units · default pen ${DATA.weight}% · bold ×${DATA.bold} · italic ${DATA.italic}°`;
$("presets").innerHTML = Object.keys(PRESETS).map((k) => `<button data-preset="${k}">${k}</button>`).join("");
$("presets").addEventListener("click", (e) => { const k = e.target.dataset.preset; if (k) { state.text = PRESETS[k]; update(); } });
$("text").addEventListener("input", (e) => { state.text = e.target.value; renderSample(); });
for (const [id, key] of [["size", "size"], ["weight", "weight"], ["track", "track"]]) {
  $(id).addEventListener("input", (e) => { state[key] = parseFloat(e.target.value); syncControls(); renderSample(); renderGrid(); renderDetail(); });
}
for (const k of ["bold", "italic", "guides", "boxes"]) {
  $(k).addEventListener("change", (e) => { state[k] = e.target.checked; renderSample(); renderGrid(); renderDetail(); });
}
$("grid").addEventListener("click", (e) => { const c = e.target.closest(".cell"); if (c) { state.pick = parseInt(c.dataset.cp, 10); renderGrid(); renderDetail(); } });

renderGrid(); renderReport(); update();
</script>
</body>
</html>
"""


if __name__ == "__main__":
    main()
