# Technical lessons

## White seams in the textarea selection highlight (WKWebView)

Status: fixed by painting the highlight ourselves (see "Solution" below). Rounding
`line-height` to whole pixels (9780197) did not fix it.

### What was established (September 2026, measured in the live app from window screenshots)

- Original symptom (Word Wrap on, `line-height: 1.6` = a fractional 30.4px at 19px): a
  white band under part of a line in 5 of 8 frames; after 9780197 (`line-height` in whole
  px) 0 of 8 in that scenario, with or without the `scrollTop` nudge on `selectionchange`.
- A case that reproduced every time: Word Wrap off, 120 lines of 400 characters, Menlo
  19px / `line-height: 30px`, everything selected, scrolled to the bottom — a 6pt band
  across the full width between the last two lines. Present before horizontal autoscroll
  too, so not a regression.
- The seams are tied to the content, not the screen: in content coordinates they end
  exactly on multiples of 512 (2560, 3072, 3584) — WebKit's tile boundaries — and grow
  from tile to tile (2 → 4 → 6pt). Partial seams (a band under only the left part of a
  line) are the original symptom.
- No effect: selecting after scrolling instead of before; nudging `scrollTop` after
  scrolling. So it is tile painting geometry, not stale invalidation.
- A full-document sweep (400px steps, each frame captured after painting, seams searched
  in 120px-wide slices): seams at almost any `line-height` — Menlo 19px at 28/29/30/34px
  gave 1–3 per document (31/32/33px gave 0 in that run), Menlo 13px (the default) at
  20/21/22/24px gave 2–4 — and with Word Wrap on as well (the first "wrap is fixed"
  conclusion came from a short document and was wrong).
- None of these help: `will-change: transform`, `transform: translateZ(0)`,
  `overflow-y: scroll`, `contain: paint`, `text-rendering: geometricPrecision`,
  `font-kerning: none`, the `wrap="off"` attribute instead of `white-space: pre`,
  `padding-top: 0`.
- Conclusion: a WKWebView defect in painting a textarea's native selection at tile
  boundaries; no CSS on the textarea fixes it.

### Solution (2026-09-28)

The native highlight is transparent (`#editor::selection { background: transparent }`,
selected text stays white); `#sel-layer` under the transparent textarea holds rectangles
drawn by `paintSelection()` in `src/app.js`. Geometry comes from a hidden mirror with the
same text and styles (tabs, wide glyphs and wrapping are measured by the engine). WebKit's
own rules, reproduced:
- a row the selection continues past (a newline or a soft wrap) is filled to the right
  edge of the text (`clientWidth - padding-right`, in content coordinates; with Word Wrap
  off further if the line is longer);
- rows stack without gaps, shifted up by half the leading; only the document's first row
  starts at the glyph top;
- edges on device pixels; the selection's own start and end one device pixel inside the
  glyphs; the layer colour `#5990fe` shows on screen as the native `#4d90fe` does (WebKit
  shifts the native one slightly);
- no highlight while the editor is unfocused (as natively); it stays blue in an
  inactive window.

Verified: pixel-for-pixel against the native highlight — 0 differing rows (Word Wrap
off/on, Menlo 19/30 and 13/21, horizontal scroll, selections starting on the first line
and mid-document); 0 seams in the sweep at any `line-height`; live drag, autoscroll on
both axes, double-click, Cmd+A, typing over a selection. Regression tests: `tests/ui`
(`highlight`, `seams`, and `control` to prove the detector sees native seams).

Performance: WebKit finds a position inside a text node by scanning it; on a 3.6 MB
document one coordinate query took 3.2 ms. The mirror is split into text nodes of 256
lines: a query takes 0.02 ms, a repaint < 1 ms. The text is copied into the mirror only
after it changes ("input" events plus an `editor.value` setter hook).

### How it was reproduced

Hooks run in a debug build (`LIKENOTEPAD_TEST_HOOK`, see `tests/ui/README.md`) fill the
editor, select all, set the wrap mode and report state through the window title; the
window alone is captured with `screencapture -l <windowID>` (it must be visible — WebKit
does not paint an occluded window); seams are rows that are almost free of highlight
colour between highlighted rows, searched in 120px slices (a single pixel column is not
enough: white glyphs give false positives).

## Horizontal drag-autoscroll

Works (70ef173). The key is calibrating against the click point: the focus column is the
boundary in a one-line mirror closest to `mirrorX(anchor) + (x − clickX)`, so the
textarea's inner text inset (absent from the mirror) cancels out. The anchor is the
native caret right after mousedown (`setTimeout 0`), not a computation from Y: inside
mousedown itself `selectionStart` is still stale. Verified with real CGEvent drags:
right, left, diagonal, vertical, tabs, a click past the end of a short line.

## Drag-and-drop of files onto a window (Tauri `WindowEvent::DragDrop`)

Works (dropped files open like files from Finder, in new windows). Automating it is not
reliable, so there is no UI test:
- Synthetic CGEvent drags do not start a drag in Finder at all (no events reach the app).
- A helper AppKit app that calls `beginDraggingSession` on mouse down delivered a real
  drop once (`Enter`/`Over`/`Drop` with the file path, new window opened), but in about ten
  further runs macOS refused the session (`error in CoreDragDispose: -1850`) regardless of
  activation order or `acceptsFirstMouse`.
- Checked by hand-driven runs instead; to see the events, log
  `WindowEvent::DragDrop` in `on_window_event` of a debug build.
