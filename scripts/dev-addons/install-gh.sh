#!/bin/sh
# Installs the GitHub CLI with this computer's package manager. Ledgeling runs
# this only when you press Install, in a terminal you can watch.
set -e
if command -v brew >/dev/null 2>&1; then
  brew install gh
elif command -v apt-get >/dev/null 2>&1; then
  sudo apt-get update && sudo apt-get install -y gh
elif command -v dnf >/dev/null 2>&1; then
  sudo dnf install -y gh
elif command -v pacman >/dev/null 2>&1; then
  sudo pacman -S --noconfirm github-cli
else
  echo "No supported package manager was found."
  echo "Download the GitHub CLI from https://cli.github.com/ and install it."
  exit 1
fi
echo
echo "Done. Press Sign in in the GitHub view to connect your account."
