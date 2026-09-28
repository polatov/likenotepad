// Tauri v2 exposes globals via window.__TAURI__
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const { ask } = window.__TAURI__.dialog;
const { getCurrentWindow } = window.__TAURI__.window;

const editor = document.getElementById("editor");
const statLines = document.getElementById("stat-lines");
const statWords = document.getElementById("stat-words");
const statChars = document.getElementById("stat-chars");
const statPos = document.getElementById("stat-pos");

const findbar = document.getElementById("findbar");
const ctxMenu = document.getElementById("ctx-menu");
const findInput = document.getElementById("find-input");
const findCount = document.getElementById("find-count");
const findClose = document.getElementById("find-close");
const gotobar = document.getElementById("gotobar");
const gotoInput = document.getElementById("goto-input");

// The dictionary lives in i18n.js (shared by all windows) and is loaded in index.html
// as a plain <script> BEFORE this module, so window.I18N is ready at startup.
const lang = window.I18N.lang;
let findbarNavMode = false;
let windowShown = false;
let isHandlingClose = false;
let isHandlingQuit = false;

const t = (key, ...args) => window.I18N.t(key, ...args);

// Translate static panel text (data-i18n attributes in index.html)
window.I18N.localizeDOM();

// --- State ---
let currentPath = null;
let dirty = false;
let untitledNum = 0;
// Path the window's title-bar proxy icon currently points to (null = none).
let representedPath = null;

function updateTitle() {
  const name = displayName();
  const title = `${name} — LikeNotepad.exe`;
  document.title = title;
  getCurrentWindow().setTitle(title);
  try { invoke("set_tab_title", { title: name }); } catch (e) {}
  if (currentPath !== representedPath) {
    representedPath = currentPath;
    try { invoke("set_represented_file", { path: currentPath }); } catch (e) {}
  }
}

async function releaseUntitled() {
  if (untitledNum === 0) return;
  untitledNum = 0;
  try { await invoke("release_untitled_number"); } catch (e) {}
}

let autoNameEnabled = false;

function autoNameRaw() {
  if (currentPath) return null;
  if (!autoNameEnabled) return null;
  const first = (editor.value.split("\n")[0] || "").replace(/\r$/, "").trim();
  return first || null;
}

function truncateName(s, max) {
  return s.length > max ? s.slice(0, max) + "…" : s;
}

function displayName() {
  if (currentPath) return currentPath.split("/").pop();
  const auto = autoNameRaw();
  if (auto) return truncateName(auto, 40);
  const base = t("unsaved");
  return untitledNum > 1 ? `${base} ${untitledNum}` : base;
}

function markDirty() { dirty = true; updateTitle(); syncDirty(); }
function markClean() { dirty = false; updateTitle(); syncDirty(); }

function dateStamp() {
  const now = new Date();
  const locale = window.I18N.locale;
  return now.toLocaleString(locale, {
    day: "2-digit", month: "2-digit", year: "numeric",
    hour: "2-digit", minute: "2-digit"
  });
}

// The .LOG easter egg: if the first line is exactly ".LOG", append a timestamp
function applyLogStamp() {
  const firstLine = editor.value.split("\n")[0].replace(/\r$/, "");
  if (firstLine !== ".LOG") return;
  editor.focus();
  const end = editor.value.length;
  editor.setSelectionRange(end, end);
  document.execCommand("insertText", false, "\n" + dateStamp());
  markDirty();
  updateStatus();
  updatePos();
}

function syncDirty() {
  try { invoke("set_dirty", { dirty }); } catch (e) {}
}

// --- Status bar ---
function updateStatus() {
  const text = editor.value;
  const chars = [...text].length;
  const words = text.split(/\s+/).filter(Boolean).length;
  const lines = text.split("\n").length;
  statLines.textContent = t("lines", lines);
  statWords.textContent = t("words", words);
  statChars.textContent = t("chars", chars);
}

function updatePos() {
  const pos = editor.selectionStart;
  const before = editor.value.slice(0, pos);
  const line = before.split("\n").length;
  const col = pos - before.lastIndexOf("\n");
  statPos.textContent = t("lnCol", line, col);
}

// --- File operations ---
function loadFileIntoEditor(path, content) {
  editor.value = content;
  currentPath = path;
  releaseUntitled();
  markClean();
  applyLogStamp();
  updateStatus();
}

async function openFile() {
  if (dirty) {
    const ok = await ask(t("confirmNew"), { title: t("confirmNewTitle"), okLabel: t("yes"), cancelLabel: t("no"), kind: "warning" });
    if (!ok) return;
  }
  const result = await invoke("open_file");
  if (!result) return;
  const [path, content] = result;
  loadFileIntoEditor(path, content);
}

