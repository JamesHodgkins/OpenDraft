#!/usr/bin/env bash
# Run OpenDraft in a virtual X display and drive it (scripts/linux_smoke.py).
#
#   scripts/linux_smoke.sh <folder with opendraft and opendraft-mcp> <output folder> [scale] [label]
#
# Two runs of the program: one that opens a drawing and draws on it, and (at the
# normal scale) one that starts with nothing open and quits. Needs Xvfb, xdotool and python3 on the machine, and the libraries OpenDraft
# opens at run time (see docs/linux.md). `scale` sets the display scale factor
# (WINIT_X11_SCALE_FACTOR), to see the program at 1.5 or 2 times.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
repo=$(dirname "$here")
bin=$(cd "${1:?usage: linux_smoke.sh <binary folder> <output folder> [scale] [label]}" && pwd)
out=${2:?usage: linux_smoke.sh <binary folder> <output folder> [scale] [label]}
scale=${3:-}
label=${4:-smoke${scale:+-x$scale}}
mkdir -p "$out"
out=$(cd "$out" && pwd)

# A library the program is linked against that this system lacks is the
# commonest reason it will not start: say so plainly instead of failing later.
if ldd "$bin/opendraft" | grep -q "not found"; then
  echo "opendraft needs libraries this system does not have:"
  ldd "$bin/opendraft" | grep "not found"
  exit 1
fi

fb=$(mktemp -d)
display=:99
Xvfb "$display" -screen 0 1600x1000x24 -fbdir "$fb" -nolisten tcp >"$out/$label-xvfb.log" 2>&1 &
xvfb=$!
trap 'kill "$xvfb" 2>/dev/null || true; rm -rf "$fb"' EXIT
for _ in $(seq 100); do
  [ -S "/tmp/.X11-unix/X${display#:}" ] && break
  sleep 0.1
done
export DISPLAY=$display

args=(--binary "$bin/opendraft" --mcp "$bin/opendraft-mcp"
      --sample "$repo/crates/od-files/tests/fixtures/ezdxf_sample.dxf"
      --out "$out" --fbdir "$fb" --label "$label")
[ -n "$scale" ] && args+=(--scale "$scale")

status=0
python3 "$here/linux_smoke.py" "${args[@]}" --scenario drawing || status=1
# Starting with no drawing and quitting needs no repeat at every scale.
if [ -z "$scale" ]; then
  home_args=(--binary "$bin/opendraft" --mcp "$bin/opendraft-mcp"
             --sample "$repo/crates/od-files/tests/fixtures/ezdxf_sample.dxf"
             --out "$out" --fbdir "$fb" --label "$label-home")
  python3 "$here/linux_smoke.py" "${home_args[@]}" --scenario home || status=1
fi
exit $status
