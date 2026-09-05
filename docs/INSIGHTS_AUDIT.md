# Patterns and insights audit — September 5, 2026

## Findings and changes

The previous detector split routines by exact weekday and clock hour. It found
no Slay the Spire 2 routine despite evening sessions on ten of the preceding
fourteen completed dates. The eight-week baseline diluted the recent habit.
The replacement detects recent everyday use and foreground visit starts, reports
its actual eligible and matching dates, and includes it in the overview.

The audit also corrected comparisons using insufficiently observed days,
concentration described as unusual without a historical baseline, foreground
blocks split by title changes, and app switches counted across recording gaps.
Daily anomalies now require seven eligible baseline dates and use a robust
median-based threshold. Missing-observation percentages use elapsed time.
The engine supplies consistent descriptive wording to all consumers, including
the TUI, which previously substituted explanations with different claims.

Old recurrence, momentum, and streak implementations that were calculated and
then discarded have been removed. Existing insight kind identifiers remain
readable. No database migration or telemetry rewrite is needed.

## Performance

Release builds before and after the change were compared against the same SQLite
snapshot. Seven measured runs per lens followed warmups, with binary order
alternated. The benchmark checks that focus, open, idle, locked, and sleep totals
are identical. Elapsed and unobserved totals are clock-dependent and are not
compared across separate CLI executions.

| Local snapshot | Before | After | Improvement |
| --- | ---: | ---: | ---: |
| Day | 69.95 ms | 60.40 ms | 13.7% |
| Week | 70.82 ms | 69.02 ms | 2.5% |
| Month | 69.08 ms | 65.21 ms | 5.6% |
| Lifetime | 126.61 ms | 103.56 ms | 18.2% |

The synthetic workload contains 200,000 focused intervals plus 200,000 open
intervals across 56 days and twelve apps, with a fully observed historical span.

| Synthetic history | Before | After | Improvement |
| --- | ---: | ---: | ---: |
| Day | 449.69 ms | 322.79 ms | 28.2% |
| Week | 457.82 ms | 417.54 ms | 8.8% |
| Month | 435.90 ms | 384.30 ms | 11.8% |
| Lifetime | 1178.42 ms | 962.66 ms | 18.3% |

These are local measurements, not latency guarantees. Reproduce with
`python3 packaging/dev/benchmark-insights.py OLD NEW --database SNAPSHOT`;
add `--synthetic-intervals 200000` for generated telemetry.

The following bounded-query call counts are from the report call graph, rather
than a SQL trace; independent daily/total queries still exist:

| Shared calculation | Before | After |
| --- | --- | --- |
| Foreground input for overview rollups and recurrence | Two bounded reads | One shared metadata read |
| Domain attribution for overview and recurrence | Two attribution queries | One shared attribution query |
| Same-time comparison foreground history | Up to nine additional reads | No additional reads |
| Same-time observation history | One additional read | No additional read |

Coverage overlap lookup is now logarithmic instead of restarting a linear scan.
Observation-gap subtraction uses sorted interval sweeps. Overview reports skip
building activity charts that they previously immediately discarded.

## Verification

- All 157 Rust tests pass, covering the recent gaming pattern, weekly routines, censored starts,
  random schedules, all-day activity, insufficient evidence, historical cutoffs,
  missing domain capture, pauses, zero-use comparisons, and robust anomalies.
- Integration tests check overview/detail equality across every lens, diversity
  before truncation, and local-time recurrence through both DST transitions.
- Existing widget, controller, browser-attribution, and QML validation pass.
- Clippy passes with warnings denied; JSON evidence is additive and CSV exports
  include routine evidence and matching dates.

Performance validation uses isolated databases and leaves live history untouched.
The repository's existing post-commit hook rebuilds and installs the app and
refreshes its service and widget.

## Plain-language presentation

Selected routines are described after ranking, so wording cannot change which
patterns are found. Titles describe habits (for example, "Slay the Spire 2 most
evenings"), with readable local times and explicit counts. Recent or weak
findings remain qualified, and "How we know" explains which days were compared
and why missing tracking is left out. Other insights describe app use, breaks,
and comparisons without statistical jargon. Structured evidence and identifiers
remain unchanged.