async function saveFile() {
  if (!currentPath) return saveFileAs();
  await invoke("save_file", { path: currentPath, content: editor.value });
  markClean();
  return true;
}

async function saveFileAs() {
  let suggestedName;
  if (currentPath) {
    suggestedName = currentPath.split("/").pop();
  } else {
    const auto = autoNameRaw();
    suggestedName = (auto ? auto.replace(/[\/\\]/g, "-") : displayName()) + ".txt";
  }
  const result = await invoke("save_file_as", { content: editor.value, suggestedName });
  if (!result) return false;
  currentPath = result;
  await releaseUntitled();
  markClean();
  return true;
}

// --- Keyboard shortcuts ---
document.addEventListener("keydown", (e) => {
  if ((e.metaKey || e.ctrlKey) && !e.altKey) {
    if (e.key === "o" || e.key === "O") { e.preventDefault(); invoke("open_file_new_window"); }
    else if (e.key === "s" || e.key === "S") {
      e.preventDefault();
      if (e.shiftKey) saveFileAs(); else saveFile();
    }
    else if (e.shiftKey && (e.key === "g" || e.key === "G")) {
      e.preventDefault();
      goToMatch(-1);
    }
  }
  else if (e.key === "Escape" && !findbar.hidden) {
    e.preventDefault();
    closeFind();
  }
  else if (e.key === "Escape" && !ctxMenu.hidden) {
    e.preventDefault();
    hideCtxMenu();
  }
});

// --- Tab key ---
editor.addEventListener("keydown", (e) => {
  if (e.key === "Tab") {
    e.preventDefault();
    const s = editor.selectionStart, end = editor.selectionEnd;
    editor.value = editor.value.slice(0, s) + "\t" + editor.value.slice(end);
    editor.selectionStart = editor.selectionEnd = s + 1;
  }
});

// --- Find highlight ---
let replaceMode = false;
let matchCase = false;
let wordWrap = false;

function applyWordWrap(enabled) {
  wordWrap = enabled;
  editor.style.whiteSpace = enabled ? "pre-wrap" : "pre";
  editor.style.overflowX = enabled ? "hidden" : "auto";
}

function applyStatusBar(visible) {
  document.getElementById("statusbar").style.display = visible ? "flex" : "none";
}

function applyShowCounter(visible) {
  document.getElementById("stat-right").style.display = visible ? "flex" : "none";
}

// --- Find ---
function openFind(mode = "find") {
  replaceMode = (mode === "replace");
  findbar.hidden = false;
  document.getElementById("findbar-row2").hidden = !replaceMode;
  const selected = editor.value.slice(editor.selectionStart, editor.selectionEnd);
  if (selected) {
    findInput.value = selected;
  }
  findInput.focus();
  findInput.select();
}

function closeFind() {
  findbar.hidden = true;
  findCount.textContent = "";
  replaceMode = false;
  document.getElementById("findbar-row2").hidden = true;
  editor.focus();
}

function openGoto() {
  gotobar.hidden = false;
  gotoInput.value = "";
  gotoInput.focus();
}

function closeGoto() {
  gotobar.hidden = true;
  editor.focus();
}

function doGoto() {
  const n = parseInt(gotoInput.value, 10);
  if (isNaN(n) || n < 1) return;
  const lines = editor.value.split("\n");
  const target = Math.min(n, lines.length);
  let pos = 0;
  for (let i = 0; i < target - 1; i++) pos += lines[i].length + 1;
  editor.focus();
  editor.setSelectionRange(pos, pos);
  const style = getComputedStyle(editor);
  const lineHeight = parseFloat(style.lineHeight) || parseFloat(style.fontSize) * 1.5;
  const paddingTop = parseFloat(style.paddingTop) || 0;
  const targetScrollTop = paddingTop + (target - 1) * lineHeight - editor.clientHeight / 2;
  editor.scrollTop = Math.max(0, targetScrollTop);
  closeGoto();
  updatePos();
}

function findAllMatches() {
  const term = findInput.value;
  if (!term) return [];

  const text = editor.value;
  const haystack = matchCase ? text : text.toLowerCase();
  const needle = matchCase ? term : term.toLowerCase();

  const matches = [];
  let pos = 0;
  let idx;
  while ((idx = haystack.indexOf(needle, pos)) !== -1) {
    matches.push(idx);
    pos = idx + needle.length;
  }
  return matches;
}

