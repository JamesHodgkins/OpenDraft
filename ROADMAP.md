# Roadmap

How OpenDraft gets from "a capable early project" to "actually, that's quite an impressive project". This is the plan for trust, reach and depth. The command-by-command to-do list is in `ui_plan.md`; this file decides what to do first and why.

Written 2026-10-31. Update it when something is done or the plan changes: tick the box, add the date and the PR.

## Who it is for, in order

1. **Open-source and Linux users.** Often leaving a closed tool or an ageing one. They try it, report bugs and spread it, so they get the first impression.
2. **Hobbyists and makers.** Laser cutting, CNC, 3D-print sketches, woodwork, floor plans. They need clean files going out as much as in.
3. **Small-scale professionals.** Drafters and small firms who need AutoCAD-compatible files and depth. This comes last; it needs the trust built by the first two.

## What sets it apart (keep these sharp)

- Reads **DWG** (AutoCAD 2010 to 2018) as well as DXF, free. The free 2D alternatives are weak or paid here.
- An **AutoCAD-style command line** with the usual names and aliases, and prompts with options.
- One **small native executable**; **scriptable by an AI assistant** through MCP with every change undoable.

## How to read this

- **Size:** S is up to a day, M is a few days, L is weeks.
- **Why** says which audience it serves and what it earns.
- Each phase has an **exit test**: how we know it is done, not just that the boxes are ticked.
- Phases are ordered by what earns trust soonest. Within a phase, items can go in any order unless stated. Phases 1 and 2 are the priority; later phases overlap with normal development.

---

## Phase 1: Linux as a first-class platform

*Why: the first audience runs Linux, and so far it has been run on one machine (Linux Mint).*

- [ ] **Test matrix (M).** Ubuntu LTS, Fedora and Arch, on X11 and Wayland, with HiDPI and fractional scaling. Check the clipboard, file dialogs, drag and drop, key handling and window behaviour. Record what was tested in the repo.
- [ ] **GUI smoke test in CI (M).** Launch the app under `xvfb`; open a file, draw a line, save, quit. It catches whole classes of Linux breakage.
- [ ] **Release artifacts tested (S).** The AppImage and .deb already have test scripts (`scripts/test_appimage.sh`, `scripts/test_deb.sh`); run them in the release workflow.
- [ ] **Flathub (M).** One package that works on every distribution.
- [ ] **AUR package (S).** Then Debian/Ubuntu, Fedora and nixpkgs as people ask.
- [ ] **Fix what the matrix finds (S to L).**

**Exit test:** a fresh install on each tested distribution opens a DWG, draws, saves and quits, and the CI smoke test guards that.

## Phase 2: Trust in files

*Why: "opens your drawings" has to be provable. All audiences.*

- [ ] **Fuzz the readers (M).** `cargo-fuzz` targets for the DWG, DXF and `.odx` readers; a short run in CI and a longer run on a schedule. Fix what it finds.
- [ ] **Golden-image tests (M).** `od-canvas::preview` renders a document to pixels without egui. Snapshot drawings that cover hatches, dimensions, dashed linetypes, text and blocks, so rendering regressions are caught by a test and not by eye.
- [ ] **Interoperability checks (M).** Open OpenDraft's DXF in LibreCAD, QCAD and FreeCAD, and in an independent DXF validator; open it in AutoCAD if it can be had (the dimension DXF has never been opened there). Keep the results and any sample files in the repo; fix what breaks.
- [ ] **Sample drawings pack (S).** Original or CC0 drawings for tests and for new users. (The AutoCAD sample drawings in `private_material/` must never be committed.)
- [ ] **Benchmarks in CI (M).** Run the existing benches against stored baselines so speed claims stay true.
- [ ] **Property tests for geometry (M).** Offset and back, trim and extend, fillet invariants, in `od-core`.

**Exit test:** the readers survive a fixed amount of fuzzing, a documented set of drawings round-trips through other free CAD tools, and a rendering regression fails CI.

## Phase 3: What makers need

*Why: audience 2. Files going out matter as much as files coming in.*

