// Tauri v2 exposes globals via window.__TAURI__
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const { ask } = window.__TAURI__.dialog;

const editor = document.getElementById("editor");
const statWords = document.getElementById("stat-words");
const statChars = document.getElementById("stat-chars");

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
    unsaved: "Без названия",
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

function setTitle() {
  const name = currentPath ? currentPath.split("/").pop() : t("unsaved");
  document.title = dirty ? `${name} •` : name;
}

function markDirty() { dirty = true; setTitle(); }
function markClean() { dirty = false; setTitle(); }

// --- Status bar ---
function updateStatus() {
  const text = editor.value;
  const chars = [...text].length;
  const words = text.split(/\s+/).filter(Boolean).length;
  statWords.textContent = t("words", words);
  statChars.textContent = t("chars", chars);
}

// --- File operations ---
async function newFile() {
  if (dirty) {
    const ok = await ask(t("confirmNew"), { title: t("confirmNewTitle"), okLabel: t("yes"), cancelLabel: t("no"), kind: "warning" });
    if (!ok) return;
  }
  editor.value = "";
  currentPath = null;
  markClean();
  updateStatus();
}

async function openFile() {
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
  const result = await invoke("save_file_as", { content: editor.value });
  if (!result) return;
  currentPath = result;
  markClean();
}

// --- Keyboard shortcuts ---
document.addEventListener("keydown", (e) => {
  if ((e.metaKey || e.ctrlKey) && !e.altKey) {
    if (e.key === "n" || e.key === "N") { e.preventDefault(); newFile(); }
    else if (e.key === "o" || e.key === "O") { e.preventDefault(); openFile(); }
    else if (e.key === "s" || e.key === "S") {
      e.preventDefault();
      if (e.shiftKey) saveFileAs(); else saveFile();
    }
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
  editor.addEventListener("input", () => { markDirty(); updateStatus(); });
  try {
    lang = navigator.language.toLowerCase().startsWith("ru") ? "ru" : "en";
    console.log("[lang]", lang, navigator.language);
    await listen("menu-new", newFile);
    await listen("menu-open", openFile);
    await listen("menu-save", saveFile);
    await listen("menu-save-as", saveFileAs);
    await listen("menu-theme", (e) => setTheme(e.payload));
    await listen("menu-edit", async (e) => {
      switch (e.payload) {
        case "undo":       document.execCommand("undo"); break;
        case "redo":       document.execCommand("redo"); break;
        case "cut":        document.execCommand("cut"); break;
        case "copy":       document.execCommand("copy"); break;
        case "select-all": document.execCommand("selectAll"); break;
        case "paste": {
          try {
            const text = await invoke("plugin:clipboard-manager|read_text");
            if (text) {
              const s = editor.selectionStart, end = editor.selectionEnd;
              editor.value = editor.value.slice(0, s) + text + editor.value.slice(end);
              editor.selectionStart = editor.selectionEnd = s + text.length;
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
    setTitle();
  } catch (e) {
    console.error(e);
  }
  editor.focus();
}

init();
