import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import vm from "node:vm";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const modelPath = resolve(scriptDir, "../omarchy/nagori/Model.js");
const source = readFileSync(modelPath, "utf8");
const context = vm.createContext({});

vm.runInContext(source, context, { filename: modelPath });
vm.runInContext(readFileSync(resolve(scriptDir, "../omarchy/nagori/InsightCopy.js"), "utf8"), context);

assert.equal(context.fmt(0), "0s");
assert.equal(context.fmt(65), "1m");
assert.equal(context.fmt(7200), "2h");
assert.equal(context.percent(0.624), "62%");

const grouped = context.groupedApps(
  [
    { app: "A", seconds: 50, open_seconds: 60 },
    { app: "B", seconds: 40, open_seconds: 50 },
    { app: "C", seconds: 30, open_seconds: 40 },
    { app: "D", seconds: 20, open_seconds: 30 },
  ],
  3,
);
assert.equal(grouped.length, 3);
assert.equal(grouped[2].app, "Other");
assert.equal(grouped[2].seconds, 50);
assert.equal(grouped[2].pct, 36);

const categories = context.categoryBreakdown(
  [
    { app: "Code", category: "productive", seconds: 3600 },
    { app: "Browser", category: "research", seconds: 1800 },
    { app: "Chat", category: "productive", seconds: 900 },
    { app: "Other", category: "mixed", seconds: 300 },
  ],
  6600,
);
assert.equal(categories.length, 3);
assert.equal(categories[0].label, "Productive");
assert.equal(categories[0].seconds, 4500);
assert.equal(categories[0].pct, 68);
assert.equal(categories[2].label, "Other");
assert.equal(context.hasMeaningfulCategories(categories), true);
assert.equal(
  context.hasMeaningfulCategories([
    { key: "neutral", seconds: 3600 },
    { key: "mixed", seconds: 300 },
  ]),
  false,
);
const clearPresence = context.presenceSummary(0, 0, 0, 0);
assert.equal(clearPresence.label, "Not counted");
assert.equal(clearPresence.value, "None");
assert.equal(clearPresence.detail, "No away or tracker gaps");
assert.equal(clearPresence.tone, "clear");
const awayPresence = context.presenceSummary(1800, 600, 0, 0);
assert.equal(awayPresence.label, "Away");
assert.equal(awayPresence.value, "30m");
assert.equal(awayPresence.detail, "Not counted 40m");
assert.equal(awayPresence.tone, "away");
const lockedPresence = context.presenceSummary(180, 47520, 0, 0);
assert.equal(lockedPresence.label, "Locked");
assert.equal(lockedPresence.value, "13h 12m");
assert.equal(lockedPresence.detail, "Not counted 13h 15m");
assert.equal(lockedPresence.tone, "away");
const gapPresence = context.presenceSummary(0, 0, 0, 900);
assert.equal(gapPresence.label, "Tracker off");
assert.equal(gapPresence.value, "15m");
assert.equal(gapPresence.detail, "Not counted 15m");
assert.equal(gapPresence.tone, "gap");

const heatmap = context.heatmapCells([
  { weekday: 2, hour: 9, focused_seconds: 1800 },
  { weekday: 2, hour: 10, seconds: 900 },
]);
assert.equal(heatmap.length, 168);
assert.equal(heatmap[2 * 24 + 9].seconds, 1800);
assert.equal(heatmap[2 * 24 + 10].seconds, 900);

const hourly = context.hourlyTrendCells([
  { weekday: 0, hour: 9, focused_seconds: 600 },
  { weekday: 1, hour: 9, focused_seconds: 900 },
]);
assert.equal(hourly.length, 24);
assert.equal(hourly[9].seconds, 1500);
assert.equal(hourly[9].label, "9A");

