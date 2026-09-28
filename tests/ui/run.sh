#!/bin/zsh
# UI tests for LikeNotepad.exe — see tests/ui/README.md.
# Usage: tests/ui/run.sh [highlight] [seams] [autoscroll] [perf] [control]
#   no arguments: all but control
set -u

ROOT=${0:A:h:h:h}
UI=$ROOT/tests/ui
BUILD=$UI/.build
APP=$ROOT/src-tauri/target/debug/notepad-mac
mkdir -p $BUILD/shots

for t in window drag seams highlight-diff permissions; do
  if [[ ! $BUILD/$t -nt $UI/tools/$t.swift ]]; then
    swiftc -O $UI/tools/$t.swift -o $BUILD/$t || exit 1
  fi
done
$BUILD/permissions || exit 1
echo "building the debug app…"
cargo build --quiet --manifest-path $ROOT/src-tauri/Cargo.toml || exit 1

PID=""
WIN=""
FAILED=0

pass() { print -P "%F{green}PASS%f $1  ($2)"; }
fail() { print -P "%F{red}FAIL%f $1  ($2)"; FAILED=1; }

stop() {
  [[ -n $PID ]] && kill $PID 2>/dev/null && wait $PID 2>/dev/null
  PID=""
}
trap stop EXIT INT TERM

# launch <hook> <params as a JS object literal>
launch() {
  { print -r -- "window.__uiParams = $2;"; cat $UI/hooks/lib.js $UI/hooks/$1.js } > $BUILD/hook.js
  LIKENOTEPAD_TEST_HOOK=$BUILD/hook.js $APP >/dev/null 2>&1 &
  PID=$!
  WIN=""
  for i in {1..100}; do
    WIN=$($BUILD/window $PID | cut -d'|' -f1)
    [[ -n $WIN ]] && break
    sleep 0.2
  done
  if [[ -z $WIN ]]; then echo "the app window did not appear"; stop; return 1; fi
  osascript -e "tell application \"System Events\" to set frontmost of (first process whose unix id is $PID) to true"
}

title() { $BUILD/window $PID | cut -d'|' -f2-; }

# wait_title <pattern> [seconds]
wait_title() {
  local T
  for i in {1..$(( ${2:-20} * 10 ))}; do
    T=$(title)
    [[ $T =~ $1 ]] && return 0
    sleep 0.1
  done
  return 1
}

# Mouse input goes to whatever is in front: never send it anywhere but the test window.
frontmost() {
  [[ "$(osascript -e 'tell application "System Events" to get unix id of first process whose frontmost is true')" == "$PID" ]]
}

shot() { screencapture -x -o -l $WIN $1; }

# ---- highlight: our painted selection matches WebKit's native one pixel for pixel ----
case_highlight() {
  local name params
  for name params in \
    nowrap "{wrap: false}" \
    wrap "{wrap: true}" \
    hscroll "{wrap: false, scrollLeft: 300}"; do
    for mode in native custom; do
      local p=${params%\}}
      [[ $mode == native ]] && p="$p, native: true}" || p="$p}"
      launch highlight "$p" || { fail "highlight/$name" "launch"; return; }
      wait_title READY || { fail "highlight/$name" "no READY"; stop; return; }
      sleep 0.3
      shot $BUILD/shots/highlight-$name-$mode.png
      stop
    done
    local r=(${=$($BUILD/highlight-diff $BUILD/shots/highlight-$name-native.png $BUILD/shots/highlight-$name-custom.png)})
    # r: rows in native, rows in ours, rows that differ, max edge difference (px)
    if (( r[1] > 100 && r[1] == r[2] && r[3] <= 5 )); then
      pass "highlight/$name" "${r[1]} rows, ${r[3]} differ"
    else
      fail "highlight/$name" "native ${r[1]} rows, ours ${r[2]}, ${r[3]} differ, max ${r[4]}px"
    fi
  done
}

