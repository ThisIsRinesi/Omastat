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
    date: `2026-04-${String(index + 1).padStart(2, "0")}`,
    focused_seconds: 60,
    observed_seconds: 120,
  })),
  "life",
);
assert.equal(lifeBuckets.length, 13);
