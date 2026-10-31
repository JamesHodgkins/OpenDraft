#!/usr/bin/env bash
# Run OpenDraft under a headless Wayland compositor (sway, with the pixman
# renderer, so no GPU is needed) and drive it (scripts/linux_smoke.py).
#
#   scripts/linux_smoke_wayland.sh <folder with opendraft and opendraft-mcp> <output folder> [label]
#
# Needs sway, grim, wtype and python3 on the machine, and the libraries OpenDraft
# opens at run time (see docs/linux.md). The program is given no DISPLAY, so it
# cannot fall back to X11: a pass means the Wayland path itself worked.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
repo=$(dirname "$here")
bin=$(cd "${1:?usage: linux_smoke_wayland.sh <binary folder> <output folder> [label]}" && pwd)
out=${2:?usage: linux_smoke_wayland.sh <binary folder> <output folder> [label]}
label=${3:-smoke-wayland}
mkdir -p "$out"
out=$(cd "$out" && pwd)

if ldd "$bin/opendraft" | grep -q "not found"; then
  echo "opendraft needs libraries this system does not have:"
  ldd "$bin/opendraft" | grep "not found"
  exit 1
fi

# The compositor's own runtime folder (the test's program gets another one).
runtime=$(mktemp -d)
chmod 700 "$runtime"
export XDG_RUNTIME_DIR=$runtime
unset DISPLAY

cat >"$runtime/sway.conf" <<'EOF'
output HEADLESS-1 mode 1600x1000
default_border none
default_floating_border none
font pango:monospace 8
EOF

export WLR_BACKENDS=headless WLR_LIBINPUT_NO_DEVICES=1 WLR_RENDERER=pixman
sway --config "$runtime/sway.conf" >"$out/$label-sway.log" 2>&1 &
sway_pid=$!
trap 'kill "$sway_pid" 2>/dev/null || true; rm -rf "$runtime"' EXIT

# Wait for the compositor to be ready and find its sockets.
for _ in $(seq 100); do
  sock=$(ls "$runtime"/sway-ipc.*.sock 2>/dev/null | head -1 || true)
  wl=$(ls "$runtime" 2>/dev/null | grep -E '^wayland-[0-9]+$' | head -1 || true)
  [ -n "$sock" ] && [ -n "$wl" ] && break
  sleep 0.2
done
if [ -z "${sock:-}" ] || [ -z "${wl:-}" ]; then
  echo "sway did not start:"
  cat "$out/$label-sway.log"
  exit 1
fi
export SWAYSOCK=$sock WAYLAND_DISPLAY=$wl
swaymsg -t get_outputs | head -c 400 || true

sample="$repo/crates/od-files/tests/fixtures/ezdxf_sample.dxf"
status=0
python3 "$here/linux_smoke.py" --session wayland --binary "$bin/opendraft" --mcp "$bin/opendraft-mcp" \
  --sample "$sample" --out "$out" --label "$label" --scenario drawing || status=1
python3 "$here/linux_smoke.py" --session wayland --binary "$bin/opendraft" --mcp "$bin/opendraft-mcp" \
  --sample "$sample" --out "$out" --label "$label-home" --scenario home || status=1
exit $status
