#!/bin/sh
# The MCP program of the Flatpak: run it with
#   flatpak run --command=opendraft-mcp uk.openengineering.opendraft
# It looks for the running program in the folder opendraft.sh made for it.
export OD_AUTOMATION_DIR="${XDG_RUNTIME_DIR}/app/${FLATPAK_ID}/automation"
exec /app/libexec/opendraft-mcp "$@"
