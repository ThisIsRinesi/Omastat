# Context insights

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

[Apple Health trends](https://support.apple.com/en-il/guide/iphone/iphe3d379c32/ios) informed comparisons against personal history. [Apple Journal suggestions](https://www.apple.com/newsroom/2023/12/apple-launches-journal-app-a-new-app-for-reflecting-on-everyday-moments/) informed contextual summaries of recorded activity. The thresholds above are Omastat heuristics, not Apple algorithms or validated measures of productivity.
