# OpenDraft Rust Code Review Findings

## Executive Summary

OpenDraft is healthier than a typical codebase of this size. The workspace has a deliberate, acyclic crate structure: `od-core` owns the domain model, `od-commands` owns undoable edits, format crates stay below the application layer, and `od-app` composes the system. The closed `Entity` enum, newtyped identifiers, domain-specific error enums, bulk document operations, spatial index, atomic file writes, and parser limits in the DXF/DWG paths are all sound Rust-oriented design choices. Dynamic dispatch is used at legitimate heterogeneous boundaries (`Action`, `FileFormat`, file pickers, callbacks), not as a deep class hierarchy. There is no production `unsafe`, no generic `Box<dyn Error>` error surface, and no pervasive `Rc<RefCell<_>>` ownership architecture.

The repository-wide validation baseline is strong: `cargo clippy --workspace --all-targets --all-features` completed without diagnostics, and `cargo test --workspace --all-targets` completed with 2,170 passing tests, 5 ignored tests, and no failures. Recursive inventory and static scans covered all 9 workspace crates, 557 Rust source files, and approximately 130,000 lines of Rust. The review also inspected every crate manifest and the architecture rules. This was a static and test-based review; it did not include fuzzing or release-mode profiling.

The main systemic risks are at complexity and trust boundaries rather than memory safety. Native `.odx` input has neither a byte limit nor collection budgets, is parsed twice, accepts identifier values that overflow the document's counters, and can encode small acyclic block graphs whose flattened geometry grows exponentially. Separately, save/export/autosave snapshotting deep-clones the drawing and its derived spatial index on the UI thread. The edit API documents that all changes pass through `Session`, but public fields, an externally implementable infallible `Action` trait, and debug-only validation do not enforce that contract.

Severity distribution: **0 Critical, 6 Major, 6 Minor**.

## Status update (2026-10-31)

Every finding below has been checked against the code since it was written and carries a **Status** line. Fixed: OD-006, OD-008, OD-009, OD-010 and OD-011. Mostly fixed: OD-001, OD-003 and OD-005. Partly: OD-002. Measured and left alone: OD-004. Open: OD-007 and OD-012 (both minor).

A separate pass over the automation socket, the DWG and DXF readers, panic handling and the transaction layer added two findings of its own, both since fixed: a panic inside a transaction left its edits in the drawing with no undo step, and eight idle local connections could fill every automation slot (a connection now has ten seconds to say hello). Still open from that pass: on Windows the automation descriptor file (which holds the session token) has no restricted permissions, where on Unix it is written `0600`; it lives under `%LOCALAPPDATA%`, which is already per-user.

## Detailed Findings

### OD-001 — Native files have no resource budget and are parsed twice

- **Severity:** Major
- **Status (2026-10-31):** Mostly fixed. The 512 MB byte limit was already in place; the file is now parsed once; the long lists (objects, table rows, objects in a block, points of a shape) are bounded as they are read, and building the drawing honours the cancel flag (`native/limits.rs`). Not closed: the per-shape lists sit inside an internally tagged enum that serde buffers whole first, so those limits bound the conversion step, not the parse-time allocation.
- **File path & line(s):** `crates/od-files/src/lib.rs:65-74`, `crates/od-files/src/lib.rs:120-124`, `crates/od-files/src/native.rs:70-90`, `crates/od-files/src/native/block.rs:128-129`
- **Description:** `load_cancellable` checks the selected format's size policy and then reads the whole file into memory. `FileFormat::check_size` defaults to accepting any length, and `NativeFormat` does not override it. Its `read` implementation first deserializes the entire byte slice into `Header`, then deserializes the same slice into `FileDto`. Although `Header` only retains three fields, Serde still has to traverse the ignored `drawing` value to finish the first parse. Large native drawings are therefore resident as raw bytes while the JSON is scanned twice and then expanded into DTOs and domain objects. Native collection lengths are also not bounded before allocations such as `Vec::with_capacity(dto.objects.len())`. Unlike the defensive DXF/DWG readers, the native path cannot be cancelled during parsing. A malformed or merely huge local file can exhaust memory or keep a worker busy for an excessive period.
- **Recommendation:** Give the native format explicit limits for bytes, top-level objects, layers, styles, layouts, blocks, objects per block, aggregate string bytes, and aggregate control points. Enforce counts with custom Serde visitors before reserving memory. Parse once: either put a small fixed preamble/version envelope before the JSON payload, or use a streaming/deserializer strategy that validates the header and continues into the drawing without reparsing. Override `read_cancellable` and check cancellation between bounded collections. Add boundary and over-limit tests plus a large-file memory benchmark.