function goToMatch(direction) {
  const term = findInput.value;
  const matches = findAllMatches();
  if (matches.length === 0) {
    findCount.textContent = "";
    return;
  }

  // Reference point: the end of the previous selection (if it equals a match, current is that match's index)
  // For "next", search strictly AFTER the selection end so we do not get stuck on the same match
  const anchorNext = editor.selectionEnd;
  const anchorPrev = editor.selectionStart;

  let pos;
  if (direction === 1) {
    pos = matches.findIndex((m) => m >= anchorNext);
    if (pos === -1) pos = 0; // wrap to start
  } else {
    pos = -1;
    for (let i = matches.length - 1; i >= 0; i--) {
      if (matches[i] + term.length <= anchorPrev) { pos = i; break; }
    }
    if (pos === -1) pos = matches.length - 1; // wrap to end
  }
  const idx = matches[pos];

  // Select the match and scroll to it, then return focus to the search field.
  // Scroll while the editor has focus (WebKit ignores scrollTop on setSelectionRange for an unfocused textarea).
  editor.focus();
  editor.setSelectionRange(idx, idx + term.length);
  scrollEditorToSelection(idx);

  findCount.textContent = t("findPos", pos + 1, matches.length);
}

// Scrolls the textarea to an offset using a mirror div with the same font/wrap.
// Shared builder for a mirror div that copies the editor's computed styles.
function buildEditorMirror() {
  const ta = editor;
  const cs = window.getComputedStyle(ta);
  const mirror = document.createElement("div");
  mirror.style.position = "absolute";
  mirror.style.visibility = "hidden";
  mirror.style.whiteSpace = cs.whiteSpace;
  mirror.style.wordWrap = cs.wordWrap;
  mirror.style.overflowWrap = cs.overflowWrap;
  mirror.style.font = cs.font;
  mirror.style.lineHeight = cs.lineHeight;
  mirror.style.padding = cs.padding;
  mirror.style.border = cs.border;
  mirror.style.boxSizing = cs.boxSizing;
  mirror.style.width = ta.clientWidth + "px";
  mirror.style.top = "0";
  mirror.style.left = "0";
  document.body.appendChild(mirror);
  return mirror;
}

function scrollEditorToSelection(offset) {
  const ta = editor;
  const cs = window.getComputedStyle(ta);
  const mirror = buildEditorMirror();

  const before = ta.value.substring(0, offset);
  mirror.textContent = before;
  const marker = document.createElement("span");
  marker.textContent = "​";
  mirror.appendChild(marker);

  const markerTop = marker.offsetTop;
  document.body.removeChild(mirror);

  const lineH = parseFloat(cs.lineHeight) || parseFloat(cs.fontSize) * 1.2;
  const visibleTop = ta.scrollTop;
  const visibleBottom = visibleTop + ta.clientHeight;

  if (markerTop < visibleTop || markerTop + lineH > visibleBottom) {
    ta.scrollTop = Math.max(0, markerTop - ta.clientHeight / 3);
  }
}

// Reverse mapping Y → character offset (end of the logical line at that Y).
// targetY is in content coordinates (scrollTop is applied by the caller).
function offsetAtContentY(targetY) {
  const ta = editor;
  const val = ta.value;
  const mirror = buildEditorMirror();
  const marker = document.createElement("span");
  marker.textContent = "\u200b";
  let bestOffset = 0;
  let lineStart = 0;
  while (true) {
    mirror.textContent = val.substring(0, lineStart);
    mirror.appendChild(marker);
    const top = marker.offsetTop;
    if (top <= targetY) {
      const nl = val.indexOf("\n", lineStart);
      bestOffset = nl === -1 ? val.length : nl;
    } else {
      break;
    }
    const nl = val.indexOf("\n", lineStart);
    if (nl === -1) break;
    lineStart = nl + 1;
  }
  document.body.removeChild(mirror);
  return bestOffset;
}

// ---- Drag-autoscroll on both axes: WKWebView does not autoscroll a textarea
// when a selection is dragged past its edge, so we drive scroll + selection ourselves. ----
let dragAnchor = null;
let dragActive = false;
let dragPointerX = 0;
let dragPointerY = 0;
let dragRaf = null;
// Click reference point for the horizontal axis (Word Wrap OFF): the click X in content
// coordinates and the anchor X in the mirror. The focus column is computed relative to it,
// not to the line/document start, so the textarea's inner text inset (absent from
// the mirror) cancels out.
let dragRef = null;
let dragClick = null;
let dragLineStarts = null;
let dragMeasure = null;

function autoscrollSpeed(out) {
  return Math.min(40, 2 + Math.abs(out) * 0.35);
}

