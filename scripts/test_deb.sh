#!/usr/bin/env bash
# Run INSIDE a clean ubuntu:22.04 container (see .github/workflows/release.yml), with
# the repository mounted so the smoke test can run:
#   scripts/test_deb.sh /path/to/OpenDraft-<version>-linux-amd64.deb
# Installs the package with only what it declares (plus a virtual display and a
# software GL driver, which a desktop has), checks the files it should install,
# then starts OpenDraft and checks it stays open. This is what tells us the
# package's `Depends` line is enough on a bare system.
set -euxo pipefail

deb=${1:?usage: scripts/test_deb.sh <package.deb>}
export DEBIAN_FRONTEND=noninteractive

apt-get update
apt-get install -y --no-install-recommends "$deb" xvfb xauth libgl1-mesa-dri desktop-file-utils shared-mime-info

# What the package should have installed.
test -x /usr/bin/opendraft
test -x /usr/bin/opendraft-mcp
test -f /usr/share/applications/opendraft.desktop
test -f /usr/share/mime/packages/opendraft.xml
ls /usr/share/icons/hicolor/*/apps/opendraft.png
desktop-file-validate /usr/share/applications/opendraft.desktop
cat /usr/share/applications/opendraft.desktop

# The desktop knows what an .odx file is: the package's file lands in the type
# database that file managers read (the package manager runs this on install; it
# is run again here so the check does not depend on that).
update-mime-database /usr/share/mime
grep -F 'application/x-opendraft-drawing:*.odx' /usr/share/mime/globs2

# Every library the program is linked against must be found (the ones it opens at
# run time are covered by starting it below).
if ldd /usr/bin/opendraft | grep "not found"; then
  echo "opendraft is linked against a library the package does not bring"
  exit 1
fi

# Does it start, and keep running? `timeout` ends it with 124 if it is still open.
export LIBGL_ALWAYS_SOFTWARE=1 OPENDRAFT_PROFILE=ci-smoke
set +e
timeout 15 xvfb-run -a opendraft
code=$?
set -e
if [ "$code" != 124 ]; then
  echo "opendraft exited with code $code instead of staying open for 15 s"
  exit 1
fi

# Does the installed program work? The whole smoke test (scripts/linux_smoke.py)
# against what the package put in /usr/bin: opens a drawing, takes keyboard and
# mouse input, draws and undoes a line, runs opendraft-mcp --check and quits. It
# needs the repository's scripts and test drawing, mounted at /repo.
here=$(cd "$(dirname "$0")" && pwd)
if [ -f "$here/linux_smoke.sh" ] && [ -f "$here/../crates/od-files/tests/fixtures/ezdxf_sample.dxf" ]; then
  apt-get install -y --no-install-recommends xdotool python3 fonts-dejavu-core
  bash "$here/linux_smoke.sh" /usr/bin /tmp/smoke "" deb
else
  echo "the repository is not mounted: skipping the full smoke test"
fi

# And is it removed cleanly?
apt-get remove -y "$(dpkg-deb -f "$deb" Package)"
test ! -e /usr/bin/opendraft
echo "the .deb installs, starts and removes cleanly"
