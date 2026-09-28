// 120 long lines (every tenth one short, every tenth + 7 starting with two tabs), Word
// Wrap off, Menlo 19/30. Reports "sl=<scrollLeft> st=<scrollTop> sel=<line>:<col>-<line>:<col>"
// every 50ms for the runner's mouse drags.
window.__uiTest(({ ed, app, report, longLine }) => {
  let t = "";
  for (let i = 0; i < 120; i++) {
    let line = longLine(i, 400);
    if (i % 10 === 5) line = "L" + String(i).padStart(3, "0") + " short";
    else if (i % 10 === 7) line = "\t\tTAB" + line;
    t += line + "\n";
  }
  ed.style.fontSize = "19px";
  ed.style.lineHeight = "30px";
  app.applyWordWrap(false);
  ed.value = t;
  ed.focus();
  ed.setSelectionRange(0, 0);
  const lc = (o) => {
    const before = ed.value.slice(0, o);
    return (before.split("\n").length - 1) + ":" + (o - before.lastIndexOf("\n") - 1);
  };
  setInterval(() => report(`sl=${Math.round(ed.scrollLeft)} st=${Math.round(ed.scrollTop)} sel=${lc(ed.selectionStart)}-${lc(ed.selectionEnd)}`), 50);
});