- [ ] **PDF plot at true scale (L).** Today plotting writes PNG. PDF is the usual way to print at 1:1 and share drawings.
- [ ] **SVG export (M).** For laser cutters, vinyl cutters and Inkscape. Closed shapes, correct units, no stray duplicates.
- [ ] **Clean DXF for fabrication (M).** Correct units, closed polylines, splines handled sensibly. OVERKILL and JOIN already help; add a checklist test of a typical laser-cutting file.
- [ ] **DXF gaps (M).** TEXT import (skipped today), anonymous blocks, block export. Each is a known gap in `CLAUDE.md`.
- [ ] **Templates and a first-hour tutorial (S).** A metric sheet, A4 and A3, a laser-cutting template, and a ten-command walkthrough.
- [ ] **Hatch patterns from `.pat` files and linetypes from `.lin` files (M).**

**Exit test:** a drawing made in OpenDraft cuts correctly on a laser cutter or prints at 1:1 from the PDF, with no hand-fixing.

## Phase 4: Community and project health

*Why: the main risk to a one-maintainer project is that it stays one maintainer.*

- [ ] **Contributor path (S).** Issue templates already exist; add "good first issue" labels and the "how to add a command" recipe (now only in `CLAUDE.md`) as a section in `CONTRIBUTING.md`.
- [ ] **A short demo video and a plain project page (S).** Include the MCP assistant drawing, with every change undoable and reviewable.
- [ ] **Readable architecture docs (M).** Split `ARCHITECTURE.md` into short pages by topic; its paragraphs are accurate but very long.
- [ ] **Dependency advisories (S).** `cargo deny check advisories` fails on `paste`, `rustybuzz` and `tokio`, all transitive. Fix them or record why each is accepted in `deny.toml`.
- [ ] **A place to talk (S).** GitHub Discussions is enough.
- [ ] **A regular release rhythm and a public statement of what is not supported yet (S).** An honest "not yet" list builds trust.
- [ ] **Windows code signing (M).** Unsigned downloads put off exactly the people we want; also winget.
- [ ] **A second maintainer (ongoing).** Not a task; the aim of the items above.

**Exit test:** someone who is not the author has made a merged contribution, and a new user can install, learn and report a bug without help.

## Phase 5: Declare `.odx` stable

*Why: people will not keep real work in a format that "changes between builds".*

- [ ] Follow "Declaring .odx stable" in `CONTRIBUTING.md`, set `CURRENT_VERSION` to 1 as a deliberate release step, and from then on keep a conversion for every older version. Never as part of a feature.
- [ ] Publish the schema (there is already `docs/odx-schema-audit.md`) and a round-trip test corpus.

**Exit test:** a file saved by the first stable release opens in every later one.

## Phase 6: Depth for small professionals

*Why: audience 3. Only after phases 1 to 5, and in this rough order.*

- [ ] Text: TEXT/MTEXT in all import and export paths, block attributes, `ByBlock`.
- [ ] DWG export (L).
- [ ] Associative arrays (ARRAY makes plain copies today).
- [ ] Spline editing (trim, extend, break, join, offset), Lengthen for ellipses and splines, PEDIT's Fit and Spline.
- [ ] Dimension depth: other arrowheads, styles with colours and linetypes, `DIMBREAK`, jogged radius and `ARC_DIMENSION` as real DXF, annotative scales.
- [ ] Multileader depth (spline leaders, block content, MLEADEREDIT).
- [ ] A plugin system for third-party commands; an AutoLISP-compatible subset is a stretch idea, not a promise.
- [ ] External references (xrefs).

Dynamic blocks are last, and may never be worth it.

## Not now

Dynamic blocks, 3D, a cloud or collaboration layer, macOS (unless a contributor takes it on), and any feature that would be only half built. A visible "not yet" is better than a feature that misleads.

## Principles for choosing

- Reliability and evidence before breadth: a claim is made only once a test or a check backs it.
- Prefer small steps that can be reviewed and released.
- Linux, then Windows, then the rest; AutoCAD behaviour and wording unless there is a reason not to.
- Review the plan every few releases; drop items that no user asks for.

## Related files

- `ui_plan.md`: what was built and why, and the detailed to-do list.
- `code_review_findings.md`: the engineering review and its current status. Only OD-007 (modal flags) is open.
- `parking_lot.md`: things looked at and left for later.