const month = context.monthCells(
  [
    { date: "2026-08-03", label: "Aug 3", focused_seconds: 3600 },
    { date: "2026-08-04", label: "Aug 4", focused_seconds: 0 },
  ],
  "month",
);
assert.equal(month[0].date, "2026-08-03");
assert.equal(month[0].day, 3);
assert.equal(month[1].seconds, 0);

const monthWeeks = context.monthWeekCells([
  { date: "2026-08-01", label: "Aug 1", focused_seconds: 600, observed_seconds: 1200 },
  { date: "2026-08-02", label: "Aug 2", focused_seconds: 900, observed_seconds: 1800 },
  { date: "2026-08-03", label: "Aug 3", focused_seconds: 300, observed_seconds: 600 },
]);
assert.equal(monthWeeks.length, 2);
assert.equal(monthWeeks[0].label, "Aug 1-2");
assert.equal(monthWeeks[0].seconds, 1500);
assert.equal(monthWeeks[0].activeDays, 2);
assert.equal(monthWeeks[1].label, "Aug 3");

const weekdays = context.weekdayFocusCells([
  { weekday: 0, hour: 9, focused_seconds: 600 },
  { weekday: 0, hour: 10, focused_seconds: 900 },
  { weekday: 6, hour: 22, focused_seconds: 300 },
]);
assert.equal(weekdays.length, 7);
assert.equal(weekdays[0].label, "Mon");
assert.equal(weekdays[0].seconds, 1500);
assert.equal(weekdays[6].seconds, 300);

const browserRows = context.browserActivity(
  [
    { label: "GitHub", seconds: 1200, pct: 40 },
    { label: "YouTube", seconds: 2400, pct: 60 },
  ],
  1,
);
assert.equal(browserRows.length, 1);
assert.equal(browserRows[0].label, "YouTube");

const trend = context.trendDays(
  [
    {
      date: "2026-08-03",
      label: "Aug 3",
      focused_seconds: 3600,
      elapsed_seconds: 7200,
      observed_seconds: 5400,
      idle_seconds: 900,
      unobserved_seconds: 1800,
    },
  ],
  "2026-08-03",
  "day",
);
assert.equal(trend[0].observed_seconds, 5400);
assert.equal(trend[0].densityText, "67%");
assert.equal(context.trendAverageText(trend), "avg 1h  1/1 active");

const yearBuckets = context.activityCells(
  [
    { date: "2026-01-02", focused_seconds: 600, observed_seconds: 1200 },
    { date: "2026-01-03", focused_seconds: 900, observed_seconds: 1800 },
    { date: "2026-02-01", focused_seconds: 300, observed_seconds: 900 },
  ],
  "year",
);
assert.equal(yearBuckets.length, 2);
assert.equal(yearBuckets[0].weekly, true);
assert.equal(yearBuckets[0].label, "2026-01-02 - 2026-01-03");
assert.equal(yearBuckets[0].seconds, 1500);

const lifeBuckets = context.activityCells(
  Array.from({ length: 98 }, (_, index) => ({
    date: context.dateKey(new Date(2026, 3, index + 1)),
    focused_seconds: 60,
    observed_seconds: 120,
  })),
  "life",
);
assert.equal(lifeBuckets.length, 13);

assert.equal(context.dayOffsetFromToday("2026-08-12", "2026-08-15"), -3);
assert.equal(context.dayOffsetFromToday("2026-08-15", "2026-08-15"), 0);
assert.equal(context.dayOffsetFromToday("bad", "2026-08-15"), null);

const retro = context.yearRetroFacts(
  [
    { date: "2026-01-05", label: "Jan 5", focused_seconds: 3600 },
    { date: "2026-01-06", label: "Jan 6", focused_seconds: 7200 },
    { date: "2026-02-02", label: "Feb 2", focused_seconds: 1800 },
  ],
  12600,
  "2026",
);
assert.equal(retro.length, 6);
assert.deepEqual(
  Array.from(retro, (item) => item.label),
  ["Active days", "Longest streak", "Top month", "Weekday rhythm", "Peak day", "Daily pace"],
);
assert.equal(retro[0].value, "3 / 3");
assert.equal(retro[2].value, "Jan");
assert.equal(retro[4].value, "Jan 6");

