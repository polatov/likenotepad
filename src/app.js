// Tauri v2 exposes globals via window.__TAURI__
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const { ask } = window.__TAURI__.dialog;
const { getCurrentWindow } = window.__TAURI__.window;

const editor = document.getElementById("editor");
const statWords = document.getElementById("stat-words");
const statChars = document.getElementById("stat-chars");

const findbar = document.getElementById("findbar");
const findInput = document.getElementById("find-input");
const findCount = document.getElementById("find-count");
const findClose = document.getElementById("find-close");
const findOverlay = document.getElementById("find-overlay");
const findHighlight = document.getElementById("find-highlight");

// --- i18n ---
let lang = "en";

function pluralRu(n, forms) {
  const n100 = n % 100, n10 = n % 10;
  if (n100 >= 11 && n100 <= 14) return forms[2];
  if (n10 === 1) return forms[0];
  if (n10 >= 2 && n10 <= 4) return forms[1];
  return forms[2];
}

const T = {
  en: {
    words: (n) => `${n} word${n !== 1 ? "s" : ""}`,
    chars: (n) => `${n} character${n !== 1 ? "s" : ""}`,
    unsaved: "Untitled",
    confirmNew: "Discard unsaved changes?",
    confirmNewTitle: "New",
    yes: "Yes",
    no: "No",
  },
  ru: {
    words: (n) => `${n} ${pluralRu(n, ["слово", "слова", "слов"])}`,
    chars: (n) => `${n} ${pluralRu(n, ["символ", "символа", "символов"])}`,
    unsaved: "Без имени",
    confirmNew: "Отменить несохранённые изменения?",
    confirmNewTitle: "Новый",
    yes: "Да",
    no: "Нет",
  },
};

function t(key, arg) {
  const v = T[lang]?.[key] ?? T.en[key];
  return typeof v === 'function' ? v(arg) : v;
}

// --- State ---
let currentPath = null;
let dirty = false;

function updateTitle() {
  const name = currentPath ? currentPath.split("/").pop() : t("unsaved");
  const title = `${dirty ? "• " : ""}${name} — LikeNotepad.exe`;
  document.title = title;
  getCurrentWindow().setTitle(title);
}

function markDirty() { dirty = true; updateTitle(); }
function markClean() { dirty = false; updateTitle(); }

// --- Status bar ---
function updateStatus() {
  const text = editor.value;
  const chars = [...text].length;
  const words = text.split(/\s+/).filter(Boolean).length;
  statWords.textContent = t("words", words);
  statChars.textContent = t("chars", chars);
}

// --- File operations ---
async function openFile() {
  if (dirty) {
    const ok = await ask(t("confirmNew"), { title: t("confirmNewTitle"), okLabel: t("yes"), cancelLabel: t("no"), kind: "warning" });
    if (!ok) return;
  }
  const result = await invoke("open_file");
  if (!result) return;
  const [path, content] = result;
  editor.value = content;
  currentPath = path;
  markClean();
  updateStatus();
}

async function saveFile() {
  if (!currentPath) return saveFileAs();
  await invoke("save_file", { path: currentPath, content: editor.value });
  markClean();
}

async function saveFileAs() {
  const suggestedName = currentPath ? currentPath.split("/").pop() : (lang === "ru" ? "Без имени.txt" : "Untitled.txt");
  const result = await invoke("save_file_as", { content: editor.value, suggestedName });
  if (!result) return;
  currentPath = result;
  markClean();
}

