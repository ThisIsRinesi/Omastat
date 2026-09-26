# Context insights

## Coming up

Current dashboard periods can show up to two possible upcoming app or website starts. These are based on known foreground visits of at least five minutes, not on an app merely staying open. Each compared one-hour local-clock window needs at least 90% tracking coverage. The card appears from 30 minutes before the window until the window ends, then expires; a visit during that time removes it. Stale tracking and pauses suppress suggestions. Historical views do not show predictions.

An established pattern requires five matching starts among seven observed days at a 70% rate, spread across at least three weeks. A same-weekday pattern requires four matches among five observed weeks. A recent pattern can qualify from the last 14 completed days with five matches among seven observed days at a 60% rate; its wording stays tentative. Both types need matches on at least two of the last three eligible occasions and a concentration of starts in the predicted hour. The displayed frequency is historical evidence, not a probability for today.

Dashboard Settings → Insight tone offers Warm, Concise, and Playful. Prediction wording changes by day and stays stable during refreshes. How we know keeps the same dates and method in every tone. Predictions are local to the dashboard and never enter the bar widget rotation.

The dashboard groups the day’s composition and ring clock on the left, the timeline and Explore activity in the center, and insights on the right at expansive widths. Narrower presets collapse these columns.

Insights describe recorded behavior using local data. Open a card to inspect its method, dates and supporting activity. They do not score productivity or infer the effect of audio on focus.

## Changes in app stretches

The engine compares daily median uninterrupted app stretches over the last seven completed local days with matching weekdays in the preceding four weeks. Each day requires five stretches, 30 minutes of focused use, four hours of observation and 90% observation between its first and last stretch. Each recent day needs two earlier matching weekdays; at least four recent and eight earlier days must qualify.

A change must reach all three thresholds: one minute, 25% of the baseline and twice the baseline median absolute deviation. At least 75% of paired days must agree on the direction. Cards expose the earlier and recent values and exact dates. Pauses and observation gaps break stretches; title and audio changes within the same app do not.

## Recurring app transitions

The strongest directed app pair in the last 28 completed days appears after six direct transitions across three days. Both app stays must last 30 seconds. Gaps, pauses, midnight and same-app title changes cannot create a transition. App detail limits pairs to those involving that app; website detail does not infer app transitions.

## Background audio companions

A period needs 30 minutes of focused use and at least ten minutes from one audio source covering half of recorded background audio overlap. Source identity follows the existing app/domain attribution and privacy settings. Simultaneous streams from one source are unioned, and playback overlaps focused time. Missing audio history is never treated as silence or used to infer a historical audio trend.

## Design references

[Apple Health trends](https://support.apple.com/en-il/guide/iphone/iphe3d379c32/ios) informed comparisons against personal history. [Apple Journal suggestions](https://www.apple.com/newsroom/2023/12/apple-launches-journal-app-a-new-app-for-reflecting-on-everyday-moments/) informed contextual summaries of recorded activity. The thresholds above are Nagori heuristics, not Apple algorithms or validated measures of productivity.

## Prediction lifecycle and evaluation

The engine fits windows from completed days before checking the current clock.
Each date contributes its earliest qualifying start within a window; reopening an
app many times on one date cannot move the typical time toward that date. History
is clipped at the training cutoff. Ambiguous, missing, or non-hour-long local
clock windows are not emitted as precise predictions.

The JSON `predictions` array contains a dedicated occurrence ID, strength,
generation time, display start/end, expected start, and the existing finding and
evidence fields. Ranking prefers established patterns, historical match rate,
number of supporting dates, then proximity. Activity filtering happens before the
dashboard's two-card limit. Cards expire locally even when refreshing fails; data
older than two minutes is hidden. The dashboard uses the controller's refresh
schedule, capped at one minute while open.

Run the chronological synthetic replay with:

```sh
cargo test --locked --lib chronological_replay -- --nocapture
```

Each run trains only on earlier observed days and scores the withheld day's
actual start against the predicted window. The 42-day evaluation currently gives:

| Scenario | Hits | Misses | Abstentions | Unobserved, excluded | Mean timing error on hits |
| --- | ---: | ---: | ---: | ---: | ---: |
| Steady, five-minute variation | 42 | 0 | 0 | 0 | 200 seconds |
| Occasional skipped visits | 35 | 7 | 0 | 0 | 197 seconds |
| Abrupt shift two hours later | 14 | 14 | 14 | 0 | 192 seconds |
| Scattered starts | 0 | 0 | 42 | 0 | — |
| Observation gaps | 34 | 0 | 0 | 8 | 194 seconds |

These are regression fixtures, not an estimate of real-world accuracy. The abrupt
shift exposes the deliberate inertia in weekly cohorts: an established weekday
pattern can survive one missed weekly occasion and fade after the second. The
replay also verifies that refreshing after expiry cannot extend that occurrence.