# sweep <params>: scrolls a fully selected document, sets STEPS and SEAMS
sweep() {
  local last="" T f
  STEPS=0 SEAMS=0
  rm -f $BUILD/shots/sweep-*.png
  launch sweep "$1" || return 1
  # Only capture while scrolling (analysis is slow and would miss steps), analyse after.
  for i in {1..600}; do
    T=$(title)
    [[ $T == DONE ]] && break
    if [[ $T == STEP* && $T != $last ]]; then
      last=$T
      shot $BUILD/shots/sweep-${T#STEP }.png
    fi
    sleep 0.05
  done
  stop
  for f in $BUILD/shots/sweep-*.png(N); do
    (( STEPS++, SEAMS += $($BUILD/seams $f) ))
  done
  return 0
}

# ---- seams: no unpainted seams anywhere in fully selected documents ----
case_seams() {
  local name params
  for name params in \
    menlo13 "{lines: 300, fontSize: 13, lineHeight: 21}" \
    menlo19 "{lines: 200, fontSize: 19, lineHeight: 30}" \
    wrap19 "{lines: 40, fontSize: 19, lineHeight: 30, wrap: true}"; do
    sweep "$params" || { fail "seams/$name" "launch"; return; }
    if (( STEPS >= 3 && SEAMS == 0 )); then pass "seams/$name" "$STEPS screens"; else fail "seams/$name" "$SEAMS seams in $STEPS screens"; fi
  done
}

# ---- control (not run by default): with WebKit's native highlight the same documents DO
# show seams — proof that the seam detector can see them at all ----
case_control() {
  sweep "{lines: 300, fontSize: 13, lineHeight: 21, native: true}" || { fail control "launch"; return; }
  local a=$SEAMS
  sweep "{lines: 200, fontSize: 19, lineHeight: 30, native: true}" || { fail control "launch"; return; }
  if (( a + SEAMS > 0 )); then pass control "native highlight: $(( a + SEAMS )) seams found"; else fail control "no seams found in the native highlight: the detector may be blind"; fi
}

# ---- autoscroll: dragging a selection past the window edge scrolls and extends it ----
case_autoscroll() {
  local T
  launch autoscroll "{}" || { fail autoscroll "launch"; return; }
  wait_title "sl=" || { fail autoscroll "no state"; stop; return; }
  frontmost || { fail autoscroll "test window not in front"; stop; return; }
  $BUILD/drag $WIN 200 100 830 100 2.5
  T=$(title)
  # sel=2:16-2:N — anchored where the click was, extended far to the right
  if [[ $T =~ 'sl=([0-9]+) .* sel=2:16-2:([0-9]+)' ]] && (( match[1] >= 1000 && match[2] >= 150 )); then
    pass autoscroll/right "$T"
  else
    fail autoscroll/right "$T"
  fi
  stop

  launch autoscroll "{}" || { fail autoscroll "launch"; return; }
  wait_title "sl=" || { fail autoscroll "no state"; stop; return; }
  frontmost || { fail autoscroll "test window not in front"; stop; return; }
  $BUILD/drag $WIN 200 145 300 650 2.5
  T=$(title)
  if [[ $T =~ 'st=([0-9]+) sel=3:16-([0-9]+):' ]] && (( match[1] >= 1000 && match[2] >= 40 )); then
    pass autoscroll/down "$T"
  else
    fail autoscroll/down "$T"
  fi
  stop
}

# ---- perf: repainting the selection stays cheap on a large document ----
case_perf() {
  launch perf "{}" || { fail perf "launch"; return; }
  if wait_title PERF 60 && [[ $(title) =~ 'median=([0-9.]+)' ]] && (( match[1] <= 3 )); then
    pass perf "$(title)"
  else
    fail perf "$(title)"
  fi
  stop
}

cases=($@)
(( $#cases )) || cases=(highlight seams autoscroll perf)
for c in $cases; do case_$c; done
exit $FAILED