### OD-002 — Unchecked identifier arithmetic lets a native file overflow allocation counters

- **Severity:** Major
- **Status (2026-10-31):** Partly fixed. `Document` counters advance with `saturating_add`. Not re-checked: the other tables' counters, rejecting `u64::MAX` from a file, and a final `Document::validate()` after load.
- **File path & line(s):** `crates/od-core/src/document.rs:170-173`, `crates/od-core/src/document.rs:206-228`, `crates/od-core/src/document/object_list.rs:74-109`, `crates/od-core/src/document/layer_table.rs:33-35`, `crates/od-core/src/document/layer_table.rs:76-83`, `crates/od-core/src/document/blocks.rs:29-31`, `crates/od-core/src/document/blocks.rs:47-56`, `crates/od-core/src/document/layouts.rs:22-40`, `crates/od-core/src/document/text_styles.rs:36-55`, `crates/od-files/src/native.rs:383-442`
- **Description:** Identifier allocation and counter advancement use unchecked `+= 1` and `id.0 + 1`. Native JSON accepts `u64` identifiers and pushes them into the document before reserving the serialized counters. An object, layer, block, layout, viewport, or style with ID `u64::MAX` therefore panics in debug builds while being loaded. In optimized builds the addition wraps to zero, violating the stated never-reuse identity invariant and allowing a later allocation to collide with an existing ID. `DrawingDto::into_document` performs several targeted checks but returns after `finish_bulk_load` without a final `Document::validate()`, so counter defects are not caught at the input boundary.
- **Recommendation:** Reject `u64::MAX` for every externally supplied ID and next-ID field. Centralize counter advancement behind a checked helper and make exhaustion explicit rather than build-mode dependent, for example:

  ```rust
  fn successor(id: u64) -> Result<u64, DocumentError> {
      id.checked_add(1).ok_or(DocumentError::IdSpaceExhausted)
  }
  ```

  Prefer fallible `try_allocate_*`/`try_push_*` APIs at import boundaries. Run `document.validate()` before returning a native load, and add regression tests for maximum IDs in every ID family and for serialized counters behind their maximum observed ID.

### OD-003 — Acyclic block graphs can expand exponentially

- **Severity:** Major
- **Status (2026-10-31):** Mostly fixed. Expansion has a shared budget of `MAX_EXPANSION` (1,000,000 pieces) as well as `MAX_NESTING`, so a doubling graph is bounded. Still open: pictures are rebuilt on every paint and hit test, and nothing is cached.
- **File path & line(s):** `crates/od-core/src/block/mod.rs:42`, `crates/od-core/src/block/picture.rs:28-62`, `crates/od-files/src/native/block.rs:100-128`, `crates/od-canvas/src/view/paint.rs:129-132`, `crates/od-canvas/src/hit.rs:71-76`
- **Description:** Block loading rejects cycles and limits nesting to 64, but it does not limit fan-out or the number of flattened pieces. `Insert::picture_with` recursively appends every leaf into a new `Vec`. A compact graph in which each definition inserts the next definition twice has only O(depth) stored definitions but produces O(2^depth) pieces. The maximum depth of 64 is much too high to be a meaningful work bound. Painting and hit testing call `picture()` synchronously, so a crafted native file can cause allocation blow-up or freeze the UI without violating the current cycle/depth checks. Repeated calls also rematerialize the flattened geometry.
- **Recommendation:** Replace the depth integer with a shared expansion budget containing at least remaining depth and remaining output pieces. Make expansion fallible or visitor-based so callers can stop safely, and reject block tables whose bounded expansion estimate exceeds the document policy. Prefer a lazy iterator/visitor for paint, hit testing, and bounds so culling can happen before allocation. If flattened pictures are cached, cache only successful, budgeted results and invalidate them with immutable block-definition replacement. Add adversarial doubling-graph tests.

### OD-004 — Background file operations deep-clone the drawing on the UI thread

