// Shared by the UI test hooks. tests/ui/run.sh concatenates the params, this file and one
// case into the file LIKENOTEPAD_TEST_HOOK names; it runs before the page's own scripts
// in every webview of a debug build.
window.__uiTest = function (run) {
  const start = () => {
    const ed = document.getElementById("editor");
    if (!ed) return; // the settings and font windows
    const timer = setInterval(() => {
      if (!window.__likeNotepad || !document.hasFocus()) return;
      clearInterval(timer);
      const win = window.__TAURI__.window.getCurrentWindow();
      run({
        ed,
        app: window.__likeNotepad,
        params: window.__uiParams || {},
        // The window title is how a hook reports state to the runner.
        report: (s) => win.setTitle(s),
        // "L000 |005abcdef|015abcdef…" — each cell names its own column.
        longLine: (i, width) => {
          let l = "L" + String(i).padStart(3, "0") + " ";
          while (l.length < width) l += "|" + String(l.length).padStart(3, "0") + "abcdef";
          return l.slice(0, width);
        },
        // Replaces the native highlight with WebKit's own, hiding ours (for comparisons).
        useNativeHighlight: () => {
          const st = document.createElement("style");
          st.textContent = "#editor::selection{background:#4d90fe !important} #sel-layer{visibility:hidden}";
          document.head.appendChild(st);
        },
      });
    }, 50);
  };
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", start);
  else start();
};