const varied = context.diverseInsights([
  {kind: "app-routine", supporting: {activity_key: "game"}, value: "Monday"},
  {kind: "app-routine", supporting: {activity_key: "game"}, value: "Friday"},
  {kind: "app-routine", supporting: {activity_key: "browser"}, value: "Tuesday"},
]);
assert.deepEqual(Array.from(varied, x => x.value), ["Monday", "Tuesday", "Friday"]);

const routineEvidence = context.insightEvidence({
  explanation: "Recent evening use.", confidence: "medium", evidence: { data_points: 7 },
  supporting: { occurrence_count: 5, eligible_count: 7, matching_dates: ["2026-09-01"],
    routine: { status: "recent", cadence: "everyday", eligible_dates: ["2026-09-01", "2026-09-02"] } }
});
assert.match(routineEvidence, /5 of 7 days with enough tracking/);
assert.match(routineEvidence, /at least 90% tracking/);
assert.doesNotMatch(routineEvidence, /Days we could compare|Days it happened/);
assert.match(routineEvidence, /Some supporting history/);
assert.equal(context.insightExplanation({ explanation: "Legacy fact" }), "Legacy fact");
assert.equal(context.insightEvidence({ explanation: "Legacy fact" }), "");

assert.equal(context.insightDateRange("2026-08-22", "2026-09-04"), "Aug 22–Sep 4, 2026");
assert.equal(context.insightDateRange("2025-12-22", "2026-01-04"), "Dec 22, 2025–Jan 4, 2026");
assert.equal(context.insightDate("not a date"), "not a date");

// Recording quality belongs in data details, even when the full habits list opens.
const habit = { kind: 'app-routine', category: 'patterns', title: 'Slay the Spire 2 most evenings', confidence: 'low', explanation: 'Original evidence stays intact.', supporting: { activity_key: 'Slay the Spire 2', matching_dates: ['2026-09-01'] } };
const visibleHabits = context.widgetInsights([
  { kind: 'unobserved-anomaly', category: 'patterns' },
  { kind: 'idle-excluded' },
  { kind: 'future-diagnostic', category: 'system-signals' },
  habit,
  { kind: 'top-app', category: 'apps', supporting: { activity_key: 'Browser' } },
]);
assert.equal(visibleHabits.length, 2);
assert.equal(visibleHabits[0], habit);
assert.equal(context.insightQualifier(habit), 'Recent pattern');
assert.equal(context.insightQualifier({ confidence: 'high' }), '');
assert.match(context.insightExplanation(habit), /Original evidence stays intact/);
assert.doesNotMatch(context.insightEvidence(habit), /Original evidence stays intact/);
assert.match(context.insightEvidence(habit), /Sep 1, 2026/);
assert.equal(context.widgetInsights(null).length, 0);
assert.equal(context.visitSummary({ days_used: 1, visits: 1 }), '1 day · 1 visit');
assert.equal(context.visitSummary({ days_used: 2, visits: 3 }), '2 days · 3 visits');
assert.match(context.trendDefaultText([{seconds: 3600, label: 'Mon'}], 'day'), /Daily average 1h/);
assert.match(context.trendDefaultText([{seconds: 3600, label: 'Jan'}], 'month'), /Monthly average 1h/);
assert.match(context.trendDefaultText([{seconds: 3600, label: 'Week 1'}], 'week'), /Weekly average 1h/);

for (const [lens, title] of [["week", "Daily time"], ["month", "Daily time"], ["year", "Weekly time"], ["life", "Weekly time"]]) {
  assert.equal(context.trendTitle(lens), title);
}
assert.equal(context.clockLabel(0), "00:00");
assert.equal(context.clockLabel(6), "06:00");
assert.equal(context.clockLabel(18), "18:00");
assert.equal(context.insightExplanation({detail: "Older insight"}), "Older insight");
assert.equal(context.insightExplanation({}), "");