- **Severity:** Major
- **Status (2026-10-31):** Measured, not changed. `Document::clone` takes 1.3 ms at 10,000 plain objects, 17 ms at 100,000 and 42 ms at 250,000 (release build; lines, circles and rectangles), so a save costs one to three dropped frames at those sizes. A persistence snapshot is not worth its complexity unless drawings well beyond that are routine. Drawings full of dimensions, inserts or hatches were not measured.
- **File path & line(s):** `crates/od-core/src/document.rs:23-73`, `crates/od-core/src/document/spatial.rs:59-88`, `crates/od-app/src/file_io.rs:212-235`, `crates/od-app/src/file_io.rs:281-309`, `crates/od-app/src/autosave.rs:28-48`
- **Description:** Save, export, plot, and autosave correctly move encoding and I/O to a worker, but they first call `Document::clone()` synchronously. `Document` owns all objects and symbol tables, and `SpatialIndex::clone` explicitly clones its `slots`, including built R-trees, even though the index is derived state and is not needed for serialization. Large drawings can therefore pause the UI before the worker starts and temporarily require roughly another drawing's worth of memory. Autosave repeats this for every modified document in one UI-side pass.
- **Recommendation:** Introduce an explicit immutable persistence snapshot instead of using general-purpose `Clone`. At minimum, omit the spatial index and other derived caches from the snapshot and rebuild them lazily only if a snapshot is queried. A stronger design stores stable drawing collections in chunked/COW `Arc` data so taking a point-in-time snapshot shares unchanged content and mutations copy only touched chunks. Instrument and benchmark the snapshot phase separately from encoding; moving only the latter to a worker hides the expensive part in current telemetry.

### OD-005 — The edit boundary is documented but not enforced by the type system

- **Severity:** Major
- **Status (2026-10-31):** Mostly fixed. `Session.document` and `Session.history` are `pub(crate)`, and a panic inside a transaction now reverts its edits before it continues. Still open: `Action` is not sealed, and the post-edit validation is debug-only outside automation.
- **File path & line(s):** `ARCHITECTURE.md:21-29`, `crates/od-commands/src/action.rs:5-17`, `crates/od-commands/src/session.rs:28-40`, `crates/od-commands/src/session.rs:114-143`, `crates/od-commands/src/session.rs:145-199`
- **Description:** The architecture requires every document change to be an undoable `Action` run by `Session`, yet `Session::document` and `Session::history` are public. Callers can mutate the document or history independently and bypass revision tracking, change reporting, save-position accounting, and undo coupling. The public `Action` trait is also infallible and implementable outside the crate; `perform` applies an arbitrary action before checking validity. Normal `perform` and `transaction` validate only under `debug_assertions` and panic on failure, while only the automation-specific `checked_transaction` validates and rolls back in every build. The most important architectural invariant is consequently a convention rather than an enforced API.
- **Recommendation:** Make the fields private and expose read-only accessors plus narrowly scoped mutation operations. Seal `Action` unless third-party actions are an intentional supported extension point. Make public mutation paths fallible and rollback-capable, returning a typed `EditError`; keep an explicitly named `pub(crate)` fast path only for actions whose constructors prove their invariants. Avoid a full document scan after every small edit by moving local invariant checks into typed action constructors and validating the complete document at transaction/trust boundaries.

### OD-006 — Power Select wildcard matching has exponential worst-case cost

- **Severity:** Major
- **Status (2026-10-31):** Fixed. The wildcard match now uses a linear remembered-star matcher.
- **File path & line(s):** `crates/od-app/src/power_select.rs:277-285`, `crates/od-app/src/power_select.rs:292-343`, `crates/od-commands/src/sysvar.rs:243-260`
- **Description:** The `*` branch recursively tries every remaining text suffix and has no memoization. Patterns such as repeated `*a` groups followed by a non-matching suffix cause combinatorial backtracking. `run` applies this synchronously to every candidate object, and `passes` rebuilds both `Vec<char>` values for every object. A modest adversarial filter over a large drawing can monopolize the UI thread. A second, separate wildcard implementation already exists for system variables, increasing semantic drift and duplicated maintenance.
- **Recommendation:** Compile and normalize the pattern once before iterating over objects. Use a linear-time glob matcher with the standard remembered-star/backtrack-position algorithm, or bounded dynamic programming if full Unicode scalar semantics are required. Consolidate wildcard behavior into one tested utility, define whether matching is ASCII-insensitive or Unicode-aware, cap pattern length at the UI/API boundary, and add adversarial benchmarks rather than only happy-path unit cases.

### OD-007 — Modal state is represented by parallel flags and detached drafts

- **Severity:** Minor
- **Status (2026-10-31):** Open. `modal_open` (`dialogs.rs`) still lists thirteen flags by hand.
- **File path & line(s):** `crates/od-app/src/state.rs:92-122`, `crates/od-app/src/app.rs:58-77`, `crates/od-app/src/dialogs.rs:5-18`
- **Description:** Dialog identity is split between a set of booleans/options in `AppState` and separate draft/window fields in `OpenDraftApp`. `modal_open` manually repeats most of the list. The representation permits invalid combinations: several exclusive dialogs can be marked open, an open flag can exist without its draft, or a draft can outlive its flag. Adding a dialog requires coordinated edits to state, initialization, commands, focus suppression, draft setup, rendering, and teardown. This is primitive state-flag modeling where an enum could make the valid states explicit.
- **Recommendation:** Model mutually exclusive ownership of keyboard focus as an enum whose variants carry their draft state, for example `Modal::Units(UnitsDraft)` and `Modal::Settings(SettingsDraft)`. Use a small stack only for the few intentionally nested manager/editor flows. Centralize `open`, `close`, `active`, and focus policy on that type. Split unrelated lifecycle, I/O, automation, and document-tab state from the modal controller as `AppState` evolves.