// --- Keyboard shortcuts ---
document.addEventListener("keydown", (e) => {
  if ((e.metaKey || e.ctrlKey) && !e.altKey) {
    if (e.key === "o" || e.key === "O") { e.preventDefault(); openFile(); }
    else if (e.key === "s" || e.key === "S") {
      e.preventDefault();
      if (e.shiftKey) saveFileAs(); else saveFile();
    }
    else if (e.key === "f" || e.key === "F") { e.preventDefault(); openFind("find"); }
  }
  else if ((e.metaKey || e.ctrlKey) && e.altKey && (e.key === "f" || e.key === "F")) {
    e.preventDefault();
    openFind("replace");
  }
  else if (e.key === "Escape" && !findbar.hidden) {
    e.preventDefault();
    closeFind();
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
const tabSize = 4;
let charW = 0;
let lineHeight = 0;
let editorPaddingTop = 0;
let editorPaddingLeft = 0;
let lastMatchStart = null;
let lastMatchLen = 0;
let replaceMode = false;
let matchCase = false;

function measureFindMetrics() {
  const canvas = document.createElement("canvas");
  const ctx = canvas.getContext("2d");
  ctx.font = "13px Menlo";
  charW = ctx.measureText("M").width;
  const cs = getComputedStyle(editor);
  lineHeight = parseFloat(cs.lineHeight);
  editorPaddingTop = parseFloat(cs.paddingTop);
  editorPaddingLeft = parseFloat(cs.paddingLeft);
}

function advanceCol(str, startCol) {
  let col = startCol;
  for (const ch of str) {
    if (ch === "\t") {
      col = Math.floor(col / tabSize) * tabSize + tabSize;
    } else {
      col += 1;
    }
  }
  return col;
}

function hideHighlight() {
  findOverlay.hidden = true;
  lastMatchStart = null;
}

function positionHighlight(matchStart, matchLen) {
  lastMatchStart = matchStart;
  lastMatchLen = matchLen;

  const text = editor.value;
  const before = text.slice(0, matchStart);
  const lines = before.split("\n");
  const lineNum = lines.length - 1;
  const lineText = lines[lineNum];

  const visualCol = advanceCol(lineText, 0);
  const matchText = text.slice(matchStart, matchStart + matchLen);
  const visualEndCol = advanceCol(matchText, visualCol);
  const visualLen = visualEndCol - visualCol;

  const x = editorPaddingLeft + visualCol * charW - editor.scrollLeft;
  const y = editorPaddingTop + lineNum * lineHeight - editor.scrollTop;

  findHighlight.style.left = `${x}px`;
  findHighlight.style.top = `${y}px`;
  findHighlight.style.width = `${visualLen * charW}px`;
  findHighlight.style.height = `${lineHeight}px`;
  findOverlay.hidden = false;
}

editor.addEventListener("scroll", () => {
  if (lastMatchStart !== null) positionHighlight(lastMatchStart, lastMatchLen);
});

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
  hideHighlight();
  replaceMode = false;
  document.getElementById("findbar-row2").hidden = true;
  editor.focus();
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
    hideHighlight();
    return;
  }

  const current = editor.selectionStart;
  let pos;
  if (direction === 1) {
    pos = matches.findIndex((m) => m > current);
    if (pos === -1) pos = 0;
  } else {
    pos = -1;
    for (let i = matches.length - 1; i >= 0; i--) {
      if (matches[i] < current) { pos = i; break; }
    }
    if (pos === -1) pos = matches.length - 1;
  }
  const idx = matches[pos];

  editor.setSelectionRange(idx, idx + term.length);
  findInput.focus();
  findCount.textContent = (pos + 1) + (lang === "ru" ? " из " : " of ") + matches.length + (lang === "ru" ? " совпадений" : " matches");
  positionHighlight(idx, term.length);
}

function doReplace() {
  const term = findInput.value;
  const replacement = document.getElementById("replace-input").value;
  if (!term) return;
  const matches = findAllMatches();
  if (matches.length === 0) return;
  // найти текущее совпадение (ближайшее к selectionStart)
  const current = editor.selectionStart;
  let pos = matches.findIndex(m => m === current);
  if (pos === -1) pos = matches.findIndex(m => m >= current);
  if (pos === -1) pos = 0;
  const idx = matches[pos];
  editor.focus();
  editor.setSelectionRange(idx, idx + term.length);
  document.execCommand("insertText", false, replacement);
  findInput.focus();
  // перейти к следующему совпадению
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
  hideHighlight();
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

// --- Init ---
async function init() {
  measureFindMetrics();
  editor.addEventListener("input", () => { markDirty(); updateStatus(); hideHighlight(); });
  findInput.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      e.preventDefault();
      e.stopPropagation();
      goToMatch(e.shiftKey ? -1 : 1);
    }
  });
  findInput.addEventListener("input", () => {
    if (!findInput.value) {
      findCount.textContent = "";
      hideHighlight();
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
  findClose.addEventListener("click", closeFind);
  try {
    lang = navigator.language.toLowerCase().startsWith("ru") ? "ru" : "en";
    console.log("[lang]", lang, navigator.language);
    try {
      const savedTheme = await invoke("get_theme");
      setTheme(savedTheme);
    } catch (e) {
      console.error("get_theme:", e);
    }
    await listen("menu-open", openFile);
    await listen("menu-save", saveFile);
    await listen("menu-save-as", saveFileAs);
    await listen("menu-open-recent", async (e) => {
      const [path, content] = e.payload;
      if (dirty) {
        const ok = await ask(t("confirmNew"), { title: t("confirmNewTitle"), okLabel: t("yes"), cancelLabel: t("no"), kind: "warning" });
        if (!ok) return;
      }
      editor.value = content;
      currentPath = path;
      markClean();
      updateStatus();
    });
    await listen("menu-theme", async (e) => {
      setTheme(e.payload);
      try {
        await invoke("set_theme", { theme: e.payload });
      } catch (err) {
        console.error("set_theme:", err);
      }
    });
    await listen("menu-edit", async (e) => {
      switch (e.payload) {
        case "undo":       document.execCommand("undo"); break;
        case "redo":       document.execCommand("redo"); break;
        case "cut":        document.execCommand("cut"); break;
        case "copy":       document.execCommand("copy"); break;
        case "select-all": document.execCommand("selectAll"); break;
        case "replace": openFind("replace"); break;
        case "paste": {
          try {
            const text = await invoke("plugin:clipboard-manager|read_text");
            if (text) {
              editor.focus();
              const ok = document.execCommand("insertText", false, text);
              if (!ok) {
                console.error("insertText не сработал");
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
    updateStatus();
    updateTitle();
  } catch (e) {
    console.error(e);
  }
  editor.focus();
}

init();