const visualHabit = {
  kind: 'app-routine', category: 'patterns', value: 'Original value',
  supporting: {
    hour_label: '8–10 PM', occurrence_count: 2, eligible_count: 3, weekday: 0,
    activity_kind: 'domain', activity_key: 'example.com', app_label: 'Example',
    matching_dates: ['2026-09-07', '2026-08-24'],
    routine: { status: 'recent', eligible_dates: ['2026-09-07', '2026-08-24', '2026-08-31'] }
  }
};
const originalHabit = JSON.stringify(visualHabit);
assert.equal(context.insightPresentation(visualHabit).value, '8–10 PM');
assert.equal(context.insightPresentation(visualHabit).frequency, '67% of tracked days');
assert.equal(context.insightPresentation(visualHabit).activityKind, 'domain');
assert.equal(context.insightQualifier(visualHabit), 'Recent pattern · Last two weeks');
assert.equal(JSON.stringify(context.insightDays(visualHabit)), JSON.stringify([
  { date: '2026-08-24', matched: true },
  { date: '2026-08-31', matched: false },
  { date: '2026-09-07', matched: true }
]));
assert.equal(JSON.stringify(visualHabit), originalHabit);
assert.equal(context.insightDays({}).length, 0);
assert.equal(context.insightPresentation({value: 'Legacy fact'}).value, 'Legacy fact');
assert.equal(context.insightPresentation({}).frequency, '');

assert.deepEqual(Array.from(context.durationLegend(7200)), ["0s", "2h"]);
assert.deepEqual(Array.from(context.durationLegend(0)), ["0s", "0s"]);
assert.deepEqual(Array.from(context.durationLegend(90)), ["0s", "1m"]);
assert.deepEqual(Array.from(context.durationLegend(undefined)), ["0s", "0s"]);

assert.equal(context.fittingInsightCount([100, 100, 100, 100, 100, 100, 100], 650, 8), 6);
assert.equal(context.fittingInsightCount([200, 200, 200], 430, 8), 2);
assert.equal(context.fittingInsightCount([100], 20, 8), 1);
assert.equal(context.fittingInsightCount([], 650, 8), 0);

