# CLAUDE.md

OpenDraft: a 2D-only CAD app in Rust on egui/eframe, with an AutoCAD-2020-style ribbon, dockable panels and a command line. Windows and Linux are both targets (Linux has never been run yet). Licence GPL-3.0-or-later. Repo: https://github.com/JamesHodgkins/OpenDraft, branch `main`.

## Read first
- `ARCHITECTURE.md`: the rules new code follows (crates, commands, Editor, files, geometry). Authoritative.
- `ui_plan.md`: history of what was built and why; its last section, "Where we are and what next", is the to-do list.
- `2d_commands_reference.md`: the AutoCAD command list we work through as a roadmap.
- `Command_specifications.md` is the user's untracked scratch file: never stage it.
- `private_material/` is git-ignored (specs, PDFs, screenshots, scratch notes and the AutoCAD sample drawings): nothing in code or tests may depend on it, except the optional `bench_features` example, which skips files it cannot find. The sample drawings carry a work login in their headers, so they are never committed; tests use small hand-made data (or the ezdxf fixtures in `od-files/tests/fixtures/`) instead.

## Crates (dependencies point down)
`od-core` (document, entities, pure geometry) <- `od-commands` (registry, undo, Session, Editor, no egui) <- `od-files` (`.odx` JSON, DXF) / `od-canvas` (camera, hit, snap, grips, paint) / `od-ui` (theme, widgets, ribbon, icons) <- `od-app` (binary `opendraft`: AppState, command table, panels, `automation/` host). `od-api` (wire contract for outside callers, depends only on `od-core`) and `od-mcp` (binary `opendraft-mcp`, the MCP server; depends on `od-api`, never on the app) sit beside them; see ARCHITECTURE.md "Automation and MCP" and `docs/mcp.md`.

## How a command is built
A command is `fn(&mut Editor) -> Result<(), Cancelled>` running on its own thread; it asks (`ask_point`, `get_point`, `ask_object`, `ask_selection`, `pickset_or_select`...) and edits through `ed.transaction("Name", |tx| ...)` (one undo step). Maths goes in `od-core` as pure, tested functions; the command only asks and records. To add one: geometry + tests in `od-core`, a file in `od-app/src`, an `editor_command!` line in `commands.rs`, an alias in `cmdline.rs`, a ribbon item in `ribbon_def.rs` (Home tab: `ribbon_def/home.rs`, and take the id off its `NOT_BUILT` list if it is there), an icon codepoint in `od-ui/src/icons.rs` (then `python scripts/export_icons.py` adds its SVG to `assets/icons`; icons are drawn through `od_ui::icon_set`, never the icon font directly), a section in `ui_plan.md`. Recent examples to copy: `join.rs`, `offset.rs`, `edges.rs`.

## Conventions
- Entities are referred to by `EntityId`, never list position. Arc entity is CCW radians. Polyline `bulges` is empty or one per point (`bulges[i]` shapes segment i to i+1); always use `od_core::bulge`, never assume straight segments.
- No `unwrap`, `todo!`, `dbg!`, prints in production code; `unsafe` forbidden (tests may unwrap).
- Files stay under about 400 production lines; `od-app/tests/file_sizes.rs` enforces 450 and lists `widgets.rs` and `dxf/read.rs` as still too big (they may not grow).
- Adding an entity kind bumps the `.odx` version (unknown kinds are refused, never skipped); an optional field does not. Current version 2 (leaders and multileaders).
- Use `text::plural` for counts. Match the surrounding code's comment density and naming.

## Checks (CI runs these on Windows and Linux)
```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
Gate commits on the real exit codes (don't pipe cargo into grep and then commit).

## Working with the user (James)
- Commit as James <jlh33278@gmail.com> with NO Co-Authored-By trailer and never list Claude as a contributor.
- `main` is protected by a ruleset (pull request required, no force-push, no deletion) and Claude is not a bypass actor: work on a feature branch, push it when asked, and open a PR; the `check` jobs must pass before it merges. Push only when asked; the user usually says yes after each finished feature. Stage with `git add -u` plus explicitly named new files, never `git add -A`; check `git status` first.
- Keep `ARCHITECTURE.md` and `ui_plan.md` updated in the same change as a feature.
- Follow AutoCAD behaviour and wording for commands unless told otherwise. If a selection exists when a command starts, skip its first selection stage.
- Finish with a short report: what changed, what is not done, and what to take next. Do not start new feature work without confirmation.

## Tooling notes (Windows)
- Shell is Git Bash/PowerShell. Avoid two heredocs in one Bash call; for multi-line Python edit scripts use the Write tool. Files may be CRLF in the working tree (git normalises); edits via the Edit tool work.
- Lucide icon font: `crates/od-ui/assets/fonts/lucide.ttf`; find a glyph's codepoint with fontTools (`getBestCmap`).
- Testing in the real app: `cargo build -p od-app` (always rebuild first), then drive the exe with PowerShell (launch, keys, clicks, screenshots). A click on a selected object's grip starts a grip edit, so pick selection points away from grips. Finishing a drawing click also selects the new object.
- Unit tests drive commands with `AppState`: `start_command`, `settle_command`, `answer_object`, `answer_enter`, `select_all` (see `edges.rs` and `join.rs` tests).

## Known gaps (details in `ui_plan.md`)
Lengthen for ellipses and splines, PEDIT, associative arrays (ARRAY makes plain copies), spline edits (Trim, Extend, Break, Join, Offset, Fillet, closed creation, tangents, SPLINEDIT; and so ellipse offset), ellipse Isocircle/PELLIPSE, JOIN across gaps and its Close option, text, block attributes, ByBlock, DXF block export and dynamic blocks, hatch gap tolerance, Draw boundary, Reassociate, writing hatch associations to DXF, drawn gradients and `.pat` loading, dimensions (done apart from colours/linetypes in styles, DIMBREAK, DIMINSPECT, annotative scales, arrowhead blocks, `ARC_DIMENSION` and DIMASSOC in DXF, DWG import, and AutoCAD-checking the DXF we write); multileaders (`MLEADER` and its styles are baseline: DXF is written to AutoCAD 2018's layout but not yet opened in AutoCAD, no DWG multileaders, legacy `LEADER` is baseline (straight, text or none; it does not follow its text; no DWG), one leader line, no overrides, block content, spline leaders or MLEADEREDIT/ALIGN; see `private_material/leader_and_mleader_specification.md`); Fillet/Chamfer next to arc segments; Fuzz for Join; fillet/chamfer variables are per user, not saved in the drawing; DXF import reads MTEXT and the STYLE table but skips plain TEXT and anonymous blocks, DWG import reads TEXT, MTEXT and the STYLE table but skips hatches, dimensions, attributes and MTEXT in columns; DXF and DWG import skip fit-point-only splines with end tangents or not cubic; DXF writes R12, or R2000 for layouts, ellipses, splines, hatches and text; DWG is import only (no DWG export) and skips layouts; Linux untested.
