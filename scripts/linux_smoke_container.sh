#!/usr/bin/env bash
# Run INSIDE a clean distribution container (see .github/workflows/linux.yml):
#
#   SMOKE_SCALES="1 2" scripts/linux_smoke_container.sh <folder with opendraft and opendraft-mcp> <output folder>
#
# Installs what a desktop of that distribution has and OpenDraft needs (a
# virtual X server, a software GL driver, the window-system libraries it opens
# at run time, xdotool and python for the test), then runs the smoke test at
# each scale in SMOKE_SCALES (default "1"). The package lists here are the ones
# docs/linux.md tells users to install.
set -euxo pipefail

bin=${1:?usage: linux_smoke_container.sh <binary folder> <output folder>}
out=${2:?usage: linux_smoke_container.sh <binary folder> <output folder>}
here=$(cd "$(dirname "$0")" && pwd)

. /etc/os-release
echo "distribution: ${PRETTY_NAME:-$ID} (glibc $(ldd --version | head -1 | awk '{print $NF}'))"

case "$ID" in
  ubuntu | debian)
    export DEBIAN_FRONTEND=noninteractive
    apt-get update
    apt-get install -y --no-install-recommends \
      xvfb xauth xdotool python3 fonts-dejavu-core \
      libgl1-mesa-dri libegl1 libgl1 libx11-6 libx11-xcb1 libxcb1 libxcursor1 libxi6 \
      libxkbcommon0 libxkbcommon-x11-0 libwayland-client0 libwayland-cursor0 libwayland-egl1
    ;;
  fedora)
    dnf install -y --setopt=install_weak_deps=False \
      xorg-x11-server-Xvfb xorg-x11-xauth xdotool python3 dejavu-sans-fonts \
      mesa-dri-drivers mesa-libEGL mesa-libGL libX11 libX11-xcb libxcb libXcursor libXi \
      libxkbcommon libxkbcommon-x11 libwayland-client libwayland-cursor libwayland-egl
    ;;
  arch)
    pacman -Syu --noconfirm --needed \
      xorg-server-xvfb xorg-xauth xdotool python ttf-dejavu \
      mesa libglvnd libx11 libxcb libxcursor libxi libxkbcommon libxkbcommon-x11 wayland
    ;;
  opensuse* | sles)
    zypper --non-interactive install --no-recommends \
      xorg-x11-server-Xvfb xauth xdotool python3 dejavu-fonts \
      Mesa-dri Mesa-libEGL1 Mesa-libGL1 libX11-6 libX11-xcb1 libxcb1 libXcursor1 libXi6 \
      libxkbcommon0 libxkbcommon-x11-0 libwayland-client0 libwayland-cursor0 libwayland-egl1
    ;;
  *)
    echo "this script does not know how to install packages on $ID"
    exit 1
    ;;
esac

status=0
for scale in ${SMOKE_SCALES:-1}; do
  if [ "$scale" = 1 ]; then
    bash "$here/linux_smoke.sh" "$bin" "$out" "" "smoke-$ID" || status=1
  else
    bash "$here/linux_smoke.sh" "$bin" "$out" "$scale" "smoke-$ID-x$scale" || status=1
  fi
done
exit $status