// Calendar navigation uses real local dates, including DST and year boundaries.
for (const [cell, today, lens, offset] of [
  [{ date: "2026-03-08" }, "2026-03-09", "day", -1],
  [{ key: "2026-11-01" }, "2026-11-02", "day", -1],
  [{ date: "2024-02-29", seconds: 0 }, "2024-03-01", "day", -1],
  [{ date: "2025-12-01", monthly: true }, "2026-01-15", "month", -1],
  [{ date: "2025-12-29", weekly: true }, "2026-01-04", "week", 0],
  [{ date: "2025-12-29", weekly: true }, "2026-01-05", "week", -1],
  [{ date: "2026-09-07" }, "2026-09-07", "day", 0],
]) {
  const target = context.cellDestination(cell, today);
  assert.equal(target.lens, lens);
  assert.equal(target.offset, offset);
}
for (const cell of [null, {}, { blank: true }, { date: "2026-02-30" }, { date: "2026-13-01" }, { date: "2026-9-01" }, { date: "2026-09-08" }]) {
  assert.equal(context.cellDestination(cell, "2026-09-07"), null);
}
assert.equal(context.cellDestination({ date: "2026-09-07" }, "invalid"), null);
const calendarWeeks = context.weekCells([
  { date: "2026-04-01", focused_seconds: 60 },
  { date: "2026-04-05", focused_seconds: 120 },
  { date: "2026-04-06", focused_seconds: 30 },
]);
assert.equal(calendarWeeks.length, 2);
assert.equal(calendarWeeks[0].date, "2026-03-30");
assert.equal(calendarWeeks[0].seconds, 180);
assert.equal(calendarWeeks[0].label, "2026-04-01 - 2026-04-05");
assert.equal(calendarWeeks[1].date, "2026-04-06");
assert.equal(context.activityMetricValues(null, true, "").time, "…");
assert.equal(context.activityMetricValues(null, false, "Failed").time, "—");
const emptyMetrics = context.activityMetricValues({ activities: [] }, false, "");
assert.equal(emptyMetrics.time, "0s");
assert.equal(emptyMetrics.days, "0");
assert.equal(emptyMetrics.visits, "0");
assert.equal(emptyMetrics.typical, "—");
assert.equal(context.activityMetricValues({ activities: [{ visits: 0, median_visit_seconds: 0 }] }, false, "").typical, "—");
console.log("Calendar navigation and empty activity checks passed");
assert.equal(context.chartDateLabel({ key: '2026-09-08' }), 'Tue, Sep 8');
assert.equal(context.chartDateLabel({ date: '2026-09-01', monthly: true, label: 'Sep 2026' }), 'Sep 2026');
assert.equal(context.chartDateLabel({ date: '2026-09-07', weekly: true, label: 'Sep 7–13' }), 'Sep 7–13');
assert.equal(context.chartDateLabel(null), '');
assert.equal(context.chartDateLabel({ label: 'Earlier history' }), 'Earlier history');
const overnightWindow = context.routineWindows({ supporting: { routine: { start_minute: 1380, end_minute: 60 } } });
assert.equal(overnightWindow.length, 2);
assert.equal(overnightWindow[0].start, 23 / 24);
assert.equal(overnightWindow[1].start, 0);
assert.equal(overnightWindow[0].width + overnightWindow[1].width, 2 / 24);
assert.equal(context.routineWindows({ supporting: { routine: { start_minute: 480, end_minute: 600 } } })[0].width, 2 / 24);
assert.equal(context.routineWindows({ supporting: { routine: {} } }).length, 0);
assert.equal(context.routineWindows({ supporting: { routine: { start_minute: 60, end_minute: 60 } } }).length, 0);
assert.equal(context.routineWindows({}).length, 0);

const concurrent = context.withMultitaskingDays([
  {date:'2026-09-14', focused_seconds:3600},
  {date:'2026-09-15', focused_seconds:1800}
], [{date:'2026-09-14',seconds:1200},{date:'2026-09-15',seconds:2400}]);
assert.equal(concurrent[0].focused_seconds,3600);
assert.equal(concurrent[1].multitasked_seconds,1800);
assert.equal(context.weekCells(concurrent)[0].multitasked_seconds,3000);
assert.equal(context.trendDays(concurrent,'','week')[0].multitasked_seconds,1200);
assert.match(context.trendDetailText(context.trendDays(concurrent,'','week')[0]),/20m multitasked/);
assert.equal(context.withMultitaskingHours([{seconds:3600}], [{hour:0,focused_seconds:900}])[0].multitasked_seconds,900);
console.log('Multitasking chart overlays and weekly aggregation checks passed');

