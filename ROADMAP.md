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

*Why: the first audience runs Linux, and until now it had been run on one machine (Linux Mint).*

**Status (2026-10-31): done as far as it can be shown without real machines.** What is built, and what was found, is in `docs/linux.md`; the remainder needs people with the hardware.

- [x] **Test matrix (M), on virtual displays.** CI builds once on Ubuntu 22.04 and runs that program on Ubuntu 22.04 and 24.04, Debian 12, Fedora, Arch and openSUSE Tumbleweed (X11), on Ubuntu 24.04, Fedora and Arch under Wayland (a headless sway), and at scale 1.5 and 2 (`.github/workflows/linux.yml`). `docs/linux.md` records what it does and does not show.
- [ ] **Test on real machines (M, needs you).** Real GPUs, GNOME (mutter) and KDE (KWin) on X11 and Wayland, the clipboard, drag and drop, real file dialogs and input methods, and Wayland keyboard and mouse input (a headless compositor has no input devices). Reports from users are the way to cover this; say what was tried in `docs/linux.md`.
- [x] **GUI smoke test in CI (M).** `scripts/linux_smoke.py` starts the real program, opens a drawing, draws a line with the keyboard and mouse, undoes it, runs `opendraft-mcp --check` against it, quits with Ctrl+Q, and also starts without a drawing and without `XDG_RUNTIME_DIR`. It reads the screen and the program's state from outside, and leaves screenshots as artifacts.
- [x] **Release artifacts tested (S).** The release workflow already installed the .deb and ran the AppImage on a bare Ubuntu 22.04; the .deb now also passes the whole smoke test.
- [x] **Flatpak (M), built in CI.** `installer/linux/flatpak/` builds offline from `Cargo.lock`, has its files checked and starts under a virtual display. [ ] **Submit it to Flathub (S, needs you):** a pull request to `flathub/flathub` following `installer/linux/flatpak/README.md`.
- [x] **Arch package (S), built in CI.** `installer/linux/arch/PKGBUILD` builds, installs and links in an Arch container. [ ] **Publish it to the AUR (S, needs you):** an AUR account and `updpkgsums`. Debian/Ubuntu, Fedora and nixpkgs as people ask.
- [x] **Fix what the matrix finds.** Found and fixed: the program would not open at all where the graphics setup offered no multisampled configuration (every distribution with a recent Mesa under software rendering); Open and Save As did nothing when there was no file dialog backend (the status bar now says what to install); the assistant connection needed `XDG_RUNTIME_DIR` (a private cache folder is used without it). Noticed and not changed: a drawing opened from DXF counts as unsaved at once, so closing it asks to save. That looks deliberate (it has no `.odx` file yet) but is worth a look.

**Not covered, and why:** opening a *DWG* and *saving* were part of the original exit test. CI opens a DXF, because the DWG sample drawings are private and cannot be committed; and saving needs a file dialog or an automation grant that only the user can give, so a headless test cannot do it. A small hand-made DWG fixture would close the first; the second would need a test-only way to grant permission, which would weaken the rule that only the user grants write access, so it waits for a better idea.

**Exit test:** on each tested distribution the program starts and draws, takes keyboard and mouse input (X11), opens a drawing, answers the assistant connection and quits, and the CI smoke test guards that. Real-machine coverage is the open item above.

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
