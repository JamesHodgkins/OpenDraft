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

A debug build keeps its settings, recent files and recovery copies in its own `OpenDraft Dev` folder, apart from an installed release's; `cargo run --release -p od-app` uses the release's. Set `OPENDRAFT_PROFILE=<name>` to use another separate set (for example for a test run).

A debug build never checks for new versions. To try the update notice, serve a releases list locally (a JSON array like GitHub's `/releases`, with `tag_name`, `draft`, `prerelease` and `assets[].name` such as `OpenDraft-0.2.0-windows.zip`) and set `OD_UPDATES_URL=http://127.0.0.1:<port>/releases`; that also lets a debug build check. Click "Check automatically" on the Home page's Updates card.

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
- **A new dependency's licence must be on the list in `deny.toml`** (and the same list in `about.toml`). OpenDraft is GPL-3.0-or-later and ships its dependencies' notices in `THIRD-PARTY-LICENSES.txt`; CI runs `cargo deny check licenses` and makes that file, so a licence that is not on the list fails the build. To see it locally: `cargo install cargo-deny cargo-about --locked` (cargo-about also needs `--features cli`), then `cargo deny check licenses` and `bash scripts/third_party.sh out.txt`. A font or icon file that is not a Rust crate needs its licence file beside it and a line in `scripts/third_party.sh`.

## 6. Test your changes

The Windows build links the C runtime into the program (`.cargo/config.toml`). Setting `RUSTFLAGS` yourself replaces that file's flags, so include `-C target-feature=+crt-static` if you do, or the exe will need the Visual C++ Redistributable.

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

A release is a version tag on `main`. Stable releases are `vX.Y.Z`; a version with a suffix (`v0.2.0-beta.1`) is published as a prerelease. A published release and its files are never changed: a mistake is fixed by a new version.

1. Set `version` in the root `Cargo.toml` to the release version and merge that to `main` through a pull request.
2. Decide the "What's new" note (below).
3. Tag that commit and push the tag: `git tag v0.2.0 && git push origin v0.2.0`.
4. The `release` workflow then checks that the tag is `v` plus the `Cargo.toml` version and that the commit is on `main`, runs formatting, clippy and the tests on Windows and Linux, builds both packages, and publishes one release holding the Windows zip, the Linux tarball and `SHA256SUMS`. The release is created as a draft and made public only once every file is attached, so a failed run leaves nothing visible; push the fix and re-run it (a leftover draft for the tag is replaced, a published release is refused).
5. Open the release page and check the files and the generated notes.

A release that is not a prerelease also gets a Windows installer, `OpenDraft-<version>-windows-x64.msi`, built by `cargo packager` from `installer/windows/main.wxs` and the `[package.metadata.packager]` settings in `crates/od-app/Cargo.toml` (an MSI's version is numeric only, so a `-beta` tag gets the zip and tarball only). It installs per user, takes its version from Cargo, and upgrades older installs in place. **Do not change its `identifier` (`uk.openengineering.opendraft`)**: Windows Installer's upgrade code is derived from it, so a new one would leave the old version installed beside the new one. Its pictures are `installer/windows/banner.bmp` and `dialog.bmp`, made from the PNGs in `installer/windows/source/` by `python scripts/installer_images.py` (commit both; the script says where the installer draws its text). To try it locally: `cargo install cargo-packager --locked`, then `bash scripts/package.sh` (it builds both programs and leaves `dist/THIRD-PARTY-LICENSES.txt`, which the installer installs beside the programs with `LICENSE.txt`), then `cargo packager --release -f wix` (the file lands in `dist/packager`).

For Linux the same step builds `OpenDraft-<version>-linux-amd64.deb` and `OpenDraft-<version>-linux-x86_64.AppImage` (again for plain versions only; a Debian version with a hyphen would sort above the final release). The `.deb` is installed in a bare `ubuntu:22.04` container by `scripts/test_deb.sh` and OpenDraft is started under a virtual display, which checks the package's `Depends` line is enough; the AppImage is started the same way. The menu entry, MIME type and icons are in `installer/linux/` and `assets/app-icons/` (made by `cargo run -p od-app --example export_app_icon`); the window's app id (`opendraft`, from `profile::app_id`) must match `StartupWMClass` in the desktop entry. `cargo packager` does not build `.deb` on Windows, so this runs only in CI.

Running the workflow by hand from the Actions tab builds and tests the packages without publishing anything.

The Linux package is built on Ubuntu 22.04 and has not been run extensively; call it experimental in the notes.

Before tagging a release, decide what the Home page's "What's new" card says. `crates/od-app/src/whats_new.rs` holds one `Note` per version, keyed by the exact `CARGO_PKG_VERSION`: a short title and at most three plain-language changes users will notice (capabilities and workflow, not refactors or dependency updates). Add one when the release has changes worth announcing; leave it out when it does not, and the card simply does not show. The card's "View release notes" opens `https://github.com/JamesHodgkins/OpenDraft/releases/tag/v<version>`, so publish the release with that tag.

## Getting help

If you have a question, ask it in [Discussions](https://github.com/JamesHodgkins/OpenDraft/discussions) (Q&A). Bugs and feature requests have [issue forms](https://github.com/JamesHodgkins/OpenDraft/issues/new/choose), and security problems go through the [security policy](./SECURITY.md).

Thank you for contributing to OpenDraft!
