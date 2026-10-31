# OpenDraft as a Flatpak

`uk.openengineering.opendraft.yml` builds OpenDraft against the Freedesktop runtime
and installs it with its desktop entry, icons, file types and AppStream description
(`../share/`). `.github/workflows/packages.yml` builds it on every change to
`installer/linux/`.

## Build it yourself

You need `flatpak`, `flatpak-builder` and the Freedesktop 26.08 platform, SDK and
Rust extension from Flathub:

```bash
flatpak remote-add --if-not-exists --user flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak install --user flathub org.freedesktop.Platform//26.08 org.freedesktop.Sdk//26.08 \
    org.freedesktop.Sdk.Extension.rust-stable//26.08

# Flatpak builds have no network, so every crate is listed in a file made from Cargo.lock.
git clone --depth 1 https://github.com/flatpak/flatpak-builder-tools
pip install aiohttp toml tomlkit
python3 flatpak-builder-tools/cargo/flatpak-cargo-generator.py Cargo.lock \
    -o installer/linux/flatpak/cargo-sources.json

flatpak-builder --user --install --force-clean build-dir \
    installer/linux/flatpak/uk.openengineering.opendraft.yml
flatpak run uk.openengineering.opendraft
```

`cargo-sources.json` is generated, so it is not committed; make it again whenever
`Cargo.lock` changes.

## What the sandbox allows, and why

| Permission | Why |
|---|---|
| Wayland, X11 (only when there is no Wayland), GPU | The window and OpenGL |
| Network | The optional update check and the optional assistant |
| Home folder | A save writes a temporary file beside the drawing and renames it; autosave and crash recovery write beside it too. A single file handed over by the file chooser portal would not allow that |
| `org.freedesktop.secrets` | Keeping the assistant's API key in the desktop's password store |

File dialogs go through the desktop portal, which a Flatpak always has.

## The assistant connection (MCP)

The program starts with the automation socket in `$XDG_RUNTIME_DIR/app/uk.openengineering.opendraft/`,
the one folder a sandbox shares with the host. Turn the connection on in Settings as
usual, then point the assistant at either of:

```bash
flatpak run --command=opendraft-mcp uk.openengineering.opendraft
```

or, if the assistant runs outside the sandbox and you have `opendraft-mcp` installed
on the host, run that with `OD_AUTOMATION_DIR=$XDG_RUNTIME_DIR/app/uk.openengineering.opendraft/automation`.

The path on the Automation page is the path inside the sandbox; use the command above.

## Submitting to Flathub

Flathub builds from its own repository, `flathub/uk.openengineering.opendraft`, so a
release is a pull request there, not a change here. Make it from this manifest with
two changes:

1. The first source becomes the release, not this checkout:

   ```yaml
   - type: git
     url: https://github.com/JamesHodgkins/OpenDraft.git
     tag: v0.1.3
     commit: <the commit the tag points at>
   ```

2. `cargo-sources.json` (made as above, for that tag's `Cargo.lock`) is committed beside
   the manifest.

The application id is `uk.openengineering.opendraft`, which Flathub only lists as "verified" if the project proves it controls `openengineering.uk`. After the app is accepted, open it in the Flathub developer portal and follow its verification steps: it gives a token to publish as a plain file under `https://openengineering.uk/.well-known/` (the portal shows the exact file name and contents, which are the authority). Until then the app is installable but unverified. An id on a domain nobody can prove ownership of would have to be `io.github.JamesHodgkins.OpenDraft` instead, which is why the id was checked with the project's owner first.

The in-app update check is switched off inside a Flatpak (`FLATPAK_ID` is set), as Flathub asks, because Flathub updates the app.

Also check that the AppStream file passes `flatpak run --command=flatpak-builder-lint
org.flatpak.Builder appstream <file>` and that the screenshot URL in it is reachable.
The reviewers will ask about `--filesystem=home` and `--share=network`; the table above
is the answer.
