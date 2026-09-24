#!/usr/bin/env bash
# Temporary: drives the real app on a composited X11 desktop and on headless
# sway, records what happens, and saves screenshots. Delete once
# docs/verification.md records the result.
set -uo pipefail
out="${1:-desktop-run}"
mkdir -p "$out"
app="$(pwd)/src-tauri/target/debug/perch"
summary="$out/summary.txt"
pids=()
cleanup() { kill "${pids[@]}" 2>/dev/null; }
trap cleanup EXIT
note() { echo "$*" | tee -a "$summary"; }
wait_for() {
  for _ in $(seq 150); do
    if eval "$1" >/dev/null 2>&1; then return 0; fi
    sleep 0.1
  done
  return 1
}
last_diag() { grep '"scanMs"' "$1" | tail -1; }
# The root's child that contains a window: the WM frame, or the window itself.
toplevel() {
  local w=$1 parent
  # xwininfo with no usable id waits for a click, so never hand it one.
  while [ -n "$w" ] && [ $((w)) -ne 0 ] && [ $((w)) -ne $((root)) ]; do
    parent=$(xwininfo -tree -id "$w" 2>/dev/null | sed -n 's/.*Parent window id: \(0x[0-9a-f]*\).*/\1/p')
    [ -z "$parent" ] || [ $((parent)) -eq 0 ] || [ $((parent)) -eq $((root)) ] && break
    w=$parent
  done
  echo $((w))
}
under_pointer() { toplevel "$(xdotool getmouselocation --shell | sed -n 's/^WINDOW=//p')"; }

# ------------------------------------------------------------------ X11
note "== X11: Xvfb 1920x1080, openbox, picom (xrender)"
export DISPLAY=:92
unset WAYLAND_DISPLAY
export XDG_SESSION_TYPE=x11
Xvfb :92 -screen 0 1920x1080x24 -nolisten tcp & pids+=($!)
wait_for "xdpyinfo"
openbox & pids+=($!)
wait_for "xprop -root _NET_SUPPORTING_WM_CHECK | grep -q window"
picom --backend xrender --no-fading-openclose & pids+=($!)
root=$(xwininfo -root | sed -n 's/.*Window id: \(0x[0-9a-f]*\).*/\1/p')
xterm -name terma -bg '#fdf6e3' -fg black -geometry 110x32+200+320 & pids+=($!)
xterm -name termb -bg '#eee8d5' -fg black -geometry 80x24+1150+180 & pids+=($!)
wait_for "xdotool search --classname '^termb$'"
term_a="$(xdotool search --classname '^terma$' | head -1)"
term_b="$(xdotool search --classname '^termb$' | head -1)"

export HOME="$(mktemp -d)"
PERCH_DIAGNOSTICS=1 WEBKIT_DISABLE_DMABUF_RENDERER=1 "$app" >"$out/x11-app.log" 2>&1 &
perch=$!; pids+=($perch)
sleep 30
import -window root "$out/x11-1-settled.png"
note "diagnostics after 30 s: $(last_diag "$out/x11-app.log")"
overlay=""
for w in $(xdotool search --pid $perch 2>/dev/null); do
  state="$(xprop -id "$w" _NET_WM_STATE 2>/dev/null)"
  note "perch window $w:${state#*:}"
  case "$state" in *ABOVE*) overlay=$(toplevel "$w") ;; esac
done
note "overlay frame: $overlay; A frame: $(toplevel "$term_a"); B frame: $(toplevel "$term_b")"

# Click-through: with B focused, a click on A's body must reach A.
xdotool windowactivate --sync "$term_b"; sleep 0.5
xdotool mousemove 700 700; sleep 0.5
under="$(under_pointer)"
xdotool click 1; sleep 0.8
active="$(xdotool getactivewindow)"
note "click-through: pointer over frame $under, after the click active=$active (A=$term_a) -> $([ "$active" = "$term_a" ] && [ "$under" != "$overlay" ] && echo PASS || echo FAIL)"

