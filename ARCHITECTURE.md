# OpenDraft architecture

A 2D CAD app on egui/eframe. This file records the rules new code follows.

## Crates (dependencies point downward)

| Crate | Owns | Must not depend on |
|---|---|---|
| `od-core` | Document, entities, geometry | anything else |
| `od-commands` | `Registry`/`Command`, `Action`, undo `History`, `Session`, transactions, the `Editor` (user interaction) | egui |
| `od-files` | File formats (`FileFormat` trait; native `.odx` JSON, DXF import and export, DWG import by converting `od-dwg`'s output), atomic save | egui, the app |
| `od-dwg` | A standalone DWG reader (AC1024, AC1027, AC1032): container, header, tables, flat model-space shapes (ellipses, splines and text included), the text style table, named blocks with block references, and a typed report. No workspace dependencies; `od-files` converts its output to a `Document` | everything else in the workspace |
| `od-api` | The automation wire contract for out-of-process callers: decimal-string ids, framed JSON messages and the token handshake, the instance descriptor file, request/response envelopes, stable error codes, scopes, limits, version negotiation, the read operations' arguments (with JSON Schema) and results, and read-side entity DTOs. Depends only on `od-core` | egui, MCP SDKs, the app |
| `od-mcp` | The `opendraft-mcp` binary: MCP over stdio (official `rmcp`, pinned) in front of a blocking client for the app's automation socket. Never owns a drawing | `od-app`, egui, `od-core` internals |
| `od-canvas` | Camera, drawing view, hit-testing, snapping, selection | the app |
| `od-ui` | Theme, widgets, ribbon, icons, shortcuts | the document |
| `od-app` | `AppState`, command table, panels, dock, settings; the binary | n/a |

## Rules

1. **Every user action is a command** in `od-app/src/commands.rs`, built with
   `Command::new(..).key(..).when(..).checked(..)`. Ribbon, quick access, status
   bar, shortcuts and the command line refer to commands by id and read enabled
   and checked state from the registry. Never match on command ids in UI code
   to find state; give the command a `checked` predicate.
2. **Every document change is an `Action`** run by the `Session`, so it is
   undoable. A command that makes several edits groups them in a transaction
   so they are one undo step (see "Commands and the Editor"). Entity edits
   that keep ids and positions stable use `ModifyEntities`.
3. **Logic is pure and tested; UI only draws.** Anything with rules (property
   values, parsing, snapping, selection) lives in a function over plain data
   with unit tests. `state.rs` has no egui code.
4. **UI state that belongs to a widget** (open menu, typed text) lives in egui
   temp memory; **state that belongs to the user** (settings, layout) is
   persisted; **state that belongs to the drawing** lives in `Doc`.
5. **Files stay under about 400 lines of production code** (tests do not
   count). `od-app/tests/file_sizes.rs` enforces it at 450 and lists the
   files still over, which may not grow: `widgets.rs` and `dxf/read.rs`
   (`view.rs` is done: its painting moved to `view/paint.rs`). Split them, then remove them from the list. Split by
   responsibility, not by layer. `od-app/src/app/` shows the pattern: `app.rs`
   owns the struct and the frame loop, and each submodule adds one area as an
   `impl` block.
6. **No `unwrap`, `todo!`, `dbg!` or prints in production code** (clippy warns).
   Unwrap is fine in tests. `unsafe` is forbidden.
7. **Wording helpers** (`text::plural`) are shared; do not repeat inline
   `if n == 1` pluralisation.
8. **An edit that touches many objects is one pass, never a loop of single
   edits.** `Document` keeps a lookup from id to position, so taking an object
   out or putting one in costs a pass over every object after it. Use
   `remove_many`, `insert_many`, `push_many`, `replace_many` and `replace_each`
   (`document/object_list.rs`, `document.rs`) for erase, the undo of an add,
   adding copies, changing geometry, and exploding blocks or dimensions for a
   file. They also update the spatial index once (see "Spatial index").
   `remove` and `insert` are for one object. A guard test for each such path
   (erase and undo, undo of adding copies, exploding references) fails if
   someone writes the loop; add one when you add a bulk path. Measure with
   `cargo run --release -p od-app --example bench_features` before and after
   a change that may affect speed (see `ui_plan.md`, "Performance").

## Data model (design decision)

The drawing follows the AutoCAD/DXF shape, in typed Rust structs rather than a
generic property table:

- **Entities** are an ordered list (draw order). Each `Object` has a permanent
  `EntityId` that never changes and is never reused, a geometry enum (Line,
  Circle, ...) and common attributes: layer, colour, linetype, lineweight.
  Code that filters or groups objects by kind uses `Entity::kind()`, which
  returns the `EntityKind` enum (one variant per entity, so a new entity fails
  to build until every `match` has it), never a name. `EntityKind` also holds
  the three names, so a label can be reworded without changing behaviour:
  `display_name` (running text), `title` (headings, lists) and `wire_name`
  (the MCP and JSON token, part of the contract; `from_wire` parses it).
- **Symbol tables** hold layers, text styles and blocks (done), and later
  linetypes. Entities refer to them by id, so renaming a layer edits one record.
  Each object has a layer plus colour, linetype and lineweight that are either
  `ByLayer` or set on the object (`Attr<T>`). A layer can be on, frozen or
  locked: hidden layers are not drawn, snapped to or picked; locked layers are
  drawn and snapped to but not picked. Layer 0 always exists and cannot be
  deleted, renamed, or removed while current or in use. Ask the `Document`
  (`color_of`, `is_visible`, `is_selectable`), never resolve this by hand.
- **Extension data**: each entity can carry a small key/value map namespaced by
  plugin id, so plugins attach data without new entity types. The core stores
  and saves it without interpreting it.

**Invariants.** A drawing has layer 0 first, unique layer ids and names
(ignoring case), unique object ids, every object on an existing layer, a
current layer that exists, and id counters ahead of every id in use.
`Document::validate()` checks all of that. The low-level mutators
(`push`, `insert`, `push_layer`, `replace_attrs`...) are what undo and redo
use to restore state, so they trust their caller; the net under them is that
`Session` validates after every `perform`, transaction, undo and redo in debug
builds and tests (an edit that breaks the drawing panics where it happened),
and `NativeFormat::write` refuses a drawing that fails it, so a corrupt
drawing is never written to disk. Not done yet: making the mutators return
`Result` so release builds also reject a bad edit at the call (see
`ui_plan.md`, "Audit"). `layer_of` still falls back to layer 0 for a missing
layer so that drawing never panics; `validate` is what reports it.

Rules that follow:

1. **Refer to entities by `EntityId`, never by list position.** Selection,
   actions, the undo history, files and plugins all use ids. `Document` hides
   its list behind `objects()`, `get(id)`, `add`, `remove`, `replace`; ids
   come from `allocate_id` and are never reused. The selection is a set of
   ids and survives edits, undo and redo (ids that no longer exist drop out).
2. **Geometry types stay closed** (the enum). Plugins build from built-in
   entities plus extension data; custom entity types are out of scope.
3. **Files are `od-files`' job** (see "Files" below). The in-memory types are
   not serialised directly; each format converts to and from its own schema.
4. Properties shown in the panel come from the entity type plus the common
   attributes, so a new attribute adds its row in one place.

## Files

- **Native format:** `.odx`, a JSON file with a header (`"format":
  "opendraft-drawing"`, `"version"`) and a `drawing` holding the layers, the
  objects in draw order, the current layer and the id counters (so ids of
  deleted objects are never reused, even after reopening). It is readable and
  diff-friendly on purpose, and the schema (`od-files/src/native.rs`) is
  separate from the in-memory types so refactoring cannot change what is on
  disk by accident.
- **Versioning:** only `CURRENT_VERSION` is read. A newer file is refused
  ("saved by a newer version") and so is an older one ("older pre-release
  version"); the file is parsed once, and only when that fails is its header
  read to say which. Adding an optional field needs no new version (readers
  ignore unknown fields); anything else bumps `CURRENT_VERSION`.
- **Block expansion budget:** nesting is a graph, so depth alone does not bound
  the work (a block inserting the next one twice, forty levels, is 2^40
  objects from forty definitions). Expanding an insert visits at most
  `MAX_EXPANSION` (1,000,000) objects, nested ones included, and then stops;
  `BlockDef::expansion_size` counts it per definition (memoised), and the
  native loader refuses a file with a block over the budget.
- **Limits and ids:** a file over `MAX_BYTES` (512 MB) is refused before it is
  read. Id counters advance with `document::after`, which saturates, and a
  loaded drawing goes through `Document::validate` before it is handed over,
  so an id of `u64::MAX` is refused as damaged, not wrapped or panicked on.
- **Unknown entity kinds are refused, never skipped.** A reader that dropped
  an object it does not understand would delete it from the user's drawing the
  next time they saved. So adding an entity kind bumps `CURRENT_VERSION`, and
  an older build reports the file as saved by a newer version. Only optional
  fields can be added without a bump.
- **Version history:** none is kept before the first release (the repo is
  private and no drawings need to keep opening). From then on each older
  version gets a conversion in `read`.
- **Validation:** loading rejects what cannot exist (an object on a missing
  layer, repeated ids, a circle with no radius, no layer 0) with a message, and
  never loads half a drawing. Saving refuses what could not be read back.
- **Formats are pluggable:** `FileFormat` (name, extensions, read, write);
  `format_for(path)` picks by extension. `read` returns the drawing plus
  warnings (what did not come across) and whether it was an import; `write`
  returns the bytes plus notes (what the format could not keep). Two optional
  methods: `read_cancellable` (a slow reader gives up when the flag is set;
  the default ignores it) and `check_size` (refuse a file by its length
  before it is read into memory). The app opens through `load_cancellable`,
  which checks the size, reads, then dispatches, and passes the open job's
  Cancel flag; `FORMATS` is native, DXF, DWG, and the Open dialog's filters
  come from it.
- **DWG** (`od-files/src/dwg/`, over the `od-dwg` crate) is import only, on
  the same lifecycle as DXF: opening a `.dwg` installs an unsaved drawing with
  no file of its own and says "Imported", Save asks for an `.odx` name, and
  `write` refuses (no Export DWG command exists). Files over the reader's
  1 GiB limit are refused before they are read.
- **DXF** (`od-files/src/dxf/`) is an exchange format, so it is treated as one:
  opening a `.dxf` is an *import* (a new unsaved drawing with no file of its
  own, so Save asks for an `.odx` name) and `file.export_dxf` writes a copy
  without changing the drawing, its name or its unsaved state. We wrote our own
  reader and writer rather than use a crate because the available crate drops
  layer frozen/locked flags and cannot report what it skipped.
  - *Reading* takes any ASCII DXF version: lines, circles, arcs, splines (see
    "Splines"), ellipses (in any group order; a -Z extrusion runs the parameters the other way, a
    tilted one is skipped), lightweight and old-style polylines (with their bulges as arc segments), layers (colour, true colour, linetype, lineweight,
    on/frozen/locked), entity colour, linetype and lineweight. ARC entities
    stay arcs and polyline bulges stay bulges. Layouts and their viewports and sheet objects come in too (see "Layouts"), and named blocks with their references (see "Blocks"); hatches (see "Hatches"); everything else (text, dimensions)
    is skipped and counted in a warning shown in the command line. Files over 1 GiB are refused before
    they are read.
  - *Writing* is R12 (AC1009), which every CAD program opens and which needs no
    handles or object tables, except for a drawing with layouts, ellipses, splines or hatches, which is written as R2000 (AC1015; see "Layouts"): R12 has no ellipse, spline or hatch entity, and the R12 writer refuses one rather than approximate it. R12 cannot hold lineweights or colours beyond the
    255 AutoCAD indexes, so those are dropped or approximated and the notes say
    so; rectangles are written as closed polylines.
  - *Tests:* the writer's output is parsed by an independent library (`dxf`, a
    dev-dependency) and, in `tests/fixtures`, a DXF made by `ezdxf` checks the
    reader while `check_export.py` has `ezdxf` audit our output. Neither is
    AutoCAD itself, so a file that matters should still be opened in one.
- **Saving is atomic:** bytes go to a temporary file in the same folder, then
  rename over the target, so a crash or full disk never replaces a good file
  with a broken one. The temporary name (`.name.odx.<pid>-<n>.tmp`) is unique
  to this process and save and is created exclusively, so it can never be
  another instance's file or one the user made, and the cleanup after a
  failure only ever deletes a file this save created. The file is synced
  before the rename and, on Unix, the folder after it.
- **File work runs off the UI thread** (`od-app/src/io.rs`, `file_io.rs`).
  Open, save, export and autosave each have a `begin_*` half, which takes what
  the job needs (a path, or a *copy* of the drawing) and hands it to a worker,
  and a `finish_*` half that `AppState::poll_io` runs on the UI thread when the
  result arrives. The workers never see application state, so there is nothing
  to lock. They wake the window when they finish, since egui only draws when
  something happens. There are two kinds of job:
  - *Ordered* (recovery copies and their removal) share one worker and run in
    the order submitted, so a copy is never deleted before the write that made
    it has finished.
  - *Cancellable* (open, save, export: what the user waits for) each get a
    thread of their own, so one stuck on a slow disk or a dead network path
    blocks nothing else.
  - *Edits during a save:* the save writes the copy taken when it began and
    records the history position of that copy (`Session::mark_saved_at`), so
    anything done meanwhile still shows as unsaved.
  - *One at a time:* a drawing already being saved, or a file already being
    read, is not started twice. An autosave pass is skipped while the last
    one is still writing.
  - *Closing:* the "save changes?" question stays up showing "Saving..." until
    the file is written, and only then closes the tab or moves on. The window
    does not close while a file is being written (`AppState::can_quit`).
  - *Tests:* the app runs the worker on its own thread; most tests use an
    inline worker (`Io::inline`) that runs each job at once, and
    `file_io.rs` tests use the real thread.
  - *Cancelling:* after a second of file work the status bar shows a Cancel
    button, and Esc does the same (a quick save is not cancelled by a habitual
    Esc). A thread cannot be stopped from outside, so cancelling means
    forgetting it: its result is thrown away when it arrives, and a save that
    was cancelled checks its flag before the rename (`save_cancellable`), so a
    write that was stuck and finishes later never replaces the user's file. The
    drawing stays unsaved, a quit that was waiting on the write is called off,
    and the unsaved-changes question comes back so the user can try again.
    A cancelled job's thread may linger until the OS call returns.
  - *Not moved off yet:* recovering drawings after a crash reads its few
    files on the UI thread, and the file dialogs are the system's own.
- **Unsaved changes** are tracked by the undo history: `Session::is_modified`
  compares where the history is now with where it was at the last save, so
  undoing back to the saved state clears the star. Closing a tab or the window
  asks Save / Don't Save / Cancel for each modified drawing, one at a time
  (`od-app/src/files.rs`; the dialog is `app/file_dialogs.rs`).
- **Folders the user may move** are `locations::Location` (Templates, Recovery): the choice is an `Option<PathBuf>` field in `Settings` (none means the default, beside the saved settings), picked on Settings > Files, and read only through `Location::resolve`. New code never builds a path from `eframe::storage_dir`; a new movable folder is a variant in `Location`. Templates apply at once (`apply_settings` updates `AppState::templates_dir`); Recovery is read at start. Approved automation folders stay on the Automation page (a security grant, not a location).
- **Autosave and recovery** (`od-app/src/recovery.rs`, `autosave.rs`): every
  few minutes (Settings > Files; default 2, can be off) each drawing with
  unsaved changes is written to this run's session folder under the app data
  directory, and the copy is deleted when the drawing is saved, closed or clean
  again. A normal exit deletes the whole folder, so only a crash leaves one. A
  running instance holds an exclusive lock on a file in its folder; a folder
  whose lock can be taken belongs to a dead instance, which is how the next
  start finds the leftovers without mistaking another running copy's files for
  a crash. Recovered drawings open as unsaved (and, if they had a file, keep
  its path, so Save writes back to it); closing the offer keeps the copies for
  next time.
- **The app** opens files from the Open command (Ctrl+O), the recent list in the
  application menu (the "A" button), drag and drop onto the window, and paths on
  the command line (so a file association works). A drawing already open is
  switched to, not opened twice; a blank untouched drawing makes way for the
  one being opened. File dialogs sit behind `FilePicker`, so tests script them.

## Commands and the Editor (built)

The command model follows the AutoCAD .NET SDK, using the same words so people
who know it find their way, minus its ceremony.

**Status.** Every interactive command is written this way: Move, Copy, Rotate,
Mirror (`od-app/src/modify.rs`), Scale and Rotate with Copy and Reference
(`scale_rotate.rs`), Line (`draw.rs`), Circle (`circle.rs`), Arc (`arc_cmd.rs`),
Rectangle (`rectangle.rs`, settings in `rect_memory.rs`), Polyline
(`polyline_cmd.rs`, arc options in `polyline_arc.rs`), the window polygon, crossing polygon and fence
selections (`selection.rs`), Trim and Extend (`edges.rs`), Offset
(`offset.rs`), Fillet and Chamfer (`corners.rs`, settings in
`corner_memory.rs`), Join (`join.rs`), Explode (`explode.rs`), Break (`break_cmd.rs`), Stretch (`stretch.rs`) and the grip editor (`grips.rs`, which
starts a "Stretch" command when a grip is pressed). The old model, where a canvas `Tool`
was armed and each command was spread over a prompt table, a typed-input
handler and the canvas, is gone. The canvas now only does three things for a
command: show its ghost and rubber band, snap and report placed points, and
select by clicking or dragging a box when no command is waiting for a point.
**A new interactive command is one function in one place.**

**Names.**

| Name | Meaning |
|---|---|
| `Document` (`od-core`) | The drawing data: objects, layers. Pure, no UI. |
| `Session` | A `Document` plus its undo history. Headless and testable. |
| `Doc` (`od-app`) | An open window: a `Session` plus view and selection. |
| `Editor` | What a running command is handed to talk to the user and to edit. |
| `Runner` | The UI side of a running command: its thread, its pending prompt. |
| `Command` | A named function that takes an `Editor`. |

**A command is straight-line code that asks and waits**, returning
`Result<(), Cancelled>` (Esc cancels any prompt). This is MOVE, nearly as built:

```rust
pub fn move_objects(ed: &mut Editor) -> Result<(), Cancelled> {
    let ids = ed.pickset_or_select("Select objects")?;
    let base = ed.get_point("Specify base point")?;
    let to = ed.get_point(PointOptions::new("Specify second point").base(base))?;
    let (dx, dy) = (to.x - base.x, to.y - base.y);
    ed.transaction("Move", move |tx| {
        for id in ids {
            tx.modify(id, |e| e.translated(dx, dy));
        }
        Ok(())
    })
}
```

It is registered in `commands.rs` with `s.start_command("Move", modify::move_objects)`.
The selection when the command starts is its `pickset`; `pickset_or_select`
uses it or, if nothing is selected, asks the user to select objects.

**The prompt service.** `get_point`, `get_distance`, `get_angle`, `get_number`,
`get_string`, `get_keyword`, `get_selection` return the value, or `Err(Cancelled)`.
When a prompt has keywords (`Options::keyword("Undo", "U")`) or a bare Enter
means something, use `ask_point`, `ask_distance`, `ask_angle`, `ask_number` or
`ask_string`, which return an `Answer`: `Value`, `Keyword(label)` or `Enter`.
An option menu that ends on Enter uses `ask_keyword` (`Some(label)` or `None`):
typed letters match its keywords, which `ask_string` would take as text.
Leave the keyword list out of the message; the prompt line adds it.
Options (`PointOptions`, ...) hold the message, keywords, a default (Enter
takes it), a base point (rubber band, relative input, distances and angles
measure from it), a `ghost` callback that returns the entities to preview at
each pointer position and a `marker` callback for marks that keep one size on
the screen (it is given how many drawing units a screen point spans). An object prompt (`ask_object`) picks only objects
that can be edited unless `Options::pick_locked` lets it reach locked layers
too (LAYULK). Angles are radians; the user types degrees. The canvas
and the command line answer the same prompt, so the status-bar text, the
clickable options and the typed forms (`x,y`, `@dx,dy`, `d<a`, a bare distance
along the cursor direction) live in one place (`od-app/src/interact.rs`).

**A running command owns the drawing.** While a command waits for the user,
`Registry::busy_when(AppState::command_running)` disables every command
except those marked `.transparent()` (cancel, zoom and the view toggles,
panels, settings, exit, and starting another interactive command, which
replaces the running one). Undo, redo, erase, save, open, new, close and the
layer commands wait. Edits made outside the registry (the Layers and
Properties panels) go through `AppState::perform`, `set_property`, `undo` and
`redo`, which end the running command first, so a command never works on a
drawing that changed under it. A new command is blocked by default; it only
runs mid-command if it says so.

**Threads.** A command runs on its own thread and blocks on a channel for each
answer. Anything touching the drawing (`ed.read`, `ed.transaction`) is sent to
the UI thread and run there between frames, so the drawing is never shared and
a transaction is atomic as far as the UI goes. `Runner::poll` does that work
each frame; Esc drops the reply channel, which makes the command's wait fail
with `Cancelled`. The work a command hands to the UI thread
(`ed.read`, `ed.transaction`) runs under `catch_unwind`, so a panic in it
ends that command as `Crashed` instead of the application. (A panic inside a
transaction can leave its edits half applied; the debug validation reports
that. Plugins stay out of process, so this only concerns built-in commands.) This is exactly how a Python plugin will call it, and tests
drive a command with `run_scripted` (a list of answers in, a `Transcript` out).

**Transactions.** `Session::transaction` and `ed.transaction` record edits as
they are made. Commit merges them into one undo step. An error or cancel
inside it reverts them, so a cancelled command leaves no trace. There are no
read/write open modes and nothing to dispose: `tx.add(entity)` appends,
`tx.modify(id, |e| ...)` changes, `tx.remove(ids)` deletes, `tx.document()`
reads. Code never keeps an object reference across transactions, only ids.

**Registration.** A command has a stable id, a typed name (`MOVE`), a label,
optional shortcut, `enabled` and `checked` predicates. The one flag built is
`transparent` (may run inside another command, for example zoom). Still to
build: finer flags (document-mutating, file operation) if `transparent` proves
too coarse, and a user-editable alias table in place of `cmdline::ALIASES`.

**Change reporting.** Every `Action` reports a `Changes` value: the ids it
added, removed and changed, and whether it touched layers. `Session::perform`
returns it, and undo and redo return it inverted in a `Step`. The app can use
it to refresh panels, and plugins can later subscribe to document events.

**Owner of objects.** Objects of model space and of each layout share the
document's list (`Object::layout`); objects of a block definition live in its
`BlockDef` (see "Blocks").

**Not taken from the SDK:** a global application object (the context is passed
in), mutable entity objects that are built and then appended, `Dispose`/`using`,
and 3D types.

**Migration order:** (1) rename `Editor` to `Session` and add change reporting
(done); (2) transactions (done); (3) the `Editor`, prompt service and runner
(done); (4) port every command and remove the `Tool` enum and `prompt_for`
(done).

## Home tab (built)
The house at the left of the document tabs shows the Home page (`app/home_page.rs`: new, open, drawings open now, recent files with date and size) in place of the dock. `AppState::home` says it is showing. The page is built from named sections (`header_section`, `open_drawings_section`, `recent_section`, `recovery_section`, `start_section` (the From template dropdown is `od_ui::menu::popup`, never a window), `whats_new_section`), each under a fixed `push_id`, with rows keyed by what they stand for, never by list position. The placeholder is filtered out of everything outside the UI (automation lists, ids and events). The recent list (`recent.rs`) is a list of records with stable `RecentId`s, ten at most with five pinned; only `add`, `add_replacing` (Locate), `remove` and `clear_unpinned` change what is remembered, so a failed open never forgets a drawing. `home_scan.rs` reads the files behind it on a worker thread; the page never does disk access itself. Items of a list that can reorder are keyed by record id with explicit widget ids (never egui auto ids, which shift with position), so keyboard focus follows the file. Previews: `od-canvas::preview` renders a document to a 512 x 320 RGBA picture with no egui (it asks `od_core::plot_model` for the paths, so it follows the plot rules); `od-app/src/previews.rs` owns the worker thread, PNG sidecars (`previews/` beside the settings, keyed by real path, date, length and `RENDERER_VERSION`) and their trimming; `preview_textures.rs` the textures the page shows. Pictures are made only after a drawing has opened or saved, from the copy already in memory. Recovery is offered on Home, per crashed session (`AppState::pending_recovery`, `recover_session`, `discard_session`, `defer_recovery`): a recovered drawing has no `path`, only `recovery_origin` (where Save As starts), so Save can never overwrite the original. The page lays out by `home_page/layout.rs`: one column below 900 px of window width, two (main and side) from there; test any change at 320 to 3840 px and 1x to 2x (`dynamic_tests/home_layout.rs`). Colour contrast of text and the focus ring is enforced by `od-ui/src/theme_contrast_tests.rs`. The What's new card is a compile-time table (`od-app/src/whats_new.rs`, one `Note` per exact version; see "Releasing" in `CONTRIBUTING.md`); dismissal is stored by exact version and nothing is fetched. Templates are `.odt` files (`od-files/src/template.rs`: the same contents as `.odx`) in a `templates/` folder beside the settings, plus starters built into `od-app/src/templates.rs`; `AppState::new_from_template` only reads one and makes a new unsaved drawing. No drawing open is allowed: `docs` then holds one hidden `Doc` with `placeholder` set (no tab, never saved, not counted by `has_drawings()`), so the many `docs[active]` sites cannot panic; `push_document` drops it. While `home` is set the registry (`Registry::needs_drawing_when`) enables only commands marked `.global()` (file, exit, settings, panel toggles). A new command is drawing-only unless it says `.global()`. The app starts on Home when it was given no files. Selecting tabs goes through `show_home` and `show_document`, never by writing `active`.

## Ribbon (built)
`od-ui::ribbon` draws a ribbon declared as data and knows nothing about the document: a `Panel` holds `Column`s (`Large`, `LargeSplit`, `Stack`, `Rows`), a `Rows` column holds up to three rows of `Cell`s (labelled or icon-only buttons, split buttons, and `Slot`s), and every item names a command id. A `Split` runs the item last picked from its list (`RibbonState::split_picks`, by the command of its first item); `Split::labelled` keeps a fixed label, as AutoCAD's Paste. A `Slot` is room for a control the app draws: `Ribbon::show` returns a `RibbonOutput` with the click and each slot's rect, and `od-app/src/ribbon_slots.rs` draws the layer list and the colour, lineweight and linetype lists there and returns a `SlotEdit` (they change the selection; with nothing selected the layer list sets the current layer and the others set the colour, linetype and lineweight of new objects, `Document::new_object_attributes`). The layouts are in `od-app/src/ribbon_def.rs` (the Home tab in `ribbon_def/home.rs`, panel for panel as AutoCAD 2020's). An item whose command is not registered shows dimmed; Home's are listed in its `NOT_BUILT` test, which fails when a listed command gets registered or an id is misspelt, so take an id off the list when its command lands.

## Icons (built)
An icon is a constant in `od-ui/src/icons.rs` (declared with `icons!`, which also lists them all in `icons::ALL`), named after the command it is for (`DRAW_LINE`: the command id `draw.line`) or the part of the window (`LAYER_PANEL_FROZEN`, `RIBBON_SPLIT_ARROW`), so each has its own drawing, `assets/icons/<name>.svg` (`draw-line.svg`). A constant's value only identifies the icon (a private use character, not a font character); `icons::glyph` gives the Lucide font glyph that is drawn when there is no file, and two icons may share a glyph. A ribbon item or menu row for a command uses that command's icon, `command.id` upper-cased with `_`; if a menu wants a different picture for the same command it gets a `CONTEXT_` name. **Draw icons through `od_ui::icon_set`**: `paint` for the painter, the `Icon` widget for buttons, toggles and labels, `image` for egui widgets that take an image; never `FontId::new(.., icon_family())` directly. They draw the SVG when there is one, else the glyph. Every drawing is also built into the program (`od-ui/build.rs` lists `assets/icons/*.svg` into `EMBEDDED`; the glyph is only for an icon with no file at all), so an install needs no icon folder. A folder replaces drawings by name, and is read once at start-up (`od-app/src/icon_library.rs`: `OD_ICONS`, else `icons/` beside the program, else, in a debug build only, the source tree's `assets/icons`), so an edited drawing shows on the next launch with no rebuild; a file that cannot be read is named in the command line and the built-in drawing is kept. A release build carries no source path. A drawing is rasterised with `resvg` the first time it is drawn at a pixel size. A *plain* drawing (every colour grey or white, as Lucide's; `currentColor` is white) is kept as a white mask and tinted with the colour asked for, so it follows the theme and dims when disabled; a *coloured* one (any clearly coloured part) is drawn in its own colours and only fades when disabled. Which it is is worked out from the file when it is read. `scripts/export_icons.py` writes the library (Lucide's original SVGs; files that exist are kept unless `--force`) and its index, `assets/icons/README.md`.

## Dynamic Input (built)

Dynamic Input is another input surface over the Editor prompt service. Commands
still run on the Runner and all accepted answers pass through `interact.rs`;
there is no second command engine. `coordinates.rs` supplies the shared finite
Cartesian/polar parser: command-line coordinates remain absolute by default,
while cursor input is relative at prompts with a base. `#` forces absolute and
`@` forces relative in both surfaces.

- Each `Doc` owns `DynamicInputState` (`dynamic_input.rs`): semantic field IDs,
  draft, scalar/coordinate locks, menu selection, placement, cursor and errors.
  A reply, cancellation, drawing switch, disabled DYN or another text editor
  clears transient state. Cursor motion preserves it.
- `Options::dimensions(DynamicDimensions::{Polar, Cartesian})` declares point
  dimension fields without adding UI dependencies to `od-commands`. Line,
  Move, Copy, Mirror and grip point edits opt into polar dimensions; Rectangle
  opts into Cartesian. Scalar prompts already declare Distance/Angle/Number.
- `app/dynamic_input.rs` arbitrates keyboard ownership and submits answers.
  The raw-input hook captures navigation before egui applies spatial focus
  movement, so Tab follows semantic fields and Down opens options. The docked
  command line and other text fields keep their normal keyboard ownership.
- `app/dynamic_overlay.rs` draws compact rectangular foreground tooltips with
  stable TextEdit IDs. At a prompt with a base point the fields sit on ghost
  dimensions, as in AutoCAD (`dynamic_input/dimensions.rs`, pure screen-space
  geometry): a vector gets a linear dimension standing 30 px off the line and an
  angular arc from the zero direction (honouring ANGBASE/ANGDIR); a rectangle
  gets two orthogonal dimensions outside its edges; a lone distance gets the
  linear one. Too short to draw (under 24 px), absolute coordinates, or no base:
  the fields sit in a row with the prompt beside the cursor. The prompt stays
  near the cursor and flips and clamps with a 24-point dead band
  (`dynamic_input/placement.rs`). Boxes are nudged by `avoid` so none overlaps
  another, the prompt, or the 56-point square kept clear around the crosshair.
  Hovering an input freezes its anchor so it can be clicked. Numeric fields do
  not open secondary hover tooltips over the drawing.
- `View` receives a cursor constraint and overlay hit rectangles. Its snapped
  or trailing cursor drives values; constraints drive ghosts, rubber bands
  and placed points. Only accepted answers update the last-point origin.
- `DynamicInputSettings` is persisted with Settings. F12 taps toggle and holds
  temporarily suppress input (350 ms distinguishes a hold); the DYN status
  button is the same transparent registry command. Its context menu opens the
  Dynamic Input group in Settings > Drafting. Variables: `DYNMODE`,
  `DYNPROMPT`, `DYNPICOORDS`, `DYNPIFORMAT`, `DYNPIVIS`.

Still to build: entity-specific grip dimensions and `DYNDIVIS`/`DYNDIGRIP`,
recent-input recall and tracking aids. The fields show and read lengths and
angles in the drawing's units (see "Units"). Headless egui tests exercise focus, input/navigation in a single frame,
locking, menu keyboard/mouse actions, rejected clicks, F12 and viewport edges
at 100%, 125% and 150% scale.

## Ortho (built)

`od_core::ortho` holds the maths (`axis_of`, `on_axis`, `constrain`). `od-canvas` applies it where the pointer is resolved: `View::aim` builds an `Aim` (document, snap modes, grid step, aperture, `Ortho`, and the prompt's base when ortho is in force) and `Aim::resolve` returns the final point and the snap behind it, so the preview, the placed point and the readout cannot disagree. `Ortho { enabled, strict }` is a per-user setting passed in with `View::set_ortho`. The snap is found first, as without ortho. Standard lets an object snap win (the grid snap and a bare cursor are held to the axis); strict projects the snapped point onto the axis chosen by the cursor (`hold` in `snap_axis.rs`), keeps the marker where the snap was found and draws a faint guide line from it to the held point. `ORTHOMODE` and the F8 command change `Settings::ortho.enabled`, and `ORTHOLOCK` (0 standard, 1 strict; our own name, AutoCAD has no equivalent) changes `Settings::ortho.strict`. Neither is saved in the drawing.

## Object snap (built: running snaps, Tab cycling; overrides and tracking not yet)

Follows `private_material/autocad_snapping_behaviour.md`. One pass gathers every snap point under the pointer and ranks them; the view shows the first, or the one the user cycled to.

- **Pure core in `od-canvas/src/snap/`.** `snap_all(objects, &Probe)` returns the ranked list; `snap` is its first element. `fixed.rs` holds what an entity offers by itself (endpoint, midpoint, centre, node, quadrant, insertion), `context.rs` what needs two objects (intersection) or the command's last point (perpendicular, tangent), `cycle.rs` the Tab choice. The maths for feet and tangent points is `od-core/src/curve_snap.rs` (`Curve::perpendicular_feet`, `tangent_points`, `bounds`), over lines, circles, arcs and ellipses; intersections reuse `Curve::intersections`.
- **Ranking is our policy, not AutoCAD's (the spec says it publishes none).** Classes, best first: a point inside the aperture (every object snap, Perpendicular and Tangent included: the pointer has to be near the foot or tangent point itself, not merely over the curve); Nearest; the grid. Inside a class the closer to the pointer wins, then the more specific mode (`SnapKind::priority`), then source ids and position, so the order is deterministic. A point several modes find is listed once, as the most specific.
- **Identity and cycling.** `SnapPoint` carries its source objects; `SnapPoint::id()` (kind, sources, position to a micro-unit) lets the choice survive the pointer drifting. `View` keeps the chosen `SnapId`: Tab moves to the next (Shift+Tab back, our extension), wrapping; a choice no longer listed is dropped. The pointer used for the search is always the physical one; the magnet only moves the drawn crosshair (`SnapModes::magnet`), so a snap never feeds itself.
- **Tab versus Dynamic Input.** Tab is the canvas's when more than one snap point is under the pointer and no dynamic field is being typed in (`View::snap_cycles`, checked in `route_dynamic_input`); otherwise it moves between dynamic fields as before.
- **Settings.** `SnapModes` (a bool per mode, plus `marker`, `tooltip`, `magnet`, `aperture_box`) is saved with the user's settings; `OSMODE`, `AUTOSNAP` and `APBOX` change it. The status bar has an INT button and the SNAP button's right-click menu lists every mode; Settings > Drafting has only the AutoSnap display options (marker, tooltip, magnet, aperture box), not the modes or the Snap on switch. Each mode has a `view.snap_*` command.
- **Power Select** (`power_select.rs`, `app/power_select_dialog.rs`; AutoCAD's QSELECT under our own name). `POWERSELECT` (aliases `PSE`, `QSELECT`, `QSE`), the Selection panel's extra row and the context menu open a dialog laid out like QSELECT's: Apply to (Entire drawing, or Current selection, with a pick button that clears the selection, runs a selection prompt and comes back), Object type (Multiple or a kind present in scope), Properties (Color, Layer, Linetype, Linetype scale, Lineweight, then the kind's Properties-panel rows through `properties::rows` and `properties::value`), Operator, Value, Include or Exclude, and Append. Everything is pure and tested in `power_select.rs`: `kinds_in`, `props_for`, `values_present` (the Value list is what the drawing has), `value_is_usable` and `run`, which returns the ids. Numbers compare in the drawing's units with a 1e-6 relative tolerance (`Equals`, `Not Equal`, `Greater`, `Less`); text compares ignoring case, and layers and linetypes also take `*` and `?` (Wildcard Match). Exclude selects everything in scope that is not a hit. Only selectable objects of the space being edited are searched. Flags, fills, styles and arrowheads are picked from the values present rather than typed. Not built: Color as an ACI number or name, Lineweight as a number, properties of block attributes, and a `QSELECT` that takes its answers on the command line.
- **Clipboard** (`clipboard.rs`, `od-core/src/document/transplant.rs`, `od-commands/src/paste_action.rs`). `COPYCLIP` (Ctrl+C), `CUTCLIP` (Ctrl+X), `COPYBASE` (Ctrl+Shift+C), `PASTECLIP` (Ctrl+V), `PASTEBLOCK` (Ctrl+Shift+V) and `PASTEORIG`. egui turns Ctrl+C/X/V into `Event::Copy`/`Cut`/`Paste` rather than key presses, so `clipboard::shortcut` reads those events. What is copied is a small drawing: `Document::extract(ids)` clones the objects and every table (layers, text, dimension and multileader styles, linetypes, blocks). `Document::absorb(source, Placement, layout)` brings it into any drawing: layers, text, multileader and dimension styles and blocks are matched by name (ignoring case; the target's definition wins) and the missing ones are copied with their ids remapped, including the text style inside MText, dimensions and multileaders and the contents of nested blocks; linetypes the objects or new layers use are copied by name. `Placement::Loose` moves the objects; `Placement::Block` makes one `A$Cn` block (base point = the clipboard's) and one reference to it. `absorb` returns an `Absorbed` (tables before and after, the objects), so `PasteDrawing` (the action) undoes and redoes it exactly in one step. The clipboard lives in `AppState::clipboard`, a `ClipboardSlot` (an `Arc<Mutex<..>>`) so COPYBASE, which runs on a command thread, can fill it. Copying also writes the drawing to the system clipboard as text (`opendraft-clipboard <base x> <base y>` then the `.odx` JSON, via `NativeFormat`; `arboard`). A paste first looks at the system clipboard: text that is not what this app last wrote and parses as ours (another window) is used; anything else leaves the last internal copy in place. Not built: pasting plain text as MText, pasting pictures, copying other drawings' layouts or viewports, and keeping a changed definition when a name already exists (the target's wins silently). Outside callers cannot use these commands (`automation/policy.rs`).
- **Context menu.** A right click on the drawing with no command running opens `app/context_menu.rs` (`AppState::context_menu`, set from `CanvasOutput::context_menu`; a running command still takes the right click as Enter, and Shift/Ctrl+right-click at a point prompt is the snap menu). Rows are data (`Row`, `Kind`, `Action`): Repeat the last command, Recent Input (`CommandLine::recent`, newest first, set by `note_recent` with the commands that make something), Clipboard, Isolate, the Modify rows, Draw Order, Group, Select Similar (`AppState::select_similar`: same kind and layer), Deselect All, Power Select, QuickCalc, Find, Properties. The editing rows only show with a selection; without one the menu offers Undo, Redo, Select All and Zoom Extents. Rows with nothing behind them yet (Isolate, Draw Order, Group, QuickCalc, Find) are greyed (`Kind::Soon`); filling one in is a change of `Kind` to `Item`.
- **One-shot overrides.** Typed at a point prompt (`END`, `MID`, `CEN`, `NOD`, `QUA`, `INT`, `INS`, `PER`, `TAN`, `NEA`: three letters or more of the name, several with commas, `NON` for none), or picked from the menu Shift+right-click (or Ctrl+right-click) opens at the pointer (`app/snap_menu.rs`). They replace the running snaps for the next point only, even with SNAP off (`SnapOverride::applied`), and are spent by any answer. A click that finds no snap places nothing and does not fall back to the grid (`Aim::place`; the spec's better-supported reading); it says "No Endpoint snap found" and lets go of the override so the prompt is never stuck behind it (`CanvasOutput::refused`). A word the prompt itself offers as an option is the option's. `snap_once.rs` is the pure state (`OneShot`); `once_input.rs` hooks it into `reply`, `answer_typed` and `answer_dynamic`. `FRO` (base point, then an offset typed relative to it or clicked) and `M2P` (middle of two points) are chains in the same state: each point is consumed until the chain gives one, and the rubber band starts at the base meanwhile. The status line shows `[Endpoint]` or the chain's question.
- **Deferred Perpendicular and Tangent** (spec 7.11, 7.12). With no last point (the first point of a Line, say) the foot or tangent point cannot be worked out, so the engine offers a *deferred* snap at the nearest point of the object under the pointer (`SnapPoint::deferred`, tooltip "Deferred Perpendicular"; Tangent only on curves). The click is held, not given to the command (`AppState::defer_one_shot`, only at a point prompt with no base): `OneShot` keeps the object and where it was picked (`Deferred`). From then on the rubber band starts at the point the object would give for the pointer as the next point (`Deferred::point`, the foot, or the tangent point nearest the pick), so the line stays square to, or tangent to, the object. The next point settles it: the command is given the settled point first and the clicked point after (`reply`, `send`). If there is no solution from that point (a foot past a segment's end, a tangent from inside a circle) it says so and waits for another; Enter drops it.
- **Extension and Parallel follow what the pointer paused over** (`snap/track.rs`, pure; `view/acquire.rs`, the state). Resting on a line, arc or elliptical-arc end for 0.3 s keeps it (a "+" appears); resting on a straight segment, when no end is in reach and there is a last point and ortho is not holding the point, keeps its direction. Resting again on something kept lets it go. What is kept lasts until a point is placed or the prompt ends, and survives the pointer crossing a prompt box. Each kept item is resolved against the drawing every frame (`resolve`), so a changed or erased object drops out. Extension continues a line along its direction and an arc or elliptical arc round its own circle or ellipse (never along the tangent), over the part the curve does not cover, and not within the aperture of the end (the end's own snap is there). Parallel is the line through the last point in the kept direction, both ways; its "+" sits where the pointer paused and its path is drawn as the whole line through the last point across the view. An Extension path is drawn dotted from the end to the snap point. Sources: lines, straight polyline segments and rectangle edges. `acquire.rs` is the one place that decides what a pause keeps, so object snap tracking can reuse it.
- **Object snap tracking and polar tracking** (spec Part II; `snap/otrack.rs` is the pure geometry, `view/acquire.rs` and `view/snap_choice.rs` the state). A snap point the pointer rests on for 0.3 s (endpoint, midpoint, centre, node, quadrant, intersection, insertion) is kept with a "+" (`Acquired::Origin`), and resting on it again lets it go; Shift stops a pause from keeping (or, with `POLARMODE` bit 8, is needed for it). Running snaps off suspends the kept points. Each kept point gives paths (infinite lines) along the horizontal and vertical, or along every polar angle with `POLARMODE` bit 2; polar tracking (F10) gives the paths from the command's last point. A path gives a snap point where the pointer is within `REACH` (0.75) of the aperture of it and outside the aperture of where it starts; two paths from different points give their crossing while the pointer is within reach of both and the crossing is on screen. Paths that are straight lines through a point are exact on the axes (`directions`). Ranking, best first: direct snap points; path crossings; path points (Tracking, Extension, Parallel); Nearest; polar points; the grid (spec section 22: an explicit override, a direct snap, tracking, Nearest, then the constrained free point). The tooltip names where the path starts and the distance and angle from it (`Endpoint: 100.0000 < 0°`), and flips to the other side of the marker at the canvas edge. Extension and Parallel points carry their straight path too, so their tooltips read `Extension: 60.0000 < 0°`. **A typed distance follows a path:** with the pointer on exactly one straight path (a tracking, polar, Extension or Parallel point), a distance typed in the command line (`track_distance`) or in the dynamic input's distance field (`commit_dynamic_input`) goes that far from where the path starts, in the pointer's direction along it, not from the last point. **`TT`** (typed, or in the Shift+right-click menu) takes the next point as a temporary tracking point instead of the answer: it shows a "+" and tracks like a kept point until the point it helps with is given (`OneShot::temps`; it cannot be taken back by pausing on it, only by Esc). **`TK`** collects tracking points, each held to an axis from the one before (ortho is forced strict while it runs), and Enter answers with the last. `TRACKPATH` chooses full-screen or to-the-cursor drawing for polar and for object snap paths (`Tracking::full_path`, `shows`). Settings: `Settings::tracking` (`Tracking`), `AUTOSNAP` bits 8, 16 and 32, `POLARANG`, `POLARADDANG`, `POLARMODE` (bit 1, measuring from the last segment, is refused), `TRACKPATH`; F10 and F11 and the POLAR and OTRACK status buttons. Ortho and polar tracking exclude each other (`Settings::set_ortho`, `set_polar`). Intentional difference from AutoCAD's initial `AUTOSNAP` of 63: polar tracking starts off (ours is 55), since ortho already exists and polar snapping changes how the pointer feels.
- **Pieces, not only entities (`snap/context.rs`).** Intersection, Perpendicular and Tangent work over *pieces*: the segments, arcs, circles and ellipses of an entity and its spline, and, for a block reference or a dimension, the pieces of what it draws (`Scene`). Splines are cut into short straight pieces and each crossing is refined on the spline itself (`od-core/src/spline/snap.rs`: chord zoom against a curve, Newton against another spline), so the point is on the curve; feet and tangent points are found by sign changes of the derivative test along the parameter. A polyline's or rectangle's own corners are Endpoints, but where two of its segments that are not neighbours cross is an Intersection (`Chain::adjacent`). At most 48 pieces in the aperture are crossed with each other.
- **Geometric Center, Apparent and Extended Intersection.** Geometric Center (`OSMODE` 1024, `GCEN`) is the centre of the area a closed polyline (arcs included) or closed spline encloses (`od-core/src/outline.rs`); none for an outline that crosses itself or has no area; circles, ellipses and rectangles show theirs as Center. Apparent Intersection (2048, `APPINT`) is the same points as Intersection in 2D, labelled and marked as AutoCAD's. Extended Intersection (`EXTINT`; the Extended Apparent form is the same in 2D) is an override only (never an `OSMODE` bit): with it, resting on a line, arc or elliptical-arc end keeps it (as for Extension) and the points where its extension meets another object are offered, ranked with path crossings.
- **Not built:** taking back a `TT` point by pausing on it, `POLARMODE` bit 1 (angles measured from the last segment), Z tracking (2D only), tracking from the base point of a command that gives none, Extension past the ends of polylines and splines, intersections of a spline with itself, hatch and dimension-extension-line exclusions as options (`OSOPTIONS`; hatches are always excluded and dimensions have no extension-line endpoints to ignore), `OSNAPZ`. Points (the entity) do not exist yet, so Node only finds dimension definition points.

## System variables (first slice built)

Settings that a user reads and sets by name, as in AutoCAD: type `GRIDMODE`
at the command line to see it and be asked for a new value (Enter keeps it),
or run `SETVAR` and give the name; `SETVAR` then `?` (or `?GRID*`) lists
variables with their values. Names are upper case and matched ignoring case.

- **The table is generic.** `od-commands::SysVarTable<C>` holds `SysVar<C>`
  entries (name, description, `Kind` with its range, read-only flag, getter,
  setter), like the command `Registry<C>`. It parses and range-checks typed
  text, then calls the setter, which may still refuse a value. It has no
  egui and no knowledge of the app.
- **The list is the app's.** `od-app/src/sysvars.rs` binds the variables to
  `Settings` and the open drawing, so a variable that mirrors a setting is
  saved with the user's settings for free. Built: `GRIDMODE`, `GRIDUNIT`,
  `OSMODE`, `SNAPMODE`, `SAVETIME`, `CURSORSIZE`, `PICKBOX`, `APERTURE`, `GRIPSIZE` (pixel radii of the pick, snap and grip targets; see `PointerSizes` in `od-canvas`), `FILLETRAD`, `CHAMFERA`, `CHAMFERB`,
  `CHAMFERC`, `CHAMFERD`, `CHAMMODE`, `TRIMMODE` (the last seven live in the
  global `corner_memory.rs` and are saved with the user's settings, not the
  drawing), and read-only `CMDACTIVE`, `DWGNAME`.
- **Use AutoCAD's names and values** where the feature matches, so people and
  scripts carry over. `OSMODE` is a bit code (1 endpoint, 2 midpoint,
  4 centre, 8 node, 16 quadrant, 32 intersection, 64 insertion, 128 perpendicular, 256 tangent,
  512 nearest, 16384 off); a bit we have no snap for is refused rather than
  stored. `AUTOSNAP` (1 marker, 2 tooltip, 4 magnet) and `APBOX` are in `snap_vars.rs`. When a variable's meaning differs from AutoCAD's, give it its own
  name instead.
- **A new variable is one entry** in `sysvars.rs::build`. Settings that have
  a UI toggle keep their command; the variable and the command change the
  same field. Do not store state in a variable that is not also somewhere
  the app reads it.
- **Not undoable,** except the drawing-saved ones. Variables kept in the user's
  settings are not drawing edits, so they do not go through `Action`.
- **Scopes.** AutoCAD saves some variables in the drawing (`LTSCALE`,
  `LUPREC`), some with the user (`OSMODE`), some not at all. The user and
  read-only kinds are built, and so is the drawing kind, for the unit
  variables: the getter reads the active `Document` and the setter makes an
  `Action` (`AppState::set_units`), so setting one is undoable and marks the
  drawing modified. A new drawing-saved variable needs a field on `Document`,
  an optional field in the native format (no version bump if optional), a DXF
  header mapping and an `Action`; `units_vars.rs` is the example.
- **Not built:** transparent use inside a running command (`'GRIDMODE`),
  reading and setting from plugins (the table is the API, as the registry
  is), and a Settings window that lists them.

## Units (built)

The UNITS command (alias `UN`, `DDUNITS`; ribbon Manage > Drawing, and the
application menu) sets how a drawing writes and reads lengths and angles.
**The numbers in the drawing never change**: a length is a drawing unit and
an angle is radians counter-clockwise from east. Units only decide the text.

- **The settings are `od_core::Units`**, held by the `Document` and changed
  only by the `SetUnits` action (one undo step, `Changes::units`). They are the
  AutoCAD variables: `LUNITS` (scientific, decimal, engineering, architectural,
  fractional), `LUPREC` (decimals, or the fraction step 1/2^n), `AUNITS`
  (decimal degrees, deg/min/sec, grads, radians, surveyor's), `AUPREC`,
  `ANGDIR`, `ANGBASE` and `INSUNITS` (21 insertion units, kept but not yet used
  to scale anything). All seven are system variables (`units_vars.rs`).
- **Writing and reading are pure** (`od-core/src/units/`): `format_length`,
  `format_angle`, `format_point`, `parse_length`, `parse_angle`, with round-trip
  tests for every format. Architectural and engineering lengths take one drawing
  unit to be one inch. `parse_length` accepts feet and inches (`5'-3 1/2"`), mixed
  fractions and plain numbers in every format. `parse_angle` takes a bare number
  in the drawing's angle format, or letters that say what was meant (`45d30'15"`,
  `100g`, `1.5r`, and bearings `N 45d E`). A typed angle counts from `ANGBASE` in
  the `ANGDIR` direction (`Units::to_display_angle` / `from_display_angle`);
  surveyor's bearings are absolute.
- **Everything the user reads or types goes through them**: command-line and
  dynamic-input answers (`interact.rs`, `coordinates.rs`, `dynamic_input.rs`: a
  Distance prompt reads lengths, an Angle prompt reads angles, a Number prompt
  stays plain), prompt defaults (`Hint`, written by `units_state::hint_text`),
  the status bar readout, the Properties panel and the Rotate message. A command
  asks for a length with `get_distance`, never `get_number`. A new place that
  shows a length or angle uses `AppState::units()` and these functions, not
  `format!("{:.4}")`.
- **Saved in the drawing**: an optional `units` object in the `.odx` file (no
  version bump; a file without it reads as decimal, an unknown code is refused)
  and the DXF header (`$LUNITS`, `$LUPREC`, `$AUNITS`, `$AUPREC`, `$ANGDIR`,
  `$ANGBASE`). `$INSUNITS` is read but not written, because the writer is R12;
  the export notes say so.
- **The dialog** (`app/units_dialog.rs`) edits a draft and OK commits it.
  Direction... opens Direction Control (East, North, West, South and an angle).
  Not built: the Pick button of Direction Control, the Lighting units (the app is
  2D), architectural display of areas (areas stay decimal), and a unit-aware
  grid spacing and Settings suffix (both still say `mm`).

## Layouts (built, phases 1-3)

A layout is a sheet (millimetres) with viewports that show model space; the design follows AutoCAD's paper space but keeps this project's structures (no slotmap graph, handles or `nalgebra`).

- **Data** (`od-core/src/layout.rs`, `document/layouts.rs`): `Layout { id, name, width, height, viewports }` and `Viewport { id, center, width, height, view_center, scale, locked, on }`. `scale` is paper millimetres per model unit (the specification's `Hp / Hm`), so the mapping is `paper = center + scale * (model - view_center)`, a pure uniform scale and offset, tested for round trips. Layout and viewport ids are permanent like `EntityId`. `Document::validate` checks unique ids and names, sheet sizes and counters. Layouts are replaced as one list by the `SetLayouts` action (`Changes::layouts`), so every edit is one undo step; builders (`new_layout`, `copied_layout`) are pure and number from the counters.
- **Spaces share one object list.** `Object::layout` is `None` for model space or the layout whose sheet holds it; ids are shared. The document has an active space (`active_layout`, set by the app when the view changes space; not undoable or saved), and `visible_objects`, `selectable_objects`, `bounds`, `has_objects_here` and the add actions follow it, so every command, snap, pick, grip and undo works on a sheet unchanged. `visible_in(space)`, `model_bounds` and `model_objects` ask for one space explicitly (viewports show model space; DXF writes model space). Actions that create objects remember their space the first time they run, so undo and redo put them back where they were. Deleting a layout removes its objects in the same transaction.
- **Spaces** (`od-canvas/src/view/layout.rs`): `Space::{Model, Paper, Viewport}` per drawing, held by `View`. `View::camera` always maps the space being edited to the screen: the sheet in paper space, and in a viewport the sheet camera composed with the viewport's scale and offset, so picking, snapping, grips and ghosts are unchanged and only the clip window (`View::blocked`) is new. `show` reports `viewport_edit`, `enter_viewport` and `leave_viewport`; the app applies them (`layouts.rs`). On the bare sheet the commands edit paper space: new objects are tagged with the layout, so a drawing command never puts an object into the model at sheet coordinates.
- **App** (`od-app/src/layouts.rs`, `layout_cmd.rs`, `layout_tabs.rs`): switching spaces, the tab strip, and `LAYOUT`, `MVIEW`, `VPSCALE`, `VPLOCK`, `MSPACE`, `PSPACE`. Every command is a registry entry like the rest.
- **Per-viewport layer freeze:** `Viewport::frozen`, edited by `VPLAYER`. `Doc::keep_space_valid` runs every frame and copies the active viewport's list into `Document::active_frozen`, which `visible_objects` and `selectable_objects` honour; viewport painting passes the list to `paint_entities`. Stale ids (a layer deleted since) are harmless and are not validated.
- **Plotting:** `od_core::plot::plot_layout` turns a layout into `PlotSheet` paths in millimetres (pure, tested); `od_files::plot` writes them as SVG or PDF (`plot_cancellable`, atomic like a save through the shared `write_atomically`); `AppState::plot_layout` (`plot_state.rs`) asks for the file and runs it as a background job. A sheet always plots at 1:1; scale lives in the viewports.
- **Page Setup** (`app/page_setup_dialog.rs`, `AppState::set_page_setup`): name, paper size, orientation, custom size; the dialog edits a draft kept in `AppState::page_setup`.
- **Files:** native `.odx` stores layouts (`od-files/src/native/layout.rs`) and an optional `layout` on objects. DXF keeps layouts too: a drawing with layouts is written as R2000 (`dxf/write2000*`: block records, `LAYOUT` objects, viewports with scale, lock and frozen layers, sheet entities where AutoCAD puts them) and read back by `dxf/read/layouts.rs`. Without layouts the writer stays R12. Validated with ezdxf, not AutoCAD.

## Dimensions (built, first slice)

`Entity::Dimension(Box<Dimension>)` (`od-core/src/dimension/`) follows the AutoCAD model of a semantic entity plus a generated picture, in this project's structures (ids, `Point`, no handles or `nalgebra`). The definition points (`DimKind::{Linear, Radial, ArcLength, Angular, Ordinate}`; a radial one may be jogged), the style, the text (and its angle and position) and the ties to objects are authoritative; `Dimension::picture()` regenerates lines, arcs, arrowheads (tapering wide polylines) and text (multiline text from `Dimension::contents`, which stacks tolerances) from them, and every other system reaches a dimension through that picture: the canvas paints, picks, snaps and window-selects its pieces, plotting flattens them, Explode returns them. Nothing caches the picture; a dimension is a handful of pieces.

- **Style.** `DimStyle` carries the `DIM*` variables the code uses (lines, arrows including a second one, text with fill and frame, fit, primary, angular and alternate units, tolerances, fixed extension lines, baseline spacing). A dimension holds its own copy, so the drawing's current style (`Document::dim_style`, `SetDimStyle`, the `DIM*` system variables in `dim_vars.rs` and `dim_vars_extra.rs`) only affects new ones. The drawing also keeps a table of named styles (`Document::dim_styles`, changed by `SetDimStyles`, which restyles the dimensions made in a style that changed); a dimension finds its style by the name it carries; DIMOVERRIDE changes a dimension's own copy. `DIMSCALE` multiplies sizes (`DimStyle::resolved`), never the measurement.
- **Modules.** `style`, `text` (measurement text, `<>` templates), `tolerance` (alternate units, stacked tolerances), `font`, `fit` (`DIMATFIT`), `sketch` (primitives, readable text angle, text box and mask, line-around-box clipping), `linear`, `radial`, `jogged`, `arc_length` (also draws angular), `angular`, `ordinate`, `centre`, `chain` (baseline and continue), `jogline` (jog lines, spacing), `assoc` (ties), `edit` (transforms, stretch, grips, DIMEDIT's changes) and `pick` (object ends, automatic linear direction, radial from a click). Pure and tested; the `od-app/src/dimension_*_cmd.rs` files only ask, and `dimension_commands.rs` registers them.
- **Associativity.** `Dimension::assoc` ties defining points to points of objects (`AssocPoint`: start, end, centre, vertex, circumference). A dimension made by a command is tied to the objects its points sit on (`Dimension::associated`, in `draw::add`), and `od-commands/src/associations.rs` (the hook the associative hatches use) refreshes the points of dimensions whose objects changed in the same undo step; a dimension edited on its own, or whose object is erased, lets go of the points it no longer shares. DIMDISASSOCIATE and DIMREASSOCIATE are the commands; paste clears the ties (the ids differ). Not saved to DXF (no `DIMASSOC`), and the angle between two lines is not tied.
- **Files.** `.odx` (`od-files/src/native/dimension.rs`, `dimension_style.rs`) stores the style as its differences from the stock style and the ties. DXF (`dxf/write2000/dimension.rs`, `dxf/read/dimension.rs`, shared group codes in `dxf/dimstyle_codec.rs`) writes linear, aligned, radius, diameter, angular and ordinate dimensions as `DIMENSION` entities with anonymous `*D` picture blocks, a DIMSTYLE table of all the styles and `DSTYLE` extended data for overrides, and reads them back; arc length, jogged and jog line dimensions are written as pictures. Checked with ezdxf, not AutoCAD. DWG dimensions are skipped.
- **Not built, in the order to take them:** DWG dimension import (the DWG reader skips the dimension objects; needs the DIMSTYLE table object too), colours, linetypes and lineweights in styles (the picture's pieces need roles), the standard arrowhead set, `ARC_DIMENSION` and jogged radius as real DXF, DIMBREAK and DIMINSPECT, `DIMASSOC` in files (needs a handle per written entity), then annotative scales and user-block arrowheads (which need DXF block export). Also: tolerances and alternate units on angular dimensions, and the angle between two lines is not tied. See `ui_plan.md`, "Dimensions: what is done and what is outstanding".

## Multiline text (built: data, parser, layout and painting)

`Entity::MText(Box<MText>)` (`od-core/src/mtext/`) keeps AutoCAD's `MTEXT` fields and, as its authority, the exact format-code stream in `contents`. Nothing derived from the contents is stored: glyphs, wrapping and the real extent belong to a text engine that does not exist yet.

- **Styles.** `Document` owns a text-style table (`text_style.rs`, `document/text_styles.rs`): `TextStyleId` (never reused, counter kept), `Standard` (id 0) always first, unique names ignoring ASCII case, a current style. A font is a `FontDescriptor`; what a style asks for is kept even where this build would draw something else.
- **Parser.** `od_core::text::parse(&str)` is lossless: every byte belongs to one `Node`, in order. What it understands becomes a typed node (text, `\P`, escapes, `\U+`, groups, character formats, stacks, fields); anything else, or anything malformed, is an `Opaque` node with an `OpaqueReason` and a `Diagnostic`. An edit may rewrite only nodes it understands. Limits (`TextLimits`: 1 MiB, 100,000 nodes and paragraphs, eight group levels, 64-byte numeric and 1 KiB other payloads) are checked before work. Paragraph controls (`\p...;`) are decoded only as far as `private_material/MText Example.dxf` (made in AutoCAD) shows: `\pxi<first>,l<left>,q<l|c|r|j>,t<stop>,<stop>...;` become a `NodeKind::Paragraph(ParagraphStyle)` (distances in drawing units). A control holds for its whole paragraph, wherever it sits, and for every paragraph after it until another one (the second sample writes a list's control only on its first item), so `ParsedMText::paragraphs[i].style` is the effective style and `apply_paragraph_edit` writes a control only where the style changes. Lists are plain text in AutoCAD files (`1.` and a tab, or a Symbol-font bullet), not a control; `text::list::apply_list` writes the markers and the hanging indent together. Anything else in a `\p` (right indent, spacing, tab kinds) stays opaque; its paragraph and those inheriting from it are `protected`. Layout aligns each line after the line breaker has run (`align_line`): right and centre shift it within the wrapping width (the widest line when there is none), justified spreads the spaces of every line that does not end its paragraph. See `private_material/`.
- **Refusal.** `Entity::mirror_refusal` and `scale_refusal` say why an object cannot take a transform; Mirror and Scale (and their grips) ask first and change nothing when any object refuses, so the contents are never rewritten by guess.
- **Layout.** `od_core::text::layout_text(&MText, &TextStyle) -> TextLayout` shapes each run with `rustybuzz` in the bundled DejaVu Sans, in the regular, bold, oblique or bold-oblique face the run's `\f` control or the style asks for (`od-core/assets/fonts`), scales it so a capital is the text height tall, wraps greedily at spaces, hyphens and ideographs (a word with no break runs past the width, as in AutoCAD), runs tabs to a default interval, spaces lines (at least / exactly), and returns positioned glyphs, per-line spans, merged underline/overline/strike bars, frame and text extents and a list of `LayoutIssue`s for whatever it did not do (stacks drawn as text, fields skipped, fonts substituted, columns and vertical flow ignored, missing glyphs). A stack (`\S1/2;`) is one unbreakable cluster of smaller, offset glyphs with an optional bar, so it wraps, takes carets and is deleted like a letter. Local coordinates: origin at the block's top left, x along the text, y up. `MText::world_point(&layout, x, y)` applies attachment, direction and insertion. `glyph_contours(id, tolerance)` gives an outline as flat closed contours in font units.
- **Remembered layouts.** `MText::layout(&style)` goes through a small thread-local memory (`text/memo.rs`) keyed by exactly what layout reads, so moving or rotating text keeps its entry and every bounds query or repaint is a lookup. It only remembers; the answer is the same without it.
- **Context-free queries use the stock style.** `MText::frame`, bounds, snapping, grips and picking lay out in `Standard` because an `Entity` does not know its style; painting uses the real style. A style with another width factor or slant is therefore picked and snapped slightly off its drawn size. `Document::object_bounds` (style-aware) is the fix and is not built.
- **Painting** (`od-canvas/src/view/text_paint.rs`): each glyph's outline is cut into triangles by `lyon_tessellation` (non-zero fill) once per size bucket and cached, then a repaint only moves vertices into one mesh per text. Text under 12 px tall is instead rasterised whole (`text_raster.rs`: exact horizontal and eight-fold vertical coverage into a small texture, kept by a hash of what it shows and the zoom rounded to an eighth of an octave, drawn as a quad so rotation is free; too large a picture falls back to the fill), and under 0.75 px as a line per text line that fades with its height. In the fill (above 12 px) a stroke-font pen is never thinner than 1.1 px, and the window uses 4x MSAA for the filled text above 12 px. Colours: the object's own, `\C` indexes (7 is the foreground) and `\c` true colour; a selected text is drawn in the selection colour with its frame. ACI colours now live in `od_core::aci` (moved from `od-files`).
- **Files.** Native `mtext` entities and `text_styles` (omitted for the stock table). DXF export writes `MTEXT` (R2000) and the STYLE table; DXF import reads `MTEXT` (`dxf/read/text.rs`: contents as written, box width, direction or rotation, attachment, flow, spacing, mask; columns and non-XY planes are skipped by reason) and the STYLE table (font family from the style's extended data, else the file name). Plain DXF `TEXT` is still skipped. DWG import reads `TEXT`, `MTEXT` and the STYLE table (`od-dwg/src/text.rs`, converted by `od-files/src/dwg/text.rs`): a single line becomes an `MText` with no box, hung from its alignment point (text hung by its baseline is the same as text hung by its top, one height up), with its own width factor and slant as `\W` and `\Q` codes and `%%c %%d %%p %%u %%o` turned into characters and switches; "aligned" and "fit" text is read as left-aligned text along the line between its points and reported; MTEXT in more than one column, in a tilted plane or in a mirrored one is skipped by reason. Attributes: each visible single-line `ATTRIB` follows its reference as text and a constant `ATTDEF` shows its default; other `ATTDEF`s are counted and left out; there is no attribute entity yet. Plotting writes text as filled glyph outlines (`PlotSheet::fills`, see "Layouts"). The MTEXT command (`od-app/src/mtext.rs`) takes two corners then command-line text; `od_core::mtext::place_in_corners` is its geometry. A text prompt keeps typed spaces (the command line otherwise treats a space as Enter).
- **OD Roman (built: data, outlines, layout, painting, plotting, style and editor UI).** The application's own single-stroke font, drawn as one SVG per glyph in `fonts/od_roman/glyphs/` (`U+0041.svg`, whose viewBox is the advance box; red strokes are the glyph, `stroke-width` over `pen` is a stroke's weight, `class="dot"` a dot) with `fonts/od_roman/font.cfg` for the shared metrics, and described in `private_material/od_roman_font_specification.md`. `scripts/od_roman_import.py` builds `od-core/assets/fonts/od_roman.odf` from them and warns about ink outside or too near an advance box; after every build it also writes `fonts/od_roman/preview/` (gitignored): `index.html`, a self-contained viewer with live text, a weight slider and a glyph grid, plus an overview and a specimen at the real advances; `od_roman_new_glyph.py` starts a glyph file, `od_roman_respace.py` changes margins by hand and `od_roman_autospace.py` spaces the glyphs listed in `fonts/od_roman/spacing.cfg` by the shape of their left and right edges (stem, round, open, diagonal, ...); `text/stroke_font.rs` reads it (`od_roman()`, `StrokeFont`, bounded and tested) and `text/stroke_outline.rs` turns a glyph and a pen weight into counter-clockwise capsule contours that the existing non-zero fills draw, so painting and plotting need no new path. Advances do not depend on weight. Layout: `FontDescriptor::OdRoman { bold, italic }` and `TextStyle::stroke_weight` (a fraction of text height, default 0.10, clamped 0.01 to 0.40); `FontFace::OdRoman` glyph ids are slots in the font; `PositionedGlyph::weight` is the pen in thousandths of cap height (0 for DejaVu) and `glyph_outline(face, id, weight, tolerance)` returns contours for either kind, which painting (`text_paint.rs`, cache keyed by weight) and plotting (`plot_text.rs`) call. `layout/items/stroke.rs` shapes a run character by character from the font's advances, so weight and shear never move a caret; bold multiplies the pen by 1.6, italic adds 15 degrees to the style and `\Q` obliques; a character the font lacks is drawn in DejaVu (`MissingGlyph` only if neither has it); a `\fOD Roman;` or `\fDejaVu Sans;` control switches font mid-text. `.odx` keeps the font (`FontDto::OdRoman`) and an optional `stroke_weight`; DXF writes `RomanS.shx` for now. Per-run weight is a field of the font control, `\fOD Roman|b0|i0|w0.18;` (`FontFlags::weight`, thousandths of the cap height, clamped 0.01 to 0.40; a control without it hands the weight back to the style, and bold still multiplies it); `CharState::weight` carries it, `CharEdit::weight` and `SelectionFormat::weight` are how the editor sets and reads it (0 clears it), and `apply_char_edit` writes it in the font control for the family it is given (`OD Roman` for an OD Roman style). The Text Style dialog lists OD Roman and shows \"Line Weight\" (percent of height) for it; the MTEXT toolbar shows \"Weight %\" when the style is OD Roman (`MTextEditor::uses_stroke_font`). Whether AutoCAD tolerates `|w` in `\f` is untested, so DXF export may need to strip it. Still to do: DXF round trip data, the PDF text layer (see the specification's milestones 6 and 7).
- **The in-place editor** (`od-app/src/mtext_editor/`). A command opens it with `Editor::edit_mtext(text)`, a prompt of kind `Wants::MTextEditor` carrying a copy of the text; `AppState::sync_mtext_editor` opens an `MTextEditor` on the running document when the command waits and drops it when the command ends. The editor edits only the copy and its own undo history. Accepting answers the command with `Reply::MText` (or `Reply::Enter` when empty or unchanged), which then makes one `AddEntity` or `ModifyEntities`; Esc cancels the command like any prompt. `state.rs` and `input.rs` are egui-free and tested (input with made-up `egui::Event`s); `window.rs` paints and reads the pointer. While `AppState::mtext_editing()` the command line, dynamic input, shortcuts and canvas picking stand aside. Carets, selection boxes and edits come from `od_core::text` (`TextLayout::carets`, `caret_at`, `step`, `selection_rects`, `replace_text`); the editor never lays out text or writes controls itself. Character formatting follows the same rule: the toolbar names a `CharEdit` and `od_core::text::apply_char_edit` writes the controls (a `{ }` group opened with them); `format_in` reads what a selection looks like, with the same `CharState` the layout draws with. Paragraph formatting is the same: the ruler (`ruler.rs`, drawn along the box's top edge in the text's own direction) names a `ParagraphEdit` and `od_core::text::apply_paragraph_edit` rewrites each touched paragraph's `\p` control; `paragraph_view_in` reads it. A new paragraph simply inherits the control above it.

## Blocks (built, first slice)

A block definition is a record in the drawing's block table and a block reference is an entity that points at it, as in AutoCAD; the design follows `private_material/blocks_dynamic_blocks_specification.md` in this project's structures (typed ids never reused, no slotmap, handles or `nalgebra`).

- **Data** (`od-core/src/block/`, `document/blocks.rs`): `BlockDef { id, name, base, description, entities }`, each entity a `BlockEntity` (geometry in the block's own coordinates plus `Attributes`). `Entity::Insert(Box<Insert>)` holds `block: Arc<BlockDef>`, `insertion`, `scale_x`, `scale_y` (never zero; negative mirrors) and `rotation`. The table holds the same `Arc`s, so inserts share one copy of their block and need no drawing to be drawn. **Definitions are immutable:** changing one means building a new record and repointing every insert and every definition that holds it (`Document::redefined_blocks`), which the app records as `SetBlocks` plus `ModifyEntities` in one transaction. Because a definition can only hold `Arc`s that already exist, nesting cannot loop in memory; redefinition and the file reader refuse a block that would hold itself, and expansion stops at `MAX_NESTING` (64).
- **Transform** (`block/affine.rs`): `Insert::transform()` is `T(insertion) · R(rotation) · S(scale_x, scale_y) · T(-base)`. A map that keeps shapes (`Affine::similarity`) is applied with the entities' own exact transforms, so circles stay circles and a nested insert stays an insert with the maps combined (`Insert::transformed`); a mirror is kept as a negative X scale. An uneven scale turns circles, arcs and ellipses into exact ellipses (`Ellipse::mapped`, `block/picture.rs`).
- **Picture:** `Insert::picture_with(attrs)` gives every simple object the insert draws, in drawing coordinates, nested blocks expanded, each with its layer after inheritance (an object on layer 0 takes the insert's layer). Other systems reach an insert only through it, as with dimensions: hit-testing, selection, snaps (plus the insertion point), painting and plotting (`Document::insert_pieces` and `Document::drawn` give the pieces as objects so each is drawn in its own colour and hidden layers drop out), and DXF export (`dxf/blocks.rs`). Nothing caches the picture yet.
- **Validation:** unique block ids and names (ignoring case), names allowed by AutoCAD (`block_name_problem`), objects on existing layers, and every insert (in the drawing or in a definition) using the table's own record with a valid transform. A layer used inside a block counts as in use.
- **Commands** (`od-app/src/block_cmd.rs`, `insert_cmd.rs`): `BLOCK`, `INSERT`, `PURGE` (blocks only) and `EXPLODE` (one level, parts keep their own properties). Pure logic (unused blocks, listing) is tested with the commands.
- **Files:** `.odx` (`od-files/src/native/block.rs`) has a `blocks` list and an `insert` entity naming its block by id; definitions are resolved by id in any order. DXF writes a reference as its pieces; it does not write real blocks yet.
- **Import (DXF and DWG):** both readers collect each named definition with its references to other blocks unresolved (DXF names, DWG handles) and hand them to `od-files/src/block_import.rs`, which builds them depth first (a definition can only hold `Arc`s that already exist), puts them in the table in the file's order, and resolves the model's references. A block whose name AutoCAD would not allow or that repeats another, a reference that would nest a block in itself (or past `MAX_NESTING`), a reference to a missing block and one with a zero or non-finite transform are left out and counted in the warnings. Anonymous blocks (`*D`, `*U`, `*X`: the pictures of dimensions, dynamic blocks and hatches) and external references are not read; references to them are counted. DXF reads definitions when its BLOCKS section begins (`dxf/read/blocks.rs`; entities go through `Importer::shape` into the drawing or the definition being read); `od-dwg` decodes INSERT and the block record's base point and description, and a downward extrusion on an insert becomes a negative X scale and a negated rotation. Attributes (ATTDEF/ATTRIB), MINSERT arrays and DXF extrusions are skipped or ignored; "by block" properties become "by layer" with a note.
- **Block Editor** (`od-app/src/block_editor.rs`): BEDIT swaps the tab's `Doc::session` for a session over `Document::block_editor_document(id)` (the drawing's tables and counters, no layouts, the block's objects with its base point at the origin) and keeps the drawing's session, view and selection in `Doc::block_edit`. Every command then works on the block unchanged. **Anything about the file uses `Doc::drawing()`** (save, export, autosave, the unsaved star), never `session`, which is the editor's while a block is open. BSAVE asks `Document::block_edit_save` for the changes (worked out on a copy, so a refusal changes nothing) and records them in the drawing's history as one step. A command that must open or close the editor (BEDIT's name prompt, BCLOSE's question) runs on its own thread, so it pushes a `BlockEditRequest`, which `AppState` carries out after polling the command. The ribbon switches to `block_editor_ribbon()` (a contextual tab) while a block is open.
- **Not built:** ByBlock properties, attributes, DXF block export, insert arrays, dynamic blocks; see `ui_plan.md`, "Blocks".

## Ellipses (built, baseline)

`Entity::Ellipse(Ellipse)` (`od-core/src/ellipse/`) follows AutoCAD's `ELLIPSE` in this project's structures: centre, major-axis vector, minor-to-major ratio and a counter-clockwise *parameter* interval (the eccentric anomaly, not the angle seen from the centre; `parameter_at_angle` and `angle_at` convert). These five values are the authority; points, outlines, bounds and lengths are worked out from them and never stored or cached.

- **Validity.** `Ellipse::new` is the one place values are checked and made canonical (start in [0, 2π), a full turn is exactly 0 to 2π, ratio in [1e-6, 1], spans of at least 1e-6, coordinates within 1e12). Every transform of a valid ellipse is valid. `Ellipse::validate` is what `Document::validate`, block validation and the file writers call.
- **Derived geometry is bounded.** Nearest point and flattening do a fixed amount of work. Flattening sizes even parameter steps from the bound `a·h²/8` on chord error (a = major radius), so the tolerance is guaranteed, and returns `ResourceLimit` past 8,192 points: the canvas then draws a coarse outline for that frame (`outline`), plotting fails with `PlotError::TooDetailed` rather than plot coarser than asked, and files write the semantic entity and never flatten.
- **Crossings are exact or bounded, never from the outline.** `Curve::Ellipse` takes part in `intersections` and `extended_intersections` like the other curves (its carrier is the whole ellipse), so Trim and Extend use ellipses as edges and targets. A line is met in closed form; a circle, arc or ellipse by bounded root finding on a degree-two trigonometric polynomial (`ellipse/cross.rs`).
- **Fillets and tangent circles** with an ellipse (`fillet_ellipse.rs`) find centres a radius off the ellipse along its normal that are also a radius from the other object, by bounded root finding along the parameter, keeping only centres truly a radius from the whole ellipse.
- **Edits that are not exact are refused, not approximated.** Offset refuses an ellipse by name (its offset is a spline, and fitting one is not built), Chamfer needs lines as in AutoCAD, and a display outline is never used as edit geometry.

## Splines (built, baseline)

`Entity::Spline(Box<Spline>)` (`od-core/src/spline/`) follows AutoCAD's `SPLINE` as a planar NURBS: degree, control points, the full knot vector, optional weights (`None` when all are 1), closed and periodic flags, tolerances and optional fit data. The control frame is the authority; fit data is kept and never refitted on load, draw or save. Boxed because it owns several arrays.

- **Validity.** `Spline::validate` is the one check (counts, finite values within 1e12, non-decreasing knots, multiplicity within the knot tolerance, a domain with length, positive weights in 1e-12..1e12, periodic implies closed, a closed spline's ends meeting, fit data); `new_nurbs`, `from_control_points` and `from_fit_points` call it, and so do `Document::validate`, block validation and both writers. Evaluating a valid spline cannot fail (positive weights keep the denominator positive), so evaluation returns points, not results.
- **Derived geometry is bounded and never stored.** Evaluation by homogeneous De Boor about the first control point; spans become rational Bezier pieces by blossoming (local to the span). Flattening is certified by the hull of each piece (halving until its control points lie within the tolerance of its chord; runs of spans whose control points fit in a box the tolerance wide become one chord) and capped (8,192 points on screen, 65,536 for plots, depth 32), returning `ResourceLimit` past them: the canvas then draws a coarse outline for that frame, plotting fails with `PlotError::TooDetailed`. Length is adaptive Gauss-Kronrod with a work cap; nearest point refines the outline's best stretches. Bounds are the control points' box (safe for culling, not tight).
- **Edits.** Exact transforms of control points, fit points and tangents (any affine map, so uneven block scales stay splines). Grips and Stretch move fit points (refitting) when the spline can be refitted (cubic, open, exact, no tangents), otherwise control points (dropping the fit data). Trim, Extend, Break, Offset, Join and Fillet refuse splines by name (`Entity::curves` gives none, so a spline is never a crossing by accident); no display outline is ever used as edit geometry.
- **SPLINE** (`od-app/src/spline_cmd.rs`) asks and records; `SPLMETHOD`, `SPLKNOTS` and `SPLDEGREE` live in `spline_memory.rs` with the user's settings, like the corner variables.
- **Files.** `.odx` stores splines in `native/spline.rs`; DXF `SPLINE` read in any group order with declared counts and sizes checked before gathering (`dxf/read/spline.rs`) and written as R2000. The native reader's `serde_json` uses `float_roundtrip` so every value reads back bit for bit. DWG import decodes SPLINE (`od-dwg/src/spline.rs`): a spline stored by its control frame comes in exactly, checked like a file's; one stored by its fit points alone is refitted (cubic; natural ends, or smooth where closed, as AutoCAD does without end tangents), as fit-points-only DXF records are, and skipped by name when it has end tangents or is not cubic.

## Hatches (built, baseline)

`Entity::Hatch(Box<HatchEntity>)` (`od-core/src/hatch/`) follows AutoCAD's `HATCH` (`AcDbHatch`) as a semantic entity: boundary loops (a polyline with bulges, or typed edges: line, circular arc, elliptic arc, spline), an island style (Normal, Outer, Ignore), a fill (pattern, solid or gradient), seed points, loop flag bits kept as read, and the objects each loop was made from. Strokes and areas are worked out by `HatchEntity::evaluate` and never stored. Built from `private_material/hatch_engine_specification.md`; where this differs from it, this is what the code does.

- **Pattern lines are final.** As DXF stores them, each `PatternLine` (angle, base, offset, dashes) already has the pattern's scale, angle and origin applied: the `.pat` step `(delta-x, delta-y)` is turned by the line angle, then scaled and turned with the pattern, and the origin is part of the base. The specification's section 2.7 applies scale and angle again at draw time; doing that to the lines of an AutoCAD file would scale and turn them twice. `HatchPattern::{angle, scale_or_spacing, origin}` are what the user last asked for; `with_scale`, `with_angle` and `with_origin` change them by transforming the lines about the origin, so Properties edits need no pattern file. `PatternCatalog` reads `.pat` text; the built-in catalog is a small starting set in AutoCAD's imperial units (ANGLE, ANSI31, ANSI32, ANSI37, LINE, NET, NET3), not the whole of `acad.pat`.
- **Evaluation works on flattened loops** (`boundary.rs`): arcs, ellipses and splines are followed to a tolerance with the existing flatteners, then the loops are polygons. A loop's depth is how many larger loops contain it (`topology.rs`; winding plays no part); Normal keeps every loop, Outer depths 0 and 1, Ignore depth 0, and the filled area is the even-odd area of the loops kept (outer loops run counter-clockwise and islands clockwise, so non-zero and even-odd agree). A boundary always closes itself, so a two-vertex polyline with bulges 1 and 1 is a whole circle, which is how AutoCAD stores one. Overlapping loops that are not nested are filled by parity, as AutoCAD does for Normal.
- **Pattern lines** (`clip.rs`, `dash.rs`): each family is worked in its own frame (`u` along, `v` across). Edges cross line `k` by a half-open rule, crossings sort and pair; a line along an edge is drawn along it on both sides of a region (checked against ezdxf), and stretches shorter than rounding noise are dropped. Dashes keep their absolute phase (`k * shift` along the line), so a line cut by an island resumes where the uncut one would be. Everything is relative to the middle of the hatch, so far-from-origin drawings keep their precision.
- **Bounded.** `HatchLimits` (100,000 pattern lines as `HPMAXLINES`, 1,000,000 strokes, 4,000,000 crossings; a smaller `preview()` budget) is checked before lines are made, and a family whose line numbers would pass 2^50 is refused; the result is `HatchError::TooDense`, drawn as the boundary with a note, never a hang. `HatchEntity::validate` caps loops (4,096), edges, vertices, pattern lines, dashes, seeds and colour stops, and checks every spline's knot count; `Document::validate`, both file readers and writers call it.
- **Planar.** Only hatches facing +Z are kept (others are skipped by name on import); `elevation` is kept for files. Uneven scaling is refused (`transformed` takes only maps that keep shapes).
- **Associativity** (`hatch/follow.rs`, `od-commands/src/associations.rs`). Each loop remembers the objects it was made from (`BoundarySource::Entity`) and the hatch is flagged `associative` (`HPASSOCIATIVE`, on by default). Transforms keep both. After the edits of a transaction, or of a single action through `Session::perform`, every associative hatch that was edited itself or follows an object that changed or went is checked, and what it needs becomes one more edit in the *same undo step* (`Transaction::finish`, `Session::perform`), so undo restores the hatch before the object and redo changes the object before the hatch. `follow` joins each loop's sources end to end again (the outer loop of a picked-point hatch can instead be traced again from its seed when its sources overshoot each other). If the hatch was not itself edited it takes the new loops, or is detached when the objects no longer close or one is gone; if it was edited, it is left alone when its loops are still the ones its objects make (hatch and boundary moved together) and detached when not (the hatch alone was moved, grips, or copied away). A detached hatch keeps its shape and loses `associative` and its sources. Properties edits and HATCHEDIT keep the association. There is no reverse index: the hatches are scanned after each edit. Not done: a picked-point hatch whose islands' sources change shape is detached unless they still close by themselves; copies of a hatch and its boundary together are detached (not remapped to the copied boundary); the hatch is not selected with its boundary (`PICKSTYLE`).
- **Files.** `.odx` stores hatch loops, style, fill and sources in `native/hatch.rs` and `hatch_fill.rs`; strokes are never stored. DXF read (`dxf/read/hatch.rs`): read in order with a cursor, because `97` means a fit-point count inside a spline edge and a source count after a loop; counts are checked against the groups that remain and the hatch limits before anything is gathered, with a 8,000,000-group file budget; a clockwise arc or ellipse edge (`73 = 0`) carries its angles as `360 - angle` (ezdxf's `ArcEdge.load_tags` documents this), mirrored on the way in and out; each source handle is resolved to the object that carries it (`Importer::handles`), and an associative hatch that does not match its objects (the sample file has twelve copied away from theirs) is kept without the association; a pattern's origin is recovered for built-in patterns (the first line's base less the definition's origin, scaled and turned), otherwise it reads as (0, 0); a gradient tail with `450 = 0` is dropped and `450 = 1` is kept (name, angle, shift, tint, colours); hatches inside blocks are skipped by name (the block table refuses them: `Document::redefined_blocks`). DXF write (`dxf/write2000/hatch.rs`): R2000, always non-associative with no source handles (the handles the objects will get are not known when a hatch is written; a note says so), groups in AutoCAD's order, counts of what is written, the loop's polyline bit set to match what is written, angles in degrees (the gradient's rotation in radians), a gradient's colours with both `63` and `421`.
- **Canvas.** `od-canvas/src/hatch_cache.rs` keeps what each hatch drew per object; an entry is reused only while the hatch equals the one it was made from (and the tolerance, a power of two, is the same), so edits, undo, redo and another drawing's object that shares an id can never show a stale picture; it holds 512 hatches and 2,000,000 strokes at most, and `View::invalidate_hatches` frees entries early (nothing needs to call it). Solid fills are tessellated with lyon relative to the hatch's middle. Pattern lines closer than 2.5 px on screen are drawn as a tint of the area. A gradient is drawn as its first colour (the other kinds of gradient are not drawn yet). Pick: a click inside a hatch is just inside the pick radius, so it picks the hatch only when nothing nearer is under it; window and crossing selection use the boundary; no snaps; one grip at the middle of its extent moves it.
- **HATCH** (`H`, `BH`; ribbon Home > Draw > Hatch; `od-app/src/hatch_cmd.rs`): an internal point (`Pick internal point or [Pattern/Scale/Angle/Style/Origin/Select objects]`) or objects (preselected, or chosen with Select objects), then the hatch follows the pointer as a preview and Enter or a click accepts it, one undo step, drawn behind the first boundary object in the drawing order (`AddEntity::behind`, as `HPDRAWORDER` 3). `Pattern` takes a name, `SOLID`, `USER` or `?`; `Spacing` and `Double` for user-defined lines; `Associativity` turns `HPASSOCIATIVE` on or off. **HATCHEDIT** (`HE`; ribbon Modify > Edit Hatch; `od-app/src/hatch_edit.rs`) takes the selected hatch or asks for one, fills HATCH's settings from it (restored afterwards), and offers the same options with a preview; it keeps the boundary and the association and does not edit gradients or patterns outside the catalog (use Properties). Circles, rectangles, closed polylines, ellipses and closed splines are loops by themselves; lines, arcs, open polylines, elliptic arcs and open splines are joined end to end, and a chain that does not close is refused with its size. `HPNAME`, `HPANG`, `HPSCALE`, `HPSPACE`, `HPDOUBLE` and `HPISLANDDETECTION` are user settings (`hatch_memory.rs`), with the origin. Properties (`properties/hatch.rs`): fill, island style, angle, scale (or spacing and double), origin, boundaries, associative and area; style, angle, scale, spacing, double and origin can be typed.
- **Pick internal point** (`hatch/trace.rs`, pure and tested): every visible curve of the space being edited is flattened and cut at all crossings into a planar graph (merged within 1e-9 of the drawing's size); a ray to the right finds the nearest edge, and the face on the point's side is walked (at each node the first edge clockwise from the one used), dead ends cancelled. If the ray hit an island first (the walk comes back inside out) that connected group is skipped and the ray goes on. Objects wholly inside the face whose group touches nothing on its boundary become islands (each found by walking the outside of its group); with Normal style they punch holes, Outer and Ignore apply as usual. A loop that is one whole closed object (circle, rectangle, closed polyline, whole ellipse, closed spline) is that object's exact loop; any other loop is a polygon of the flattened pieces (curves followed to 1e-5 of the drawing's size), marked Derived, and the click is kept as the seed point. The hatch goes behind the earliest object of its outer boundary. Not built: gap tolerance, Draw boundary, ray-casting options, detection limited to a boundary set, text as islands. The whole visible drawing is flattened per click (100,000 segments at most).
- **Operations** (specification 4.8): Move, Copy, Rotate, Mirror, uniform Scale, Erase, layer and colour work; Stretch moves a hatch only when the window holds all of it; Trim, Extend, Fillet, Chamfer, Offset, Break, Join and Explode do not apply (a hatch is never an edge); a block cannot hold one; plotting writes a pattern as its strokes and a solid as a fill (a pattern too dense to plot fails with `PlotError::HatchTooDense`).
- **Checked** against `HatchTest.dxf` (`private_material/`, made in AutoCAD: sixteen hatches over a square, a circle, a four-vertex polyline with arcs and a cubic spline edge) and ezdxf: stroke counts and lengths equal ezdxf's own generator (`tests/fixtures/hatch_lengths.py`; the circle's centre line, which ezdxf misses where it passes through two vertices, is checked against the analytic answer), ezdxf audits our export clean (`tests/fixtures/check_hatches.py` after `cargo test -p od-files --test hatch_fixtures`), and every edge kind, style and fill round-trips through both formats. **Not checked in AutoCAD itself:** the prompts, clockwise-edge angles beyond what ezdxf documents, and how AutoCAD draws a pattern line that falls on a boundary.
- **Not built:** Draw boundary, retained boundaries, separate hatches, gap tolerance, exact (not polygon) loops from traced pieces, writing associations to DXF, Reassociate and Recreate Boundary, loop grips, gradients drawn (kept and written, drawn as a flat colour), transparency and background colour, annotative hatches, hatches in blocks, non-XY planes, nonuniform scale, Explode, the rest of `acad.pat`, loading user `.pat` files, DWG import and export of hatches.

## Multileaders (built: the entity, MLEADER and the style table with its dialogs)
`Entity::MLeader(Box<MLeader>)` (`od-core/mleader/`) stores points, text and a copy of what its style said (`look`), and derives everything it draws (`picture`), like a dimension; `Object::dimension_picture` caches the picture for both. Every `Entity` match treats it intentionally: transforms, grips, stretch, hit, selection, snaps, plot and block pictures use the picture; Trim, Extend, Offset, Break, Join, Fillet, Explode leave it alone, and Mirror is refused while text cannot be mirrored. A style change restyles its multileaders in the same undo step (`SetMLeaderStyles` carries the edits). `.odx` version 1 holds it. DXF reads and writes `MULTILEADER` and `MLEADERSTYLE` in the AutoCAD 2018 layout (the writer picks `AC1021` when a drawing has one); see `ui_plan.md`. See `ui_plan.md`, "Multileaders".
`Document` owns a multileader-style table (`mleader/style.rs`, `document/mleader_styles.rs`) like the text-style table: `MLeaderStyleId` (never reused, counter kept), `Standard` (id 0) always first, unique names ignoring ASCII case, a current style, checked by `validate`. A style holds the values of AutoCAD's three tabs (leader format, structure, content) with typed enums, not codes; ByBlock, annotative, block content and vertical attachment are not representable yet. `SetMLeaderStyles` (`od-commands`) swaps the whole table, current style and counter as one undo step (`Changes::mleader_styles`), as `SetTextStyles` does. `.odx` keeps it as optional fields left out for the stock table. `mleader::sample` builds the preview geometry as ordinary entities in `od-core`; the dialogs in `od-app` (`mleader_style_table.rs`, `app/mleader_*.rs`) only draw it. The `Leader` and `MLeader` entities follow `private_material/leader_and_mleader_specification.md`; entities will name their style and keep overrides, not an id.

## Legacy leaders (built: baseline)
`Entity::Leader(Box<Leader>)` (`od-core/leader.rs`) is a polyline with an arrowhead and an optional reference to the MTEXT it annotates, which is a separate object. Like a multileader it is drawn from its stored points (`picture`, cached by `Object::dimension_picture`) and goes through every `Entity` match intentionally. The reference may name an erased text (ids are never reused); the leader does not follow the text. `.odx` version 2; DXF reads and writes `LEADER` in AutoCAD's layout, joining text by handle after everything is read. See `ui_plan.md`, "Legacy leaders".

`Entity::Point { at }` is a bare location. How every point looks is a property of the drawing, not of the object: `Document::point_style()` (`PointStyle { mode, size }`, `od-core/src/point_style.rs`; the `PDMODE` and `PDSIZE` variables, undoable through `SetPointStyle`, saved in `.odx` as optional `point_mode` and `point_size`, read and written as `$PDMODE` and `$PDSIZE`). `PointStyle::marks` works out the lines, circle and square of a mode at a size; the canvas (`view/point_paint.rs`) and plotting (`plot.rs`, `point_outlines`) draw them, and a size of zero or below is a share of the screen height (the viewport's height when plotted). A point is picked, boxed and snapped (Node) at its location only, has one grip, and moves, rotates, mirrors and scales with it. `.odx` version 3.

`Entity::Xline { origin, direction }` and `Entity::Ray { .. }` (`od-core/src/infinite.rs`) store a base point and a unit direction and have no finite size. Their box (`Entity::bounds`) is open to infinity in the directions they run; the spatial index keeps such boxes in its list of exceptions and tests each like any other, and `Document::bounds`, `model_bounds`, the clipboard, arrays and the render tool leave them out of extents (`Bounds::is_finite`). Where a finite stand-in is needed, `infinite::segment` gives a stretch `REACH` (1e7) either side of the base: `Entity::curves` (so intersections, Trim and Extend boundaries and the snaps that use curves work), selection edges and plotting (solid, cut to the viewport by the frame). The canvas paints only the part inside the window (`view/infinite_paint.rs`), with the linetype laid from the base point. Blocks refuse them. Grips: the base point (moves it) and a point 80 screen pixels along the line, two for a construction line, that turn it about the base (`infinite::set_pixel_size`, set by the canvas each frame, tells them how big a pixel is). Cutting (Trim, Break) and the corner commands work on `Entity::long_line`, the stand-in, and bring the result back through `Entity::resumed` (a piece with one far end is a ray). Properties types the base point and the angle. DXF `XLINE` and `RAY` need R2000. `.odx` version 3.

## Linetypes (built: the table and dashed drawing and plotting)

`Document` owns a linetype table (`od-core/src/linetype/`, `document/linetypes.rs`), following `private_material/autocad_linetype_reference_guide.md`, which is the compatibility contract.

- **Records.** `Linetype { id, name, description, elements, flags }`: `LinetypeId` (never reused, counter kept), names unique ignoring ASCII case, up to 255 bytes. A pattern is one repeat of `StrokeElement::Dash(len) | Gap(len) | Dot` (lengths above zero and finite; `from_dxf`/`to_dxf` convert DXF group 49). `CONTINUOUS` (id 1), `BYLAYER` (2) and `BYBLOCK` (3) always exist, have no pattern and cannot be changed or removed. A new drawing also has `DASHED`, `DOT`, `DASHDOT`, `CENTER`, `HIDDEN` and `PHANTOM` (ids 4 to 9) so the names layers and objects use already resolve.
- **Stock library** (`linetype/stock.rs`): the 24 families (8 x `x`, `2`, `X2`) in AutoCAD's metric (`acadiso.lin`) numbers and the 14 ISO 128 patterns. The variants are data, not computed (`CENTER2` is not half of `CENTER`); a test holds the imperial numbers apart and checks metric = imperial x 25.4. `LinetypeTable::load_stock(name)` adds one to the drawing; once loaded the drawing owns it.
- **Names, not ids, for now.** `Layer.linetype` and `Attributes.linetype` still hold names (looked up with `LinetypeTable::named`) and `LINETYPES` still feeds the UI lists. Moving them to `EntityLinetype { ByLayer, ByBlock, Named(id) }` is a later slice. `linetype::legacy_alias` maps the old `Dotted` to `DOT`; it is not applied yet and must only be where the drawing has no record of that name.
- **File.** `.odx` writes `linetypes` (id, name, description, pattern as DXF lengths, flags) and `next_linetype_id` only when the table differs from the stock one, so no version bump; a file without them reads with the stock table. Loading refuses a table missing or patterning a special record (`LinetypeTable::from_records`).
- **The generator** (`linetype/dash.rs`, pure): `fit(length, closed, elements, scale)` says which stretches of a path are drawn; `dash_path(points, closed, elements, scale)` maps that onto a flattened path (`Piece::Dash(run)` keeps the corners it crosses, `Piece::Dot { at, along }`). Rules from the guide: an open path starts and ends with a dash (a dot-first pattern with a dot), the end dashes take up the slack, a dot-first pattern stretches its gaps, a closed path is stretched to whole repeats so there is no seam, a path shorter than one repeat plus a dash is continuous, and a bad scale or more than 200,000 marks is continuous. **The slack split is our own choice, not AutoCAD's published arithmetic: lock it down with AutoCAD-made fixtures.**
- **Which curves** (`linetype/paths.rs`): `is_dashable` (line, circle, arc, ellipse, spline, rectangle, polyline) and `curve_outlines(entity, tolerance)`, the flattening plotting already used (a circle starts at angle 0). Text, hatches, dimensions and block references are not dashed as a whole.
- **Which pattern an object uses:** `Document::dash_pattern(object)` = its resolved linetype name looked up in the table (the old `Dotted` falls back to `DOT` when the drawing has no record of that name); the scale is the object's own `Attributes::linetype_scale` times the drawing's `LTSCALE` (`Document::linetype_scale`, undoable through `SetLinetypeScale` and the `LTSCALE` variable, saved in `.odx` and read and written as `$LTSCALE`).
- **Canvas** (`od-canvas/src/view/dash_paint.rs`): the entity is flattened to half a pixel, dashed, and each dash stroked; the selection glow follows the dashes. A pattern whose repeat is under 6 px on screen (`MIN_REPEAT_PX`: 10,000 dashed objects in view cost 41 ms at a 4 px repeat, 26 ms at 7 px, 8 ms solid), or a path needing over 20,000 dashes, is drawn solid: a view-only choice that never reaches the file or a plot.
- **Plot** (`plot.rs`, `dashed_outlines`): outlines are dashed in the units they are made in (model units for a viewport, so its scale applies to the dashes; millimetres on the sheet), then clipped. A dot is a hair of line that the writers' round caps make a dot.
- **Scale and generation live on `Attributes`** (`linetype_scale`, default 1, above zero; `continuous_generation`, default false), not on the polyline entity, so no entity variant changed. Linetype Generation applies to polylines and rectangles: off (AutoCAD's default) lays the pattern afresh on every segment (`curve_outlines(.., per_segment = true)`), on lays it along the whole outline. Properties' General section has Linetype scale (typed) and, when a polyline or rectangle is selected, Linetype generation. `.odx` writes both as optional fields on objects and block objects (flattened `ScaleDto`) and `LTSCALE` beside the table; DXF reads and writes group 48, bit 128 of group 70 on polylines, and `$LTSCALE` (R12 and R2000).
- **File patterns** (`linetype_import.rs` is shared by the DXF and DWG readers; `dxf/read/ltype.rs`, `dxf/linetypes.rs`): each LTYPE record (groups 2, 3, 49 in order) becomes or replaces a record of the table, so the file's numbers are what is drawn: an imperial `DASHED` from an `acad.lin` drawing replaces our metric one and keeps its id. The three special records are ignored. A definition with text or shapes (group 74 not 0) is read as its dashes and reported. A name layers or objects use that no record defines gets the stock pattern if there is one, and is reported, else is drawn solid and reported; a bad dash length skips that record. Both writers define every record of the table (not only those in use), from the table, under the record's own name (the old `Dotted` is written as `DOT`); the made-up imperial patterns are gone. Checked with ezdxf (reads R12 and R2000 output, audit clean) and with `private_material/Linetypes.dxf` and `Linetypes.dwg`, made in AutoCAD from `acad.lin` (`tests/linetype_fixtures.rs`: the 24 families' numbers match our imperial table exactly, the DXF and the DWG give the same table and descriptions, the two complex ones read as dashes and are reported).
- **DWG patterns** (`od-dwg/src/tables.rs`, `read_linetype`; §20.4.58): after the name come the xref index (BS), then the description (strings stream), the pattern length (BD), the alignment (RC, always `A`; anything else leaves the name alone), the dash count (RC) and each dash's length (BD), shape code (BS), x and y offsets (RD), scale and rotation (BD) and shape flag (BS; non-zero marks a complex dash). The spec's 64-flag and Xdep bits are not in AC1032 drawings. A pattern that does not decode keeps the name. The entity's own linetype scale and the header `LTSCALE` come across too.
- **UI** (`od-app/src/linetype_ui.rs`, `app/linetype_dialog.rs`): the linetype lists in Properties and in the Layers panel come from the table (`CONTINUOUS` and the named records; `BYLAYER` and `BYBLOCK` are not linetypes) and show a picture of each pattern at a size that suits any units (`paint_sample`, using `dash::fit`); `od_ui::menu::list_menu_sampled` and `combo_sampled` draw the picture at the right of a row. `LINETYPE` (aliases `LT`, `LTYPE`, `DDLTYPE`; Manage > Drawing > Linetypes) opens the Linetype Manager: a list with picture and description, Load from the stock library (those already in the drawing are marked and not offered again), Delete (refused for the three built in and for any linetype in use, `Document::linetype_in_use`), and the global scale factor. It edits a draft and OK commits table and `LTSCALE` as one undo step (`SetLinetypes`).
- **Not built yet:** a current linetype (`CELTYPE`) for new objects, Load from a `.lin` file, Reload and Rename in the Manager, ByBlock in the lists, `CELTSCALE` and `PLINEGEN` as defaults for new objects (new ones are scale 1 and generation off), DWG linetype generation of a polyline (its flag is not decoded), complex (text/shape) elements kept for round trip (they are read as dashes), dashes on block contents, dimensions and hatch outlines (insert pieces and dimensions are drawn solid), ByBlock, `EntityLinetype` ids, the `LINETYPE` command and UI lists.

## Spatial index (built)

`Document` keeps an R-tree (`rstar`, kept inside `od-core/src/spatial_index.rs`)
per space (model space and each sheet) over `Object::bounds()`, so asking what
is near a point or inside a rectangle costs what is found, not the size of the
drawing. It is derived state: private to `Document`, ignored by `PartialEq`,
never saved, cloned with the document.
- **Queries** (`document/spatial_query.rs`): `objects_intersecting(space, rect)`
  and the filtered `visible_in_rect` / `selectable_in_rect` (the caller passes
  the viewport's frozen layers) with `visible_here_in_rect` and
  `selectable_here_in_rect` for the space being edited. They return what a scan
  of the space would, in draw order (the tree holds ids, sorted back by current
  position), so exact code (`pick`, `snap`, `select`, painting) is unchanged and
  only given fewer objects. Layer state is never in the index: it is applied
  after. `Document::bounds` stays a filtered scan (the tree root is not a
  visible extent).
- **When it is used** (`document/spatial.rs`): a space gets a tree at 4,096
  objects and loses it below 3,072; below that, and for a rectangle that is
  invalid or covers the whole tree, a query is a scan. An object whose box is
  not finite (`NaN`, infinite, reversed) is kept in a side list tested with
  `Bounds::intersects`, so it behaves as in a scan.
- **Kept current by `Document` itself**: `push`, `insert*`, `remove*`,
  `replace*` update it; a change of many objects (at least 256 and 10% of the
  space) rebuilds the space once instead. A mismatch found on the way (a failed
  removal) rebuilds the space; one found by a query is an assertion in debug
  builds and a scan in release. File readers call `begin_bulk_load` and
  `finish_bulk_load` so the tree is built once at the end (queries scan
  meanwhile).
- **Consumers**: painting (`visible_in_rect`), `pick_here`, snapping and
  `select_in` in `od-canvas`. Use these rather than iterating
  `selectable_objects()` for anything run on every frame or mouse move.
- **Checks**: `document/spatial_tests.rs` and `spatial_edit_tests.rs` run
  random edits (every mutation path, three spaces, bad boxes) against a scan with
  an invariant check; debug builds also compare one query in 1,024 with a scan
  (`set_verify_all_queries` checks all); `od-canvas/src/indexed_tests.rs`
  compares pick, snap and region selection. `cargo run --release -p od-core
  --example bench_index` measures it.

## Picture cache (dimensions built)

A stored dimension's picture (the lines, arrows and `MText` text it draws) is a
function of its entity, so `Object::dimension_picture()` remembers it like
`Object::bounds()`: one build, shared by painting, picking, snapping, selecting,
plotting and DXF export, forgotten when `Document::replace` changes the entity,
ignored by equality. Code that has an `Object` asks it; code with only a
detached `Dimension` (previews, tests) uses `Dimension::picture`. Block
references are not cached (see `ui_plan.md`, "Picture cache").

## Geometry in `od-core` (built)

Modify commands are thin: the maths is pure, tested, and lives in `od-core`,
the command only asks, calls it and records a transaction. Modules:
`arc` (circular arcs, CCW radians), `ellipse` (ellipses and elliptical
arcs, see "Ellipses"), `spline` (NURBS curves, see "Splines"), `bulge` (polyline arc segments),
`intersect` (`Curve::{Segment,Circle,Arc,Ellipse}`, crossings, nearest point),
`split` (Trim/Break), `extend`, `offset`, `corner` and `fillet` (two-curve
corners; `Chamfer::{Distances,Angle}`), `polycorner` (rounding a whole
polyline), `grip` (`Entity::grips`, `with_grip_moved`), `join`
(`join(&[Entity], tol) -> Vec<Merged>`), `polygon` (regular polygons about a
centre or on an edge, for POLYGON), `array` (where array items go:
`Placement`s and a measured `Path`) and `transform`.

- **Polyline arcs:** `Entity::Polyline { points, closed, bulges, widths }`; `bulges`
  is empty or one per point, `bulges[i]` shapes the segment from point `i` to
  the next (the closing one for the last point of a closed polyline). Bulge =
  tan(angle/4), positive CCW, 1 = half circle; mirroring flips signs. Anything
  that reads a polyline must go through `bulge::*`, never assume straight
  segments.
- **Polyline widths:** `widths` is empty or one `(start, end)` pair per point,
  indexed like `bulges` (the width of that segment at its start and end, tapering
  between). `width::at` reads it and `width::triangles` gives the area to fill,
  which the canvas paints in the stroke colour; the centreline is still what is
  snapped to, picked and gripped. Moving, turning, mirroring, scaling (widths
  scale), stretching, grip editing and Extend keep the widths. Anything that
  cuts or joins segments (Trim, Break, Join, Offset, Fillet, Chamfer, Explode)
  produces a hairline polyline: those do not carry widths yet.
- **Building shapes from the command's options** is pure geometry in
  `od-core`, tested there: `arc_make` (an arc from start, centre, end, included
  angle, chord, radius or direction), `tangent` (the circle tangent to two
  objects with a radius) and `rect` (a rectangle at an angle with rounded or
  bevelled corners and a width, as a plain `Rectangle` when it can be).
- **Results are lists of entities.** A trim, split or join returns the
  entities that replace the old one; the first keeps the object's id and draw
  order, the rest are new objects with the same attributes.
- **Randomized tests** check invariants (crossings lie on both curves, cutting
  keeps total length) rather than only examples; keep doing that for new
  geometry.

## Automation and MCP (reads, atomic writes, measuring, rendering, commands, files, interactive commands and events built)
Spec: `private_material/mcp_server_specification.md`. An AI client talks MCP to the `opendraft-mcp` sidecar, which talks to the running app over a local socket; the app answers on the UI thread.
- **Boundary.** `od-app/src/automation/`: `host.rs` (listener, bounded queue, `poll` once per frame), `connection.rs` (one client: token handshake, then framed requests with a deadline), `read.rs` and `query.rs` (the operations, pure functions over `AppState`, so they are tested without IPC), `settings_page.rs`. Connection threads never touch `AppState`; a request abandoned by its caller is dropped before it runs.
- **Transport.** Named pipe `od-<instance>` on Windows, a Unix socket beside the descriptor on Linux (Linux untested). Each run writes `<instance>.json` (endpoint, pid, a 256-bit token) into `%LOCALAPPDATA%\OpenDraft\automation` (`$XDG_RUNTIME_DIR/OpenDraft/automation`; `OD_AUTOMATION_DIR` overrides it for both programs); a clean exit removes it and the next start sweeps ones nothing answers on. The token is checked in constant time, never logged, never shown to a model (`opendraft_instances_list` omits it). The pipe is not ACL-restricted beyond Windows defaults, so the token is what keeps other users out; anything running as the same user can read the file, so the real safety boundary is the grant.
- **Grants.** Each connection gets a grant when it authenticates: `drawing.read` only. The user ticks Allow changes for a connection in Settings > Automation to add `drawing.write` and `ui.selection` (takes effect on its next request, and unticking stops it at once); `history.any` is never offered yet. The setting `automation_enabled` (saved) is off by default; `OD_AUTOMATION=1` turns the endpoint on for one run without saving (it does not grant writes). The connection checks the grant before queueing and `poll` checks it again before running.
- **Ids and revisions.** Every id is a decimal string. A drawing's wire id is `doc_<instance>_<uid>` (`Doc::uid`, never a tab index). `Session::revision()` moves on every effective edit, undo and redo (never back); every read result carries it and a paging cursor is bound to it and to its filters (stale or foreign cursors are refused).
- **Block Editor.** While BEDIT has a drawing, `doc.session` is the temporary block. Every per-document operation on that drawing returns `block_editor_open` (retryable, no retry delay, so a client must not poll); `documents.list` shows only `available: false`.
- **Writes.** `entities.apply_batch` (`mutate.rs`), `layers.apply_batch` (`layer_mutate.rs`), `selection.set` and `history.undo`/`redo` (`ui_mutate.rs`). Each goes through `guard::open_for_change`: the drawing must exist and not be in the Block Editor, no command may be running in it (`command_busy`, never cancelled for the caller), and `expected_revision` must match (`stale_revision`; there is no force). Targets, references and limits are checked before the first edit; the edits then run in `Session::checked_transaction`, which validates the whole drawing in every build before recording and reverts if it is invalid, so a failed batch leaves no trace and no revision. A successful batch is one undo step named by the caller's label, recorded with `Provenance::Automation(grant)`; a batch that changes nothing records nothing. A full `validate()` costs 0.8 ms on the 8,741-object Pipe Fittings model (linear), so no incremental validator. Wire geometry is built by `od-api::convert`, one set of rules for add, replace and patch (finite numbers, positive radii, polyline arrays that match, `Ellipse::new`, `MText::validate`, layers, linetypes, lineweights and text styles that exist). Writable kinds: line, circle, arc, rectangle, polyline, ellipse, mtext; the rest are read-only (`unsupported_entity_write`). A replace that changes kind is refused while an associative hatch follows the entity; hatches follow same-kind edits in the same step. Undo and redo need the step token and may only touch steps made by the same grant (a restarted sidecar is a new grant) unless `history.any` is granted. Layer batches cannot delete layers, rename layer 0, freeze the current layer or make a frozen layer current. `Session::document` and `history` are private to the crate (read through accessors), so edits cannot skip the revision; view-state setters (`set_active_space`) and non-undoable viewport zoom (`store_view_layouts`) are explicit methods.
- **Geometry answers.** `geometry.measure` (`od-api::measure::measure`: length, perimeter, area, centre, radius, end points; each answer says `exact`, `numeric` (ellipse and spline lengths) or `unsupported` (text, blocks, dimensions, hatches have only bounds); a closed polyline's area is the algebraic sum, an open one has none) and `geometry.intersections` (`Entity::intersections`, the Trim/Extend code; entities with no outline are listed as `unsupported`, not skipped silently; only pairs in the same space with overlapping bounds are tested; over 10,000 pairs or 10,000 points is `result_too_large`). Both need only `drawing.read`.
- **Rendering.** `view.render` returns an SVG through the plotting pipeline: `od_core::plot_model::plot_region` (a window of model space scaled to a width, sharing `plot::add_viewed` with layout viewports so linetypes, text outlines, hatches, blocks and dimensions match what plots) or `plot_layout` for a whole sheet. The result names the revision drawn and how it differs from the canvas (black ink on white, plot lineweights, wide polylines as single lines, hidden layers not drawn). Limits: 5,000 entities, 500,000 vertices, 6 MiB, width 50 to 4096; over any of them is `result_too_large`, never a cut-off picture. The sidecar returns the SVG once, as an embedded `image/svg+xml` resource, and keeps it out of the structured part. No PNG: that needs an off-screen renderer that matches the canvas, which does not exist.
- **Commands.** `Command` carries `automation: AutomationPolicy` (`Denied` by default, `ImmediateSafe`, `InteractiveInApp`) and `effect: CommandEffect`. The one table that decides them is `od-app/src/automation/policy.rs`; `build_registry` applies it, so an unlisted command stays denied, and tests require a row for every registered command (and no stale rows). Rules the tests keep: file, dialog and lifecycle commands are denied; only view, mode and selection commands (zoom, grid, snap and its modes, dynamic input, select all) are immediate; anything that asks for points is `InteractiveInApp` and is refused from `commands.invoke` ("use the entity tools"), never pretended to be immediate; Undo, Redo and Erase are denied (history and deletion have their own checked tools). `commands.list` shows what may run and whether it can now; `commands.invoke` needs the separate `command.invoke` grant (the Allow view commands switch), acts only on the drawing in the active tab (automation never switches tabs), goes through `Registry::execute` so enabled and busy checks apply, is refused with a retryable `operation_busy` while a modal dialog is open (a dialog writes its copy of settings back on OK and would undo the command), and requires `expected_revision` for any command that changes the drawing (none do today).
- **Files.** `files.open`, `files.save`, `files.save_as`, `files.export` (DXF, or a layout as PDF or SVG), `operations.get` and `operations.cancel`, in `automation/file_ops.rs` with the registry in `file_state.rs`. A call returns an operation handle (`op_N`) at once; the work runs on the app's own file worker from a copy taken when it starts, and the caller polls (`awaiting_consent`, `running`, `succeeded`, `failed`, `cancelled`). Rules:
  - The grant is its own switch (Allow files: `file.read` and `file.write`), separate from changes and commands, and the user must also approve folders (Settings > Automation > Approved folders, saved as `automation_roots`; none until added).
  - `od_api::paths::check` is the one gate: absolute plain paths only (no `.` or `..` parts, network, device or extended-length paths, reserved names, alternate data streams, trailing dots), an allowed extension (`.odx .dxf .dwg` to open, `.odx` to save, the format's own to export), a regular file, never a link written through, and the canonical path (symlinks and junctions resolved, shown in plain `C:\` form) must lie inside an approved folder (checked per component, so a look-alike folder name does not pass). Results show only the file name, not folders. A path can still change between the check and the use; this narrows, it does not freeze the disk.
  - Saving a drawing to its own file needs no question; `files.save` never invents a file name. Replacing any other existing file (save as, export) waits for the user: a modal "Replace file?" built only from the path, the kind and the connection's self-chosen name (the safe choice is the default and Escape or X declines); it expires after 120 s, and turning automation off cancels waiting questions. A model's own say-so can never answer it.
  - Operation handles belong to the connection that made them, at most 8 unfinished each; `operations.cancel` stops only that job (`Io::cancel_job`, with the "being opened or saved" bookkeeping undone), never the user's own file work, and a cancelled save never replaces the file. Opening adds a new tab and makes it active (as the user opening a file does); an already open file is reported, not switched to.
  - No file operation touches a drawing that is in the Block Editor, and the Settings folder picker is the only way to approve a folder.
- **Interactive commands.** `commands.start_interactive` (`automation/command_runs.rs`) starts a command whose policy is `InteractiveInApp` and leaves its prompts to the user: they point and type in the app, and the caller can only start it, read what it asks and cancel it. The prompts never become remote clicks. It needs the `command.invoke` grant (the Allow commands switch), the active tab, no dialog open, and no command already running (it never replaces the user's: `command_busy`). A selection made first (`selection.set`) becomes the command's pickset, so its selection step is skipped as when a user starts it. The run is an operation (kind `command`) polled with `operations.get`: `waiting_for` carries the current prompt (app text, bounded, no control characters, data not instructions) and it ends `succeeded`, `cancelled` (Esc, or another command replacing it) or `failed` (the command's thread crashed). `AppState::command_serial` and `command_ends` (`command_record.rs`) are how a run is told from the next. `operations.get` and `operations.cancel` need only read access, because operations are private to the connection that started them. Starting waits up to 100 ms for the first prompt so the answer can say what it is.
- **Events.** `events.poll` (`automation/events.rs`): a log of the last 256 events (`document_opened`, `document_closed`, `document_changed`, `document_modified`, `document_saved`, `command_started`, `command_ended`) that the host fills by looking at the drawings and the running command once per frame, so it sees the user's edits and anyone else's without any editing code knowing. A caller polls with the `next_seq` it was given; `more` says to call again and `reset` says it fell behind the log and must reread. Because it samples, changes between two looks merge into one `document_changed` (the revision says how far it went); a drawing in the Block Editor is silent. `detail` text (titles, step names, command names) is bounded and stripped but untrusted. There are no subscriptions: the MCP SDK's resource subscriptions are not advertised, and polling by revision remains the contract. The optional adapter that turns scalar prompts into MCP `input_required` rounds is not built.
- **Idempotency keys.** Operations that would repeat their effect (`od_api::idempotency::supports`: batches, undo and redo, `commands.invoke` and `start_interactive`, the file operations) accept an optional `idempotency_key` in their arguments. `dispatch` takes it out before the operation parses its arguments (so no argument type carries it) and consults `AppState::replays` (`automation/idempotency.rs`): a key already done by the same connection for the same operation, with the same arguments (a hash of the sorted JSON), returns the stored answer with `replayed: true` and runs nothing; the same key with other arguments is `idempotency_key_reused`. Only successes are kept (a failure leaves the drawing as it was, so running again is right), at most 256 answers of at most 256 KiB, oldest out first. Keys are plain (`[A-Za-z0-9_.:-]{1,64}`). The sidecar adds the property to exactly those tools' schemas, never retries a change after a lost connection (reads still retry once) and reports `instance_gone` with `outcome_unknown` instead. Setting the selection and cancelling an operation take no key: repeating them is harmless.
- **Shipping it.** `scripts/package.sh` builds release and assembles `dist/OpenDraft-<version>-<os>/` (the app, `opendraft-mcp` beside it, the licence, the readme and `docs/mcp.md` as `MCP.md`) plus a `.zip` (Windows, via the system `tar.exe` so names use forward slashes) or `.tar.gz`; `.github/workflows/release.yml` runs it on both systems for a `v*` tag or by hand and attaches the packages to the release (the workflow has not been run). The app finds the program by looking beside its own executable (`automation/setup.rs`); Settings > Automation shows that path and copies the `mcpServers` JSON for it. `opendraft-mcp --check` (`od-mcp/src/doctor.rs`) connects, says what the connection may do, and exits 0 only if an app can be used. User documentation is `docs/mcp.md` (setup per assistant, switches, safety, troubleshooting, versions). Streamable HTTP is deliberately not built: it needs the OAuth, TLS and threat-model work the specification gates it on.
- **Measured (release, 16-10-2026).** On a 10,000-object drawing: a 100-entity page 0.63 ms at p95 (budget 16), one entity 0.01 ms (budget 5), a 1,000-operation add batch 11 ms in all, a full validation 0.4 ms (`cargo test --release -p od-app budget -- --ignored --nocapture`). `opendraft-mcp` idles at 6.9 MiB with no CPU (budget 50 MiB); the app is 17.6 MB and the program 3.4 MB.
- **Tool schemas.** Generated from the `od-api` argument types with sub-schemas inline (some hosts reject `$ref`), titles and null defaults dropped. `tools/list` is about 47 KB (25 tools; about 33 KB if sub-schemas were shared with `$defs` instead of inlined), of which `opendraft_entities_apply_batch` is about 20 KB because the attribute and geometry unions repeat per operation; if that is too much context, move shared parts to `$defs` or split add/patch/replace into separate tools.
- **Using it.** Build `cargo build --release -p od-app -p od-mcp`. In OpenDraft turn on Settings > Automation, and tick Allow changes for the connection to let it edit. Point the MCP client at the `opendraft-mcp` executable (stdio, no arguments; `--instance <id>` if several apps run), e.g. Claude Desktop `{"mcpServers": {"opendraft": {"command": "C:\\path\\to\\opendraft-mcp.exe"}}}`. Turning the setting off drops every connection. Drawing text reaches the model as data: tool descriptions never include it.

## Profiles (where the app keeps its files)
Everything the app keeps lives in one folder, `eframe::storage_dir(<profile name>)`, and only `profile.rs` asks eframe for it (a test enforces this): code needing the folder calls `profile::app_data()`, and the window system's application name and window title are `profile::name()`. The name comes from the build: a release build is `OpenDraft`, a debug build `OpenDraft Dev`, a beta (`-beta`/`-rc` version) `OpenDraft Beta`, so a debug run no longer overwrites the release's settings, recents or recovery copies, and `cargo run --release` uses the release's own. `OPENDRAFT_PROFILE=<name>` (letters, digits, space, `-`, `_`, at most 32) picks `OpenDraft <name>` for tests and for trying a migration on a copy. A beta's first run copies the release's `app.ron` and `window.json` once and never writes back. Drawings are not part of a profile. Automation descriptors are separate: set `OD_AUTOMATION_DIR` for both the app and `opendraft-mcp` to keep a development app out of the real list.

## Windows window and crash log
A Windows **release** build of `opendraft` is a windowed program (`#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]` at the top of `main.rs`); without it the exe is a console program and double-clicking it opens a console window beside the app, which also ends the app if closed. Debug builds keep their console for development, and `opendraft-mcp` stays a console program because it speaks over standard input and output. A windowed program has no console for a panic message, so `crash_log::install()` (first thing in `main`) records every panic (time, version, thread, message, place, backtrace) in `crash.log` in the profile's folder (`profile::app_data`: `%APPDATA%\<profile>\data` on Windows, `~/.local/share/<profile in lower case>` on Linux), cut back to its newest reports at 256 KiB, and then runs the ordinary hook so a terminal still shows it. A panic inside a command is caught and the drawing carries on, but it is logged too. The app menu's **Crash log** item (and `CRASHLOG`) is the command `manage.crash_log` (`AppState::show_crash_log`): it reveals the file in the file manager through `reveal.rs` (shared with the Home page), or says there is none. Not built: telling the user, when it happens, that a crash was recorded.

## Update checks (`od-app/src/updates/`)
The program can tell the user a newer version exists. It only informs: nothing is downloaded or installed (installing is a later phase that needs signed installers). It is **off until the user says yes** (`Settings::update_checks` is `None` until answered; the Updates card on Home asks, Settings > Updates mirrors it), so a fresh install makes no network request. Once allowed, `Updates::tick` (every frame, cheap) starts a check on its own thread when none was made for 24 hours (an hour after a failure; "Check now" forces one). The thread does one HTTPS GET of the project's releases list (`fetch.rs`, `ureq` with `rustls`; 10 s timeout, 2 MiB cap) sending only `User-Agent: OpenDraft/<version>` and the ETag of the last answer, so an unchanged list costs a 304. `releases::newest` (pure) takes the highest semver above this build's version, skipping drafts, tags that are not `v<semver>`, prereleases unless this is a beta build, and releases lacking this system's package (`OpenDraft-<version>-windows.zip` / `-linux.tar.gz`, the names `scripts/package.sh` makes), so a release still being uploaded is not announced. The newest `/releases/latest` is not used (it is by creation date, so a late backport would win). The answer is stored (`update_state` key: last check, ETag, newest found, skipped version, remind-after), so the notice shows at the next start with no network. "Skip this version" hides that version only; "Remind me later" hides for three days. The address opened on "View changes" is built from the version (`release_page`), never read from the reply, and release text is never shown. Failure is silent: the card says it could not check, the title bar does not change. Development (debug) builds never check; `OD_UPDATES_URL` replaces the list's address (and lets a debug build check) to try it against a local server. What it shows: the Updates card (`app/home_page/updates.rs`), the title bar's right-hand text (`app/update_notice.rs`: the name, "Allow internet access to stay up to date" before the user has answered, "Version X is available. See Home" when there is one), and Settings > Updates.

## Plugins (design decision, not yet implemented)

Plugins are written in **Python**: the likeliest authors already know it, and
the whole PyPI ecosystem (ezdxf, numpy, shapely) is available to them. They run
**out of process**: a plugin is a script that imports a small `opendraft`
package and talks to the app over a local pipe. This keeps the app free of an
embedded interpreter, lets a script block while waiting for the user
(`p = get_point("Pick start")`), and means a crashing plugin cannot take the
drawing down. Whether we bundle a Python or require the user's own is a
packaging decision that does not change the API. No custom scripting language.

What the app must keep true so the API is cheap to add later:

1. **The command registry is the API.** Plugins register commands (id, label,
   shortcut) and run existing ones by id. Command ids are stable once released;
   never rename one casually.
2. **All document changes go through `Action`s inside a transaction**, so a
   plugin's edits are undoable and grouped into one undo step like any other
   command.
3. **Prompts are a service, not UI code.** Asking the user for a point, a
   number or a selection is one request/response interface (the `Editor`'s
   `get_*` methods) that the command line, canvas and plugins all use. A
   plugin's `ed.get_point(...)` is forwarded to it.
4. **Core and commands stay free of egui** (see the crate table), so the same
   operations can be driven by a script, a test or the UI.
5. **State a plugin may read is plain data** (entities, layers, selection,
   settings) and is addressed by id. Plugins never hold references into the
   app.
6. The wire format is versioned from the first release, and plugins declare
   which API version they target in a manifest.

## Checks before finishing a change

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets`,
`cargo test --workspace`. CI runs the same on Windows and Linux.

## Where things are

- Panels: `od-app/src/app/panels.rs`; command line UI: `app/command_line.rs`;
  Properties: `properties.rs` (model) and `properties_ui.rs` (view).
- Visual constants and palettes: `od-ui/src/theme.rs`. Roadmap: `ui_plan.md`
  (its last section, "Where we are and what next", is the place to start a
  new session) and `2d_commands_reference.md` (the AutoCAD command list we
  work through).
- A new modify command: pure geometry in `od-core` (+ tests), the command in
  its own `od-app` file, an `editor_command!` line in `commands.rs`, an alias
  in `cmdline.rs`, a ribbon item in `ribbon_def.rs` or, for the Home tab,
  `ribbon_def/home.rs` (if Home already shows it dimmed, take its id off
  `NOT_BUILT` there; icons are codepoints in `od-ui/src/icons.rs`; find names
  in the font with fontTools, then run `scripts/export_icons.py` for the new
  icon's SVG), a section in `ui_plan.md`.
- Testing in the real app (Windows): build `od-app`, drive the exe with
  PowerShell (keys, clicks, screenshots). Click on a grip location starts a
  grip edit, so pick selection points away from grips.