function lineStartsOf(val) {
  const starts = [0];
  for (let i = val.indexOf("\n"); i !== -1; i = val.indexOf("\n", i + 1)) starts.push(i + 1);
  return starts;
}

// One-line mirror for measuring the X of a character boundary (tabs and wide glyphs
// are handled by the engine; we do not assume a monospace grid).
function makeLineMeasure() {
  const cs = window.getComputedStyle(editor);
  const el = document.createElement("span");
  el.style.position = "absolute";
  el.style.visibility = "hidden";
  el.style.whiteSpace = "pre";
  el.style.font = cs.font;
  el.style.letterSpacing = cs.letterSpacing;
  el.style.tabSize = cs.tabSize;
  el.style.top = "0";
  el.style.left = "0";
  document.body.appendChild(el);
  const node = document.createTextNode("");
  el.appendChild(node);
  const range = document.createRange();
  let current = null;
  return {
    xAt(text, col) {
      if (text !== current) { node.data = text; current = text; }
      range.setStart(node, col);
      range.setEnd(node, col);
      return range.getBoundingClientRect().left - el.getBoundingClientRect().left;
    },
    // The character boundary in the line closest to x.
    colAt(text, x) {
      let lo = 0, hi = text.length;
      while (lo < hi) {
        const mid = (lo + hi) >> 1;
        if (this.xAt(text, mid + 1) <= x) lo = mid + 1; else hi = mid;
      }
      if (lo < text.length) {
        const left = this.xAt(text, lo), right = this.xAt(text, lo + 1);
        if (x - left > right - x) lo += 1;
      }
      return lo;
    },
    destroy() { el.remove(); },
  };
}

function lineText(val, starts, idx) {
  const s = starts[idx];
  const e = idx + 1 < starts.length ? starts[idx + 1] - 1 : val.length;
  return val.substring(s, e);
}

// Autoscroll step with Word Wrap OFF: scroll on both axes and put the selection focus
// at the line/column under the pointer (clamped to the visible area).
function dragAutoscrollStepNoWrap(rect) {
  const ta = editor;
  const x = dragPointerX, y = dragPointerY;
  let outX = 0, outY = 0;
  if (x < rect.left) outX = x - rect.left;
  else if (x > rect.right) outX = x - rect.right;
  if (y < rect.top) outY = y - rect.top;
  else if (y > rect.bottom) outY = y - rect.bottom;
  if (outX === 0 && outY === 0) return;
  if (dragAnchor === null || dragClick === null) return;
  if (dragRef === null) computeDragRef();

  if (outX !== 0) ta.scrollLeft = Math.max(0, ta.scrollLeft + Math.sign(outX) * autoscrollSpeed(outX));
  if (outY !== 0) ta.scrollTop = Math.max(0, ta.scrollTop + Math.sign(outY) * autoscrollSpeed(outY));

  const cs = getComputedStyle(ta);
  const padT = parseFloat(cs.paddingTop) || 0;
  const lineH = parseFloat(cs.lineHeight) || parseFloat(cs.fontSize) * 1.2;
  const val = ta.value;
  if (!dragLineStarts) dragLineStarts = lineStartsOf(val);
  if (!dragMeasure) dragMeasure = makeLineMeasure();

  // The pointer clamped to the textarea's visible area, in content coordinates.
  const cx = Math.min(Math.max(x, rect.left), rect.left + ta.clientWidth - 1) - rect.left + ta.scrollLeft;
  const cy = Math.min(Math.max(y, rect.top), rect.top + ta.clientHeight - 1) - rect.top + ta.scrollTop;

  const lineIdx = Math.min(dragLineStarts.length - 1, Math.max(0, Math.floor((cy - padT) / lineH)));
  const text = lineText(val, dragLineStarts, lineIdx);
  const mx = dragRef.mirrorX + (cx - dragRef.contentX);
  const off = dragLineStarts[lineIdx] + dragMeasure.colAt(text, mx);

  if (off >= dragAnchor) ta.setSelectionRange(dragAnchor, off, "forward");
  else ta.setSelectionRange(off, dragAnchor, "backward");
  updatePos();
}

// The anchor comes from the native selection after WebKit has handled
// mousedown (inside mousedown itself selectionStart is still stale).
function captureDragAnchor(clientX, shiftKey) {
  const ta = editor;
  let anchor = ta.selectionStart;
  if (shiftKey && ta.selectionDirection === "backward") anchor = ta.selectionEnd;
  dragAnchor = anchor;
  dragClick = { contentX: clientX - ta.getBoundingClientRect().left + ta.scrollLeft, shiftKey };
}