function cubic(s, t) {
  const u = 1 - t;
  return u*u*u*s.from + 3*u*u*t*s.c1 + 3*u*t*t*s.c2 + t*t*t*s.to;
}
for (const values of [[], [50], [0,0,0], [0,100,0,200,10], [100,101,10000,0], [50,40,30,20]]) {
  const curves = context.smoothChartSegments(values.map(seconds => ({seconds})), 'seconds');
  assert.equal(curves.length, Math.max(0,values.length-1));
  curves.forEach((s,i) => {
    assert.equal(s.from,values[i]); assert.equal(s.to,values[i+1]);
    for(let t=0;t<=1;t+=0.01) {
      const y=cubic(s,t);
      assert.ok(y>=Math.min(s.from,s.to)-1e-9 && y<=Math.max(s.from,s.to)+1e-9,'curve must not invent peaks or negative usage');
    }
  });
}
const layered=[{seconds:100,multitasked_seconds:90},{seconds:10,multitasked_seconds:10},{seconds:100,multitasked_seconds:0}];
const upper=context.smoothChartSegments(layered,'seconds');
const lower=context.smoothChartSegments(layered,'multitasked_seconds',upper);
lower.forEach((s,i) => { for(let t=0;t<=1;t+=0.01) assert.ok(cubic(s,t)<=cubic(upper[i],t)+1e-9); });
console.log('Smooth chart bounds, exact samples, and multitasking containment checks passed');

const timelineSegments = [{start:10,end:20,audio:[]},{start:20,end:30,audio:[{label:"youtube.com"}]},{start:40,end:50,audio:[]}];
assert.equal(context.timelineIndexAt([], 10), -1);
for (const [at, expected] of [[9,-1],[10,0],[19.9,0],[20,1],[30,-1],[39,-1],[40,2],[50,-1]]) {
  assert.equal(context.timelineIndexAt(timelineSegments, at), expected);
}
assert.equal(context.timelineAudioLabel(timelineSegments[1]), "youtube.com");
assert.equal(context.timelineAudioLabel(null), "No background audio detected");
console.log("Timeline boundaries, gaps, and audio readout checks passed");

for (const [seconds, expected] of [[0,"0s"],[65,"1m 5s"],[3600,"1h"],[3661,"1h 1m 1s"],[-1,"0s"],[NaN,"0s"],[Infinity,"0s"]]) {
  assert.equal(context.fmtPrecise(seconds), expected);
}
assert.match(context.trendDetailText({date:"2026-09-14",seconds:3661,multitasked_seconds:65}), /1h 1m 1s focused.*1m 5s multitasked/);
console.log("Precise chart readout checks passed");

// Responsive axes preserve endpoints and never repeat or invent observations.
assert.equal(context.trendAxisTicks([], 1000).length, 0);
assert.deepEqual(Array.from(context.trendAxisTicks([{}], 1000), tick => tick.index), [0]);
for (const length of [2, 7, 30, 53]) {
  for (const width of [0, 180, 380, 760, 1200]) {
    const ticks = Array.from(context.trendAxisTicks(Array(length).fill({}), width));
    const indices = ticks.map(tick => tick.index);
    assert.equal(indices[0], 0);
    assert.equal(indices.at(-1), length - 1);
    assert.equal(new Set(indices).size, indices.length);
    assert.ok(indices.length <= 5);
    assert.ok(ticks.every(tick => tick.count === ticks.length));
  }
}
// Monthly comparisons preserve totals and drill into their actual month.
const comparisonMonths = context.monthBucketCells([
  { date: '2025-12-31', focused_seconds: 60 },
  { date: '2026-01-01', focused_seconds: 120 },
  { date: '2026-01-31', focused_seconds: 180 },
]);
assert.deepEqual(Array.from(comparisonMonths, cell => cell.seconds), [60, 300]);
assert.equal(context.cellDestination(comparisonMonths[0], '2026-02-01').lens, 'month');
assert.equal(context.cellDestination(comparisonMonths[0], '2026-02-01').offset, -2);
assert.equal(context.cellDestination(comparisonMonths[1], '2026-02-01').offset, -1);
console.log('Responsive date axes and monthly comparison drilldown checks passed');

