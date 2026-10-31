#!/bin/sh
# Starts OpenDraft inside its Flatpak sandbox.
#
# The automation socket (what an AI assistant connects to) is put in the one folder
# the sandbox shares with the host and with other sandboxes of this application:
# $XDG_RUNTIME_DIR/app/$FLATPAK_ID. Without it the socket would be invisible outside.
export OD_AUTOMATION_DIR="${XDG_RUNTIME_DIR}/app/${FLATPAK_ID}/automation"
exec /app/libexec/opendraft "$@"
