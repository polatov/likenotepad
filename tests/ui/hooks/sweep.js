// Select everything in params.lines long lines, then scroll the whole document in 400px
// steps; each step reports "STEP <k>" once painted, then "DONE".
window.__uiTest(({ ed, app, params, report, longLine, useNativeHighlight }) => {
  let t = "";
  for (let i = 0; i < params.lines; i++) t += longLine(i, 400) + "\n";
  ed.style.fontSize = params.fontSize + "px";
  ed.style.lineHeight = params.lineHeight + "px";
  app.applyWordWrap(!!params.wrap);
  if (params.native) useNativeHighlight();
  ed.value = t;
  ed.focus();
  ed.setSelectionRange(0, t.length);
  let k = 0;
  const step = () => {
    const max = ed.scrollHeight - ed.clientHeight;
    const top = Math.min(k * 400, max);
    ed.scrollTop = top;
    setTimeout(() => {
      report("STEP " + k);
      if (top < max) { k++; setTimeout(step, 700); } else setTimeout(() => report("DONE"), 700);
    }, 500);
  };
  setTimeout(step, 500);
});
