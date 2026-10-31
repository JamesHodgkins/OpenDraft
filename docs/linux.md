# OpenDraft on Linux

OpenDraft is a native Linux program: it needs no interpreter and no runtime, and runs on X11 and on Wayland. This page says how to install it, what it needs, what is tested (and what is not), and what to do when something does not work.

## Install

| How | Good for | Notes |
|---|---|---|
| **Flatpak** | Any distribution | `installer/linux/flatpak/` (see its README). Sandboxed; the assistant connection needs the one extra step described there |
| **AppImage** | Any distribution, no root | One file from the [releases](https://github.com/JamesHodgkins/OpenDraft/releases). Make it executable and run it |
| **.deb** | Debian, Ubuntu, Mint | `sudo apt install ./OpenDraft-<version>-linux-amd64.deb`. Adds the menu entry, icons and the `.odx` file type |
| **Arch (AUR)** | Arch and derivatives | The `PKGBUILD` is in `installer/linux/arch/` |
| **Archive** | Anything else | `OpenDraft-<version>-linux.tar.gz`: `opendraft` and `opendraft-mcp` side by side. Needs the libraries below |
| **From source** | Developers | `cargo build --release -p od-app -p od-mcp` |

The programs in the .deb, AppImage and archive are built on Ubuntu 22.04, so they need glibc 2.35 or newer.

## What it needs

OpenDraft opens these libraries when it starts, so a bare system must have them. Desktop installations almost always do.

| Distribution | Packages |
|---|---|
| Debian, Ubuntu, Mint | `libgl1-mesa-dri libegl1 libgl1 libx11-6 libx11-xcb1 libxcb1 libxcursor1 libxi6 libxkbcommon0 libxkbcommon-x11-0 libwayland-client0 libwayland-cursor0 libwayland-egl1` |
| Fedora | `mesa-dri-drivers mesa-libEGL mesa-libGL libX11 libX11-xcb libxcb libXcursor libXi libxkbcommon libxkbcommon-x11 libwayland-client libwayland-cursor libwayland-egl` |
| Arch | `mesa libglvnd libx11 libxcb libxcursor libxi libxkbcommon libxkbcommon-x11 wayland` |
| openSUSE | `Mesa-dri Mesa-libEGL1 Mesa-libGL1 libX11-6 libX11-xcb1 libxcb1 libXcursor1 libXi6 libxkbcommon0 libxkbcommon-x11-0 libwayland-client0 libwayland-cursor0 libwayland-egl1` |

For **file dialogs** (Open, Save As) you also need one of: `xdg-desktop-portal` with a backend for your desktop (`xdg-desktop-portal-gtk`, `-kde`, `-gnome`...), or `zenity`. Every mainstream desktop has a portal already.

## What is tested

Every change is built once on Ubuntu 22.04 and that same program is run on the distributions below (`.github/workflows/linux.yml`, `scripts/linux_smoke.py`). Each run starts the real program, opens a DXF drawing, checks the screen is drawn, types `L`, clicks twice and presses Enter to draw a line, undoes it with Ctrl+Z, checks the result through the automation socket, runs `opendraft-mcp --check` against the live program, and, in a second launch with no drawing, quits with Ctrl+Q and checks the exit code.

| | X11 | Wayland | Scaling 1.5 and 2 |
|---|---|---|---|
| Ubuntu 22.04 | yes | | |
| Ubuntu 24.04 | yes | yes | yes |
| Debian 12 | yes | | |
| Fedora | yes | yes | 2 |
| Arch | yes | yes | |
| openSUSE Tumbleweed | yes | | |

The release workflow also installs the .deb and runs the AppImage on a bare Ubuntu 22.04 and checks they start. The Arch package and the Flatpak are built in CI (`.github/workflows/packages.yml`).

### What this does not show

Be careful how far to trust a green tick. These runs use a virtual display with software rendering:

- **No real graphics hardware.** Drivers, GPUs, multi-monitor and variable refresh are untested. If your GPU stack is unusual, tell us.
- **No real desktop environment.** X11 runs with no window manager; Wayland runs under `sway`. GNOME (mutter) and KDE (KWin) have their own window handling and decorations, and are not tested.
- **No real file dialogs, clipboard, drag and drop or input methods.** These depend on the desktop and are untested.
- **Pixel-for-pixel appearance** is not compared; the tests check that the screen is drawn, not that it is identical to a reference.

## If something does not work

**It does not start, or starts and closes at once.** Run it from a terminal and read the last line.

- `NoGlutinConfigs` or "no matching configuration": the graphics setup offers no OpenGL configuration the program could use. OpenDraft asks for smooth (multisampled) edges first and settles for plain ones, so this now only appears when there is no usable OpenGL at all. Install the Mesa packages above; for a quick test try `LIBGL_ALWAYS_SOFTWARE=1 opendraft`.
- "error while loading shared libraries": install the packages in the table above.

**Open or Save As does nothing.** The status bar will say that no file dialog could be shown. Install `xdg-desktop-portal` and a backend for your desktop, or `zenity`. Until then you can open a drawing by giving its name when you start the program: `opendraft drawing.dxf`.

**The window is too small or too large on a high-resolution screen.** The program follows the desktop's scale. To force one on X11: `WINIT_X11_SCALE_FACTOR=1.5 opendraft`.

**Force X11 or Wayland.** The program uses Wayland when `WAYLAND_DISPLAY` is set and falls back to X11 otherwise. To run under X11 in a Wayland session: `env -u WAYLAND_DISPLAY opendraft`. To make sure it is on Wayland: `env -u DISPLAY opendraft`.

**The menu entry has a generic icon on Wayland.** The window's id must match the desktop file's name. OpenDraft's is `opendraft` (and `uk.openengineering.opendraft` inside the Flatpak); a custom launcher needs `StartupWMClass=opendraft`.

**The assistant (MCP) connection.** It is off by default; turn it on in Settings, Automation. The program and `opendraft-mcp` find each other through a socket in `$XDG_RUNTIME_DIR/OpenDraft/automation`. If your session has no `XDG_RUNTIME_DIR` (a plain `ssh` or `su` login, some containers), the folder `~/.cache/OpenDraft/automation` is used instead. `opendraft-mcp --check` says whether it can reach the program. `OD_AUTOMATION=1` turns the connection on without the setting, and `OD_AUTOMATION_DIR` moves the socket's folder (give both programs the same value).

**Where things are kept.** Settings, recent files, autosave copies and `crash.log` are in `~/.local/share/opendraft` (`$XDG_DATA_HOME/opendraft`). Delete the folder to start afresh. A crash report there is worth attaching to a bug report.

## Reporting a problem

Please include: your distribution and version, X11 or Wayland (`echo $XDG_SESSION_TYPE`), the desktop environment, your graphics hardware or "virtual machine", how you installed OpenDraft, what you did, what you expected and what happened, the terminal output if you started it from a terminal, and `crash.log` if there is one. A screenshot helps.

## For packagers

A distribution package needs: `opendraft` and `opendraft-mcp` in `PATH`; `installer/linux/share/opendraft.desktop`; the icons `assets/app-icons/<size>x<size>.png` as `hicolor/<size>x<size>/apps/opendraft.png`; `installer/linux/opendraft.xml` in `/usr/share/mime/packages`; and, for software centres, `installer/linux/share/uk.openengineering.opendraft.metainfo.xml`. The icon drawings the program reads at start are built in, so nothing else is needed at run time. `scripts/check_packaging.py` keeps these files in agreement with each other and with the version.
