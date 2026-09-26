import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";

const widget = readFileSync(new URL("../omarchy/nagori/BarWidget.qml", import.meta.url), "utf8");
const model = readFileSync(new URL("../omarchy/nagori/Model.js", import.meta.url), "utf8");

// Execute the actual controller functions with process and shell UI boundaries stubbed.
function controller() {
  const Model = vm.createContext({});
  vm.runInContext(model, Model);
  const context = vm.createContext({
    Model,
    Qt: { callLater() {}, formatTime: () => "12:00:00" },
    reportProcess: { running: false },
    detailProcess: { running: false },
    preloadTimer: { restart() {} },
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

for (const kind of ["app", "domain"]) {
  const c = controller();
  const today = new Date();
  const yesterday = new Date(today.getFullYear(), today.getMonth(), today.getDate() - 1);
  const date = c.Model.dateKey(yesterday);
  c.setPeriod("month", -1);
  c.setActivity(kind, "selected-activity");
  assert.equal(c.openCell({ date }), true);
  assert.equal(c.selectedLens, "day");
  assert.equal(c.selectedOffset, -1);
  assert.equal(c.selectedActivityKind, kind);
  assert.equal(c.selectedActivityKey, "selected-activity");
  deliverDetail(c, detailPayload("stale"));
  assert.equal(c.activityDetail, null);
  deliverDetail(c, JSON.stringify({ activities: [], daily: [], heatmap: [] }));
  assert.equal(c.activityDetail.activities.length, 0);
  assert.equal(c.detailRunning, false);
  assert.equal(c.Model.activityMetricValues(c.activityDetail, c.detailRunning, c.detailError).visits, "0");
  assert.equal(c.openCell({ date: "9999-12-31" }), false);
  assert.equal(c.selectedOffset, -1);
  c.setPeriod("day", 0);
  assert.equal(c.selectedActivityKey, "selected-activity");
  assert.equal(c.selectedOffset, 0);
}
{
  const c = controller();
  // A stale report's today key must not shift date navigation after midnight.
  c.todayKey = "2000-01-01";
  assert.equal(c.openCell({ date: c.Model.dateKey(new Date()) }), true);
  assert.equal(c.selectedOffset, 0);
}
console.log("Chart drilldown controller checks passed");

{
  const c = controller();
  c.refresh(true);
  c.refresh(true);
  assert.equal(c.refreshQueued, false, "opening during the same full query does not queue another");
  deliver(c, payload(c));
  c.preparePanel();
  assert.equal(c.refreshRunning, false, "fresh preloaded report opens without a query");
  c.setPeriod("week", 0);
  deliver(c, payload(c, 1200));
  c.setPeriod("day", 0);
  assert.equal(c.totalFocused, 600);
  assert.equal(c.refreshRunning, false, "returning to a fresh tab uses the cache");
  c.reportsByKey[c.currentKey].updatedAt -= 61000;
  c.preparePanel();
  assert.equal(c.totalFocused, 600, "stale cached report remains visible while refreshing");
  assert.equal(c.refreshRunning, true);
}
{
  const c = controller();
  c.preloadPanel();
  assert.equal(c.loadingFullReport, true);
  c.preparePanel();
  assert.equal(c.refreshQueued, false, "opening joins an ongoing preload");
  deliver(c, payload(c));
  c.preloadPanel();
  assert.equal(c.refreshRunning, false, "a warm cache does not keep preloading");
}
{
  const c = controller();
  c.refresh(true);
  c.setPeriod("week", 0);
  c.setPeriod("day", 0);
  deliver(c, payload(c));
  assert.equal(c.refreshRunning, false, "returning to an in-flight period cancels obsolete queued work");
}
{
  const c = controller();
  c.setActivity("app", "editor");
  c.refreshDetail(false);
  assert.equal(c.detailQueued, false, "report application does not duplicate an activity request");
  c.setActivity("domain", "example.com");
  const editorKey = c.loadingDetailKey;
  deliverDetail(c, detailPayload("editor"));
  assert.ok(c.detailCache[editorKey], "superseded successful results still warm the cache");
  deliverDetail(c, detailPayload("example.com"));
  c.setActivity("app", "editor");
  assert.equal(c.activityDetail.activities[0].key, "editor");
  assert.equal(c.detailRunning, false, "fresh activity detail requires no second query");
  c.detailCache[c.detailKey()].at -= 61000;
  c.refreshDetail(false);
  assert.equal(c.detailRunning, true, "live activity detail refreshes after its freshness window");
  assert.equal(c.detailProcess.command[1], "-c", "activity queries avoid login-shell startup");
}
{
  const c = controller();
  c.setActivity("app", "editor");
  c.loadingDetailDate = "2000-01-01";
  deliverDetail(c, detailPayload("editor"));
  assert.equal(Object.keys(c.detailCache).length, 0, "midnight-crossing detail is not cached under yesterday's key");
  assert.equal(c.activityDetail, null);
}
console.log("Cache reuse, preloading, and request deduplication checks passed");
{
  const c = controller();
  c.refresh(true);
  deliver(c, payload(c));
  c.setPeriod("week", 0);
  c.setPeriod("month", 0);
  assert.equal(c.refreshQueued, true);
  c.setPeriod("day", 0);
  assert.equal(c.refreshQueued, false, "returning to a cached tab cancels queued navigation");
  deliver(c, payload(c, 1800));
  assert.equal(c.refreshRunning, false);
  assert.equal(c.totalFocused, 600, "background result cannot replace the cached selected tab");
}

for (const offset of [0, -1]) {
  const c = controller();
  c.selectedOffset = offset;
  c.refresh(true);
  deliver(c, payload(c));
  c.reportsByKey[c.currentKey].updatedAt -= 600000;
  c.setPeriod("week", offset);
  deliver(c, payload(c, 1200));
  c.setPeriod("day", offset);
  assert.equal(c.totalFocused, 600, "expired retained report appears immediately");
  assert.equal(c.panelDataLoaded, true);
  assert.equal(c.refreshRunning, true, "expired historical and live reports refresh");
  deliver(c, "bad JSON");
  assert.equal(c.totalFocused, 600, "refresh error retains expired content");
  c.preparePanel();
  deliver(c, payload(c, 900));
  c.refresh(true);
  assert.equal(c.refreshRunning, true, "explicit refresh bypasses a fresh cache");
}
{
  const c = controller();
  c.setActivity("app", "editor");
  deliverDetail(c, detailPayload("editor"));
  c.detailCache[c.detailKey()].at -= 600000;
  c.setActivity("app", "other");
  deliverDetail(c, detailPayload("other"));
  c.setActivity("app", "editor");
  assert.equal(c.activityDetail.activities[0].key, "editor", "expired detail appears immediately");
  assert.equal(c.detailRunning, true);
  deliverDetail(c, "bad JSON", 1);
  assert.equal(c.activityDetail.activities[0].key, "editor");
  assert.notEqual(c.detailError, "");
}
{
  const c = controller();
  for (let n = 0; n < 15; n++) {
    c.cacheReport("day:" + -n, { todayKey: c.Model.dateKey(new Date()) });
  }
  assert.equal(Object.keys(c.reportsByKey).length, 12, "retained cache stays bounded");
}
{
  const c = controller();
  const target = { hostWidget: null, rows: [], totalFocused: 0, refreshRunning: false };
  c.panelLoader.item = target;
  c.refresh(true);
  deliver(c, payload(c));
  c.injectPanel();
  assert.equal(target.hostWidget.refresh, c.refresh, "closed panel still receives its host");
  assert.equal(target.totalFocused, 0, "hidden report injection is deferred");
  target.open = () => {
    assert.equal(target.totalFocused, 600, "data is injected before opening");
    c.opened = true;
  };
  c.open();
  let rows = target.rows;
  let writes = 0;
  Object.defineProperty(target, "rows", { get: () => rows, set: value => { rows = value; writes++; } });
  c.refreshRunning = true;
  c.injectPanel();
  assert.equal(target.refreshRunning, true);
  assert.equal(writes, 0, "status changes do not reassign the report model");
}
for (const exitFirst of [false, true]) {
  const c = controller();
  c.refresh(true);
  c.setPeriod("week", 0);
  c.setPeriod("month", 0);
  deliver(c, payload(c), 0, exitFirst);
  assert.equal(c.loadingLens, "month", "only the latest queued navigation runs");
  deliver(c, payload(c, 1800), 0, exitFirst);
  assert.equal(c.totalFocused, 1800);
  assert.equal(c.refreshRunning, false);
}
console.log("Stale retention, bounded caches, hidden injection, and navigation checks passed");
{
  const c = controller();
  c.refresh(true);
  deliver(c, payload(c));
  c.todayKey = "2000-01-01";
  c.reportsByKey[c.currentKey].report.todayKey = c.todayKey;
  c.activityDetail = { activities: [{ key: "yesterday" }] };
  c.preparePanel();
  assert.equal(c.panelDataLoaded, false, "yesterday's relative period is not shown on reopening");
  assert.equal(c.activityDetail, null);
  assert.equal(c.refreshRunning, true);
}
{
  const panel = readFileSync(new URL("../omarchy/nagori/Panel.qml", import.meta.url), "utf8");
  const handlers = panel.match(/  IpcHandler \{[\s\S]*?\n  \}/)[0];
  const calls = [];
  const hostWidget = { open: () => calls.push("open"), togglePanel: () => calls.push("toggle"), statusText: "current" };
  const context = vm.createContext({ root: { hostWidget, statusText: "old" } });
  for (const name of ["open", "show", "toggle", "status"]) {
    const source = handlers.match(new RegExp("function " + name + "\\(\\)(?:: string)? \\{[^\\n]+\\}"))[0].replace(": string", "");
    vm.runInContext(source, context);
  }
  context.open(); context.show(); context.toggle();
  assert.deepEqual(calls, ["open", "open", "toggle"], "IPC uses the controller's prepare-before-open path");
  assert.equal(context.status(), "current", "hidden panel status comes from the controller");
}
{
  const c = controller();
  c.selectedActivityKind = "app";
  c.selectedActivityKey = "editor";
  c.activityDetail = { activities: [{ key: "yesterday" }] };
  c.refreshDetail(false);
  assert.equal(c.activityDetail, null, "detail from another date cannot survive a refresh without a matching cache key");
}

{
  const c=controller();
  c.moduleName='local.nagori';
  c.settings={refreshIntervalSec:90,panelWidth:1200,dynamicIslandStyle:true};
  const writes=[];
  c.bar={shell:{updateEntryInline(id,entry) { writes.push({id,entry}); }}};
  c.setAppearanceSetting('richGraphs',false);
  assert.equal(c.settings.richGraphs,false);
  assert.equal(c.settings.refreshIntervalSec,90);
  assert.equal(c.settings.dynamicIslandStyle,true);
  assert.equal(writes[0].id,'local.nagori');
  c.setAppearanceSetting('reduceMotion',true);
  assert.equal(c.settings.reduceMotion,true);
  c.setAppearanceSetting('unrecognized',true);
  assert.equal(writes.length,2,'only supported appearance settings are persisted');
  for (const width of [760,1160,2000]) {
    c.setAppearanceSetting('panelWidth',width);
    assert.equal(c.settings.panelWidth,width,'width is persisted as a number');
    assert.equal(c.settings.refreshIntervalSec,90);
    assert.equal(c.settings.reduceMotion,true);
  }
  c.setAppearanceSetting('panelWidth',true);
  c.setAppearanceSetting('panelWidth',99999);
  assert.equal(c.settings.panelWidth,2000);
  assert.equal(writes.length,5,'invalid widths are not persisted');
}
console.log('Appearance persistence preserves unrelated widget settings');
