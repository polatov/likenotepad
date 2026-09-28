// A fixed selection over tabs, empty lines, wide glyphs and a long line; params.wrap,
// params.native. Reports "READY" once painted.
window.__uiTest(({ ed, app, params, report, useNativeHighlight }) => {
  const lines = [
    "First line of the document",
    "\tTabbed line\twith inner tab",
    "",
    "日本語のテキスト mixed with ASCII",
    "A very long line that keeps going and going so that it will certainly wrap when word wrap is on, more words here to be sure",
    "",
    "\t\tDouble tab",
    "Short",
    "Кириллица и латиница вместе",
    "Last selected line here",
    "After selection",
  ];
  ed.value = lines.join("\n") + "\n";
  ed.style.fontSize = "19px";
  ed.style.lineHeight = "30px";
  app.applyWordWrap(!!params.wrap);
  if (params.native) useNativeHighlight();
  ed.focus();
  ed.scrollTop = 0;
  ed.scrollLeft = params.scrollLeft || 0;
  ed.setSelectionRange(ed.value.indexOf("line of"), ed.value.indexOf("selected line") + 8);
  setTimeout(() => report("READY"), 500);
});
