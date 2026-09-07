import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import vm from "node:vm";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const modelPath = resolve(scriptDir, "../omarchy/omastat/Model.js");
const source = readFileSync(modelPath, "utf8");
const context = vm.createContext({});

vm.runInContext(source, context, { filename: modelPath });

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
assert.equal(yearBuckets[0].label, "Jan 2026");
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
assert.match(routineEvidence, /Days we could compare: Sep 1, 2026; Sep 2, 2026/);
assert.match(routineEvidence, /Days it happened: Sep 1, 2026/);
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
assert.equal(context.insightQualifier(habit), 'Early hint');
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

for (const [lens, title] of [["week", "Daily time"], ["month", "Daily time"], ["year", "Monthly time"], ["life", "Weekly time"]]) {
  assert.equal(context.trendTitle(lens), title);
}
assert.equal(context.clockLabel(0), "00:00");
assert.equal(context.clockLabel(6), "06:00");
assert.equal(context.clockLabel(18), "18:00");
assert.equal(context.insightExplanation({detail: "Older insight"}), "Older insight");
assert.equal(context.insightExplanation({}), "");

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
