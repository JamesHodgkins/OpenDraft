#!/usr/bin/env bash
# Build a release and assemble it as dist/OpenDraft-<version>-<os>/ plus an
# archive of it: the app, the MCP program that must sit next to it, the icon
# drawings it reads at start (icons/), the licence, the third-party licence
# notices (needs cargo-about, see scripts/third_party.sh) and the MCP guide. Works
# in Git Bash on Windows and on Linux.
set -euo pipefail
cd "$(dirname "$0")/.."

version=$(cargo pkgid -p od-app | sed 's/.*[#@]//')
case "$(uname -s)" in
  MINGW* | MSYS* | CYGWIN*) os=windows; ext=.exe ;;
  Linux) os=linux; ext= ;;
  *) echo "unsupported system: $(uname -s)" >&2; exit 1 ;;
esac
name="OpenDraft-${version}-${os}"

cargo build --release -p od-app -p od-mcp

rm -rf "dist/${name}"
mkdir -p "dist/${name}"
cp "target/release/opendraft${ext}" "target/release/opendraft-mcp${ext}" "dist/${name}/"
cp LICENSE "dist/${name}/LICENSE"
cp readme.md "dist/${name}/README.md"
cp docs/mcp.md "dist/${name}/MCP.md"
cp -r assets/icons "dist/${name}/icons"
bash scripts/third_party.sh "dist/${name}/THIRD-PARTY-LICENSES.txt"

rm -f "dist/${name}.zip" "dist/${name}.tar.gz"
if [ "$os" = windows ]; then
  # Windows' own bsdtar writes a proper zip (Compress-Archive in PowerShell 5
  # writes backslashes in the names, which other systems unpack wrongly).
  "${SYSTEMROOT:-/c/Windows}/System32/tar.exe" -C dist -a -c -f "$PWD/dist/${name}.zip" "${name}"
  echo "dist/${name}.zip"
else
  tar -C dist -czf "dist/${name}.tar.gz" "${name}"
  echo "dist/${name}.tar.gz"
fi
