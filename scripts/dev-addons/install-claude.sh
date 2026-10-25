#!/bin/sh
# Installs Claude Code with Anthropic's official installer. Ledgeling runs this
# only when you press Install, in a terminal you can watch.
set -e
echo "Installing Claude Code from https://claude.ai/install.sh"
curl -fsSL https://claude.ai/install.sh | bash
echo
echo "Done. Run 'claude' once in a terminal tab to sign in."
