#!/usr/bin/env bash
# Runs the Linux adapter's live tests (src-tauri/tests/linux_desktop.rs)
# against two real display servers: Xvfb with openbox, and headless sway with
# XWayland. Needs xvfb, openbox, x11-utils, sway and xwayland installed.
set -euo pipefail

manifest="$(dirname "$0")/../src-tauri/Cargo.toml"
runtime="$(mktemp -d)"
chmod 700 "$runtime"
pids=()
trap 'kill "${pids[@]}" 2>/dev/null || true; rm -rf "$runtime"' EXIT

wait_for() {
  for _ in $(seq 100); do
    if eval "$1" >/dev/null 2>&1; then return 0; fi
    sleep 0.1
  done
  echo "Timed out waiting for: $1" >&2
  return 1
}

live_test() {
  cargo test --manifest-path "$manifest" --test linux_desktop -- \
    --ignored --exact "$1" --nocapture --test-threads=1
}

echo "== X11: Xvfb + openbox"
Xvfb :91 -screen 0 1920x1080x24 -nolisten tcp &
pids+=($!)
wait_for "xdpyinfo -display :91"
DISPLAY=:91 openbox &
pids+=($!)
wait_for "xprop -display :91 -root _NET_SUPPORTING_WM_CHECK | grep -q window"
(
  unset WAYLAND_DISPLAY
  export DISPLAY=:91 XDG_SESSION_TYPE=x11
  live_test x11_adapter_sees_the_desktop_as_a_person_does
)

echo "== Wayland: headless sway + XWayland"
before="$(ls /tmp/.X11-unix 2>/dev/null || true)"
printf 'xwayland force\n' > "$runtime/sway.conf"
XDG_RUNTIME_DIR="$runtime" WLR_BACKENDS=headless WLR_RENDERER=pixman \
  WLR_LIBINPUT_NO_DEVICES=1 sway -c "$runtime/sway.conf" &
pids+=($!)
wait_for "ls $runtime/wayland-* | grep -v lock"
wayland="$(basename "$(ls "$runtime"/wayland-* | grep -v lock | head -1)")"
wait_for "[ \"\$(ls /tmp/.X11-unix)\" != \"$before\" ]"
xwayland=":$(comm -13 <(echo "$before" | sort) <(ls /tmp/.X11-unix | sort) | head -1 | tr -d X)"
(
  export XDG_RUNTIME_DIR="$runtime" WAYLAND_DISPLAY="$wayland" DISPLAY="$xwayland"
  export XDG_SESSION_TYPE=wayland
  live_test wayland_session_degrades_to_screen_edges
)
