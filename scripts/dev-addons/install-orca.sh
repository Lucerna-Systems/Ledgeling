#!/bin/sh
# Orca is a desktop app whose `orca` command is installed from inside the app.
# This opens its official download page; nothing is downloaded by Ledgeling.
URL="https://github.com/stablyai/orca/releases/latest"
echo "Opening $URL"
echo "Install Orca, then turn on its command-line tool from Orca's settings."
if command -v open >/dev/null 2>&1; then open "$URL"; else xdg-open "$URL"; fi