// Day map preserves time gaps and merges audio-only splits of a focused stretch.
const mapStart = new Date(2026, 8, 20).getTime() / 1000;
const mapEnd = new Date(2026, 8, 21).getTime() / 1000;
const mediaSource = { app_class: 'discord', label: 'Discord', attribution: 'app' };
const mapFixture = { start: mapStart, end: mapEnd, segments: [
  { start: mapStart + 3600, end: mapStart + 3900, app_class: 'editor', label: 'Editor', audio: [] },
  { start: mapStart + 3900, end: mapStart + 4500, app_class: 'editor', label: 'Editor', audio: [mediaSource] },
  { start: mapStart + 4500, end: mapStart + 4800, app_class: 'browser', label: 'Browser', audio: [mediaSource] },
  { start: mapStart + 7200, end: mapStart + 7500, app_class: 'editor', label: 'Editor', audio: [] },
] };
const mapped = context.dayMap(mapFixture, true, 6);
assert.equal(mapped.lanes[0].seconds, 1200);
assert.equal(mapped.lanes[0].spans.length, 2, 'a recording gap breaks an app stretch');
assert.equal(mapped.longest.end - mapped.longest.start, 900, 'audio changes do not split a stretch');
assert.equal(mapped.switches, 1, 'switch count excludes recording gaps');
assert.equal(mapped.audio.length, 1, 'continuous audio across app switches is unioned');
assert.equal(mapped.audio[0].end - mapped.audio[0].start, 900);
assert.equal(mapped.start, mapStart + 3600);
assert.equal(mapped.end, mapStart + 10800);
assert.equal(context.timelineIndexAt(mapped.segments, mapStart + 6000), -1);
assert.equal(context.dayMap(mapFixture, false, 6).start, mapStart);
assert.equal(context.dayMap(mapFixture, false, 6).end, mapEnd);
assert.equal(context.dayMap(null, true, 6).lanes.length, 0);
assert.equal(context.dayMap({ start: mapStart, end: mapEnd, segments: [] }, true, 6).lanes.length, 0);
const groupedMap = context.dayMap(mapFixture, true, 1);
assert.equal(groupedMap.lanes.length, 1);
assert.equal(groupedMap.lanes[0].label, 'Other apps');
assert.equal(groupedMap.lanes[0].seconds, 1500);
assert.equal(groupedMap.segments[0].label, 'Editor', 'grouping preserves the true owner for inspection');
const partialMap = context.dayMap({ start: mapStart, end: mapStart + 4000, segments: mapFixture.segments }, true, 6);
assert.equal(partialMap.lanes[0].seconds, 400, 'intervals clip at the recorded boundary');
assert.equal(partialMap.observedEnd, mapStart + 4000);
assert.equal(context.timelineClock(mapStart + 3660), '01:01');
// Run this suite with TZ=America/New_York too: full-day extent is a calendar day,
// not a fixed 24 hours, on daylight-saving transitions.
for (const [year, month, day] of [[2026, 2, 8], [2026, 10, 1]]) {
  const start = new Date(year, month, day).getTime() / 1000;
  const end = new Date(year, month, day + 1).getTime() / 1000;
  const result = context.dayMap({ start, end, segments: [] }, false, 6);
  assert.equal(result.end - result.start, end - start);
}
console.log('Day map gaps, stretches, grouping, audio union, clipping, and local-day boundaries passed');

// Context insights retain their methods and exact comparison dates in the inspector.
const stretchTrend = { kind: 'stretch-trend', supporting: { method: 'Only completed, observed days.', comparison: { current_dates: ['2026-09-20'], baseline_dates: ['2026-09-13'] } } };
assert.equal(context.insightQualifier(stretchTrend), 'Recent change · Completed days');
assert.match(context.insightEvidence(stretchTrend), /Only completed, observed days/);
assert.match(context.insightEvidence(stretchTrend), /Recent days: Sep 20, 2026/);
assert.match(context.insightEvidence(stretchTrend), /Earlier days: Sep 13, 2026/);
assert.equal(context.insightQualifier({kind: 'audio-companion'}), 'This period · Audio overlap');
assert.equal(context.insightQualifier({kind: 'app-handoff'}), 'Recurring sequence · Last 28 completed days');

