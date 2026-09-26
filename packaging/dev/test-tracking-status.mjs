import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";

const source = readFileSync(new URL("../omarchy/nagori/TrackingStatus.qml", import.meta.url), "utf8");
const model = vm.createContext({});
vm.runInContext(readFileSync(new URL("../omarchy/nagori/Model.js", import.meta.url), "utf8"), model);

function controller() {
  const context = vm.createContext({ checking: false, report: null, errorText: "", statusProcess: { running: false } });
  for (const fn of source.matchAll(/^  function \w+\([^\n]*\) \{[\s\S]*?^  \}/gm)) vm.runInContext(fn[0], context);
  return context;
}
const report = { tracker: "reporting", browser: "stale", checked_at: 2000, last_heartbeat_at: 1999, last_browser_report_at: 1000 };
for (const exitFirst of [true, false]) {
  const c = controller();
  c.refresh();
  c.output = JSON.stringify(report);
  c.exitCode = 0;
  c[exitFirst ? "exitReady" : "outputReady"] = true;
  c.finish();
  assert.equal(c.checking, true, "wait for both process exit and complete output");
  c[exitFirst ? "outputReady" : "exitReady"] = true;
  c.finish();
  assert.equal(c.report.tracker, "reporting");
  assert.equal(c.checking, false);
  c.refresh();
  c.output = "partial";
  c.refresh();
  assert.equal(c.output, "partial", "do not restart an in-flight check");
  c.outputReady = c.exitReady = true;
  c.finish();
  assert.equal(c.report, null, "invalid output must clear an old healthy status");
  assert.match(c.errorText, /unavailable/);
  c.refresh();
  c.output = JSON.stringify(report);
  c.exitCode = 1;
  c.outputReady = c.exitReady = true;
  c.finish();
  assert.equal(c.report, null, "failed commands must not display healthy status");
  assert.match(c.errorText, /doctor/);
}
assert.match(model.trackingStatusView(report).browserDetail, /may be closed/);
assert.equal(model.trackingStatusView({ browser: "disabled" }).browser, "Disabled in config");
assert.match(model.trackingStatusView({ tracker: "idle" }).trackerDetail, /not counted/);
assert.equal(model.trackingStatusView(null).tracker, "Unknown");
console.log("Tracking status completion ordering, failures, and reporting labels passed");
