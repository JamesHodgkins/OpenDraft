#!/usr/bin/env bash
# Run INSIDE a clean distribution container (see .github/workflows/linux.yml):
#
#   SMOKE_SCALES="1 2" scripts/linux_smoke_container.sh <folder with opendraft and opendraft-mcp> <output folder>
#   SMOKE_SESSION=wayland scripts/linux_smoke_container.sh <folder> <output folder>
#
# Installs what a desktop of that distribution has and OpenDraft needs (a
# software GL driver and the window-system libraries it opens at run time),
# plus the test's own tools, then runs the smoke test: under a virtual X server
# at each scale in SMOKE_SCALES (default "1"), or, with SMOKE_SESSION=wayland,
# under a headless Wayland compositor. The package lists here are the ones
# docs/linux.md tells users to install.
set -euxo pipefail

bin=${1:?usage: linux_smoke_container.sh <binary folder> <output folder>}
out=${2:?usage: linux_smoke_container.sh <binary folder> <output folder>}
session=${SMOKE_SESSION:-x11}
here=$(cd "$(dirname "$0")" && pwd)

. /etc/os-release
echo "distribution: ${PRETTY_NAME:-$ID} (glibc $(ldd --version | head -1 | awk '{print $NF}')), session: $session"

# The test's tools: a virtual X server and xdotool, or sway with grim and wtype.
case "$session" in
  x11) apt_tools="xvfb xauth xdotool"; dnf_tools="xorg-x11-server-Xvfb xorg-x11-xauth xdotool";
       pacman_tools="xorg-server-xvfb xorg-xauth xdotool"; zypper_tools="xorg-x11-server-Xvfb xauth xdotool" ;;
  wayland) apt_tools="sway grim wtype"; dnf_tools="sway grim wtype";
       pacman_tools="sway grim wtype"; zypper_tools="sway grim wtype" ;;
  *) echo "SMOKE_SESSION must be x11 or wayland"; exit 1 ;;
esac

case "$ID" in
  ubuntu | debian)
    export DEBIAN_FRONTEND=noninteractive
    apt-get update
    # shellcheck disable=SC2086
    apt-get install -y --no-install-recommends $apt_tools python3 fonts-dejavu-core \
      libgl1-mesa-dri libegl1 libgl1 libx11-6 libx11-xcb1 libxcb1 libxcursor1 libxi6 \
      libxkbcommon0 libxkbcommon-x11-0 libwayland-client0 libwayland-cursor0 libwayland-egl1
    ;;
  fedora)
    # shellcheck disable=SC2086
    dnf install -y --setopt=install_weak_deps=False $dnf_tools python3 dejavu-sans-fonts \
      mesa-dri-drivers mesa-libEGL mesa-libGL libX11 libX11-xcb libxcb libXcursor libXi \
      libxkbcommon libxkbcommon-x11 libwayland-client libwayland-cursor libwayland-egl
    ;;
  arch)
    # shellcheck disable=SC2086
    pacman -Syu --noconfirm --needed $pacman_tools python ttf-dejavu \
      mesa libglvnd libx11 libxcb libxcursor libxi libxkbcommon libxkbcommon-x11 wayland
    ;;
  opensuse* | sles)
    # shellcheck disable=SC2086
    zypper --non-interactive install --no-recommends $zypper_tools python3 dejavu-fonts \
      Mesa-dri Mesa-libEGL1 Mesa-libGL1 libX11-6 libX11-xcb1 libxcb1 libXcursor1 libXi6 \
      libxkbcommon0 libxkbcommon-x11-0 libwayland-client0 libwayland-cursor0 libwayland-egl1
    ;;
  *)
    echo "this script does not know how to install packages on $ID"
    exit 1
    ;;
esac

if [ "$session" = wayland ]; then
  exec bash "$here/linux_smoke_wayland.sh" "$bin" "$out" "smoke-$ID-wayland"
fi

status=0
for scale in ${SMOKE_SCALES:-1}; do
  if [ "$scale" = 1 ]; then
    bash "$here/linux_smoke.sh" "$bin" "$out" "" "smoke-$ID" || status=1
  else
    bash "$here/linux_smoke.sh" "$bin" "$out" "$scale" "smoke-$ID-x$scale" || status=1
  fi
done
exit $status
