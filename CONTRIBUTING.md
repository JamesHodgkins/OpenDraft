# Contributing to OpenDraft

Thank you for your interest in contributing to OpenDraft! Contributions of all sizes are welcome: bug reports, documentation, tests and code.

For anything larger than a small fix, please [open an issue](https://github.com/JamesHodgkins/OpenDraft/issues) first so we can agree on the approach before you spend time on it.

## 1. Fork and clone

Fork the repository on GitHub, then clone your fork:

```bash
git clone https://github.com/<your-name>/OpenDraft.git
cd OpenDraft
```

## 2. Set up your environment

You need a recent stable [Rust toolchain](https://rustup.rs/) with `rustfmt` and `clippy` (`rustup component add rustfmt clippy`).

On Linux, install the windowing libraries first (Debian/Ubuntu):

```bash
sudo apt-get install -y libxkbcommon-dev libwayland-dev libx11-dev libxcb1-dev libgl1-mesa-dev
```

Run the application:

```bash
cargo run -p od-app
```

## 3. Understand the project

The workspace is split into crates, and dependencies point downward:

| Crate | Purpose |
|---|---|
| `od-core` | Document, entities and geometry |
| `od-commands` | Command registry, undo history, sessions and the `Editor` used by commands |
| `od-files` | File formats: native `.odx`, DXF, atomic save |
| `od-canvas` | Camera, drawing view, hit-testing, snapping, selection |
| `od-ui` | Theme, widgets, ribbon, icons, shortcuts |
| `od-app` | The application: state, command table, panels, dock |

Read [ARCHITECTURE.md](./ARCHITECTURE.md) before writing code. It records the rules new code follows, and [ui_plan.md](./ui_plan.md) shows the interface plan and what is done.

## 4. Create a branch

```bash
git checkout -b feature/your-feature-name
```

## 5. Make your changes

The main conventions (the full list is in ARCHITECTURE.md):

- **Every user action is a command**, registered in `crates/od-app/src/commands.rs`. The ribbon, shortcuts, status bar and command line refer to commands by id.
- **Every document change is an undoable action** run by the session. A command that makes several edits groups them in a transaction so they undo as one step.
- **Logic is pure and tested; the UI only draws.** Put rules in functions over plain data, with unit tests.
- **Keep files under about 400 lines of production code.** Tests do not count. A test enforces a hard limit.
- **No `unwrap`, `todo!`, `dbg!` or prints in production code**, and `unsafe` is forbidden.
- Match the style, naming and comment density of the code around you, and avoid unnecessary abstractions.
- A new entity kind in the `.odx` format must bump the format version.

## 6. Test your changes

CI runs these on Windows and Linux, with warnings as errors. Run them before you push:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets
cargo test --workspace
```

Add tests for new behaviour and for the bug you are fixing. Also run the application and try the feature by hand.

## 7. Commit and push

Write clear commit messages that say what changed and why:

```bash
git add <files>
git commit -m "Add Offset command"
git push origin feature/your-feature-name
```

## 8. Open a pull request

Open a pull request against `main` of the original repository. Describe what you changed, why, and anything a reviewer should look at. Reference the related issue if there is one. By contributing, you agree that your work is released under the project's [GPL-3.0 licence](./LICENSE).

## 9. Review

A maintainer will review your pull request and may ask for changes. Push further commits to the same branch to update it. Once approved and CI is green, it will be merged into `main`.

## Code of Conduct

All contributors are expected to be respectful and considerate. See [CODE_OF_CONDUCT.md](./CODE_OF_CONDUCT.md) for details.

## Releasing

Before tagging a release, decide what the Home page's "What's new" card says. `crates/od-app/src/whats_new.rs` holds one `Note` per version, keyed by the exact `CARGO_PKG_VERSION`: a short title and at most three plain-language changes users will notice (capabilities and workflow, not refactors or dependency updates). Add one when the release has changes worth announcing; leave it out when it does not, and the card simply does not show. The card's "View release notes" opens `https://github.com/JamesHodgkins/OpenDraft/releases/tag/v<version>`, so publish the release with that tag.

## Getting help

If you have questions, open an issue on the GitHub repository.

Thank you for contributing to OpenDraft!
