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