// Window totals and grouped visits must explain their different duration bases.
for (const basis of ['usage', 'usage-and-visit-start']) {
  const evidence = context.insightEvidence({supporting: {routine: {timing_basis: basis}}});
  assert.match(evidence, /median daily total inside the displayed time window/);
  assert.match(evidence, /matching days with at least five minutes/);
  assert.doesNotMatch(evidence, /per visit/);
}
const visitEvidence = context.insightEvidence({supporting: {routine: {timing_basis: 'visit-start'}}});
assert.match(visitEvidence, /median recorded use per visit/);
assert.match(visitEvidence, /Returns within five minutes/);
assert.match(visitEvidence, /time away is excluded/);

const prediction = {kind: 'upcoming-activity', supporting: {activity_kind: 'app', activity_key: 'spire', app_label: 'Slay the Spire 2', display_until: 300, routine: {status: 'established'}}};
for (const tone of ['warm', 'concise', 'playful']) {
  const title = context.predictionTitle(prediction, tone, '2026-09-25');
  assert.match(title, /Slay the Spire 2/);
  assert.equal(title, context.predictionTitle(prediction, tone, '2026-09-25'));
}
assert.equal(context.visiblePredictions([prediction], 299, 'app', 'spire').length, 1);
assert.equal(context.visiblePredictions([prediction], 300, 'app', 'spire').length, 0);
prediction.supporting.generated_at = 100;
assert.equal(context.visiblePredictions([prediction], 221, 'app', 'spire').length, 0);
assert.equal(context.visiblePredictions([prediction], 299, 'domain', 'spire').length, 0);
prediction.supporting.routine.status = 'recent';
assert.match(context.predictionTitle(prediction, 'warm', '2026-09-25'), /may|might|chance|could/i);
assert.doesNotMatch(context.insightEvidence(prediction), /median recorded use per visit/);
const orderedInsights = context.dashboardInsights([
  {kind: 'app-routine', category: 'patterns', supporting: {activity_kind: 'app', activity_key: 'spire'}},
  {kind: 'audio-companion', category: 'patterns', supporting: {activity_kind: 'app', activity_key: 'music'}},
  {kind: 'stretch-trend', category: 'patterns', supporting: {activity_kind: 'app', activity_key: 'editor'}},
], [prediction]);
assert.equal(orderedInsights.length, 2);
assert.equal(orderedInsights[0].kind, 'stretch-trend');
const routineCard = {kind: 'app-routine', title: 'Spire, most evenings', supporting: {activity_kind: 'app', activity_key: 'spire', app_label: 'Spire'}};
assert.match(context.insightHeading(routineCard, 'warm', '2026-09-25'), /Spire/);
assert.match(context.insightHeading(routineCard, 'concise', '2026-09-25'), /Spire/);
assert.match(context.insightHeading(routineCard, 'playful', '2026-09-25'), /Spire/);

// Daily wording stays stable on refresh and changes on consecutive dates.
for (const tone of ['warm', 'concise', 'playful']) {
  for (const item of [routineCard, prediction]) {
    const a = context.insightHeading(item, tone, '2026-09-25');
    assert.equal(a, context.insightHeading(item, tone, '2026-09-25'));
    assert.notEqual(a, context.insightHeading(item, tone, '2026-09-26'));
  }
}
const typed = {...prediction, generated_at: 1000, display_from: 950, display_until: 1100};
assert.equal(context.visiblePredictions([typed], 949, '', '').length, 0);
assert.equal(context.visiblePredictions([typed], 1000, '', '').length, 1);
assert.equal(context.visiblePredictions([typed], 1100, '', '').length, 0);
assert.equal(context.visiblePredictions([typed], 999, '', '').length, 0);
const third = {...typed, supporting: {...typed.supporting, activity_key: 'third'}};
assert.equal(context.visiblePredictions([typed, typed, third], 1000, 'app', 'third')[0], third);