// The reference point is computed lazily, on the first edge crossing,
// so a plain click in a large file does not pay for splitting lines.
function computeDragRef() {
  const ta = editor;
  const anchor = dragAnchor;
  const { contentX, shiftKey } = dragClick;
  const val = ta.value;
  dragLineStarts = lineStartsOf(val);
  if (!dragMeasure) dragMeasure = makeLineMeasure();
  let idx = 0, lo = 0, hi = dragLineStarts.length - 1;
  while (lo <= hi) { const mid = (lo + hi) >> 1; if (dragLineStarts[mid] <= anchor) { idx = mid; lo = mid + 1; } else hi = mid - 1; }
  const text = lineText(val, dragLineStarts, idx);
  const mirrorX = dragMeasure.xAt(text, anchor - dragLineStarts[idx]);
  const padL = parseFloat(getComputedStyle(ta).paddingLeft) || 0;
  // Click past the end of the line (or shift-click): the click point is not the anchor,
  // so calibrate on the padding only.
  const charW = dragMeasure.xAt("0", 1);
  const calibrated = !shiftKey && Math.abs(contentX - padL - mirrorX) <= charW;
  dragRef = { contentX: calibrated ? contentX : mirrorX + padL, mirrorX };
}

function dragAutoscrollStep() {
  if (!dragActive) { dragRaf = null; return; }
  const ta = editor;
  const rect = ta.getBoundingClientRect();
  if (!wordWrap) {
    dragAutoscrollStepNoWrap(rect);
    dragRaf = requestAnimationFrame(dragAutoscrollStep);
    return;
  }
  const y = dragPointerY;

  let outY = 0;
  if (y < rect.top) outY = y - rect.top;
  else if (y > rect.bottom) outY = y - rect.bottom;

  if (outY !== 0) {
    const speedY = Math.min(40, 2 + Math.abs(outY) * 0.35);
    ta.scrollTop = Math.max(0, ta.scrollTop + (outY < 0 ? -1 : 1) * speedY);

    const cssPadT = parseFloat(getComputedStyle(ta).paddingTop) || 0;
    const csY = (outY < 0 ? ta.scrollTop
              : ta.scrollTop + ta.clientHeight - 1) + cssPadT;
    const off = offsetAtContentY(csY);
    if (dragAnchor !== null) {
      if (off >= dragAnchor) ta.setSelectionRange(dragAnchor, off);
      else ta.setSelectionRange(off, dragAnchor);
      updatePos();
    }
  }
  dragRaf = requestAnimationFrame(dragAutoscrollStep);
}

function doReplace() {
  const term = findInput.value;
  const replacement = document.getElementById("replace-input").value;
  if (!term) return;
  const matches = findAllMatches();
  if (matches.length === 0) return;
  // find the current match (closest to selectionStart)
  const current = editor.selectionStart;
  let pos = matches.findIndex(m => m === current);
  if (pos === -1) pos = matches.findIndex(m => m >= current);
  if (pos === -1) pos = 0;
  const idx = matches[pos];
  editor.focus();
  editor.setSelectionRange(idx, idx + term.length);
  document.execCommand("insertText", false, replacement);
  findInput.focus();
  // go to the next match
  goToMatch(1);
}

function doReplaceAll() {
  const term = findInput.value;
  const replacement = document.getElementById("replace-input").value;
  if (!term) return;
  const matches = findAllMatches();
  if (matches.length === 0) return;
  editor.focus();
  for (let i = matches.length - 1; i >= 0; i--) {
    editor.setSelectionRange(matches[i], matches[i] + term.length);
    document.execCommand("insertText", false, replacement);
  }
  findInput.focus();
  findCount.textContent = "";
}

// --- Theme ---
function setTheme(mode) {
  if (mode === "light" || mode === "dark") {
    document.documentElement.setAttribute("data-theme", mode);
  } else {
    document.documentElement.removeAttribute("data-theme");
  }
}

async function handleCloseRequested() {
  if (isHandlingQuit) return;
  if (isHandlingClose) return;
  isHandlingClose = true;
  try {
    if (!dirty) {
      await invoke("confirm_close");
      return;
    }

    const name = displayName();

    // 0 = Save, 1 = Don't Save, 2 = Cancel
    const choice = await invoke("confirm_close_dirty", { name, lang });

    if (choice === 2) return;            // Cancel: keep the window open
    if (choice === 0) {                  // Save
      const saved = await saveFile();
      if (!saved) return;                // Save As was cancelled: do not close
    }
    // choice === 1 (Don't Save) falls through without saving
    await invoke("confirm_close");
  } finally {
    isHandlingClose = false;
  }
}

