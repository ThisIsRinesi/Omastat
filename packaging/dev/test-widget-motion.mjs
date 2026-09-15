import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";

const panel = readFileSync(new URL("../omarchy/omastat/Panel.qml", import.meta.url), "utf8");
function motion() {
  const later = [];
  const timer = () => ({ running: false, starts: 0, start() { this.running = true; this.starts++; }, stop() { this.running = false; } });
  const context = vm.createContext({
    Qt: { callLater: fn => later.push(fn) },
    opened: true, refreshRunning: false, detailRunning: false,
    refreshBusyVisible: false, detailBusyVisible: false,
    refreshFeedbackDelay: timer(), detailFeedbackDelay: timer(),
    revealQueued: false, revealedViewKey: "", viewKey: "day:0", viewReady: true,
    motionDuration: 150, body: { opacity: 1 },
    dataReveal: { starts: 0, restart() { this.starts++; }, stop() {} },
    reveals: 0, viewRevealed() { context.reveals++; },
  });
  context.root = context;
  for (const name of ["updateBusyFeedback", "scheduleViewReveal"]) {
    vm.runInContext(panel.match(new RegExp("^  function " + name + "\\([^\\n]*\\) \\{[\\s\\S]*?^  \\}", "m"))[0], context);
  }
  context.flush = () => { while (later.length) later.shift()(); };
  context.fire = name => {
    const timerName = name + "FeedbackDelay";
    assert.equal(context[timerName].running, true);
    context[timerName].running = false;
    const source = panel.match(new RegExp("id: " + timerName + "\\n    interval: 150\\n    onTriggered: ([^\\n]+)"))[1];
    vm.runInContext(source, context);
  };
  return context;
}

for (const kind of ["refresh", "detail"]) {
  const c = motion();
  c[kind + "Running"] = true;
  c.updateBusyFeedback();
  assert.equal(c[kind + "BusyVisible"], false, "fast queries do not flash busy text");
  c.updateBusyFeedback();
  assert.equal(c[kind + "FeedbackDelay"].starts, 1, "unrelated updates do not restart the delay");
  c[kind + "Running"] = false;
  c.updateBusyFeedback();
  assert.equal(c[kind + "FeedbackDelay"].running, false);
  assert.equal(c[kind + "BusyVisible"], false);
  c[kind + "Running"] = true;
  c.updateBusyFeedback();
  c.fire(kind);
  assert.equal(c[kind + "BusyVisible"], true, "longer queries show feedback after the delay");
  c.opened = false;
  c.updateBusyFeedback();
  assert.equal(c[kind + "BusyVisible"], false, "closing clears feedback");
}
{
  const c = motion();
  c.scheduleViewReveal(); c.scheduleViewReveal(); c.flush();
  assert.equal(c.dataReveal.starts, 1, "one reveal after the whole view is injected");
  c.scheduleViewReveal(); c.flush();
  assert.equal(c.dataReveal.starts, 1, "same-view refreshes never replay the reveal");
  c.viewKey = "month:0"; c.viewReady = false;
  c.scheduleViewReveal(); c.flush();
  assert.equal(c.dataReveal.starts, 1, "uncached navigation waits for data");
  c.viewReady = true; c.scheduleViewReveal(); c.flush();
  assert.equal(c.dataReveal.starts, 2);
  c.viewKey = "month:0:editor"; c.viewReady = false;
  c.scheduleViewReveal(); c.flush();
  c.viewReady = true; c.scheduleViewReveal(); c.flush();
  assert.equal(c.dataReveal.starts, 3, "activity details reveal once when available");
  c.motionDuration = 0; c.body.opacity = 0.88; c.viewKey = "week:0";
  c.scheduleViewReveal(); c.flush();
  assert.equal(c.dataReveal.starts, 3, "reduced motion does not animate");
  assert.equal(c.body.opacity, 1);
  c.opened = false; c.viewKey = "day:-1";
  c.scheduleViewReveal(); c.flush();
  assert.equal(c.reveals, 4, "hidden views do not animate");
}
{
  const source = panel.slice(panel.indexOf("  component MonthRhythm:"), panel.indexOf("  component ChartReadout:"));
  const c = vm.createContext({
    root: { opened: true, motionDuration: 150 }, visible: true, revealProgress: 1,
    monthRhythmReveal: { starts: 0, restart() { this.starts++; }, stop() {} },
  });
  for (const fn of source.matchAll(/^    function (?:restartReveal|settleReveal)\([^\n]*\) \{[\s\S]*?^    \}/gm)) vm.runInContext(fn[0], c);
  c.restartReveal();
  assert.equal(c.monthRhythmReveal.starts, 1);
  c.root.motionDuration = 0; c.revealProgress = 0.2;
  c.restartReveal();
  assert.equal(c.monthRhythmReveal.starts, 1);
  assert.equal(c.revealProgress, 1);
  assert.doesNotMatch(source, /on(?:Cells|MaxSeconds)Changed: restartReveal/);
}
console.log("Delayed feedback, navigation-only reveals, and reduced-motion checks passed");
