Text to paste into the Flathub pull request (title: `Add uk.openengineering.opendraft`). Adapt it to the checklist the pull request template shows.

---

### App

OpenDraft: a free, open-source (GPL-3.0-or-later) 2D CAD application written in Rust on egui. It opens DXF and DWG drawings, has an AutoCAD-style ribbon and command line, and keeps its own `.odx` format.

- Homepage and source: https://github.com/JamesHodgkins/OpenDraft
- I am the author and maintainer of the project and the submitter of this pull request.
- The application id `uk.openengineering.opendraft` is on a domain I control (`openengineering.uk`); I will verify it through the developer portal after this is accepted.
- It builds offline from the tagged release: crates come from `cargo-sources.json`, generated from the tag's `Cargo.lock` with `flatpak-cargo-generator`.
- `flatpak-builder-lint` passes on the manifest and on the AppStream file; the AppStream file validates with `appstreamcli validate`.
- Release tested on six distributions under X11 and three under Wayland, in CI; the Flatpak itself is started under a virtual display in CI. See `docs/linux.md` in the repository for what is and is not tested.

### Permissions, and why

| Permission | Why |
|---|---|
| `--socket=wayland`, `--socket=fallback-x11`, `--share=ipc`, `--device=dri` | The window and OpenGL |
| `--share=network` | An optional AI-assistant feature that is off by default and sends nothing until the user turns it on and enters a key. The in-app update check is disabled inside a Flatpak (`FLATPAK_ID`), so Flathub is the only updater |
| `--filesystem=home` | Drawings are opened and saved anywhere in the home folder. A save writes a temporary file next to the drawing and renames it (so a crash never leaves a half-written drawing), and autosave and crash-recovery copies are written beside it; a single file granted by the file chooser portal does not allow that. I am happy to narrow this if the reviewers prefer a particular alternative |
| `--talk-name=org.freedesktop.secrets` | The optional assistant's API key can be kept in the desktop's password store rather than a file |

### Notes for the reviewers

- The binaries are in `/app/libexec`; `/app/bin/opendraft` and `/app/bin/opendraft-mcp` are two small scripts that point the program's automation socket at `$XDG_RUNTIME_DIR/app/$FLATPAK_ID`, the one folder a sandbox shares with the host. The automation feature (a local socket for tools such as an MCP server) is off by default.
- The AppStream screenshot is hosted in the project's repository; Flathub mirrors it at build time.
