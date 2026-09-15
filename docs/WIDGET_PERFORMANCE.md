# Widget loading performance

The September 2026 optimization replaces the browser/focus overlap SQL join
with a per-application event sweep. It reads each attribution once and reuses
focused metadata already loaded for analytics. Attribution precedence,
confirmation expiry, heartbeat cutoffs, and report schemas are unchanged.
Daily aggregation uses binary search to visit only overlapping local days.

The widget retains up to 12 successful reports and 12 activity details in memory.
A stale result for the same local date and selection remains visible while a
refresh runs. Freshness stays at the configured refresh interval for live periods
and five minutes for historical periods. Date rollover invalidates relative
periods. Hidden panels defer report-model updates until opening; status-only
updates do not reassign unchanged report models. Both bar clicks and IPC opening
prepare the data before displaying the panel.

Refresh feedback waits 150 ms before appearing, and the Refresh button reserves
space for its busy label. Navigation reveals wait for the selected report or
activity detail, run once per view, and do not replay on background polling.
The calendar reveal lasts 180 ms. All widget animations respect reduced motion;
turning it on also settles any active content/calendar reveal immediately.

## Measurements

Measurements use release binaries on the same machine, with alternating
before/after order after warming both binaries. Each sample includes CLI startup,
SQLite reads, analytics, and JSON output. These are backend command timings;
they do not measure compositor frame presentation or a cold filesystem cache.

The reference snapshot has 44,954 application intervals and 1,253 browser
attribution intervals. The synthetic fixture has 24,000 application intervals
and 8,000 browser attribution intervals over 56 days, including overlapping
attribution sources. No private activity names are included in this report.

Reference timings in milliseconds (median before → after):

| Period | Full report | App detail | Website detail |
| --- | ---: | ---: | ---: |
| Day | 1218 → 87 | 1188 → 51 | 1175 → 47 |
| Week | 1203 → 75 | 1190 → 51 | 1223 → 48 |
| Month | 1243 → 102 | 1178 → 52 | 1203 → 45 |
| Year | 1273 → 142 | 1192 → 53 | 1189 → 48 |
| Life | 1265 → 141 | 1195 → 57 | 1187 → 50 |

Full reports are **89–94% faster**; activity details are **95–96% faster**.
Bar summaries remain approximately **7–44 ms**, with less than 0.6 ms change
in median latency for every period. Reference full-report p95 is **80–146 ms**.

The browser-heavy synthetic fixture improves full reports from **2.0–2.1 s**
to **40–82 ms**, and details from approximately **2.0 s** to **22–26 ms**.

Reference summaries use seven measured runs per binary and period; details and
synthetic measurements use three. With these small samples, nearest-rank p95
is the slowest measured sample, not a production tail-latency estimate.
Compared report data and historical insights matched. Current-period output
sizes differ only where wall-clock-dependent values have different lengths.

## Reproduce

Keep a release binary from before the change, then build the updated binary:

```sh
cargo build --release --bin omastat
python3 packaging/dev/benchmark-insights.py /path/to/before target/release/omastat \
  --database /path/to/omastat.db --runs 7
python3 packaging/dev/benchmark-insights.py /path/to/before target/release/omastat \
  --database /path/to/omastat.db --synthetic-intervals 12000 --runs 7
```

The benchmark creates a private temporary snapshot and freezes open telemetry
at the last heartbeat; it does not edit the source database. Both binaries use
the same configuration (`--config` can select one explicitly). It reports median,
nearest-rank p95, and output bytes for every lens. Use `--commands` to narrow the
run, or `--app` / `--domain` to select detail subjects.

Current-period comparisons exclude wall-clock-dependent fields and insights.
Historical reports are additionally compared with insights included, excluding
only generation timestamps and the rotating bar insight. Differential storage
tests compare the sweep directly against the previous SQL algorithm, including
random overlaps, tied timestamps, gaps, stale confirmations, and daemon shutdown.
DST tests cover application, idle, and sleep daily totals. Controller tests cover
cache expiry, errors, date rollover, navigation races, and preparation before
opening through the bar and IPC.

The checkout passed all 166 Rust tests, the QML lint/parse checks, and the existing
JavaScript model, controller, motion, and browser-extension checks. An isolated
offscreen preview checked wide and narrow layouts, slow-refresh feedback, and
reduced motion using the real panel content with a test window wrapper. Daemon lifecycle tests
need permission to create local Unix sockets. The desktop plugin was not deployed
or restarted as part of this optimization.
