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
const findInput = document.getElementById("find-input");
const findCount = document.getElementById("find-count");
const findClose = document.getElementById("find-close");
const gotobar = document.getElementById("gotobar");
const gotoInput = document.getElementById("goto-input");

// --- i18n ---
let lang = "en";
let findbarNavMode = false;
let windowShown = false;
let isHandlingClose = false;
let isHandlingQuit = false;

function pluralRu(n, forms) {
  const n100 = n % 100, n10 = n % 10;
  if (n100 >= 11 && n100 <= 14) return forms[2];
  if (n10 === 1) return forms[0];
  if (n10 >= 2 && n10 <= 4) return forms[1];
  return forms[2];
}

const T = {
  en: {
    lines: (n) => `${n} line${n !== 1 ? "s" : ""}`,
    words: (n) => `${n} word${n !== 1 ? "s" : ""}`,
    chars: (n) => `${n} character${n !== 1 ? "s" : ""}`,
    unsaved: "Untitled",
    confirmNew: "Discard unsaved changes?",
    confirmNewTitle: "New",
    yes: "Yes",
    no: "No",
  },
  ru: {
    lines: (n) => `${n} ${pluralRu(n, ["строка", "строки", "строк"])}`,
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
let untitledNum = 0;

function updateTitle() {
  let name;
  if (currentPath) {
    name = currentPath.split("/").pop();
  } else {
    const base = t("unsaved");
    name = untitledNum > 1 ? `${base} ${untitledNum}` : base;
  }
  const title = `${dirty ? "• " : ""}${name} — LikeNotepad.exe`;
  document.title = title;
  getCurrentWindow().setTitle(title);
}

async function releaseUntitled() {
  if (untitledNum === 0) return;
  untitledNum = 0;
  try { await invoke("release_untitled_number"); } catch (e) {}
}

function displayName() {
  if (currentPath) return currentPath.split("/").pop();
  const base = t("unsaved");
  return untitledNum > 1 ? `${base} ${untitledNum}` : base;
}

function markDirty() { dirty = true; updateTitle(); syncDirty(); }
function markClean() { dirty = false; updateTitle(); syncDirty(); }
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
  statPos.textContent = (lang === "ru" ? `Лн ${line}, Ст ${col}` : `Ln ${line}, Col ${col}`);
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
  await releaseUntitled();
  markClean();
  updateStatus();
}

async function saveFile() {
  if (!currentPath) return saveFileAs();
  await invoke("save_file", { path: currentPath, content: editor.value });
  markClean();
  return true;
}

async function saveFileAs() {
  const suggestedName = currentPath ? currentPath.split("/").pop() : `${displayName()}.txt`;
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
    if (e.key === "o" || e.key === "O") { e.preventDefault(); openFile(); }
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

  // Базовая точка: конец предыдущего выделения (если оно совпадает с матчем — current будет idx этого матча)
  // Для "next" ищем строго ПОСЛЕ конца выделения, чтобы не застревать на одном и том же
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

  // Выделяем совпадение и скроллим к нему, потом возвращаем фокус в поле поиска.
  // Скролл делаем пока фокус на editor (иначе scrollTop при setSelectionRange не сработает в WebKit для textarea без фокуса).
  editor.focus();
  editor.setSelectionRange(idx, idx + term.length);
  scrollEditorToSelection(idx);

  findCount.textContent = (pos + 1) + (lang === "ru" ? " из " : " of ") + matches.length + (lang === "ru" ? " совпадений" : " matches");
}

// Скроллит textarea к позиции offset через зеркальный div с тем же шрифтом/wrap.
function scrollEditorToSelection(offset) {
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

  const before = ta.value.substring(0, offset);
  mirror.textContent = before;
  const marker = document.createElement("span");
  marker.textContent = "​";
  mirror.appendChild(marker);

  const markerTop = marker.offsetTop;
  document.body.removeChild(mirror);

  // Скроллим так, чтобы совпадение было в видимой области с отступом.
  const lineH = parseFloat(cs.lineHeight) || parseFloat(cs.fontSize) * 1.2;
  const visibleTop = ta.scrollTop;
  const visibleBottom = visibleTop + ta.clientHeight;

  if (markerTop < visibleTop || markerTop + lineH > visibleBottom) {
    ta.scrollTop = Math.max(0, markerTop - ta.clientHeight / 3);
  }
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

    // 0 = Сохранить, 1 = Не сохранять, 2 = Отмена
    const choice = await invoke("confirm_close_dirty", { name, lang });

    if (choice === 2) return;            // Отмена — окно остаётся открытым
    if (choice === 0) {                  // Сохранить
      const saved = await saveFile();
      if (!saved) return;                // отменили Save As — не закрываем
    }
    // choice === 1 (Не сохранять) проваливается сюда без сохранения
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
  // Живой клик/печать в editor выключает режим навигации по Enter:
  // после этого Enter в тексте = перенос строки, а не "найти далее".
  // Программный editor.focus() из goToMatch события mousedown не шлёт — флаг переживает.
  editor.addEventListener("mousedown", () => { findbarNavMode = false; });
  editor.addEventListener("keydown", (e) => {
    // Не сбрасываем режим навигации на: Enter (обрабатывает capture-листенер),
    // хоткеи (Cmd/Ctrl/Alt) и чистые модификаторы (Shift/Ctrl/Alt/Meta сами
    // по себе — они приходят отдельным keydown перед основной клавишей).
    if (e.key === "Enter") return;
    if (e.metaKey || e.ctrlKey || e.altKey) return;
    if (e.key === "Shift" || e.key === "Control" || e.key === "Alt" || e.key === "Meta") return;
    findbarNavMode = false;
  });
  // Document-level listener в capture phase: пока findbar открыт,
  // Enter и Shift+Enter переходят к next/prev совпадению независимо от того,
  // где сейчас фокус (findInput, editor или кнопка в баре).
  // Capture phase нужен чтобы перехватить Enter ДО того, как textarea вставит \n.
  document.addEventListener("keydown", (e) => {
    const findbar = document.getElementById("findbar");
    if (findbar.hidden) return;
    if (e.key !== "Enter") return;
    // Enter переходит к совпадению ТОЛЬКО когда фокус внутри findbar
    // (поле поиска или замены). В editor Enter = перенос строки (как TextEdit).
    // "Найти далее" из текста — через Cmd+G / Cmd+Shift+G (висят отдельно).
    if (!findbarNavMode) return;
    e.preventDefault();
    e.stopPropagation();
    goToMatch(e.shiftKey ? -1 : 1);
  }, true);
  // Инкрементальный счётчик (как в TextEdit): пересчитываем при каждом изменении запроса.
  // Перехода не делаем — для перехода нужен Enter/Cmd+G/кнопки.
  findInput.addEventListener("input", () => {
    if (!findInput.value) {
      findCount.textContent = "";
      return;
    }
    const matches = findAllMatches();
    if (matches.length === 0) {
      findCount.textContent = lang === "ru" ? "Нет совпадений" : "No matches";
    } else {
      findCount.textContent = matches.length + (lang === "ru" ? " совпадений" : " matches");
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
  document.addEventListener("selectionchange", () => { if (document.activeElement === editor) updatePos(); });
  try {
    lang = navigator.language.toLowerCase().startsWith("ru") ? "ru" : "en";
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
        const [fontName, fontSize, fontWeight, fontStyle] = await invoke("get_font");
        editor.style.fontFamily = `"${fontName}", monospace`;
        editor.style.fontSize = `${fontSize}px`;
        editor.style.fontWeight = fontWeight;
        editor.style.fontStyle = fontStyle;
      } catch (e) {
        console.error("get_font:", e);
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
      editor.value = content;
      currentPath = path;
      await releaseUntitled();
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
          const now = new Date();
          const locale = lang === "ru" ? "ru-RU" : "en-US";
          const stamp = now.toLocaleString(locale, {
            day: "2-digit", month: "2-digit", year: "numeric",
            hour: "2-digit", minute: "2-digit"
          });
          editor.focus();
          // разорвать undo-серию, чтобы дата стала отдельной записью отмены
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