### OD-008 — Entity kinds are internal display strings instead of a domain type

- **Severity:** Minor
- **Status (2026-10-31):** Fixed. `Entity::kind()` returns the `EntityKind` enum (`od-core/src/entity_kind.rs`), with separate display, title and wire names; `kind_name()` has no callers left.
- **File path & line(s):** `crates/od-core/src/lib.rs:159-176`, `crates/od-app/src/power_select.rs:55-63`, `crates/od-app/src/power_select.rs:192-220`, `crates/od-app/src/properties_ui.rs:66-111`, `crates/od-app/src/layers.rs:265-271`, `crates/od-app/src/state.rs:254-269`, `crates/od-app/src/automation/query.rs:94-112`, `crates/od-api/src/entity.rs:197-199`, `crates/od-api/src/entity.rs:426`
- **Description:** `Entity::kind_name()` returns a presentation string that is reused as an internal discriminator and transformed into wire values with `replace(' ', "_")`. Selection, property filtering, layer edits, and automation queries compare these strings. Renaming a UI label can silently change behavior or an API token, and the compiler cannot require call sites to handle a newly added entity variant. Automation query matching also allocates a new transformed `String` inside the candidate/predicate loop.
- **Recommendation:** Add a `Copy`, exhaustive `EntityKind` enum and return it from `Entity::kind()`. Give the enum separate `display_name()` and stable `wire_name()` methods, or Serde renames at the API boundary. Parse requested wire strings once into `HashSet<EntityKind>` before scanning objects. Keep human wording and protocol stability independent.

### OD-009 — `dynamic_overlay` is a 408-line mixed-responsibility function

- **Severity:** Minor
- **Status (2026-10-31):** Fixed. The function is split into `dynamic_overlay.rs` (about 150 lines), `dynamic_overlay/layout.rs` (placement as rectangle arithmetic, with tests) and `dynamic_overlay/boxes.rs` (one painter per box).
- **File path & line(s):** `crates/od-app/src/app/dynamic_overlay.rs:15-422`, `crates/od-app/tests/file_sizes.rs:1-15`
- **Description:** One function derives prompt fields, measures text, computes collision-avoiding placement, renders boxes and ghost dimensions, manages focus and text editing, records hit rectangles, renders keyword/menu/error popups, and dispatches the chosen command option. Its length is just below the 450-line production-file guard, demonstrating that the file-level metric does not prevent oversized functions. Layout policy, rendering, and state transitions are tightly interleaved, which makes behavior difficult to test without an egui frame and makes regressions likely when another input mode is added.
- **Recommendation:** Extract a pure `OverlayLayout::compute(input, viewport, metrics) -> OverlayPlan` and test it with table-driven geometry cases. Render prompt, field, dimension, keyword, menu, and error nodes in focused helpers. Move editing/focus transitions behind methods on `DynamicInputState`, returning semantic events such as `Commit`, `Choose`, or `Cancel`. Add a lightweight function-size or complexity check if the architectural size rule is intended to constrain readability rather than only files.

### OD-010 — Command defaults are hidden process-global mutable state

- **Severity:** Minor
- **Status (2026-10-31):** Fixed. `CommandDefaults` is owned by `AppState` and handed to commands through `od_commands::Context`; the ten statics are gone. `window_memory.rs` (the saved window size) is still a global: it belongs to the shell and is read before any `AppState` exists.
- **File path & line(s):** `crates/od-app/src/text_memory.rs:20`, `crates/od-app/src/rect_memory.rs:22`, `crates/od-app/src/corner_memory.rs:56`, `crates/od-app/src/spline_memory.rs:44`, `crates/od-app/src/hatch_memory.rs:58`
- **Description:** Several commands keep “last used” values in independent static `Mutex` instances. This avoids borrow plumbing, but it makes command behavior depend on invisible process-wide state, prevents two app instances/profiles in one process from having independent defaults, scatters persistence knowledge across modules, and forces tests of otherwise pure command logic to serialize around shared state. The mutexes are not a memory-safety problem; the smell is ownership and dependency visibility.
- **Recommendation:** Consolidate these values into a serializable `CommandDefaults` owned by the application/settings context and pass the relevant mutable substate to command construction or completion. Register persistence in one place and make tests instantiate defaults explicitly. If a global facade is temporarily retained, put all defaults behind one service rather than multiple module statics so the dependency and reset boundary are visible.

