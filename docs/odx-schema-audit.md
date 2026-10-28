# .odx schema audit, before the format is declared stable

Scope: `crates/od-files/src/native.rs` and `native/*.rs` (about 5,250 lines, 17 files carrying `#[serde(default...)]`, roughly 115 attributes) on the branch after PR #47. Findings only: nothing in the schema has changed. The format is at version 0, so every item below can be changed freely now and costs a conversion after version 1. Decisions that need James are marked **Decide**.

## 1. Files mean what today's defaults say (largest item)

Several tables are written as "differences from the stock value" and read back by filling the rest from the current build's defaults:

| Written sparsely | Where |
|---|---|
| dimension styles (every dimension carries its style as a sparse object) | `dimension_style.rs` (`sparse_style`, `style_from`), `dimension.rs` |
| text styles, mleader styles, linetype table, `LTSCALE`, `PDMODE`/`PDSIZE`, new-object colour/linetype/lineweight | `native.rs` (flattened `TableDto`s), `new_style.rs`, `point_style.rs` |
| per-entity flags and numbers (`#[serde(default, skip_serializing_if = ...)]`) | all entity DTOs |

The problem: a drawing saved today records "dimension text height is the stock one". If a later build changes the stock text height, an old drawing silently re-dimensions on opening, with no error and nothing a conversion could detect. For the stock tables the same applies (a changed stock `DASHED` pattern, say).

**Decide:** which of these two rules the stable format follows.
- **A. Write everything that affects appearance explicitly** (full dimension style per style, full text style, full linetype table). Larger files, but a file means the same thing in every build. Recommended for anything that changes how the drawing looks or measures.
- **B. Keep sparse, but freeze the stock.** The stock values become part of the format and a later change to them is a version bump with a conversion. Smaller files, a permanent constraint.

Entity-level "false/empty means absent" flags (`closed`, `bulges`, `widths`, `reversed`...) are fine under either rule, because their omitted value is a definition, not a tunable default.

## 2. Untyped blobs

- `dim_style: Option<serde_json::Value>`, `dim_styles: Option<Vec<Value>>` (`native.rs`) and `style: Option<Value>` on each dimension (`dimension.rs`). The typed `DimStyleDto` exists, but the file stores a `Value` merged over the stock style (`dimension_style.rs:215-296`). The schema is therefore defined by whatever `DimStyle` serialises to, which contradicts the rule that the file has its own schema ("refactoring cannot change what is on disk by accident"): renaming a field in `od-core`'s `DimStyle` changes the file.
  Fix: if rule 1A is chosen, the field is simply `Vec<DimStyleDto>` and `style: u64` (an id), which also stops every dimension repeating its style.
- `ColumnsDto.kind: String`, `knots: Option<String>` (spline). Stringly-typed enums that the reader matches by hand (`text.rs:374-377`). Fix: serde enums with `snake_case`.

## 3. DXF numbering leaking into the native schema

The native file is meant to be OpenDraft's own, but many fields store DXF integer codes that need the DXF spec to read:
- `MTextDto.attachment` (1 to 9) and `flow` (1, 3, 5): "DXF code" in the comments.
- `SpacingDto.style` (1 at least, 2 exact).
- `DimStyleDto`: `arrow: i64` (0 to 3), `length_format`, `angle_format` (`LUNITS`/`AUNITS` codes), `arrow2: i64` with -1 meaning "same".
- `BackgroundDto.extra_bits: i32` and `MTextDto.unimplemented_441: Option<i32>` (raw DXF group values "kept and never used"), `raw_typeface: Option<i32>` in text styles.

Fix: named enums (`attachment: "top_left"`), `Option` instead of the -1 sentinel, and decide whether the preserve-for-round-trip fields belong in the native format at all (they exist so a DXF to `.odx` to DXF trip loses nothing; if that is wanted, put them in one clearly named `dxf_extra` object instead of scattered fields).

## 4. References by name instead of id

`LayerDto.linetype: String`, `ObjectDto.linetype: AttrDto<String>`, `block.rs`, `new_style.rs` refer to linetypes by name; text styles, layers, layouts and blocks use ids. `ARCHITECTURE.md` already lists `EntityLinetype { ByLayer, ByBlock, Named(id) }` as a later slice. Names make rename a file-wide edit and forced the case-insensitive lookup rule. **Decide:** do the id move before version 1 (recommended; it changes `Layer`, `Attributes` and the linetype commands too, so it is its own piece of work). Note `ARCHITECTURE.md` also says `CONTINUOUS`/`BYLAYER`/`BYBLOCK` are ids 1 to 3, so the target shape already exists in `od-core`.

## 5. `#[serde(default)]` that should be required

Defaults that only make sense for files written by an earlier build are residue and should go, so a damaged or truncated file is reported rather than quietly accepted:
- Document counters: `next_layout_id`, `next_viewport_id`, `next_text_style_id`, `next_block_id`, `current_text_style` (`native.rs:205-223`), and the mleader equivalents. The writer always emits them. Loading passes them to `reserve_*` (`load.rs`); I have not checked whether that also guards against a counter below the largest id in use, which should be verified when these become required.
- `units` (`UnitsDto`) and `LayerDto.on/frozen/locked` (`on` defaults to `true` through a helper): always written, make them required.
- `ObjectDto.color/linetype/lineweight`: `ByLayer` is written explicitly today; decide whether omission is a feature (shorter files) or residue.

Keep as optional: sparse per-entity flags (section 1 note), `layouts`, `blocks`, `text_styles`, the table `TableDto`s, so a plain drawing stays small, once rule 1 is decided.

## 6. Smaller points

- Tuples and fixed arrays in the schema (`jog: Option<(PointDto, PointDto)>`, `[f64; 2]`, `[bool; 2]`, `[u8; 3]`) are positional and unreadable in a diff-friendly format; prefer small named structs for anything not a point or colour.
- `ColorDto::Foreground` versus `Rgb([u8; 3])`: fine, but write the colour as the format will live with it (hex string or named channels).
- `lineweight: u16` in hundredths of a millimetre is documented in one comment only; state units in field names (`lineweight_hundredths_mm`) or in the notes.
- `saved_by` is a free string ("OpenDraft 0.1.3"). Fine for messages; do not parse it.
- No JSON Schema file exists. Generating one from the DTOs (or writing it by hand) and testing the writer's output against it would make the stable format a published artefact; optional but cheap while there are 17 files.

## Recommended order

1. Decide rule 1 (A or B). It determines the dimension, text and mleader style shapes, so it comes first.
2. Typed styles and enums (sections 2 and 3), done together with rule 1 since they touch the same DTOs.
3. Required fields (section 5), then the linetype ids (section 4) as a separate branch.
4. Committed fixtures and the version-1 PR following the checklist in `CONTRIBUTING.md`.

Each step is a schema change in place at version 0, with the tests updated and no conversion code.
