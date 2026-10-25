#!/usr/bin/env bash
# Run INSIDE a clean ubuntu:22.04 container (see .github/workflows/release.yml):
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

# And is it removed cleanly?
apt-get remove -y opendraft || apt-get remove -y "$(dpkg-deb -f "$deb" Package)"
test ! -e /usr/bin/opendraft
echo "the .deb installs, starts and removes cleanly"
