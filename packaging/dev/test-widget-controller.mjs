import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";

const widget = readFileSync(new URL("../omarchy/omastat/BarWidget.qml", import.meta.url), "utf8");
const model = readFileSync(new URL("../omarchy/omastat/Model.js", import.meta.url), "utf8");

// Execute the actual controller functions with process and shell UI boundaries stubbed.
function controller() {
  const Model = vm.createContext({});
  vm.runInContext(model, Model);
  const context = vm.createContext({
    Model,
    Qt: { callLater() {}, formatTime: () => "12:00:00" },
    reportProcess: { running: false },
    detailProcess: { running: false },
    panelLoader: { item: null },
    opened: false,
    glyph: "timer",
    cacheMaxEntries: 12,
    fullReportTtlMs: 300000,
    summaryTtlMs: 60000,
  });
  context.root = context;
  for (const property of widget.matchAll(/^  property (?:string|bool|int|real|var) (\w+): (.+)$/gm)) {
    context[property[1]] = vm.runInContext(property[2], context);
  }
  for (const fn of widget.matchAll(/^  function \w+\([^\n]*\) \{[\s\S]*?^  \}/gm)) {
    vm.runInContext(fn[0], context);
  }
  Object.defineProperty(context, "currentKey", {
    get: () => context.reportKey(context.selectedLens, context.selectedOffset),
  });
  return context;
}

function payload(c, seconds = 600) {
  return JSON.stringify({
    today_key: c.Model.dateKey(new Date()),
    lens: "day",
    period: { label: "Today" },
    total_focused_seconds: seconds,
    rows: [{ app_class: "editor", focused_seconds: seconds }],
  });
}

function deliver(c, output, code = 0, exitFirst = false) {
  const stdout = () => { c.reportOutput = output; c.reportOutputReady = true; c.finishReport(); };
  const exit = () => { c.reportExitCode = code; c.reportExitReady = true; c.finishReport(); };
  if (exitFirst) { exit(); stdout(); }
  else { stdout(); exit(); }
}

for (const exitFirst of [false, true]) {
  const c = controller();
  c.refresh(true);
  c.setPeriod("week", -1);
  deliver(c, payload(c), 0, exitFirst);
  assert.equal(c.loadingLens, "week");
  assert.equal(c.loadingOffset, -1);
  assert.equal(c.totalFocused, 0, "old response must not populate the selected week");
  assert.equal(c.refreshRunning, true, "queued period starts after both completion signals");
  assert.ok(c.cachedReport("day:0"));
}

for (const output of ["", "not JSON", "{}", "null", '{"total_focused_seconds":"600"}']) {
  const c = controller();
  c.refresh(true);
  deliver(c, payload(c));
  c.refresh(true);
  deliver(c, output);
  assert.equal(c.totalFocused, 600, "failed refresh preserves the displayed report");
  assert.equal(c.panelDataLoaded, true);
  assert.notEqual(c.errorText, "");
  assert.equal(c.cachedReport("day:0").totalFocused, 600);
}

{
  const c = controller();
  c.refresh(true);
  deliver(c, payload(c), 1);
  assert.equal(c.cachedReport("day:0"), null, "nonzero exit must not cache stdout");
  assert.notEqual(c.errorText, "");
  assert.equal(c.refreshRunning, false);
}

{
  const c = controller();
  c.refresh(true);
  c.selectedLens = "week";
  deliver(c, "bad JSON");
  assert.equal(c.errorText, "", "superseded failures must not affect the selected period");
}

{
  const c = controller();
  c.cacheReport("day:-1", { todayKey: "2000-01-01" });
  c.cacheSummary("week:-1", { todayKey: "2000-01-01" });
  assert.equal(c.cachedReport("day:-1"), null);
  assert.equal(c.cachedSummary("week:-1"), null);
  c.refresh(true);
  c.loadingDate = "2000-01-01";
  deliver(c, payload(c));
  assert.equal(c.totalFocused, 0);
  assert.equal(c.refreshRunning, true, "midnight crossing triggers a fresh report");
  assert.equal(c.loadingDate, c.Model.dateKey(new Date()));
}

{
  const c = controller();
  c.refresh(true);
  deliver(c, payload(c));
  c.refresh(true);
  deliver(c, "bad JSON");
  c.refresh(true);
  deliver(c, payload(c, 900));
  assert.equal(c.totalFocused, 900);
  assert.equal(c.errorText, "", "successful retry clears the error");
}

console.log("Widget controller regression checks passed");

function detailPayload(key) {
  return JSON.stringify({ activities: [{ kind: "app", key, focused_seconds: 600 }], daily: [], heatmap: [], insights: [] });
}
function deliverDetail(c, output, code = 0, exitFirst = false) {
  const stdout = () => { c.detailOutput = output; c.detailOutputReady = true; c.finishDetail(); };
  const exit = () => { c.detailExitCode = code; c.detailExitReady = true; c.finishDetail(); };
  if (exitFirst) { exit(); stdout(); } else { stdout(); exit(); }
}
for (const exitFirst of [false, true]) {
  const c = controller();
  c.setActivity("app", "editor");
  c.setActivity("domain", "example.com");
  deliverDetail(c, detailPayload("editor"), 0, exitFirst);
  assert.equal(c.activityDetail, null, "superseded activity cannot populate the selection");
  assert.equal(c.detailRunning, true, "latest selection is queued");
  deliverDetail(c, detailPayload("example.com"), 0, exitFirst);
  assert.equal(c.activityDetail.activities[0].key, "example.com");
  c.refreshDetail();
  deliverDetail(c, "not json");
  assert.equal(c.activityDetail.activities[0].key, "example.com", "refresh failure preserves detail");
  assert.ok(c.detailError);
  c.refreshDetail();
  deliverDetail(c, detailPayload("example.com"));
  assert.equal(c.detailError, "");
  c.setActivity("app", "literal'$(touch /tmp/never)`name`");
  assert.equal(c.detailProcess.command.at(-1), "literal'$(touch /tmp/never)`name`", "activity identifiers are process arguments, never shell code");
  c.setActivity("", "");
  deliverDetail(c, detailPayload("old"));
  assert.equal(c.activityDetail, null, "returning to all activity discards outstanding detail");
}
{
  const c = controller();
  c.setActivity("app", "editor");
  c.loadingDetailKey = "yesterday";
  deliverDetail(c, detailPayload("editor"));
  assert.equal(c.activityDetail, null, "results from a prior date are discarded");
}
console.log("Activity detail controller checks passed");