async function handleQuitSaveWindow() {
  if (isHandlingQuit) return;
  isHandlingQuit = true;
  try {
    if (dirty) {
      try { await getCurrentWindow().setFocus(); } catch (e) {}
      const saved = await saveFile();
      if (!saved) {
        await invoke("cancel_quit");
        return;
      }
    }
    await invoke("confirm_quit_window");
  } finally {
    isHandlingQuit = false;
  }
}

// --- Init ---
async function init() {
  editor.addEventListener("input", () => { markDirty(); updateStatus(); updatePos(); });
  // A real click/keystroke in the editor turns off Enter-navigation mode:
  // after that, Enter in the text inserts a newline instead of "find next".
  // A programmatic editor.focus() from goToMatch sends no mousedown, so the flag survives.
  editor.addEventListener("mousedown", (e) => {
    findbarNavMode = false;
    if (e.button !== 0) return;
    dragActive = true;
    dragPointerX = e.clientX;
    dragPointerY = e.clientY;
    dragAnchor = null;
    dragRef = null;
    dragClick = null;
    const { clientX, shiftKey } = e;
    setTimeout(() => { if (dragActive) captureDragAnchor(clientX, shiftKey); }, 0);
  });
  function stopDrag() {
    dragActive = false;
    dragAnchor = null;
    dragRef = null;
    dragClick = null;
    dragLineStarts = null;
    if (dragMeasure) { dragMeasure.destroy(); dragMeasure = null; }
    if (dragRaf !== null) { cancelAnimationFrame(dragRaf); dragRaf = null; }
  }
  document.addEventListener("mousemove", (e) => {
    if (!dragActive) return;
    // Button released but mouseup never arrived (trackpad outside the window): stop.
    if (e.buttons === 0) { stopDrag(); return; }
    dragPointerX = e.clientX;
    dragPointerY = e.clientY;
    if (dragRaf === null) dragRaf = requestAnimationFrame(dragAutoscrollStep);
  });
  document.addEventListener("mouseup", stopDrag);
  document.addEventListener("mouseleave", stopDrag);
  window.addEventListener("blur", stopDrag);
  editor.addEventListener("keydown", (e) => {
    // Do not reset navigation mode on: Enter (handled by the capture listener),
    // shortcuts (Cmd/Ctrl/Alt) and bare modifiers (Shift/Ctrl/Alt/Meta on their
    // own arrive as a separate keydown before the main key).
    if (e.key === "Enter") return;
    if (e.metaKey || e.ctrlKey || e.altKey) return;
    if (e.key === "Shift" || e.key === "Control" || e.key === "Alt" || e.key === "Meta") return;
    findbarNavMode = false;
  });
  // Document-level listener in the capture phase: while the find bar is open,
  // Enter and Shift+Enter go to the next/prev match regardless of
  // where focus is (findInput, the editor or a bar button).
  // The capture phase intercepts Enter BEFORE the textarea inserts \n.
  document.addEventListener("keydown", (e) => {
    const findbar = document.getElementById("findbar");
    if (findbar.hidden) return;
    if (e.key !== "Enter") return;
    // Enter jumps to a match ONLY when focus is inside the find bar
    // (search or replace field). In the editor Enter inserts a newline (like TextEdit).
    // "Find next" from the text is Cmd+G / Cmd+Shift+G (bound separately).
    if (!findbarNavMode) return;
    e.preventDefault();
    e.stopPropagation();
    goToMatch(e.shiftKey ? -1 : 1);
  }, true);
  // Incremental counter (like TextEdit): recount on every query change.
  // No jump here: jumping takes Enter/Cmd+G/the buttons.
  findInput.addEventListener("input", () => {
    if (!findInput.value) {
      findCount.textContent = "";
      return;
    }
    const matches = findAllMatches();
    if (matches.length === 0) {
      findCount.textContent = t("noMatches");
    } else {
      findCount.textContent = t("findCount", matches.length);
    }
  });
  document.getElementById("find-prev").addEventListener("click", () => goToMatch(-1));
  document.getElementById("find-next").addEventListener("click", () => goToMatch(1));
  document.getElementById("find-case").addEventListener("click", (e) => {
    matchCase = !matchCase;
    e.target.classList.toggle("active", matchCase);
  });
  document.getElementById("replace-one").addEventListener("click", doReplace);
  document.getElementById("replace-all").addEventListener("click", doReplaceAll);
  findInput.addEventListener("focus", () => { findbarNavMode = true; });
  document.getElementById("replace-input").addEventListener("focus", () => { findbarNavMode = true; });
  findClose.addEventListener("click", closeFind);
  gotoInput.addEventListener("keydown", (e) => {
    if (e.key === "Enter") { e.preventDefault(); doGoto(); }
    else if (e.key === "Escape") { e.preventDefault(); closeGoto(); }
  });
  document.getElementById("goto-close").addEventListener("click", closeGoto);
  document.addEventListener("selectionchange", () => {
    if (document.activeElement !== editor) return;
    updatePos();
    // Forced repaint: WebKit leaves an unpainted seam at the boundaries of its internal
    // text tiles in a textarea during selection (translateZ/will-change do not help).
    // Nudging scrollTop forth and back forces WebKit to repaint
    // the whole visible area. See technical-lessons.md.
    const st = editor.scrollTop;
    editor.scrollTop = st + 1;
    editor.scrollTop = st;
  });
  try {
    console.log("[lang]", lang, navigator.language);
    try {
      try {
        const savedTheme = await invoke("get_theme");
        setTheme(savedTheme);
      } catch (e) {
        console.error("get_theme:", e);
      }
      try {
        const savedWrap = await invoke("get_word_wrap");
        applyWordWrap(savedWrap);
      } catch (e) {
        console.error("get_word_wrap:", e);
      }
      try {
        const savedStatus = await invoke("get_status_bar");
        applyStatusBar(savedStatus);
      } catch (e) {
        console.error("get_status_bar:", e);
      }
      try {
        const savedCounter = await invoke("get_show_counter");
        applyShowCounter(savedCounter);
      } catch (e) {
        console.error("get_show_counter:", e);
      }
      try {
        autoNameEnabled = await invoke("get_auto_name");
        updateTitle();
      } catch (e) {
        console.error("get_auto_name:", e);
      }
      try {
        const [fontName, fontSize, fontWeight, fontStyle] = await invoke("get_font");
        editor.style.fontFamily = `"${fontName}", monospace`;
        editor.style.fontSize = `${fontSize}px`;
        editor.style.fontWeight = fontWeight;
        editor.style.fontStyle = fontStyle;
        editor.style.lineHeight = `${Math.round(fontSize * 1.6)}px`;
      } catch (e) {
        console.error("get_font:", e);
      }
      try {
        const pending = await invoke("take_pending_file");
        if (pending) {
          const [path, content] = pending;
          loadFileIntoEditor(path, content);
        }
      } catch (e) {
        console.error("take_pending_file:", e);
      }
    } finally {
      try {
        await getCurrentWindow().show();
        windowShown = true;
      } catch (e) { console.error("show:", e); }
    }
    await getCurrentWindow().listen("close-requested", handleCloseRequested);
    await getCurrentWindow().listen("menu-close-window", handleCloseRequested);
    await getCurrentWindow().listen("quit-save-window", handleQuitSaveWindow);
    await getCurrentWindow().listen("menu-open", openFile);
    await getCurrentWindow().listen("pending-file-ready", async () => {
      if (dirty) return;
      try {
        const pending = await invoke("take_pending_file");
        if (pending) {
          const [path, content] = pending;
          loadFileIntoEditor(path, content);
        }
      } catch (e) {
        console.error("pending-file-ready:", e);
      }
    });
    await getCurrentWindow().listen("menu-save", saveFile);
    await getCurrentWindow().listen("menu-save-as", saveFileAs);
    await getCurrentWindow().listen("menu-font-panel", async () => {
      await openFontPanel();
    });
    await listen("font-changed", (event) => {
      const [name, size, weight, style] = event.payload;
      editor.style.fontFamily = `"${name}", monospace`;
      editor.style.fontSize = `${size}px`;
      editor.style.fontWeight = weight;
      editor.style.fontStyle = style;
      editor.style.lineHeight = `${Math.round(size * 1.6)}px`;
    });
    await getCurrentWindow().listen("menu-page-setup", async () => {
      await invoke("page_setup");
    });
    await getCurrentWindow().listen("menu-print", async () => {
      const filename = displayName();
      await invoke("print_document", { text: editor.value, filename });
    });
    await getCurrentWindow().listen("menu-open-recent", async (e) => {
      const [path, content] = e.payload;
      if (dirty) {
        const ok = await ask(t("confirmNew"), { title: t("confirmNewTitle"), okLabel: t("yes"), cancelLabel: t("no"), kind: "warning" });
        if (!ok) return;
      }
      loadFileIntoEditor(path, content);
    });
    await listen("menu-theme", async (e) => {
      setTheme(e.payload);
      try {
        await invoke("set_theme", { theme: e.payload });
      } catch (err) {
        console.error("set_theme:", err);
      }
    });
    await listen("menu-word-wrap", async (e) => {
      applyWordWrap(e.payload);
      try {
        await invoke("set_word_wrap", { enabled: e.payload });
      } catch (err) {
        console.error("set_word_wrap:", err);
      }
    });
    await listen("menu-status-bar", async (e) => {
      applyStatusBar(e.payload);
      try {
        await invoke("set_status_bar", { enabled: e.payload });
      } catch (err) {
        console.error("set_status_bar:", err);
      }
    });
    await listen("menu-show-counter", (e) => {
      applyShowCounter(e.payload);
    });
    await listen("menu-auto-name", (e) => {
      autoNameEnabled = e.payload;
      updateTitle();
    });
    await getCurrentWindow().listen("menu-edit", async (e) => {
      switch (e.payload) {
        case "undo":       document.execCommand("undo"); break;
        case "redo":       document.execCommand("redo"); break;
        case "cut":        document.execCommand("cut"); break;
        case "copy":       document.execCommand("copy"); break;
        case "select-all": document.execCommand("selectAll"); break;
        case "find": openFind("find"); break;
        case "goto": openGoto(); break;
        case "find-next": goToMatch(1); break;
        case "replace": openFind("replace"); break;
        case "datetime": {
          const stamp = dateStamp();
          editor.focus();
          // break the undo group so the timestamp is a separate undo step
          const p = editor.selectionStart;
          editor.setSelectionRange(p, p);
          document.execCommand("insertText", false, stamp);
          markDirty();
          updateStatus();
          updatePos();
          break;
        }
        case "paste": {
          try {
            const text = await invoke("plugin:clipboard-manager|read_text");
            if (text) {
              editor.focus();
              const ok = document.execCommand("insertText", false, text);
              if (!ok) {
                console.error("insertText failed");
              }
              markDirty();
              updateStatus();
            }
          } catch (err) {
            console.error("paste:", err);
          }
          break;
        }
      }
    });
    if (!currentPath) {
      try { untitledNum = await invoke("claim_untitled_number"); } catch (e) {}
    }
    updateStatus();
    updateTitle();
    updatePos();
  } catch (e) {
    console.error(e);
  }
  editor.focus();
}

