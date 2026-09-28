// Selection painting cost on a 3.6 MB document with everything selected, at 40 scroll
// positions. Reports "PERF median=<ms> max=<ms>".
window.__uiTest(({ ed, app, report }) => {
  let t = "";
  for (let i = 0; i < 50000; i++) t += "Line " + i + ": the quick brown fox jumps over the lazy dog, again and again\n";
  ed.value = t;
  ed.focus();
  ed.setSelectionRange(0, t.length);
  setTimeout(() => {
    app.paintSelection();
    const times = [];
    for (let k = 0; k < 40; k++) {
      ed.scrollTop = k * 37000;
      const a = performance.now();
      app.paintSelection();
      times.push(performance.now() - a);
    }
    times.sort((x, y) => x - y);
    report(`PERF median=${times[20].toFixed(2)} max=${times[39].toFixed(2)}`);
  }, 1500);
});