### OD-011 — Merging large change sets is quadratic

- **Severity:** Minor
- **Status (2026-10-31):** Fixed. `Changes::merged` filters through hash sets.
- **File path & line(s):** `crates/od-commands/src/changes.rs:118-149`
- **Description:** `Changes::merged` collects IDs in `Vec`s and repeatedly uses `Vec::contains` while finding cancellations and retaining added, removed, and changed IDs. The cancellation and filtering passes are O(n²) in the number of IDs. This conflicts with the otherwise careful bulk-edit architecture: a transaction that adds/removes/modifies many entities can spend disproportionate time combining its report even when the underlying document mutation is linear.
- **Recommendation:** Track membership with `HashSet<EntityId>`/`BTreeSet<EntityId>` during normalization, or fold each ID into a small transition state (`Added`, `Changed`, `Removed`, `Cancelled`). Convert back to sorted vectors once at the public boundary for deterministic output. Add a benchmark with overlapping 10k–100k ID batches.

### OD-012 — Idempotency equivalence relies only on a 64-bit non-cryptographic hash

- **Severity:** Minor
- **Status (2026-10-31):** Open. `fingerprint` in `idempotency.rs` is still a 64-bit `DefaultHasher` digest. Its comment says it guards against mistakes, not against a determined caller, which is the intent.
- **File path & line(s):** `crates/od-app/src/automation/idempotency.rs:18-29`, `crates/od-app/src/automation/idempotency.rs:73-102`
- **Description:** Replay entries retain only a `u64` `DefaultHasher` fingerprint of canonicalized JSON arguments. If two different argument values collide, reuse of an idempotency key is incorrectly treated as the same request and the earlier successful answer is returned. The comment explicitly frames the check as accidental-misuse protection, but the externally visible idempotency contract should compare request identity rather than probabilistic equality. `serde_json::to_string(...).unwrap_or_default()` also maps a serialization failure to the hash of an empty string instead of making failure explicit.
- **Recommendation:** Store the canonical serialized argument bytes for the bounded 256-entry replay window and compare them exactly. If memory measurements require digests, use a stable collision-resistant digest and retain length plus canonical bytes for collision confirmation. Make fingerprint construction fallible and reject an unrepresentable request instead of substituting an empty value.

## Architectural Assessment

- **Coupling and cycles:** No crate cycle was found. Dependencies follow the documented downward layering. The principal coupling hotspot is `od-app`, as expected for the composition root; its modal-state split and global command memories are the actionable exceptions above.
- **OOP-forced patterns:** No inheritance-shaped trait hierarchy was found. `Box<dyn Action>` is justified by heterogeneous undo history, and `&dyn FileFormat` is justified by runtime format selection. The main abstraction concern is not excessive dynamic dispatch but that `Action` is too open and infallible for the invariant it guards.
- **Ownership and concurrency:** `unsafe` is forbidden workspace-wide and absent from Rust sources. Interior mutability is localized to UI caches, clipboard/I/O adapters, scripted test helpers, and command memories; there is no broad `Rc<RefCell<_>>` domain graph. The significant ownership cost is eager deep cloning for persistence snapshots, not borrow-checker circumvention.
- **Errors and type modeling:** File, API, document, and command layers generally use domain errors rather than `Box<dyn Error>`. Production `unwrap`/`expect` misuse was not exposed by the workspace Clippy policy. Entity geometry and most protocol operations are well modeled with enums and newtypes. Identifier exhaustion, modal identity, and entity-kind identity are the notable places where stronger types should replace unchecked integers or strings/flags.

## Recommended Order of Work

1. Harden native input as one effort: byte/collection budgets, single-pass cancellable parsing, checked identifiers, final validation, and adversarial tests (OD-001, OD-002, OD-003).
2. Remove the synchronous full-document snapshot cost and measure large real drawings (OD-004).
3. Close and make fallible the `Session` mutation boundary before adding more integrations or plugin edit surfaces (OD-005).
4. Replace the wildcard matcher before exposing Power Select through automation or accepting unbounded patterns (OD-006).
5. Address the maintainability/performance items incrementally with typed modal/entity-kind state, decomposition, owned defaults, and linear change merging (OD-007 through OD-012).