init();
setTimeout(() => {
  if (windowShown) return;
  try { getCurrentWindow().show(); windowShown = true; } catch (e) {}
}, 1500);

// ─── Font Panel ───────────────────────────────────────────────────────────────

async function openFontPanel() {
  await invoke("open_font_panel");
}

// --- Editor context menu (custom, replacing the WKWebView system menu) ---
function showCtxMenu(x, y) {
  ctxMenu.hidden = false;
  const rect = ctxMenu.getBoundingClientRect();
  const maxX = window.innerWidth - rect.width - 4;
  const maxY = window.innerHeight - rect.height - 4;
  ctxMenu.style.left = Math.min(x, maxX) + "px";
  ctxMenu.style.top = Math.min(y, maxY) + "px";
}

function hideCtxMenu() {
  ctxMenu.hidden = true;
}

function ctxAction(action) {
  hideCtxMenu();
  editor.focus();
  if (action === "paste") {
    invoke("plugin:clipboard-manager|read_text").then((text) => {
      if (text) {
        document.execCommand("insertText", false, text);
        markDirty();
        updateStatus();
        updatePos();
      }
    }).catch((err) => console.error("paste:", err));
    return;
  }
  switch (action) {
    case "undo": document.execCommand("undo"); break;
    case "cut": document.execCommand("cut"); markDirty(); break;
    case "copy": document.execCommand("copy"); break;
    case "delete":
      if (editor.selectionStart !== editor.selectionEnd) {
        document.execCommand("insertText", false, "");
        markDirty();
      }
      break;
    case "selectAll": editor.select(); break;
  }
  updateStatus();
  updatePos();
}

editor.addEventListener("mousedown", (e) => {
  if (e.button === 2) e.preventDefault();
});

editor.addEventListener("contextmenu", (e) => {
  e.preventDefault();
  showCtxMenu(e.clientX, e.clientY);
});

ctxMenu.addEventListener("click", (e) => {
  const item = e.target.closest(".ctx-item");
  if (item) ctxAction(item.dataset.action);
});

document.addEventListener("mousedown", (e) => {
  if (!ctxMenu.hidden && !ctxMenu.contains(e.target)) hideCtxMenu();
});