# Hover: the pointer over a pet's opaque pixel must be caught by the overlay.
hover=FAIL
for attempt in $(seq 24); do
  point="$(last_diag "$out/x11-app.log" | sed -n 's/.*"grab":\[\[\([0-9.-]*\),\([0-9.-]*\)\].*/\1 \2/p')"
  [ -z "$point" ] && { sleep 2.5; continue; }
  set -- $point
  xdotool mousemove "${1%.*}" "${2%.*}"; sleep 0.6
  if [ "$(under_pointer)" = "$overlay" ]; then
    hover="PASS at $1,$2 on attempt $attempt"
    # Drag it: the release comes from the overlay's pointer capture.
    xdotool mousedown 1; sleep 0.3
    for step in $(seq 10); do xdotool mousemove_relative -- -30 30; sleep 0.05; done
    sleep 0.5
    import -window root "$out/x11-2-held.png"
    note "while held: $(sleep 5; last_diag "$out/x11-app.log" | grep -o '"grab":[^]]*]]')"
    xdotool mouseup 1; sleep 3
    import -window root "$out/x11-3-dropped.png"
    break
  fi
  sleep 2
done
note "hover over pet catches the pointer: $hover"
xdotool mousemove 20 1000; sleep 1
note "pointer moved off the pet: over frame $(under_pointer) -> $([ "$(under_pointer)" != "$overlay" ] && echo 'PASS (click-through again)' || echo FAIL)"

# Fullscreen suspension.
xdotool windowactivate --sync "$term_a"
wmctrl -i -r "$term_a" -b add,fullscreen; sleep 7
note "A fullscreen: $(last_diag "$out/x11-app.log" | grep -o '"suspended":[a-z]*')"
wmctrl -i -r "$term_a" -b remove,fullscreen; sleep 7
note "A windowed:   $(last_diag "$out/x11-app.log" | grep -o '"suspended":[a-z]*')"
import -window root "$out/x11-4-after.png"
note "resources (CI runner, not a budget measurement): $(ps -o rss=,%cpu= -p $perch | awk '{printf "RSS %d MB, lifetime CPU %s%%", $1/1024, $2}')"
kill $perch; sleep 1
cleanup; pids=()

# -------------------------------------------------------------- Wayland
note ""
note "== Wayland: headless sway 1920x1080 with XWayland"
runtime="$(mktemp -d)"; chmod 700 "$runtime"
before="$(ls /tmp/.X11-unix)"
printf 'xwayland force\noutput HEADLESS-1 resolution 1920x1080 bg #3a6ea5 solid_color\n' > "$runtime/sway.conf"
XDG_RUNTIME_DIR="$runtime" WLR_BACKENDS=headless WLR_RENDERER=pixman WLR_LIBINPUT_NO_DEVICES=1 \
  sway -c "$runtime/sway.conf" >"$out/sway.log" 2>&1 & pids+=($!)
wait_for "ls $runtime/wayland-* | grep -v lock"
wait_for "[ \"\$(ls /tmp/.X11-unix)\" != \"$before\" ]"
export XDG_RUNTIME_DIR="$runtime"
export WAYLAND_DISPLAY="$(basename "$(ls "$runtime"/wayland-* | grep -v lock | head -1)")"
export DISPLAY=":$(comm -13 <(echo "$before" | sort) <(ls /tmp/.X11-unix | sort) | head -1 | tr -d X)"
export XDG_SESSION_TYPE=wayland
unset GDK_BACKEND
foot --override=colors.background=fdf6e3 & pids+=($!)
export HOME="$(mktemp -d)"
PERCH_DIAGNOSTICS=1 WEBKIT_DISABLE_DMABUF_RENDERER=1 "$app" >"$out/wayland-app.log" 2>&1 &
perch=$!; pids+=($perch)
sleep 30
grim "$out/wayland-1-settled.png" 2>>"$out/sway.log"
note "diagnostics after 30 s: $(last_diag "$out/wayland-app.log")"
note "sway sees Perch as: $(swaymsg -s "$(ls "$runtime"/sway-ipc.*)" -t get_tree -r | python3 -c '
import json, sys
def walk(n):
    yield n
    for c in n.get("nodes", []) + n.get("floating_nodes", []):
        yield from walk(c)
for n in walk(json.load(sys.stdin)):
    if n.get("pid") == '"$perch"':
        print(n.get("shell"), n.get("type"), n["rect"], end="; ")
')"
kill $perch
