# UI tests

End-to-end checks of the parts of the editor that only show on screen: the painted
selection highlight and drag-autoscroll. They run the real app (a debug build), drive it
with real mouse events and compare window screenshots.

```sh
tests/ui/run.sh                 # highlight, seams, autoscroll, perf
tests/ui/run.sh seams control   # any subset; `control` is not run by default
```

Each case prints `PASS`/`FAIL`; the exit code is non-zero if anything failed.
Screenshots of the last run are kept in `tests/ui/.build/shots/`.

## Before running

- **Permissions.** The terminal (or app) that runs the script needs *Screen Recording*
  and *Accessibility* in System Settings › Privacy & Security. The script checks and
  says which one is missing.
- **Hands off.** The test window comes to the front and the mouse pointer moves by
  itself for a few seconds per case. Mouse input is only sent while the test window is
  in front; otherwise the case fails instead of clicking somewhere else.
- Takes about 4–5 minutes in total. Uses Swift (`swiftc`, from Xcode or the Command
  Line Tools) and cargo.

## Cases

| Case | What it checks |
|---|---|
| `highlight` | The selection we paint (`paintSelection()` in `src/app.js`) matches WebKit's native highlight row by row: Word Wrap off, on, and scrolled sideways. |
| `seams` | No unpainted seams anywhere in fully selected documents while scrolling them top to bottom (Menlo 13/21, 19/30, and with Word Wrap). |
| `autoscroll` | Dragging a selection past the right and bottom edges scrolls the editor and extends the selection from the click point. |
| `perf` | Repainting the selection on a 3.6 MB document takes ≤ 3 ms (median). |
| `control` | The same seam sweep with WebKit's native highlight *does* find seams — proof the detector is not blind. |

Background for `highlight`, `seams` and `control`: `technical-lessons.md`.

## How it works

- `run.sh` builds the debug app and the tools in `tools/`, then starts the app with
  `LIKENOTEPAD_TEST_HOOK` naming a script made of the case's parameters,
  `hooks/lib.js` and one of `hooks/*.js`.
- Debug builds only (`with_test_hook` in `src-tauri/src/lib.rs`): that script runs before
  the page's own, with `window.__LIKENOTEPAD_TEST__` set, so `app.js` exposes
  `window.__likeNotepad` (`applyWordWrap`, `paintSelection`). Release builds ignore the
  variable.
- A hook fills the editor, sets the scene and reports state through the window title;
  the runner reads the title (`tools/window.swift`), sends mouse drags
  (`tools/drag.swift`) and analyses screenshots (`tools/seams.swift`,
  `tools/highlight-diff.swift`).
- The app runs with your normal settings file, but the hooks only change the editor on
  screen (text, font, wrap) and never save settings or files.
